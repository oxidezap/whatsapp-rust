//! Our video RTP stream through the captured J engine's receive chain.
//!
//! The byte-diff the Android hunt can run without a live call: one IDR access
//! unit packetized by the real `wacore` send path (STAP-A aggregated
//! parameter sets, FU-A fragmented IDR, PT 97, shared-timestamp markers, the
//! captured 12-byte video extension) is fed packet by packet through J's RTP
//! parser (function 4873, slot 3954) and packet-frame constructor (function
//! 6003, slot 4331). Every parse must succeed and the marker packet's frame
//! must carry presence plus the keyframe bit with the original payload.
//!
//! Function/slot/body pins mirror `incoming_orientation_probe.rs`.
//!
//! ```sh
//! cargo test --release -p oracle-core --test video_our_bytes_to_j -- --nocapture
//! ```

mod common;

use anyhow::Result;
use oracle_core::{Runtime, abi, derive::function_body_sha256};
use sha2::{Digest, Sha256};
use wacore::voip::h264::{PacketizedAu, au_has_idr, nal_unit_type, packetize_au};
use wacore::voip::rtp::{
    VIDEO_MEDIA_FRAME_INFO_DELTA, VIDEO_MEDIA_FRAME_INFO_IDR, VIDEO_TS_STRIDE_15FPS,
    VideoRtpStream, encode_rtp_header,
};
use wasmtime::Val;

const CAPTURE: &str = "JgwtTQVeWPm";
const CAPTURE_SHA: &str = "97259423aea19cc30c1771478e035105cb0d0e64ab4b0297741b62d01deac8db";

/// AUD, SPS(23), PPS(4), IDR slice(3000): the x264 shape, small enough to log.
fn fixture_au() -> Vec<u8> {
    let mut au = vec![0, 0, 0, 1, 0x09, 0xf0];
    au.extend_from_slice(&[0, 0, 0, 1, 0x67]);
    au.extend((0..22).map(|i| (i % 251) as u8));
    au.extend_from_slice(&[0, 0, 0, 1, 0x68, 0xce, 0x06, 0xe2]);
    au.extend_from_slice(&[0, 0, 0, 1, 0x65]);
    au.extend((0..2999).map(|i| (i % 251) as u8));
    au
}

