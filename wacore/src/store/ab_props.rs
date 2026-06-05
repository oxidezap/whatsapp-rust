//! In-memory cache for server-side A/B experiment properties.
//!
//! Only stores props whose code is in the interest set. Props not in the set
//! are discarded during parsing, avoiding heap allocation for the thousands of
//! server props we never query.
//!
//! Not persisted — props are fetched on every connect.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

use async_lock::RwLock;
use wacore_binary::CompactString;

use crate::iq::abprops::{AbDefault, AbProp};
use crate::iq::props::WATCHED;

/// In-memory cache of AB experiment properties, populated on connect.
/// Only materializes props whose code is in the interest set.
/// Pre-populated with the `WATCHED` flags; extend via `watch()`.
pub struct AbPropsCache {
    props: RwLock<HashMap<u32, CompactString>>,
    interest: RwLock<HashSet<u32>>,
    seeded: AtomicBool,
}

impl AbPropsCache {
    pub fn new() -> Self {
        Self {
            props: RwLock::new(HashMap::new()),
            interest: RwLock::new(WATCHED.iter().map(|p| p.code).collect()),
            seeded: AtomicBool::new(false),
        }
    }

    /// Register a flag to be retained when props are fetched.
    /// Call before the first `fetch_props` to ensure the value is captured.
    pub async fn watch(&self, prop: AbProp) {
        self.interest.write().await.insert(prop.code);
    }

    /// Register multiple flags at once.
    pub async fn watch_many(&self, props: &[AbProp]) {
        self.interest
            .write()
            .await
            .extend(props.iter().map(|p| p.code));
    }

    /// True after the first full (non-delta) update.
    pub fn is_seeded(&self) -> bool {
        self.seeded.load(Ordering::Acquire)
    }

    /// Apply a props response, retaining only watched flag codes.
    pub async fn apply_props(
        &self,
        delta_update: bool,
        props: impl Iterator<Item = (u32, CompactString)>,
    ) {
        let interest = self.interest.read().await;
        let mut map = self.props.write().await;

        if !delta_update {
            map.clear();
        }

        for (code, value) in props {
            if interest.contains(&code) {
                map.insert(code, value);
            }
        }

        if !delta_update {
            self.seeded.store(true, Ordering::Release);
        }
    }

    pub async fn get(&self, prop: AbProp) -> Option<CompactString> {
        self.props.read().await.get(&prop.code).cloned()
    }

    /// True when the cached value is truthy (`"1"`, `"true"`, or `"enabled"`),
    /// falling back to the flag's registry default when the server didn't send
    /// it. The registry is the single source of truth for the default.
    pub async fn is_enabled(&self, prop: AbProp) -> bool {
        match self.props.read().await.get(&prop.code) {
            Some(value) => {
                value == "1"
                    || value.eq_ignore_ascii_case("true")
                    || value.eq_ignore_ascii_case("enabled")
            }
            None => matches!(prop.default, AbDefault::Bool(true)),
        }
    }

    /// The cached int value, falling back to the flag's registry default when
    /// the server didn't send it (or it's not an int flag).
    pub async fn get_int(&self, prop: AbProp) -> i64 {
        let fallback = match prop.default {
            AbDefault::Int(n) => n,
            _ => 0,
        };
        match self.props.read().await.get(&prop.code) {
            Some(value) => value.parse().unwrap_or(fallback),
            None => fallback,
        }
    }
}

