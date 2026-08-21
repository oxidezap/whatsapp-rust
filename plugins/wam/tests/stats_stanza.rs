//! One WAM event, all the way to the bytes of a `w:stats` stanza.
//!
//! The unit tests check the buffer codec against WA Web's own writer and the
//! request builder against the shape whatspec extracted. This checks the join:
//! that an observation this client makes ends up inside an `<iq xmlns="w:stats">`
//! that the binary protocol round-trips, carrying globals legal on the channel
//! it declares and nothing that identifies anybody.

use std::sync::{Arc, Mutex};

use whatsapp_rust::async_trait;
use whatsapp_rust::wacore::iq::spec::IqSpec;
use whatsapp_rust::wacore::request::RequestUtils;
use whatsapp_rust::wacore_binary::marshal::{marshal, unmarshal_packed_ref};
use whatsapp_rust::wacore_binary::{NodeContent, NodeContentRef};
use whatsapp_rust_plugin_wam::iq::SendBufferSpec;
use whatsapp_rust_plugin_wam::runtime::{PendingEvent, WamRuntime, WamWriter};
use whatsapp_rust_plugin_wam::{InMemoryWamStore, UploadFailure, WamIdentity, WamUploader};
use whatsapp_rust_wam_catalog::{WamEvent, events, globals};

/// Keeps the buffers it is handed instead of sending them.
#[derive(Default)]
struct CapturingUploader(Mutex<Vec<Vec<u8>>>);

#[async_trait]
impl WamUploader for CapturingUploader {
    async fn upload(&self, _t: i64, buffer: &[u8]) -> Result<(), UploadFailure> {
        self.0.lock().expect("capture lock").push(buffer.to_vec());
        Ok(())
    }
}

/// The records in a buffer, as `(kind, id)` pairs: kind 0 global, 1 event,
/// 2 field.
fn records(bytes: &[u8]) -> Vec<(u8, u32)> {
    let mut out = Vec::new();
    let mut i = 8;
    while i < bytes.len() {
        let flags = bytes[i];
        i += 1;
        let id = if flags & 8 != 0 {
            let id = u32::from(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
            i += 2;
            id
        } else {
            let id = u32::from(bytes[i]);
            i += 1;
            id
        };
        i += match flags & 0xf0 {
            0 | 16 | 32 => 0,
            48 => 1,
            64 => 2,
            80 => 4,
            96 | 112 => 8,
            128 => {
                let len = usize::from(bytes[i]);
                i += 1;
                len
            }
            144 => {
                let len = usize::from(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
                i += 2;
                len
            }
            _ => panic!("unexpected size class in {flags:#x}"),
        };
        out.push((flags & 3, id));
    }
    out
}

#[tokio::test]
async fn one_observation_reaches_the_wire_as_a_stats_stanza() {
    let runtime = WamRuntime::new(WamIdentity::web(), Arc::new(InMemoryWamStore::new()), 64);
    let uploader = CapturingUploader::default();
    let mut writer = WamWriter::default();

    runtime.observe(PendingEvent::WamDroppedEvent(events::WamDroppedEvent {
        dropped_event_code: Some(i64::from(events::MessageReceive::CODE)),
        dropped_event_count: Some(1),
        ..Default::default()
    }));
    // A roll of zero keeps everything, so the test is about the encoding rather
    // than about the sampler.
    runtime
        .tick(&mut writer, &uploader, 1_755_000_000, &mut || 0.0, true)
        .await;

    let buffers = uploader.0.lock().expect("capture lock").clone();
    assert_eq!(buffers.len(), 1, "one observation, one buffer");
    let buffer = &buffers[0];

    // The header the official client writes: magic, protocol version, stream id,
    // sequence, channel.
    assert_eq!(&buffer[..3], b"WAM");
    assert_eq!(buffer[3], 5);
    assert_eq!(buffer[4], 1);
    assert_eq!(u16::from_le_bytes([buffer[5], buffer[6]]), 1);
    assert_eq!(buffer[7], 0, "the regular channel");

    let records = records(buffer);
    let written_globals: Vec<u32> = records
        .iter()
        .filter(|(kind, _)| *kind == 0)
        .map(|(_, id)| *id)
        .collect();
    // Exactly the globals the default identity can derive, plus the per-event
    // timestamp, in ascending id order.
    assert_eq!(written_globals, vec![11, 15, 17, 21, 47, 6251]);
    // Nothing private-only, which a regular buffer may not carry.
    assert!(!written_globals.contains(&globals::PS_ID.id));
    assert!(records.contains(&(1, events::WamDroppedEvent::CODE)));

    // And now the stanza the client would put on the socket.
    let spec = SendBufferSpec {
        t: 1_755_000_000,
        buffer,
    };
    let node = RequestUtils::new("test".to_string()).build_iq_node(spec.build_iq(), None);
    let bytes = marshal(&node).expect("the stanza marshals");
    // `marshal` writes the leading format byte the transport carries, so the
    // reader that checks it is the one to use.
    let decoded = unmarshal_packed_ref(&bytes).expect("and unmarshals");

    assert_eq!(decoded.tag, "iq");
    assert_eq!(
        decoded.get_attr("xmlns").map(|v| v.to_string()),
        Some("w:stats".to_string())
    );
    assert_eq!(
        decoded.get_attr("type").map(|v| v.to_string()),
        Some("set".to_string())
    );
    assert_eq!(
        decoded.get_attr("to").map(|v| v.to_string()),
        Some("s.whatsapp.net".to_string())
    );
    let add = decoded
        .get_optional_child_by_tag(&["add"])
        .expect("the buffer travels in one <add>");
    assert_eq!(
        add.get_attr("t").map(|v| v.to_string()),
        Some("1755000000".to_string())
    );
    let Some(NodeContentRef::Bytes(payload)) = &add.content else {
        panic!("the buffer is the node's content, not text and not an attribute");
    };
    assert_eq!(
        payload.as_ref(),
        buffer.as_slice(),
        "the bytes on the wire are the bytes the codec produced"
    );

    // Nothing that identifies anybody: the only string a buffer of this event
    // can carry is a global this client set, and none of them is a JID.
    let text = String::from_utf8_lossy(buffer);
    assert!(!text.contains('@'));
    assert!(!text.contains("s.whatsapp.net"));

    // The content is bytes on the way in as well, so nothing re-encodes it.
    let Some(NodeContent::Bytes(_)) = spec.build_iq().content.and_then(|c| match c {
        NodeContent::Nodes(mut children) => children.pop().and_then(|n| n.content),
        other => Some(other),
    }) else {
        panic!("the <add> content is raw bytes");
    };
}
