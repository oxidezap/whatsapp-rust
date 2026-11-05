use std::collections::HashMap;
use std::io::Write;
use wacore::iq::spec::{IqSpec, IqStreamSpec};
use wacore::iq::usync::{DeviceListResponse, DeviceListSpec};
use wacore_binary::builder::NodeBuilder;
use wacore_binary::marshal::{marshal, unmarshal_ref};
use wacore_binary::util::{FORMAT_COMPRESSED, unpack};
use wacore_binary::{Jid, Node, NodeStream};

fn compressed(plain: &[u8]) -> Vec<u8> {
    let mut encoder =
        flate2::write::ZlibEncoder::new(vec![FORMAT_COMPRESSED], flate2::Compression::default());
    encoder.write_all(&plain[1..]).unwrap();
    encoder.finish().unwrap()
}

fn consume<S: IqStreamSpec<Response = DeviceListResponse>>(
    spec: &S,
    frame: &[u8],
) -> anyhow::Result<DeviceListResponse> {
    let mut stream = NodeStream::from_packed(frame)?;
    stream
        .open()?
        .ok_or_else(|| anyhow::anyhow!("missing root"))?;
    let response = spec.consume_response(&mut stream)?;
    stream.finish()?;
    Ok(response)
}

fn equivalent<S: IqStreamSpec<Response = DeviceListResponse>>(spec: &S, node: &Node) {
    let expected = spec
        .parse_response(&node.as_node_ref())
        .map(|r| format!("{r:?}"))
        .map_err(|e| e.to_string());
    let plain = marshal(node).unwrap();
    for frame in [&plain, &compressed(&plain)] {
        let actual = consume(spec, frame)
            .map(|r| format!("{r:?}"))
            .map_err(|e| e.to_string());
        assert_eq!(actual, expected);
    }
}

fn user(signed: bool) -> Node {
    let mut children = vec![
        NodeBuilder::new("device-list")
            .attr("hash", "2:fixture")
            .children([
                NodeBuilder::new("device")
                    .attr("id", 0u16)
                    .attr("key-index", 9u32)
                    .build(),
                NodeBuilder::new("device")
                    .attr("id", 3u16)
                    .attr("key-index", 11u32)
                    .attr("is_hosted", "true")
                    .build(),
            ])
            .build(),
    ];
    if signed {
        children.push(
            NodeBuilder::new("key-index-list")
                .attr("ts", "1700000000")
                .attr("expected_ts", "1699999999")
                .bytes(vec![1, 2, 3, 4])
                .build(),
        );
    }
    NodeBuilder::new("user")
        .attr("jid", Jid::lid("15550000100"))
        .attr("pn_jid", Jid::pn("15550000101"))
        .children([
            NodeBuilder::new("devices").children(children).build(),
            NodeBuilder::new("lid")
                .attr("val", Jid::lid("15550000100"))
                .build(),
        ])
        .build()
}

fn iq(children: Vec<Node>) -> Node {
    NodeBuilder::new("iq")
        .attr("type", "result")
        .children(children)
        .build()
}

fn response(children: Vec<Node>) -> Node {
    iq(vec![NodeBuilder::new("usync").children(children).build()])
}

fn list(users: Vec<Node>) -> Node {
    NodeBuilder::new("list").children(users).build()
}

fn protocol_error(tag: &'static str) -> Node {
    NodeBuilder::new(tag)
        .children([NodeBuilder::new("error")
            .attr("code", 500u32)
            .attr("text", "fixture-error")
            .build()])
        .build()
}

#[test]
fn stream_preserves_device_projection_and_refresh_modes() {
    let jids = vec![Jid::pn("15550000101"), Jid::pn("15550000102")];
    let ordinary = DeviceListSpec::new(jids.clone(), "fixture");
    let complete = ordinary.clone().require_complete_response();
    let incremental = DeviceListSpec::with_hashes(jids, "fixture", HashMap::new());
    let failed_user = NodeBuilder::new("user")
        .attr("jid", Jid::lid("15550000100"))
        .attr("pn_jid", Jid::pn("15550000101"))
        .children([protocol_error("devices")])
        .build();
    for users in [
        vec![],
        vec![user(true)],
        vec![user(false)],
        vec![failed_user],
    ] {
        let node = response(vec![list(users)]);
        equivalent(&ordinary, &node);
        equivalent(&complete, &node);
        equivalent(&incremental, &node);
    }
    let complete_one =
        DeviceListSpec::new(vec![Jid::pn("15550000101")], "fixture").require_complete_response();
    let node = response(vec![list(vec![user(true)])]);
    equivalent(&complete_one, &node);
    assert!(complete_one.parse_response(&node.as_node_ref()).is_ok());
}

