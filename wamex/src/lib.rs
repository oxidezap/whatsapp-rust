//! Typed WhatsApp mex (GraphQL persisted-query) operations.
//!
//! Each operation is a module with typed [`Variables`]/[`Response`] structs plus
//! `NAME`/`DOC_ID`/`OPERATION_KIND` consts, generated at build time from the
//! committed `mex_index.json` IR (extracted by whatspec from the WhatsApp Web
//! bundle). [`ALL`] is a flat registry of every operation.
//!
//! Only the JSON is version-controlled; the generated code lives in `OUT_DIR`.
//! Refresh by replacing `mex_index.json` with a newer whatspec
//! `generated/mex/index.json` and rebuilding.

#[allow(clippy::all)]
#[rustfmt::skip]
mod operations {
    include!(concat!(env!("OUT_DIR"), "/operations.rs"));
}

pub use operations::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_well_formed() {
        // Invariants that hold across IR refreshes (no pinned version strings).
        assert!(!SCHEMA_VERSION.is_empty());
        let parts: Vec<&str> = WA_VERSION.split('.').collect();
        assert!(parts.len() >= 3, "WA_VERSION {WA_VERSION:?} is not dotted");
        assert!(
            parts
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())),
            "WA_VERSION {WA_VERSION:?} has a non-numeric component"
        );
    }

    #[test]
    fn registry_is_populated_and_well_formed() {
        assert!(!ALL.is_empty(), "no operations extracted from the IR");
        for op in ALL {
            assert!(!op.name.is_empty(), "operation has an empty relay name");
            assert!(!op.doc_id.is_empty(), "{} has an empty doc id", op.name);
            assert!(
                op.kind == "query" || op.kind == "mutation",
                "{} has an unexpected kind {:?}",
                op.name,
                op.kind
            );
        }
    }

    #[test]
    fn operation_names_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for op in ALL {
            assert!(seen.insert(op.name), "duplicate operation name {}", op.name);
        }
    }

    #[test]
    fn typed_module_round_trips() {
        // References one stable op to exercise the generated typed structs; the
        // assertions below are on serde behavior, not on the (rotating) doc id.
        use acs_server_provider_config as op;
        assert_eq!(op::OPERATION_KIND, "query");
        assert!(!op::DOC_ID.is_empty());

        let resp: op::Response =
            serde_json::from_str(r#"{"xwa_wa_acs_config":{"id":"abc","expire_time":42}}"#)
                .expect("deserialize response");
        let cfg = resp.xwa_wa_acs_config.expect("config present");
        assert_eq!(cfg.id.as_deref(), Some("abc"));
        assert_eq!(cfg.expire_time, Some(42));

        let vars = op::Variables {
            project_name: Some("p".to_string()),
        };
        let json = serde_json::to_string(&vars).expect("serialize vars");
        assert_eq!(json, r#"{"project_name":"p"}"#);
    }
}
