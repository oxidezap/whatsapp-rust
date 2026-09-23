//! In-memory cache for server-side A/B experiment properties.
//!
//! Only stores props whose code is in the interest set. Props not in the set
//! are discarded during parsing, avoiding heap allocation for the thousands of
//! server props we never query.
//!
//! Not persisted — props are fetched on every connect.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;

use async_lock::RwLock;
use wacore_binary::CompactString;

use crate::iq::abprops::{AbDefault, AbProp};
use crate::iq::props::WATCHED;

/// In-memory cache of AB experiment properties, populated on connect.
/// Only materializes props whose code is in the interest set.
/// Pre-populated with the `WATCHED` flags; extend via `watch()`.
pub struct AbPropsCache {
    state: RwLock<PropsState>,
    interest: RwLock<HashSet<u32>>,
    seeded: AtomicBool,
}

#[derive(Default)]
struct PropsState {
    props: HashMap<u32, CompactString>,
    seeded: bool,
    generation: u64,
    applied_in_generation: bool,
}

/// Consistent view of cached props and connection-generation readiness.
#[derive(Clone)]
pub struct AbPropsSnapshot {
    props: HashMap<u32, CompactString>,
    seeded: bool,
    applied_in_generation: bool,
}

impl AbPropsSnapshot {
    /// True after a full response has seeded the delta-fetch cache.
    pub fn is_seeded(&self) -> bool {
        self.seeded
    }

    /// True after any accepted response has applied in the current connection.
    pub fn applied_in_generation(&self) -> bool {
        self.applied_in_generation
    }

    /// Read a boolean prop with the registry default as fallback.
    pub fn is_enabled(&self, prop: AbProp) -> bool {
        self.props
            .get(&prop.code)
            .map(|value| {
                value == "1"
                    || value.eq_ignore_ascii_case("true")
                    || value.eq_ignore_ascii_case("enabled")
            })
            .unwrap_or(matches!(prop.default, AbDefault::Bool(true)))
    }
}

