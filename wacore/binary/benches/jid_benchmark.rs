//! Jid parse/format hot paths: every stanza attribute that carries an
//! address goes through these, several times per message.

use divan::black_box;
use wacore_binary::jid::Jid;

fn main() {
    divan::main();
}

const PN: &str = "5511999990000@s.whatsapp.net";
const LID: &str = "123456789012345@lid";
const AD_DEVICE: &str = "5511999990000.0:7@s.whatsapp.net";
const GROUP: &str = "120363012345678901@g.us";

#[divan::bench(args = [PN, LID, AD_DEVICE, GROUP])]
fn bench_jid_parse(input: &str) -> Jid {
    black_box(black_box(input).parse().unwrap())
}

#[divan::bench]
fn bench_jid_to_string(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| {
            let mut jid = Jid::lid("123456789012345");
            jid.device = 7;
            jid
        })
        .bench_refs(|jid| black_box(jid.to_string()));
}

#[divan::bench]
fn bench_jid_to_non_ad_string(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| {
            let mut jid = Jid::pn("5511999990000");
            jid.device = 3;
            jid
        })
        .bench_refs(|jid| black_box(jid.to_non_ad_string()));
}

/// The per-recipient fan-out formatter: writes the AD form into a reused
/// buffer instead of allocating a String per device.
#[divan::bench]
fn bench_jid_push_ad_to(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| {
            let mut jid = Jid::lid("123456789012345");
            jid.device = 7;
            (jid, String::with_capacity(64))
        })
        .bench_refs(|(jid, buf)| {
            jid.push_ad_to(buf);
            // black-box the contents, not just the length: observing only
            // `len` lets LLVM elide the actual formatting writes.
            black_box(buf.as_bytes());
        });
}
