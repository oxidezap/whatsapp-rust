//! Synthetic wire fixtures shared by the isolated benchmark and its tests.
pub fn node_bytes(width: usize) -> Vec<u8> {
    use wacore_binary::{builder::NodeBuilder, jid::Jid, marshal::marshal_shallow};
    let children: Vec<_> = (0..width)
        .map(|i| {
            NodeBuilder::new("message")
                .attr(
                    "from",
                    Jid::pn_device(format!("55110000{:05}", i / 4), (i % 4) as u16),
                )
                .attr("participant", Jid::pn("5511000000000"))
                .attr(
                    "id",
                    if i % 3 == 0 {
                        "A12345678901234567890123456789012345678901234567890123456789012345"
                    } else {
                        "3EB0123456789"
                    },
                )
                .children(vec![
                    NodeBuilder::new("enc")
                        .attr("v", "2")
                        .attr("type", "msg")
                        .bytes(vec![0xAB; 128])
                        .build(),
                ])
                .build()
        })
        .collect();
    let root = NodeBuilder::new("notification")
        .attr("type", "devices")
        .children(children)
        .build();
    marshal_shallow(&root).expect("synthetic fixture")[1..].to_vec()
}
