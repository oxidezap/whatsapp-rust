//! Key inclusion at the first suspension of the pinned Web receipt helper.
use wacore::protocol::retry::{should_drop_unknown_device_retry, should_include_keys_with_policy};

#[test]
fn captured_web_key_inclusion_gate_matches_manual_core_helper() {
    use sha2::Digest;
    let bytes = include_bytes!("fixtures/retry-conformance/session-cases.json");
    assert_eq!(
        hex::encode(sha2::Sha256::digest(bytes)),
        "dc9412eef9d9241262395948b80745c320922ce6b98a61405748dc4db0a6c0b6",
        "pinned retry oracle fixture drift"
    );
    let fixture: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    let rows = fixture["keyCases"].as_array().unwrap();
    assert_eq!(rows.len(), 14);
    for row in rows {
        let count = row["retryCount"].as_u64().unwrap() as u8;
        let stateless = row["isStateless"].as_bool().unwrap();
        let expected = row["includeKeys"].as_bool().unwrap();
        assert_eq!(
            should_include_keys_with_policy(count, false, stateless),
            expected,
            "retry={count}, stateless={stateless}"
        );
        // Core explicitly permits callers, including hosted addressing, to force
        // inclusion. This is an adapter extension, not a Web default account flag.
        assert!(should_include_keys_with_policy(count, true, stateless));
    }
}

#[test]
fn verified_key_bundle_remains_an_explicit_unknown_device_exception() {
    assert!(should_drop_unknown_device_retry(false, false));
    assert!(!should_drop_unknown_device_retry(true, false));
    assert!(!should_drop_unknown_device_retry(false, true));
    assert!(!should_drop_unknown_device_retry(true, true));
}
