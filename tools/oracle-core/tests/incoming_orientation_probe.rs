//! Incoming wire metadata through the captured parser, packet constructor and H.264 passthrough.

use oracle_core::{Runtime, abi, derive::function_body_sha256};
use wacore::voip::rtp::parse_whatsapp_media_frame_info;
use wasmtime::Val;

mod common;

#[test]
fn incoming_rtp_metadata_to_frame_descriptor() -> anyhow::Result<()> {
    let bytes = common::capture("JgwtTQVeWPm")?.expect("pinned capture required");
    for (index, slot, anchor, hash) in [
        (
            4873,
            3954,
            "EXT_HDR: payload offset",
            "198c51630bcec7240cbb6472423831f89d6c1ad3676d477032030e624ea07c3b",
        ),
        (
            6003,
            4331,
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
    let packet = runtime.write_bytes(&[0; 64])?;
    let extensions = runtime.write_bytes(&[0; 136])?;
    let frame = runtime.write_bytes(&[0; 336])?;
    let stream = runtime.write_bytes(&[0; 13000])?;
    let context = runtime.write_bytes(&[0; 16])?;
    let status = runtime.write_bytes(&[0; 8])?;
    let clock = runtime.write_bytes(&[0; 8])?;
    let mut cases = 0;
    for layout in ["full", "reordered", "frame-only", "eight-byte", "absent"] {
        let mut rust_present = 0;
        for info in 0..=255u8 {
            for (payload, marker, key) in [
                (&[0x65, 0x88][..], true, true),
                (&[0x7c, 0x85, 0x88][..], false, true),
                (&[0x7c, 0x45, 0x88][..], true, true),
            ] {
                let extension = match layout {
                    "full" => vec![0x30, info, 0x51, 0, 1, 0x61, 0, 0, 0x91, 0, 1, 0],
                    "reordered" => vec![0x51, 0, 1, 0x30, info, 0x61, 0, 0, 0x91, 0, 1, 0],
                    "frame-only" => vec![0x30, info, 0, 0],
                    "eight-byte" => vec![0x37, info, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
                    "absent" => vec![0; 4],
                    _ => unreachable!(),
                };
                let mut wire = vec![
                    0x90,
                    97 | (u8::from(marker) << 7),
                    0,
                    1,
                    0,
                    0,
                    0,
                    1,
                    0,
                    0,
                    0,
                    1,
                    0xde,
                    0xbe,
                    0,
                    (extension.len() / 4) as u8,
                ];
                wire.extend_from_slice(&extension);
                let payload_offset = wire.len();
                wire.extend_from_slice(payload);
                runtime.write_bytes_at(packet, &wire)?;
                runtime.write_bytes_at(extensions, &[0; 136])?;
                runtime.shared().clear_trace();
                runtime.refuel();
                let result = runtime.call_table(
                    3954,
                    &[
                        Val::I32(packet as i32),
                        Val::I64(wire.len() as i64),
                        Val::I32(extensions as i32),
                    ],
                )?;
                assert_eq!(result[0].i32(), Some(0), "parser info={info:#04x}");
                let rust = parse_whatsapp_media_frame_info(&wire);
                rust_present += usize::from(rust.is_some());
                if let Some(rust) = rust {
                    assert_eq!(rust, info);
                }
                runtime.call_table(
                    4331,
                    &[
                        Val::I32(frame as i32),
                        Val::I32(stream as i32),
                        Val::I32(context as i32),
                        Val::I32((packet + payload_offset as u32) as i32),
                        Val::I32(payload.len() as i32),
                        Val::I32(status as i32),
                        Val::I32(packet as i32),
                        Val::I32(extensions as i32),
                        Val::I32(97),
                        Val::I32(clock as i32),
                    ],
                )?;
                let actual = runtime.read_u32_at(frame + 52)?;
                let expected = if layout == "absent" {
                    0
                } else {
                    0x800 | u32::from(info)
                };
                let expected = (expected & !8) | (u32::from(key) << 3) | (u32::from(marker) << 12);
                assert_eq!(
                    actual, expected,
                    "layout={layout}, info={info:#04x}, payload={payload:?}"
                );
                assert_eq!(
                    rust.map(|info| info & !8),
                    (actual & 0x800 != 0).then_some(actual as u8 & !8),
                    "Rust/WASM metadata except normalized keyframe bit, layout={layout}, info={info:#04x}"
                );
                assert!(
                    runtime.stubs_called().is_empty(),
                    "{:?}",
                    runtime.stubs_called()
                );
                assert!(
                    runtime.shared().hot_calls().is_empty(),
                    "{:?}",
                    runtime.shared().hot_calls()
                );
                cases += 1;
            }
        }
        eprintln!(
            "layout={layout}: WASM processed 768 packets; Rust recognized frame-info on {rust_present}"
        );
    }
    eprintln!(
        "executed actual RTP extension dispatcher -> frame constructor for {cases} wire packets"
    );
    Ok(())
}

#[test]
fn incoming_frame_info_element_boundaries() -> anyhow::Result<()> {
    let bytes = common::capture("JgwtTQVeWPm")?.expect("pinned capture required");
    for (index, hash) in [
        (
            4923,
            "19f7062204d914d74aa7c8e116af86e674d836c7106eb36ef96747c4987c09fd",
        ),
        (
            4911,
            "a83fda9022ea210689475a6dbcc671b5606992ffb278a62269af958c1ca7dcee",
        ),
        (
            4891,
            "7bfbce4785f9aa5a291f9d8e5711079248f324889ca284659b1e2176b0f8de0f",
        ),
    ] {
        assert_eq!(function_body_sha256(&bytes, index)?, hash);
    }
    let _serial = common::threaded_guard();
    let mut runtime = Runtime::instantiate(&bytes)?;
    runtime.run_ctors()?;
    // Malformed packets log through an engine logger that this isolated test does not initialize.
    runtime.set_engine_log_level(0)?;
    let packet = runtime.write_bytes(&[0; 128])?;
    let extensions = runtime.write_bytes(&[0; 136])?;
    let frame = runtime.write_bytes(&[0; 336])?;
    let stream = runtime.write_bytes(&[0; 13000])?;
    let context = runtime.write_bytes(&[0; 16])?;
    let status = runtime.write_bytes(&[0; 8])?;
    let clock = runtime.write_bytes(&[0; 8])?;
    let mut cases = vec![
        ("padding", vec![0, 0x0f, 0x30, 3]),
        ("unknown-before", vec![0xf0, 0xff, 0x30, 3]),
        ("unknown-after", vec![0x30, 3, 0xf0, 0xff]),
        ("duplicate-short", vec![0x30, 1, 0x30, 3]),
        ("duplicate-short-reverse", vec![0x30, 3, 0x30, 1]),
        (
            "short-then-long",
            vec![0x30, 1, 0x37, 3, 0, 0, 0, 0, 0, 0, 0],
        ),
        (
            "long-then-short",
            vec![0x37, 3, 0, 0, 0, 0, 0, 0, 0, 0x30, 1],
        ),
        (
            "three-then-eight",
            vec![0x32, 1, 0, 0, 0x37, 3, 0, 0, 0, 0, 0, 0, 0],
        ),
        (
            "eight-then-three",
            vec![0x37, 3, 0, 0, 0, 0, 0, 0, 0, 0x32, 1, 0, 0],
        ),
        ("unknown-truncated-before", vec![0xff, 0x30, 3, 0]),
        ("unknown-truncated-after", vec![0x30, 3, 0xff, 0]),
        ("frame-truncated-before", vec![0x3f, 3, 0, 0]),
        ("frame-truncated-after", vec![0x30, 1, 0x3f, 3]),
    ];
    for length in 1..=16 {
        let mut extension = vec![0; length + 1];
        extension[0] = 0x30 | (length - 1) as u8;
        extension[1] = 3;
        cases.push(("length", extension));
    }
    for first in 1..=16 {
        for last in 1..=16 {
            let mut extension = vec![0; first + last + 2];
            extension[0] = 0x30 | (first - 1) as u8;
            extension[1] = 1;
            extension[first + 1] = 0x30 | (last - 1) as u8;
            extension[first + 2] = 3;
            cases.push(("duplicate-lengths", extension));
        }
    }
    let count = cases.len();
    for (name, mut extension) in cases {
        let original = extension.clone();
        extension.resize(extension.len().next_multiple_of(4), 0);
        let mut wire = vec![
            0x90,
            0xe1,
            0,
            1,
            0,
            0,
            0,
            1,
            0,
            0,
            0,
            1,
            0xde,
            0xbe,
            0,
            (extension.len() / 4) as u8,
        ];
        wire.extend_from_slice(&extension);
        let offset = wire.len();
        wire.extend_from_slice(&[0x61, 0x88]);
        runtime.write_bytes_at(packet, &wire)?;
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
        assert_eq!(
            parsed[0].i32(),
            Some(if name.contains("truncated") { 70004 } else { 0 })
        );
        runtime.call_table(
            4331,
            &[
                Val::I32(frame as i32),
                Val::I32(stream as i32),
                Val::I32(context as i32),
                Val::I32((packet + offset as u32) as i32),
                Val::I32(2),
                Val::I32(status as i32),
                Val::I32(packet as i32),
                Val::I32(extensions as i32),
                Val::I32(97),
                Val::I32(clock as i32),
            ],
        )?;
        let actual = runtime.read_u32_at(frame + 52)?;
        assert_eq!(
            parse_whatsapp_media_frame_info(&wire),
            (actual & 0x800 != 0).then_some(actual as u8),
            "{name} {original:02x?}: parser={:?}",
            parsed[0].i32()
        );
        assert!(runtime.shared().hot_calls().is_empty());
        assert!(runtime.stubs_called().is_empty());
    }
    eprintln!(
        "compared Rust/WASM metadata for {count} padding, length, duplicate and truncation cases"
    );
    Ok(())
}

#[test]
fn incoming_packets_through_h264_passthrough_to_renderer() -> anyhow::Result<()> {
    let bytes = common::capture("JgwtTQVeWPm")?.expect("pinned capture required");
    assert!(
        abi::find_string_refs(&bytes, "h26x_passthrough_codec_decode")?
            .iter()
            .any(|entry| entry.referenced_by.contains(&12236))
    );
    assert!(abi::table_slots_of(&bytes, 12236)?.contains(&8773));
    assert_eq!(
        function_body_sha256(&bytes, 12236)?,
        "3be2b436a962e13f32de54a563ef8347433a6355232c617a03ca4b8e179f5c35"
    );
    assert_eq!(
        function_body_sha256(&bytes, 828)?,
        "6b4c303d8f48d3adc46ef8ba1c3e8dc0aca0db37193974b53101e1a9071b7131"
    );
    let _serial = common::threaded_guard();
    let mut runtime = Runtime::instantiate(&bytes)?;
    runtime.run_ctors()?;
    let packets = runtime.write_bytes(&[0; 128])?;
    let extensions = runtime.write_bytes(&[0; 136])?;
    let frames = runtime.write_bytes(&[0; 672])?;
    let stream = runtime.write_bytes(&[0; 13000])?;
    let context = runtime.write_bytes(&[0; 16])?;
    let status = runtime.write_bytes(&[0; 8])?;
    let clock = runtime.write_bytes(&[0; 8])?;
    let codec = runtime.write_bytes(&[0; 16])?;
    let decoder = runtime.write_bytes(&[0; 2200])?;
    let unpacker = runtime.write_bytes(&[0; 64])?;
    let output = runtime.write_bytes(&[0; 336])?;
    let buffer = runtime.write_bytes(&[0; 128])?;
    let pointers = runtime.write_bytes(&[0; 8])?;
    let list = runtime.write_bytes(&[0; 16])?;
    let size = runtime.write_bytes(&[3, 0, 0, 0, 2, 0, 0, 0])?;
    let peer = runtime.write_bytes(&[0; 24])?;
    for (ptr, value) in [
        (codec + 8, decoder),
        (decoder + 1980, 2),
        (decoder + 872, 875967048),
        (decoder + 884, 3),
        (decoder + 888, 2),
        (decoder + 2144, buffer),
        (decoder + 2148, buffer + 64),
        (decoder + 2156, 64),
        (decoder + 2184, unpacker),
        (unpacker + 4, 875967048),
        (list, 2),
        (list + 4, 2),
        (list + 8, pointers),
        (list + 12, 8),
        (pointers, frames),
        (pointers + 4, frames + 336),
    ] {
        runtime.write_bytes_at(ptr, &value.to_le_bytes())?;
    }
    runtime.write_bytes_at(unpacker + 27, &[1])?;
    for (first, last) in [
        (Some(1u8), Some(0u8)),
        (Some(0), Some(1)),
        (Some(1), Some(2)),
        (Some(2), Some(1)),
        (Some(3), Some(1)),
        (None, Some(3)),
        (Some(3), None),
        (None, None),
    ] {
        runtime.shared().clear_trace();
        runtime.refuel();
        for (index, info) in [first, last].into_iter().enumerate() {
            let packet = packets + index as u32 * 64;
            let mut wire = [
                0x90, 97, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0xde, 0xbe, 0, 1, 0, 0, 0, 0, 0x61, 0x88,
            ];
            wire[1] |= (index as u8) << 7;
            wire[3] += index as u8;
            if let Some(info) = info {
                wire[16] = 0x30;
                wire[17] = info;
            }
            runtime.write_bytes_at(packet, &wire)?;
            runtime.write_bytes_at(extensions, &[0; 136])?;
            assert_eq!(
                runtime.call_table(
                    3954,
                    &[
                        Val::I32(packet as i32),
                        Val::I64(wire.len() as i64),
                        Val::I32(extensions as i32),
                    ]
                )?[0]
                    .i32(),
                Some(0)
            );
            runtime.call_table(
                4331,
                &[
                    Val::I32((frames + index as u32 * 336) as i32),
                    Val::I32(stream as i32),
                    Val::I32(context as i32),
                    Val::I32((packet + 20) as i32),
                    Val::I32(2),
                    Val::I32(status as i32),
                    Val::I32(packet as i32),
                    Val::I32(extensions as i32),
                    Val::I32(97),
                    Val::I32(clock as i32),
                ],
            )?;
        }
        assert_eq!(
            runtime.call_table(
                8773,
                &[
                    Val::I32(codec as i32),
                    Val::I32(list as i32),
                    Val::I32(64),
                    Val::I32(output as i32),
                ]
            )?[0]
                .i32(),
            Some(0)
        );
        let actual = runtime.read_u32_at(output + 52)?;
        let expected = [first, last]
            .into_iter()
            .flatten()
            .fold(0x1000u32, |acc, info| acc | 0x800 | u32::from(info));
        assert_eq!(actual, expected);
        assert_eq!(runtime.read_u32_at(output + 16)?, 12);
        let data = runtime.read_u32_at(output + 8)?;
        assert_eq!(
            runtime.read(data, 12)?,
            [0, 0, 0, 1, 0x61, 0x88, 0, 0, 0, 1, 0x61, 0x88]
        );
        assert!(
            runtime.shared().hot_calls().is_empty(),
            "{:?}",
            runtime.shared().hot_calls()
        );
        runtime.call_table(
            427,
            &[
                Val::I32(output as i32),
                Val::I32(size as i32),
                Val::I32(peer as i32),
            ],
        )?;
        let calls = runtime.shared().calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].symbol(), "env::renderVideoFrame_js");
        let orientation = if actual & 0x800 == 0 {
            0
        } else {
            [1, 4, 3, 2][(actual & 3) as usize]
        };
        assert_eq!(calls[0].args[5], orientation);
        eprintln!(
            "wire {first:?}, {last:?} -> AU info {actual:#x} -> JS orientation {orientation}"
        );
    }
    Ok(())
}
