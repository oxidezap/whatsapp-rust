//! Preserve peer measurements without changing relay election or transport state.
use crate::types::relay_latency::{PackedRelayLatency, RelayLatencyEntry, RelayLatencyKind};
use wacore_binary::NodeRef;

/// The captured native transport holds at most eight reports per message.
pub const MAX_RELAY_LATENCY_ENTRIES: usize = 8;

/// Normal latency has precedence; an absent/malformed value may fall back to xlatency.
/// Invalid entries are skipped independently; an unknown relay name is retained as metadata.
pub fn parse_relay_latency_entries(node: &NodeRef<'_>) -> Vec<RelayLatencyEntry> {
    node.children()
        .unwrap_or_default()
        .iter()
        .filter(|child| child.tag == "te")
        .filter_map(|child| {
            let mut attrs = child.attrs();
            let (kind, value) = [RelayLatencyKind::Normal, RelayLatencyKind::Xpop]
                .into_iter()
                .find_map(|kind| {
                    attrs
                        .optional_string(kind.as_str())
                        .and_then(|s| s.parse::<i32>().ok())
                        .map(|value| (kind, value as u32))
                })?;
            let bytes = child.content_bytes()?;
            if !matches!(bytes.len(), 6 | 18) {
                return None;
            }
            let name = attrs.optional_string("relay_name").map(|s| s.into_owned());
            Some(
                RelayLatencyEntry::builder()
                    .kind(kind)
                    .latency(PackedRelayLatency::new(value))
                    .maybe_relay_name(name)
                    .address(bytes.to_vec())
                    .build(),
            )
        })
        .take(MAX_RELAY_LATENCY_ENTRIES)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore_binary::builder::NodeBuilder;

    #[test]
    fn capture_shape_keeps_normal_and_unknown_xpop_reports_in_both_families() {
        let mut children = Vec::new();
        for (name, attr, value) in [
            ("normal-a", "latency", "33554468"),
            ("normal-b", "latency", "33554485"),
            ("peer-only", "xlatency", "33554688"),
        ] {
            for address in [vec![192, 0, 2, 1, 0x0d, 0x96], vec![0; 18]] {
                children.push(
                    NodeBuilder::new("te")
                        .attr("relay_name", name)
                        .attr(attr, value)
                        .bytes(address)
                        .build(),
                );
            }
        }
        let node = NodeBuilder::new("relaylatency").children(children).build();
        let entries = parse_relay_latency_entries(&node.as_node_ref());
        assert_eq!(entries.len(), 6);
        assert_eq!(
            entries
                .iter()
                .map(|e| e.latency.rtt_ms())
                .collect::<Vec<_>>(),
            [Some(36), Some(36), Some(53), Some(53), Some(256), Some(256)]
        );
        assert_eq!(entries[4].kind, RelayLatencyKind::Xpop);
        assert_eq!(entries[4].relay_name.as_deref(), Some("peer-only"));
        assert!(entries.iter().all(|e| e.latency.round() == 1));
        assert_eq!(entries[4].latency.raw(), 33_554_688);
    }

    #[test]
    fn precedence_malformed_values_and_address_bounds() {
        for (normal, xpop, expected) in [
            ("33554468", "33554688", Some(RelayLatencyKind::Normal)),
            ("bad", "33554688", Some(RelayLatencyKind::Xpop)),
            ("-1", "33554688", Some(RelayLatencyKind::Normal)),
            ("2147483648", "33554688", Some(RelayLatencyKind::Xpop)),
            ("-2147483649", "33554688", Some(RelayLatencyKind::Xpop)),
            ("4294967296", "33554688", Some(RelayLatencyKind::Xpop)),
            ("bad", "bad", None),
        ] {
            for len in [0, 5, 6, 7, 18, 19, 1024] {
                let node = NodeBuilder::new("relaylatency")
                    .children([NodeBuilder::new("te")
                        .attr("latency", normal)
                        .attr("xlatency", xpop)
                        .bytes(vec![0; len])
                        .build()])
                    .build();
                let entries = parse_relay_latency_entries(&node.as_node_ref());
                assert_eq!(
                    entries.first().map(|e| e.kind),
                    if matches!(len, 6 | 18) {
                        expected
                    } else {
                        None
                    }
                );
            }
        }
    }

    #[test]
    fn report_limit_and_invalid_siblings() {
        let node = NodeBuilder::new("relaylatency")
            .children(
                (0..20)
                    .map(|i| {
                        NodeBuilder::new("te")
                            .attr("latency", if i % 2 == 0 { "bad" } else { "33554468" })
                            .bytes(vec![0; 6])
                            .build()
                    })
                    .collect::<Vec<_>>(),
            )
            .build();
        assert_eq!(
            parse_relay_latency_entries(&node.as_node_ref()).len(),
            MAX_RELAY_LATENCY_ENTRIES
        );
    }

    #[test]
    fn packed_values_preserve_flags_rounds_signed_values_and_sentinel() {
        for (raw, round, rtt) in [
            (0, 0, Some(0)),
            (0x0200_0024, 1, Some(36)),
            (0x0300_0024, 1, Some(36)),
            (0x7e00_0100, 63, Some(256)),
            (0x02ff_ffff, 1, Some(-1)),
            (0x0280_0000, 1, Some(-8_388_608)),
            (0x027f_ffff, 1, None),
            (u32::MAX, 63, Some(-1)),
        ] {
            let value = PackedRelayLatency::new(raw);
            assert_eq!(
                (value.raw(), value.round(), value.rtt_ms()),
                (raw, round, rtt)
            );
        }
    }
}
