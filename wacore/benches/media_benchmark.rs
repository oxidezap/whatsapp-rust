//! Media crypto throughput: the two paths every attachment pays for.
//!
//! Encryption is measured with and without a streaming sidecar (audio/video
//! carry one, images and documents do not), and decryption on both shapes a
//! caller can pick: the streaming reader/writer path used for file downloads,
//! and the in-place path used when a buffered HTTP client already holds the
//! whole ciphertext.
//!
//! Every arm reports a `BytesCount` over the *plaintext*, so divan prints MB/s
//! directly comparable across all of them. Fixtures are built once and handed
//! to `with_inputs`, whose generation time divan excludes: only the crypto and
//! the buffer traffic it drives are inside the measurement.

use divan::counter::BytesCount;
use divan::{Bencher, black_box};
use wacore::download::{DownloadUtils, MediaType};
use wacore::upload::{
    MediaEncryptor, encrypt_media_streaming_with_key, encrypt_media_with_key_and_sidecar,
    encrypted_len,
};

fn main() {
    divan::main();
}

const MB: usize = 1024 * 1024;
const MEDIA_KEY: [u8; 32] = [0x3C; 32];

/// Deterministic payload. Content does not affect AES or SHA-256 cost, but a
/// fixed pattern keeps runs comparable.
fn payload(len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut x: u32 = 0x9E37_79B9;
    for _ in 0..len {
        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        out.push((x >> 24) as u8);
    }
    out
}

fn encrypted(len: usize, media_type: MediaType) -> Vec<u8> {
    encrypt_media_with_key_and_sidecar(&payload(len), media_type, Some(&MEDIA_KEY), None)
        .expect("fixture encryption")
        .data_to_upload
}

// ---------------------------------------------------------------------------
// Encryption
// ---------------------------------------------------------------------------

fn bench_encrypt(bencher: Bencher, len: usize, media_type: MediaType, sidecar: bool) {
    let plaintext = payload(len);
    bencher
        .counter(BytesCount::new(len))
        .with_inputs(|| Vec::with_capacity(encrypted_len(len)))
        .bench_refs(|out| {
            let mut enc =
                MediaEncryptor::with_key_and_sidecar(MEDIA_KEY, media_type, sidecar).unwrap();
            enc.update(black_box(&plaintext), out);
            black_box(enc.finalize(out).unwrap());
        });
}

/// A 1 MiB image: no sidecar, so this is pure AES-CBC + HMAC + two SHA-256
/// passes. The floor the batched encryptor is measured against.
#[divan::bench]
fn media_encrypt_image_1mb(bencher: Bencher) {
    bench_encrypt(bencher, MB, MediaType::Image, false);
}

/// A 10 MiB video: the same work plus the streaming sidecar, which hashes the
/// whole ciphertext a second time in 64 KiB windows. The gap against the image
/// arm is what the sidecar costs per byte.
#[divan::bench]
fn media_encrypt_video_10mb_with_sidecar(bencher: Bencher) {
    bench_encrypt(bencher, 10 * MB, MediaType::Video, true);
}

/// The streaming upload path: 8 KiB reads, encrypt, flush whole runs to the
/// writer. A `Vec` writer keeps this a measurement of crypto plus buffer
/// traffic; against a real `File` the write batching is worth much more.
#[divan::bench]
fn media_encrypt_stream_video_10mb(bencher: Bencher) {
    let len = 10 * MB;
    let plaintext = payload(len);
    bencher
        .counter(BytesCount::new(len))
        .with_inputs(|| Vec::with_capacity(encrypted_len(len)))
        .bench_refs(|out| {
            black_box(
                encrypt_media_streaming_with_key(
                    std::io::Cursor::new(black_box(&plaintext)),
                    out,
                    MediaType::Video,
                    Some(&MEDIA_KEY),
                    None,
                )
                .unwrap(),
            );
        });
}

// ---------------------------------------------------------------------------
// Decryption
// ---------------------------------------------------------------------------

/// The download path for a `File`/writer target: read in chunks, HMAC, decrypt
/// in place, write the batch. `Vec` stands in for the writer so the measurement
/// is crypto plus buffer traffic rather than the filesystem — a real `File`
/// makes the per-write batching worth far more than this arm shows.
#[divan::bench]
fn media_decrypt_stream_10mb(bencher: Bencher) {
    let len = 10 * MB;
    let ciphertext = encrypted(len, MediaType::Video);
    bencher
        .counter(BytesCount::new(len))
        .with_inputs(|| Vec::with_capacity(len))
        .bench_refs(|out| {
            black_box(
                DownloadUtils::decrypt_stream_to_writer(
                    std::io::Cursor::new(black_box(&ciphertext)),
                    &MEDIA_KEY,
                    MediaType::Video,
                    out,
                )
                .unwrap(),
            );
        });
}

/// The buffered-HTTP path: authenticate, then decrypt into the response's own
/// allocation. No second copy of the file exists at any point, so the delta
/// against the streaming arm is the writer copy plus the chunked refills.
fn bench_decrypt_in_place(bencher: Bencher, len: usize, media_type: MediaType) {
    let ciphertext = encrypted(len, media_type);
    bencher
        .counter(BytesCount::new(len))
        .with_inputs(|| ciphertext.clone())
        .bench_refs(|buf| {
            DownloadUtils::verify_and_decrypt_in_place(black_box(buf), &MEDIA_KEY, media_type)
                .unwrap();
            black_box(&*buf);
        });
}

#[divan::bench]
fn media_decrypt_in_place_10mb(bencher: Bencher) {
    bench_decrypt_in_place(bencher, 10 * MB, MediaType::Video);
}

/// A 1 MiB image over the in-place path: the common inbound case (a photo from
/// a buffered client) at a size where per-call overheads still show.
#[divan::bench]
fn media_decrypt_in_place_1mb(bencher: Bencher) {
    bench_decrypt_in_place(bencher, MB, MediaType::Image);
}
