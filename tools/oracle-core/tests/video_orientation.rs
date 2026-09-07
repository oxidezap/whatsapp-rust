//! Encoded browser frame metadata compared with the shipped WhatsApp WASM.

use oracle_core::{Runtime, abi, derive::function_body_sha256, patch};
use sha2::{Digest, Sha256};
use wacore::voip::rtp::{VIDEO_MEDIA_FRAME_INFO_DELTA, VIDEO_MEDIA_FRAME_INFO_IDR};
use wasmtime::Val;

mod common;

#[test]
fn received_frame_rotation_matches_whatsapp_wasm() -> anyhow::Result<()> {
    let bytes = common::capture("JgwtTQVeWPm")?
        .expect("receive orientation proof requires captured WASM; set WA_WASM_DIR");
    assert_eq!(
        hex::encode(Sha256::digest(&bytes)),
        "97259423aea19cc30c1771478e035105cb0d0e64ab4b0297741b62d01deac8db"
    );
    assert_eq!(
        function_body_sha256(&bytes, 828)?,
        "6b4c303d8f48d3adc46ef8ba1c3e8dc0aca0db37193974b53101e1a9071b7131"
    );
    assert!(abi::table_slots_of(&bytes, 828)?.contains(&427));
    let mut runtime = Runtime::instantiate(&bytes)?;
    runtime.run_ctors()?;
    let payload = runtime.write_bytes(&[0, 0, 0, 1, 0x65, 0x88])?;
    let frame = runtime.write_bytes(&[0; 80])?;
    let size = runtime.write_bytes(&[3, 0, 0, 0, 2, 0, 0, 0])?;
    let peer = runtime.write_bytes(&[0; 24])?;
    runtime.write_bytes_at(frame + 4, &875967048u32.to_le_bytes())?;
    runtime.write_bytes_at(frame + 8, &payload.to_le_bytes())?;
    runtime.write_bytes_at(frame + 16, &6u32.to_le_bytes())?;
    for info in 0..=255u32 {
        runtime.write_bytes_at(frame + 52, &(0x800 | info).to_le_bytes())?;
        runtime.shared().clear_trace();
        runtime.refuel();
        runtime.call_table(
            427,
            &[
                Val::I32(frame as i32),
                Val::I32(size as i32),
                Val::I32(peer as i32),
            ],
        )?;
        let calls = runtime.shared().calls();
        assert_eq!(calls.len(), 1, "info={info:#04x}");
        assert_eq!(calls[0].symbol(), "env::renderVideoFrame_js");
        assert_eq!(
            calls[0].args,
            [
                i64::from(peer + 8),
                i64::from(payload),
                6,
                3,
                2,
                [1, 4, 3, 2][(info & 3) as usize],
                100,
                0,
                i64::from(info & 8 != 0),
                0,
            ],
            "info={info:#04x}"
        );
    }
    eprintln!(
        "executed JgwtTQVeWPm.wasm receive renderer: 256 metadata bytes; rotation bits 0,1,2,3 -> JS enum 1,4,3,2"
    );
    Ok(())
}