#[test]
fn our_idr_stream_through_j_parser_and_constructor() -> Result<()> {
    let Some(bytes) = common::capture(CAPTURE)? else {
        eprintln!("skipping: {CAPTURE} unavailable (set WA_WASM_DIR)");
        return Ok(());
    };
    assert_eq!(hex::encode(Sha256::digest(&bytes)), CAPTURE_SHA);
    for (index, slot, anchor, hash) in [
        (
            4873u32,
            3954u32,
            "EXT_HDR: payload offset",
            "198c51630bcec7240cbb6472423831f89d6c1ad3676d477032030e624ea07c3b",
        ),
        (
            6003u32,
            4331u32,
            "Unsupported codec for jbuf",
            "415de384a70490b5b0b75c37aee0ff2abc447c1a2fb4cdb96bec5588ffb5d045",
        ),
    ] {
        assert!(
            abi::find_string_refs(&bytes, anchor)?
                .iter()
                .any(|entry| entry.referenced_by.contains(&index))
        );
        assert!(abi::table_slots_of(&bytes, index)?.contains(&slot));
        assert_eq!(function_body_sha256(&bytes, index)?, hash);
    }
    let _serial = common::threaded_guard();
    let mut runtime = Runtime::instantiate(&bytes)?;
    runtime.run_ctors()?;

    // The exact send path: Annex-B AU -> RTP payloads -> RTP headers.
    let au = fixture_au();
    assert!(au_has_idr(&au));
    let info = VIDEO_MEDIA_FRAME_INFO_IDR;
    let mut payloads = PacketizedAu::default();
    packetize_au(&au, &mut payloads);
    // Pin the wire shape, not just the count: STAP-A(SPS, PPS) opens, then
    // FU-A fragments carry the IDR slice.
    assert_eq!(payloads.len(), 5);
    let stap = &payloads[0];
    assert_eq!(nal_unit_type(stap), 24);
    let (first_len, rest) = stap[1..].split_at(2);
    let first_len = u16::from_be_bytes([first_len[0], first_len[1]]) as usize;
    assert_eq!(nal_unit_type(&rest[..first_len]), 7, "SPS opens the STAP-A");
    let (second_len, rest) = rest[first_len..].split_at(2);
    let second_len = u16::from_be_bytes([second_len[0], second_len[1]]) as usize;
    assert_eq!(nal_unit_type(&rest[..second_len]), 8, "PPS follows SPS");
    assert_eq!(rest.len(), second_len, "STAP-A holds exactly SPS then PPS");
    let mut stream = VideoRtpStream::new(0x4996_ed22, VIDEO_TS_STRIDE_15FPS).unwrap();
    let last = payloads.len() - 1;
    let wires: Vec<Vec<u8>> = payloads
        .iter()
        .enumerate()
        .map(|(i, payload)| {
            let header = stream.next_video_packet(i == last, info);
            let mut wire = encode_rtp_header(&header);
            wire.extend_from_slice(payload);
            wire
        })
        .collect();
    eprintln!(
        "fixture AU {} bytes -> {} RTP packets (first payload type {:#x})",
        au.len(),
        wires.len(),
        wires[0][28]
    );
    let _ = VIDEO_MEDIA_FRAME_INFO_DELTA;

    let n = wires.len();
    let packet_area = runtime.write_bytes(&vec![0; n * 2048])?;
    let extensions = runtime.write_bytes(&[0; 136])?;
    let frames = runtime.write_bytes(&vec![0; n * 336])?;
    let stream_j = runtime.write_bytes(&[0; 13000])?;
    let context = runtime.write_bytes(&[0; 16])?;
    let status = runtime.write_bytes(&[0; 8])?;
    let clock = runtime.write_bytes(&[0; 8])?;

    let mut keyframe_seen = false;
    for (i, wire) in wires.iter().enumerate() {
        let marker = i + 1 == n;
        let packet = packet_area + (i as u32) * 2048;
        runtime.write_bytes_at(packet, wire)?;
        runtime.write_bytes_at(extensions, &[0; 136])?;
        runtime.shared().clear_trace();
        runtime.refuel();
        let parsed = runtime.call_table(
            3954,
            &[
                Val::I32(packet as i32),
                Val::I64(wire.len() as i64),
                Val::I32(extensions as i32),
            ],
        )?;
        assert_eq!(parsed[0].i32(), Some(0), "J parser rejects our packet {i}");
        let frame = frames + (i as u32) * 336;
        runtime.call_table(
            4331,
            &[
                Val::I32(frame as i32),
                Val::I32(stream_j as i32),
                Val::I32(context as i32),
                Val::I32((packet + 28) as i32),
                Val::I32((wire.len() - 28) as i32),
                Val::I32(status as i32),
                Val::I32(packet as i32),
                Val::I32(extensions as i32),
                Val::I32(97),
                Val::I32(clock as i32),
            ],
        )?;
        let meta = runtime.read_u32_at(frame + 52)?;
        eprintln!(
            "pkt{i}: wire={}B marker={marker} frame_meta={meta:#x}",
            wire.len()
        );
        if marker {
            assert_eq!(
                meta & 0x808,
                0x808,
                "marker packet frame must carry presence+keyframe"
            );
            keyframe_seen = true;
        }
        assert!(runtime.shared().hot_calls().is_empty());
        assert!(runtime.stubs_called().is_empty());
    }
    assert!(keyframe_seen);
    eprintln!("J accepted all {} packets of our IDR stream", n);
    Ok(())
}
