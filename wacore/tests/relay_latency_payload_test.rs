//! Public payload construction, optional data, and shared wire/JSON mapping.
use wacore::stanza::call::{RelayLatencyReportParams, build_relay_latency_report};
use wacore::stanza::relay_latency::parse_relay_latency_entries;
use wacore::types::relay_latency::{
    PackedRelayLatency, RelayLatencyEntry, RelayLatencyKind, RelayLatencyReport,
};
use wacore_binary::Jid;

#[test]
fn relay_payload_builders_preserve_absent_empty_and_present_names() {
    let peer = Jid::lid("2");
    let creator = Jid::lid("1");
    for name in [None, Some(""), Some("synthetic")] {
        let entry = RelayLatencyEntry::builder()
            .kind(RelayLatencyKind::Xpop)
            .latency(PackedRelayLatency::new(33_554_688))
            .maybe_relay_name(name.map(str::to_owned))
            .address(vec![192, 0, 2, 1, 13, 150])
            .build();
        let report = RelayLatencyReport::builder().entries(vec![entry]).build();
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(
            json["entries"][0].get("relay_name"),
            name.map(serde_json::Value::from).as_ref()
        );
        let wire = build_relay_latency_report(&RelayLatencyReportParams {
            call_id: "synthetic-call",
            to: &peer,
            call_creator: &creator,
            entries: &report.entries,
            devices: &[],
        })
        .unwrap();
        let nr = wire.as_node_ref();
        let action = nr.get_optional_child("relaylatency").unwrap();
        let te = action.get_optional_child("te").unwrap();
        assert_eq!(te.attrs().optional_string("relay_name").as_deref(), name);
        assert_eq!(parse_relay_latency_entries(action), report.entries);
    }
}

#[test]
fn relay_kind_uses_the_same_wire_and_json_mapping() {
    for (kind, wire) in [
        (RelayLatencyKind::Normal, "latency"),
        (RelayLatencyKind::Xpop, "xlatency"),
    ] {
        assert_eq!(kind.as_str(), wire);
        assert_eq!(RelayLatencyKind::try_from(wire).unwrap(), kind);
        assert_eq!(serde_json::to_value(kind).unwrap(), wire);
        assert_eq!(
            serde_json::from_value::<RelayLatencyKind>(serde_json::json!(wire)).unwrap(),
            kind
        );
    }
}

#[test]
fn wire_boundary_rejects_invalid_addresses_built_by_a_consumer() {
    let peer = Jid::lid("2");
    let creator = Jid::lid("1");
    let entry = RelayLatencyEntry::builder()
        .kind(RelayLatencyKind::Normal)
        .latency(PackedRelayLatency::new(0))
        .address(vec![0; 7])
        .build();
    assert!(entry.relay_name.is_none());
    assert!(
        build_relay_latency_report(&RelayLatencyReportParams {
            call_id: "synthetic-call",
            to: &peer,
            call_creator: &creator,
            entries: &[entry],
            devices: &[],
        })
        .is_err()
    );
}