#[test]
fn upright_video_frame_info_matches_whatsapp_wasm() -> anyhow::Result<()> {
    let Some(bytes) = common::capture("JgwtTQVeWPm")? else {
        eprintln!("skipping: JgwtTQVeWPm unavailable (set WA_WASM_DIR)");
        return Ok(());
    };
    assert_eq!(
        hex::encode(Sha256::digest(&bytes)),
        "97259423aea19cc30c1771478e035105cb0d0e64ab4b0297741b62d01deac8db"
    );

    for (index, hash) in [
        (
            4942,
            "a92611af6bc97b1593bf3a167e60f7c8512a5731a9c223194b859fc8d02529d4",
        ),
        (
            13065,
            "e95d64415b62bb0b4e0f4c4bb07a96e72eb703d46af8dc6d359522b7f2d0f8ad",
        ),
    ] {
        assert_eq!(function_body_sha256(&bytes, index)?, hash);
    }
    for (index, slot, anchor) in [
        (
            13128,
            9602,
            "onEncodedVideoDataFromJsForStream: Manager not initialized!",
        ),
        (4943, 3681, "media_frame_info_build_header_ext"),
    ] {
        assert!(
            abi::find_string_refs(&bytes, anchor)?
                .iter()
                .any(|entry| entry.referenced_by.contains(&index))
        );
        assert!(abi::table_slots_of(&bytes, index)?.contains(&slot));
    }
    assert!(abi::table_slots_of(&bytes, 4942)?.contains(&3680));

    // The pinned f4942 only writes the callback's first output word. With our
    // zeroed port it writes frame.type=1 and returns zero, leaving metadata intact.
    // f13065 instruction 1075 calls invoke_iii with (local3, local13, local9+120).
    // Record local9 there, not at callback entry, to identify this exact call site.
    let (instrumented, _) = patch::instrument(
        &bytes,
        &patch::Plan {
            value_at: vec![(13065, 1075, 9, false)],
            ..Default::default()
        },
    )?;
    let mut runtime = Runtime::instantiate(&instrumented)?;
    runtime.run_ctors()?;
    runtime.shared().watch_markers("env::on_call_event_js_sync");
    let manager = runtime.write_bytes(&[1])?;
    runtime.write_bytes_at(1_722_176, &manager.to_le_bytes())?;
    let mut port_bytes = [0u8; 112];
    port_bytes[108..112].copy_from_slice(&3680u32.to_le_bytes());
    let port = runtime.write_bytes(&port_bytes)?;
    runtime.write_bytes_at(1_719_056, &port.to_le_bytes())?;
    let payload = runtime.write_bytes(&[0, 0, 0, 1, 0x65, 0x88])?;
    let extender = runtime.write_bytes(&[0; 24])?;
    let output = runtime.write_bytes(&[0; 8])?;
    let mut upright = Vec::new();

    // WAWebVoipMediaEnums in bundle 561b4bd5677d24a29707db4c05b63edc019b73744b66699fb6bbfd6b361d6c1a
    // defines Unknown=0, Normal=1, Rotate90=2, Rotate180=3, Rotate270=4.
    for keyframe in [false, true] {
        for (orientation, rotation_bits) in [(0, 0), (1, 0), (2, 3), (3, 2), (4, 1)] {
            runtime.shared().clear_trace();
            let previous_markers = runtime.shared().markers().len();
            runtime.refuel();
            let result = runtime.call_table(
                9602,
                &[
                    Val::I32(0),
                    Val::I32(payload as i32),
                    Val::I32(6),
                    Val::I32(1280),
                    Val::I32(720),
                    Val::F64(1.0f64.to_bits()),
                    Val::I32(i32::from(keyframe)),
                    Val::F64(1.0f64.to_bits()),
                    Val::I32(orientation),
                ],
            )?;
            assert_eq!(result[0].i32(), Some(0));
            let markers = runtime.shared().markers();
            assert_eq!(
                markers.len(),
                previous_markers + 1,
                "the encoder port must receive one frame"
            );
            let frame = u32::try_from(markers[previous_markers].1)?
                .checked_add(120)
                .expect("frame pointer");
            let info = runtime.read_u32_at(frame + 52)?;
            assert_eq!(info, 0x800 | (u32::from(keyframe) << 3) | rotation_bits);
            assert_eq!(runtime.read(frame + 72, 4)?, [0, 5, 208, 2]);
            assert_eq!(runtime.read_u32_at(frame + 8)?, payload);
            assert_eq!(runtime.read_u32_at(frame + 16)?, 6);

            // f4943, anchored by "media_frame_info_build_header_ext", copies
            // the low byte to extension 3. Presence is internal bit 0x800.
            runtime.write_bytes_at(extender + 4, &[info as u8])?;
            let built = runtime.call_table(
                3681,
                &[
                    Val::I32(extender as i32),
                    Val::I32(0),
                    Val::I32(output as i32),
                    Val::I32(1),
                    Val::I32(0),
                ],
            )?;
            assert_eq!(built[0].i32(), Some(0));
            let wire = runtime.read(output, 1)?[0];
            assert_eq!(wire, (u8::from(keyframe) << 3) | rotation_bits as u8);
            if orientation == 1 {
                upright.push(wire);
            }
            assert_eq!(
                runtime.stubs_called(),
                vec![("env::on_call_event_js_sync".into(), 1)]
            );
        }
    }
    assert_eq!(
        upright,
        [VIDEO_MEDIA_FRAME_INFO_DELTA, VIDEO_MEDIA_FRAME_INFO_IDR],
        "Rust must not add a quarter turn to upright browser frames"
    );
    eprintln!(
        "executed JgwtTQVeWPm.wasm: 10 orientation/keyframe cases; \
         upright delta=0x{:02x}, IDR=0x{:02x}; dimensions and payload identity preserved",
        upright[0], upright[1]
    );
    Ok(())
}
