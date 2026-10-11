//! Signal projections are exhaustive even though the public generated API is not.
use std::collections::BTreeSet;
use std::io;

const ROOTS: &[&str] = &[
    "IdentityKeyPairStructure",
    "PreKeyRecordStructure",
    "SignedPreKeyRecordStructure",
    "RecordStructure",
    "SessionStructure",
    "SenderKeyRecordStructure",
    "SenderKeyStateStructure",
];

pub fn check(api: &BTreeSet<String>) -> io::Result<()> {
    let actual: BTreeSet<_> = api
        .iter()
        .filter(|line| {
            line.strip_prefix("wire .whatsapp.")
                .and_then(|path| path.split_once('.'))
                .is_some_and(|(root, _)| ROOTS.contains(&root))
        })
        .map(String::as_str)
        .collect();
    let expected: BTreeSet<_> = include_str!("../signal-storage.snapshot").lines().collect();
    if actual != expected {
        return Err(io::Error::other(format!(
            "Signal persistence schema changed. Update and test record_components, sender-key \
             and skipped-key projections before updating signal-storage.snapshot. \
             Added: {:?}; removed: {:?}",
            actual.difference(&expected).collect::<Vec<_>>(),
            expected.difference(&actual).collect::<Vec<_>>()
        )));
    }
    Ok(())
}
