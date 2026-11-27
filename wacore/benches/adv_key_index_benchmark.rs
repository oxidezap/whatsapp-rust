#![allow(clippy::disallowed_methods)]

use divan::black_box;
use std::sync::OnceLock;
use wacore::adv::{verify_hosted_key_index_list, verify_key_index_list};
use wacore::libsignal::protocol::KeyPair;
use waproto::protobuf::{Message as _, encoding::put_varint};
use waproto::whatsapp::{ADVKeyIndexList, ADVSignedKeyIndexList};

struct Fixture {
    wire: Vec<u8>,
    account: [u8; 32],
    count: usize,
}

fn fixture(case: &str) -> &'static Fixture {
    static FIXTURES: OnceLock<Vec<Fixture>> = OnceLock::new();
    let fixtures = FIXTURES.get_or_init(|| {
        [0usize, 16, 256, 16]
            .into_iter()
            .enumerate()
            .map(|(case, count)| {
                let mut rng = <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(0xa117);
                let account = KeyPair::generate(&mut rng);
                let account_key = *account.public_key.public_key_bytes();
                let mut index = ADVKeyIndexList::default();
                index.raw_id = Some(7);
                index.timestamp = Some(42);
                index.current_index = Some(256);
                index.valid_indexes = (0..count as u32).collect();
                let details = index.encode_to_vec();
                let signature = account
                    .private_key
                    .calculate_signature(&[&[6, 2], details.as_slice()].concat(), &mut rng)
                    .expect("synthetic signature");
                let mut signed = ADVSignedKeyIndexList::default();
                signed.details = Some(details);
                signed.account_signature = Some(signature.to_vec());
                signed.account_signature_key = Some(account_key.to_vec());
                let mut wire = signed.encode_to_vec();
                if case == 3 {
                    put_varint(100 << 3, &mut wire);
                    put_varint(7, &mut wire);
                }
                Fixture {
                    wire,
                    account: account_key,
                    count,
                }
            })
            .collect()
    });
    let index = match case {
        "empty" => 0,
        "typical" => 1,
        "wide" => 2,
        "unknown" => 3,
        _ => unreachable!(),
    };
    &fixtures[index]
}

#[divan::bench(args = ["empty", "typical", "wide", "unknown"])]
fn ordinary(bencher: divan::Bencher, case: &str) {
    let fixture = fixture(case);
    let decoded = verify_key_index_list(&fixture.wire, &fixture.account)
        .expect("synthetic ordinary signature");
    assert_eq!(decoded.valid_indexes.len(), fixture.count);
    bencher.bench(|| {
        black_box(verify_key_index_list(
            black_box(&fixture.wire),
            black_box(&fixture.account),
        ))
    });
}

#[divan::bench(args = ["empty", "typical", "wide", "unknown"])]
fn hosted(bencher: divan::Bencher, case: &str) {
    let fixture = fixture(case);
    let (decoded, key) =
        verify_hosted_key_index_list(&fixture.wire).expect("synthetic hosted signature");
    assert_eq!(decoded.valid_indexes.len(), fixture.count);
    assert_eq!(key, fixture.account);
    bencher.bench(|| black_box(verify_hosted_key_index_list(black_box(&fixture.wire))));
}

fn main() {
    divan::main();
}
