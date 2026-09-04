//! Transport-frame crypto: every stanza in and out of the socket pays one of
//! these AES-256-GCM passes. 1.5 KB approximates a typical message stanza;
//! 64 KB approximates a media-chunk-sized frame.
//!
//! The framing rows cover the buffer handoff around the crypto: `feed` copies
//! a read into the accumulation buffer while `feed_owned` adopts a
//! uniquely-owned `Bytes` instead. 28 KB is the AB-props response that aborted
//! the ESP32-C3 with two full-size copies alive (`docs/esp32c3.md` in
//! whatsapp-rust-esp32); the delta between the two rows is what that adoption
//! saves per large frame.

use bytes::Bytes;
use divan::black_box;
use wacore_noise::NoiseCipher;
use wacore_noise::framing::{FrameDecoder, encode_frame};

fn main() {
    divan::main();
}

const KEY: [u8; 32] = [0x42; 32];

fn frame(len: usize) -> Vec<u8> {
    (0..len).map(|i| i as u8).collect()
}

#[divan::bench(args = [1500, 65536])]
fn bench_frame_encrypt_in_place(bencher: divan::Bencher, len: usize) {
    bencher
        .with_inputs(|| (NoiseCipher::new(&KEY).unwrap(), frame(len)))
        .bench_refs(|(cipher, buf)| {
            cipher.encrypt_in_place_with_counter(7, buf).unwrap();
            // Contents, not length: keeps the in-place writes observable.
            black_box(buf.as_slice());
        });
}

#[divan::bench(args = [1500, 65536])]
fn bench_frame_decrypt_in_place(bencher: divan::Bencher, len: usize) {
    bencher
        .with_inputs(|| {
            let cipher = NoiseCipher::new(&KEY).unwrap();
            let mut buf = frame(len);
            cipher.encrypt_in_place_with_counter(7, &mut buf).unwrap();
            (cipher, buf)
        })
        .bench_refs(|(cipher, buf)| {
            cipher.decrypt_in_place_with_counter(7, buf).unwrap();
            // Contents, not length: keeps the in-place writes observable.
            black_box(buf.as_slice());
        });
}

/// The 28 KB AB-props-shaped frame, fed as a borrowed slice: one copy into
/// the accumulation buffer on top of the caller's allocation.
#[divan::bench]
fn frame_feed_copy_28k(bencher: divan::Bencher) {
    let wire = encode_frame(&frame(28_204), None).expect("encode");
    bencher
        .with_inputs(FrameDecoder::new)
        .bench_values(|mut decoder| {
            decoder.feed(black_box(&wire));
            black_box(decoder.decode_frame().map(|f| f.len()))
        });
}

/// Same frame, handed over as a uniquely-owned `Bytes`: adopted in place, no
/// second copy. The delta against `frame_feed_copy_28k` is the ESP32-C3 fix.
#[divan::bench]
fn frame_feed_owned_28k(bencher: divan::Bencher) {
    let wire = encode_frame(&frame(28_204), None).expect("encode");
    bencher
        .with_inputs(|| (FrameDecoder::new(), Bytes::from(wire.clone())))
        .bench_values(|(mut decoder, owned)| {
            decoder.feed_owned(black_box(owned));
            black_box(decoder.decode_frame().map(|f| f.len()))
        });
}
