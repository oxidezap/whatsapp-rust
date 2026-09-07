use anyhow::Result;
use std::path::Path;

pub fn compare(expected: &Path, actual: &Path) -> Result<()> {
    let expected = oracle_core::read_media_trace(expected)?;
    let actual = oracle_core::read_media_trace(actual)?;
    oracle_core::compare_media(&expected, &actual)?;
    println!("{} media record(s) match", expected.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn media_compare_checks_persisted_payloads() {
        let dir = tempfile::tempdir().unwrap();
        let expected = dir.path().join("expected");
        let actual = dir.path().join("actual");
        let observation = oracle_core::MediaObservation {
            stream: oracle_core::MediaStream::Audio,
            symbol: "env::audio".to_owned(),
            ordinal: 0,
            sequence: Some(1),
            timestamp: Some(960),
            payload: vec![1, 2, 3],
        };
        oracle_core::write_media_trace(&expected, std::slice::from_ref(&observation)).unwrap();
        oracle_core::write_media_trace(&actual, &[observation]).unwrap();
        compare(&expected, &actual).unwrap();

        std::fs::write(actual.join("record-0000.bin"), [1, 2, 4]).unwrap();
        assert!(compare(&expected, &actual).is_err());
    }
}