impl AbPropsCache {
    pub fn new() -> Self {
        Self {
            state: RwLock::new(PropsState::default()),
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

    /// Mark the start of a connection generation without discarding cached props.
    /// Retaining values supports reconnect work while the server delta is pending;
    /// generation-local readiness keeps privacy-sensitive decisions conservative.
    pub async fn begin_generation(&self, generation: u64) {
        let mut state = self.state.write().await;
        state.generation = generation;
        state.applied_in_generation = false;
    }

    /// The codes a fetch has to keep: a snapshot of the interest set, taken at
    /// request time so the response can be filtered as it streams in without
    /// holding the lock across the read loop.
    pub async fn interest(&self) -> HashSet<u32> {
        self.interest.read().await.clone()
    }

    /// True after the first full (non-delta) update.
    pub fn is_seeded(&self) -> bool {
        self.seeded.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Return cached values and readiness from one lock-protected state.
    pub async fn snapshot(&self) -> AbPropsSnapshot {
        let state = self.state.read().await;
        Self::snapshot_locked(&state)
    }

    fn snapshot_locked(state: &PropsState) -> AbPropsSnapshot {
        AbPropsSnapshot {
            props: state.props.clone(),
            seeded: state.seeded,
            applied_in_generation: state.applied_in_generation,
        }
    }

    /// Apply a props response, retaining only watched flag codes.
    pub async fn apply_props(
        &self,
        delta_update: bool,
        props: impl Iterator<Item = (u32, CompactString)>,
    ) {
        let interest = self.interest.read().await;
        let mut state = self.state.write().await;
        self.apply_props_locked(delta_update, props, &interest, &mut state);
        state.applied_in_generation = true;
    }

    /// Apply a response only if it belongs to the connection generation that
    /// is still current. The generation check and mutation share the state lock
    /// with [`begin_generation`](Self::begin_generation), so late responses cannot
    /// overwrite a newer generation. Returns a snapshot of the accepted response
    /// for post-fetch side effects that must use that exact observation.
    pub async fn apply_props_for_generation(
        &self,
        generation: u64,
        delta_update: bool,
        props: impl Iterator<Item = (u32, CompactString)>,
    ) -> Option<AbPropsSnapshot> {
        let interest = self.interest.read().await;
        let mut state = self.state.write().await;
        if state.generation != generation {
            return None;
        }
        self.apply_props_locked(delta_update, props, &interest, &mut state);
        state.applied_in_generation = true;
        Some(Self::snapshot_locked(&state))
    }

    fn apply_props_locked(
        &self,
        delta_update: bool,
        props: impl Iterator<Item = (u32, CompactString)>,
        interest: &HashSet<u32>,
        state: &mut PropsState,
    ) {
        if !delta_update {
            state.props.clear();
        }
        for (code, value) in props {
            if interest.contains(&code) {
                state.props.insert(code, value);
            }
        }
        if !delta_update {
            state.seeded = true;
            self.seeded
                .store(true, std::sync::atomic::Ordering::Release);
        }
    }

    /// Panics in debug builds when `prop` is read without being watched.
    ///
    /// `apply_props` discards anything outside the interest set, so such a read
    /// can never see the server's value: it returns the registry default now and
    /// forever, with no error and no log line. That has silently disabled a
    /// shipped feature gate more than once, and neither the type system nor a
    /// test of the reading code can catch it, because the reading code is
    /// correct -- what is missing sits in another file.
    ///
    /// Guards only the accessors that substitute a default. [`get`](Self::get)
    /// returns `Option`, so a caller there is told the value is absent rather
    /// than handed a plausible one.
    #[cfg(debug_assertions)]
    async fn debug_assert_watched(&self, prop: AbProp) {
        assert!(
            self.interest.read().await.contains(&prop.code),
            "AB prop {:?} (code {}) was read but is not watched, so the server's \
             value is discarded and this read always yields the registry default. \
             Add it to `WATCHED` in wacore/src/iq/props.rs, or call `watch()` \
             before the first fetch.",
            prop.name,
            prop.code,
        );
    }

    #[cfg(not(debug_assertions))]
    async fn debug_assert_watched(&self, _prop: AbProp) {}

    pub async fn get(&self, prop: AbProp) -> Option<CompactString> {
        self.state.read().await.props.get(&prop.code).cloned()
    }

    /// True when the cached value is truthy (`"1"`, `"true"`, or `"enabled"`),
    /// falling back to the flag's registry default when the server didn't send
    /// it. The registry is the single source of truth for the default.
    pub async fn is_enabled(&self, prop: AbProp) -> bool {
        self.debug_assert_watched(prop).await;
        self.get_bool(prop)
            .await
            .unwrap_or(matches!(prop.default, AbDefault::Bool(true)))
    }

    /// The server's value for `prop` read as a boolean, with the same truthy
    /// set as [`is_enabled`](Self::is_enabled), or `None` when no value is
    /// cached: the prop is not watched, props have not been fetched yet, or
    /// the server did not send it. Unlike `is_enabled` it substitutes no
    /// default, so it does not assert that `prop` is watched.
    pub async fn get_bool(&self, prop: AbProp) -> Option<bool> {
        self.state.read().await.props.get(&prop.code).map(|value| {
            value == "1"
                || value.eq_ignore_ascii_case("true")
                || value.eq_ignore_ascii_case("enabled")
        })
    }

    /// The cached int value, falling back to the flag's registry default when
    /// the server didn't send it (or it's not an int flag).
    pub async fn get_int(&self, prop: AbProp) -> i64 {
        self.debug_assert_watched(prop).await;
        let fallback = match prop.default {
            AbDefault::Int(n) => n,
            _ => 0,
        };
        match self.state.read().await.props.get(&prop.code) {
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
        // 999 is watched but never sent, which is the "absent" case asserted
        // below. Watching it is what distinguishes absent-from-the-response
        // from never-retained-at-all.
        cache
            .watch_many(&[flag(1), flag(2), flag(3), flag(4), flag(5), flag(999)])
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
    async fn get_bool_reports_absence_instead_of_a_default() {
        let cache = AbPropsCache::new();
        let on_by_default = AbProp {
            default: AbDefault::Bool(true),
            ..flag(3)
        };
        cache.watch_many(&[flag(1), flag(2), on_by_default]).await;
        assert_eq!(cache.get_bool(flag(1)).await, None); // not fetched yet

        let props = vec![
            (1u32, CompactString::from("true")),
            (2, CompactString::from("0")),
            (4, CompactString::from("1")),
        ];
        cache.apply_props(false, props.into_iter()).await;

        assert_eq!(cache.get_bool(flag(1)).await, Some(true));
        assert_eq!(cache.get_bool(flag(2)).await, Some(false));
        // Watched but not sent: absent, even though its default is `true`.
        assert_eq!(cache.get_bool(on_by_default).await, None);
        // Sent but not watched: discarded on apply, so also absent.
        assert_eq!(cache.get_bool(flag(4)).await, None);
    }

    #[tokio::test]
    async fn new_generation_keeps_props_and_rejects_stale_responses() {
        let cache = AbPropsCache::new();
        cache.watch(flag(100)).await;
        cache
            .apply_props(false, [(100, CompactString::from("1"))].into_iter())
            .await;
        assert!(cache.is_seeded());

        cache.begin_generation(1).await;
        assert!(cache.is_seeded());
        assert!(!cache.snapshot().await.applied_in_generation());
        assert_eq!(cache.get(flag(100)).await.as_deref(), Some("1"));
        assert!(
            cache
                .apply_props_for_generation(
                    0,
                    false,
                    [(100, CompactString::from("0"))].into_iter(),
                )
                .await.is_none(),
            "a late full response from the retired connection must be discarded"
        );
        assert!(!cache.snapshot().await.applied_in_generation());
        assert!(
            cache
                .apply_props_for_generation(1, true, [(100, CompactString::from("0"))].into_iter(),)
                .await
                .is_some()
        );
        assert!(cache.snapshot().await.applied_in_generation());
        assert!(cache.is_seeded());
        assert_eq!(cache.get_bool(flag(100)).await, Some(false));
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
            (
                web::RECEIPT_MODE_BITMASK_ENABLED.code,
                CompactString::from("1"),
            ),
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
        // A flag whose registry default is false is the case that proves the
        // interest set matters: without it the server's "on" is dropped and
        // the read falls through to false forever.
        assert!(cache.is_enabled(web::RECEIPT_MODE_BITMASK_ENABLED).await);
        // Unwatched code should NOT be retained
        assert_eq!(cache.get(flag(99999)).await, None);
    }

    /// The guard has to fire on the read, not on the fetch: at fetch time a
    /// missing prop is indistinguishable from one the server chose not to send.
    #[tokio::test]
    #[should_panic(expected = "is not watched")]
    #[cfg(debug_assertions)]
    async fn reading_an_unwatched_prop_trips_the_guard() {
        AbPropsCache::new().is_enabled(flag(4242)).await;
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