impl Default for AbPropsCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iq::abprops::{AbDefault, AbPropType, web};

    /// Synthetic flag for cache-mechanics tests (only the `code` matters).
    fn flag(code: u32) -> AbProp {
        AbProp {
            name: "test",
            code,
            value_type: AbPropType::Bool,
            default: AbDefault::Bool(false),
        }
    }

    #[tokio::test]
    async fn watched_props_are_retained() {
        let cache = AbPropsCache::new();
        cache.watch(flag(100)).await;
        cache.watch(flag(200)).await;

        let props = vec![
            (100u32, CompactString::from("1")),
            (200, CompactString::from("0")),
            (300, CompactString::from("ignored")),
        ];
        cache.apply_props(false, props.into_iter()).await;

        assert!(cache.is_seeded());
        assert_eq!(cache.get(flag(100)).await, Some(CompactString::from("1")));
        assert_eq!(cache.get(flag(200)).await, Some(CompactString::from("0")));
        assert_eq!(cache.get(flag(300)).await, None); // not watched
    }

    #[tokio::test]
    async fn is_enabled_checks_truthy_values() {
        let cache = AbPropsCache::new();
        cache
            .watch_many(&[flag(1), flag(2), flag(3), flag(4), flag(5)])
            .await;

        let props = vec![
            (1u32, CompactString::from("1")),
            (2, CompactString::from("true")),
            (3, CompactString::from("enabled")),
            (4, CompactString::from("0")),
            (5, CompactString::from("false")),
        ];
        cache.apply_props(false, props.into_iter()).await;

        assert!(cache.is_enabled(flag(1)).await);
        assert!(cache.is_enabled(flag(2)).await);
        assert!(cache.is_enabled(flag(3)).await);
        assert!(!cache.is_enabled(flag(4)).await);
        assert!(!cache.is_enabled(flag(5)).await);
        assert!(!cache.is_enabled(flag(999)).await); // absent
    }

    #[tokio::test]
    async fn delta_merges_without_clearing() {
        let cache = AbPropsCache::new();
        cache.watch_many(&[flag(100), flag(200), flag(300)]).await;

        cache
            .apply_props(
                false,
                vec![
                    (100u32, CompactString::from("old")),
                    (200, CompactString::from("keep")),
                ]
                .into_iter(),
            )
            .await;

        cache
            .apply_props(
                true,
                vec![
                    (100u32, CompactString::from("new")),
                    (300, CompactString::from("added")),
                ]
                .into_iter(),
            )
            .await;

        assert_eq!(cache.get(flag(100)).await.as_deref(), Some("new"));
        assert_eq!(cache.get(flag(200)).await.as_deref(), Some("keep"));
        assert_eq!(cache.get(flag(300)).await.as_deref(), Some("added"));
    }

    /// Regression test: the default interest set (`WATCHED`) must include the
    /// production flags. Without this, apply_props would silently drop all props
    /// and every is_enabled/get_int call would fall through to its default.
    #[tokio::test]
    async fn default_interest_retains_production_flags() {
        let cache = AbPropsCache::new();

        // Simulate a full props response containing some production flags.
        let props = vec![
            (
                web::PRIVACY_TOKEN_SENDING_ON_ALL_1_ON_1_MESSAGES.code,
                CompactString::from("1"),
            ),
            (
                web::WA_NCT_TOKEN_SEND_ENABLED.code,
                CompactString::from("true"),
            ),
            (web::TCTOKEN_DURATION.code, CompactString::from("604800")),
            (web::TCTOKEN_NUM_BUCKETS.code, CompactString::from("4")),
            (99999u32, CompactString::from("unwatched")),
        ];
        cache.apply_props(false, props.into_iter()).await;

        assert!(cache.is_seeded());
        assert!(
            cache
                .is_enabled(web::PRIVACY_TOKEN_SENDING_ON_ALL_1_ON_1_MESSAGES)
                .await
        );
        assert!(cache.is_enabled(web::WA_NCT_TOKEN_SEND_ENABLED).await);
        assert_eq!(cache.get_int(web::TCTOKEN_DURATION).await, 604800);
        assert_eq!(cache.get_int(web::TCTOKEN_NUM_BUCKETS).await, 4);
        // Unwatched code should NOT be retained
        assert_eq!(cache.get(flag(99999)).await, None);
    }

    /// Verify seeded flag is only set AFTER all props are inserted (not before).
    #[tokio::test]
    async fn seeded_set_after_inserts() {
        let cache = AbPropsCache::new();
        assert!(!cache.is_seeded());

        cache
            .apply_props(
                false,
                vec![(web::TCTOKEN_DURATION.code, CompactString::from("100"))].into_iter(),
            )
            .await;

        assert!(cache.is_seeded());
        assert_eq!(cache.get_int(web::TCTOKEN_DURATION).await, 100);
    }
}
