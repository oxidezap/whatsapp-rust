//! Transport-frame crypto: every stanza in and out of the socket pays one of
//! these AES-256-GCM passes. 1.5 KB approximates a typical message stanza;
//! 64 KB approximates a media-chunk-sized frame.

use divan::black_box;
use wacore_noise::NoiseCipher;

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
