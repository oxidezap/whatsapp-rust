// Shared by the CodSpeed workload and its fixture validation test.

use std::io::Write;
use wacore::iq::spec::IqSpec;
use wacore::iq::usync::{DeviceListResponse, DeviceListSpec};
use wacore_binary::Jid;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::marshal::{marshal, unmarshal_ref};
use wacore_binary::util::{FORMAT_COMPRESSED, unpack};

pub struct Fixture {
    spec: DeviceListSpec,
    pub plain: Vec<u8>,
    pub compressed: Vec<u8>,
}

impl Fixture {
    pub fn new(width: usize) -> Self {
        let jids: Vec<_> = (0..width)
            .map(|i| {
                let user = format!("1555000{i:05}");
                if i % 2 == 0 {
                    Jid::pn(user)
                } else {
                    Jid::lid(user)
                }
            })
            .collect();
        let users = jids.iter().enumerate().map(|(index, jid)| {
            // Opaque fictitious signed-key bytes. Parsing retains them; signature
            // verification happens after the response parser and is not measured.
            let mut seed = 0x9e37_79b9_7f4a_7c15u64 ^ index as u64;
            let signed_key: Vec<u8> = (0..128)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    seed as u8
                })
                .collect();
            let key = NodeBuilder::new("key-index-list")
                .attr("ts", "1700000000")
                .bytes(signed_key)
                .build();
            NodeBuilder::new("user")
                .attr("jid", jid.clone())
                .children([NodeBuilder::new("devices")
                    .children([
                        NodeBuilder::new("device-list")
                            .attr("hash", "2:0123456789abcdef")
                            .children((0..4u16).map(|id| {
                                NodeBuilder::new("device")
                                    .attr("id", id)
                                    .attr("key-index", id)
                                    .build()
                            }))
                            .build(),
                        key,
                    ])
                    .build()])
                .build()
        });
        let node = NodeBuilder::new("iq")
            .attr("type", "result")
            .attr("id", "usync-bench")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list").children(users).build()])
                .build()])
            .build();
        let plain = marshal(&node).expect("marshal fixture");
        let mut encoder = flate2::write::ZlibEncoder::new(
            vec![FORMAT_COMPRESSED],
            flate2::Compression::default(),
        );
        encoder.write_all(&plain[1..]).expect("compress fixture");
        let compressed = encoder.finish().expect("finish fixture");
        let fixture = Self {
            spec: DeviceListSpec::new(jids, "usync-bench"),
            plain,
            compressed,
        };
        // Catch fixtures that silently skip users or companions, outside timing.
        for frame in [&fixture.plain, &fixture.compressed] {
            let parsed = fixture.receive(frame);
            assert_eq!(parsed.device_lists.len(), width);
            assert!(
                parsed
                    .device_lists
                    .iter()
                    .all(|user| user.devices.len() == 4)
            );
        }
        fixture
    }

    pub fn receive(&self, frame: &[u8]) -> DeviceListResponse {
        let bytes = unpack(frame).expect("unpack response");
        let node = unmarshal_ref(&bytes).expect("decode response");
        self.spec.parse_response(&node).expect("parse response")
    }
}