#[test]
fn stream_preserves_first_children_optional_result_and_error_priority() {
    let spec = DeviceListSpec::new(vec![], "fixture");
    let bad_user = NodeBuilder::new("user")
        .attr("jid", Jid::pn("15550000101"))
        .children([NodeBuilder::new("devices")
            .children([NodeBuilder::new("device-list")
                .children([NodeBuilder::new("device").build()])
                .build()])
            .build()])
        .build();
    let bad_result = NodeBuilder::new("result")
        .children([NodeBuilder::new("devices")
            .attr("refresh", "not-a-number")
            .build()])
        .build();
    let rejection = NodeBuilder::new("result")
        .children([NodeBuilder::new("contact")
            .children([NodeBuilder::new("error")
                .attr("code", 500u32)
                .attr("text", "fixture-error")
                .build()])
            .build()])
        .build();
    for node in [
        iq(vec![]),
        response(vec![]),
        response(vec![list(vec![user(true)])]),
        response(vec![bad_result.clone()]),
        response(vec![list(vec![bad_user.clone()]), bad_result.clone()]),
        response(vec![bad_result.clone(), list(vec![bad_user.clone()])]),
        response(vec![list(vec![bad_user]), rejection.clone()]),
        response(vec![list(vec![user(true)]), rejection]),
        response(vec![
            list(vec![user(true)]),
            NodeBuilder::new("result")
                .children([protocol_error("devices")])
                .build(),
        ]),
        response(vec![
            NodeBuilder::new("ignored")
                .children([bad_result.clone()])
                .build(),
            list(vec![user(true)]),
            list(vec![]),
            NodeBuilder::new("result").build(),
            bad_result,
        ]),
        iq(vec![
            NodeBuilder::new("ignored").build(),
            NodeBuilder::new("usync")
                .children([list(vec![user(true)])])
                .build(),
            NodeBuilder::new("usync").build(),
        ]),
    ] {
        equivalent(&spec, &node);
    }
}

#[test]
fn stream_rejects_truncation_bad_trailers_and_leftover_bytes() {
    let spec = DeviceListSpec::new(vec![], "fixture");
    let plain = marshal(&response(vec![list(vec![user(true)])])).unwrap();
    let packed = compressed(&plain);
    for cut in 0..plain.len() {
        assert!(consume(&spec, &plain[..cut]).is_err(), "plain cut {cut}");
    }
    for cut in 0..packed.len() {
        assert!(
            consume(&spec, &packed[..cut]).is_err(),
            "compressed cut {cut}"
        );
    }
    let mut corrupt = packed;
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(consume(&spec, &corrupt).is_err());
    let mut leftover = plain.clone();
    leftover.push(0);
    assert!(consume(&spec, &leftover).is_err());
    for frame in [corrupt, leftover] {
        assert!(
            unpack(&frame)
                .and_then(|bytes| unmarshal_ref(&bytes).map(drop))
                .is_err()
        );
    }
}

#[test]
fn binary_validation_precedes_semantic_rejections() {
    let spec = DeviceListSpec::new(vec![], "fixture");
    for node in [
        iq(vec![]),
        response(vec![]),
        response(vec![
            list(vec![user(true)]),
            NodeBuilder::new("result")
                .children([protocol_error("contact")])
                .build(),
        ]),
    ] {
        let plain = marshal(&node).unwrap();
        let mut frame = compressed(&plain);
        *frame.last_mut().unwrap() ^= 1;
        let error = consume(&spec, &frame).unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<wacore_binary::BinaryError>(),
                Some(wacore_binary::BinaryError::Zlib(_))
            ),
            "{error}"
        );
        let mut frame = plain;
        frame.push(0);
        let error = consume(&spec, &frame).unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<wacore_binary::BinaryError>(),
                Some(wacore_binary::BinaryError::LeftoverData(_))
            ),
            "{error}"
        );
    }
}
