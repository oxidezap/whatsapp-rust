//! Byte ancestry for untransformed members, never APK authenticity or tool execution.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use zip::CompressionMethod;

use crate::{
    ApkLock, ExtractionMode, ExtractionStatus, ExtractionVerification, SET_LIMIT, hash_reader,
    open_artifact,
};

const CENTRAL_LIMIT: u64 = 16 * 1024 * 1024;
const EOCD_FIXED: usize = 22;
const CENTRAL_FIXED: usize = 46;

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn validate_extra(extra: &[u8]) -> Result<()> {
    let mut at = 0;
    while at < extra.len() {
        let header = extra
            .get(at..at + 4)
            .context("APK ZIP extra header truncated")?;
        ensure!(u16_at(header, 0) != 1, "unsupported ZIP64 APK extra field");
        at += 4 + u16_at(header, 2) as usize;
        ensure!(at <= extra.len(), "APK ZIP extra payload truncated");
    }
    Ok(())
}

/// Bound central metadata before the general ZIP parser allocates any records.
/// This deliberately accepts classic single-disk ZIP only, not ZIP64 or SFX.
fn preflight(file: &mut File) -> Result<(usize, u64)> {
    let length = file.metadata()?.len();
    // Include the parser's entire EOCD search window and any preceding ZIP64 locator.
    let tail_length = length.min((EOCD_FIXED + 65536 + 20) as u64) as usize;
    ensure!(tail_length >= EOCD_FIXED, "APK ZIP end record is missing");
    file.seek(SeekFrom::End(-(tail_length as i64)))?;
    let mut tail = vec![0; tail_length];
    file.read_exact(&mut tail)?;
    let offset = (0..=tail.len() - EOCD_FIXED)
        .rev()
        .find(|&at| {
            tail[at..at + 4] == *b"PK\x05\x06"
                && at + EOCD_FIXED + u16_at(&tail, at + 20) as usize == tail.len()
        })
        .context("APK ZIP end record is missing or has trailing bytes")?;
    ensure!(
        tail.windows(4)
            .filter(|part| *part == b"PK\x05\x06")
            .count()
            == 1,
        "unsupported ambiguous APK ZIP end records"
    );
    ensure!(
        !tail
            .windows(4)
            .any(|part| part == b"PK\x06\x06" || part == b"PK\x06\x07"),
        "unsupported ZIP64 APK metadata"
    );
    let end = &tail[offset..];
    ensure!(
        u16_at(end, 4) == 0 && u16_at(end, 6) == 0,
        "unsupported multi-disk APK ZIP"
    );
    let count = u16_at(end, 10);
    let central_size = u32_at(end, 12) as u64;
    let central_start = u32_at(end, 16) as u64;
    ensure!(
        count != u16::MAX && central_size != u32::MAX as u64 && central_start != u32::MAX as u64,
        "unsupported ZIP64 APK"
    );
    ensure!(u16_at(end, 8) == count, "APK ZIP entry counts differ");
    ensure!(
        central_size <= CENTRAL_LIMIT,
        "APK ZIP central directory exceeds budget"
    );
    let eocd_start = length - tail_length as u64 + offset as u64;
    ensure!(
        central_start.checked_add(central_size) == Some(eocd_start),
        "APK ZIP central directory bounds are inconsistent"
    );
    ensure!(
        count as u64 * CENTRAL_FIXED as u64 <= central_size,
        "APK ZIP central directory is truncated"
    );
    file.seek(SeekFrom::Start(central_start))?;
    let mut central = vec![0; central_size as usize];
    file.read_exact(&mut central)?;
    let mut names = BTreeSet::new();
    let mut at = 0;
    let mut local_metadata_total = 0_u64;
    for _ in 0..count {
        let header = central
            .get(at..at + CENTRAL_FIXED)
            .context("APK ZIP central header truncated")?;
        ensure!(
            header[..4] == *b"PK\x01\x02",
            "invalid APK ZIP central header"
        );
        let name_length = u16_at(header, 28) as usize;
        let extra_length = u16_at(header, 30) as usize;
        let comment_length = u16_at(header, 32) as usize;
        ensure!(
            u16_at(header, 34) == 0,
            "unsupported multi-disk APK ZIP member"
        );
        ensure!(
            u32_at(header, 20) != u32::MAX
                && u32_at(header, 24) != u32::MAX
                && u32_at(header, 42) != u32::MAX,
            "unsupported ZIP64 APK member"
        );
        ensure!(
            (u32_at(header, 42) as u64)
                .checked_add(30)
                .is_some_and(|end| end <= central_start),
            "APK ZIP member header escapes content area"
        );
        at += CENTRAL_FIXED;
        let name = central
            .get(at..at + name_length)
            .context("APK ZIP member name truncated")?;
        ensure!(names.insert(name), "duplicate APK ZIP member name");
        file.seek(SeekFrom::Start(u32_at(header, 42) as u64))?;
        let mut local = [0; 30];
        file.read_exact(&mut local)?;
        ensure!(local[..4] == *b"PK\x03\x04", "invalid APK ZIP local header");
        let local_metadata = 30 + u16_at(&local, 26) as u64 + u16_at(&local, 28) as u64;
        local_metadata_total += local_metadata;
        ensure!(
            local_metadata_total <= CENTRAL_LIMIT,
            "APK ZIP local metadata exceeds budget"
        );
        ensure!(
            (u32_at(header, 42) as u64)
                .checked_add(local_metadata)
                .and_then(|start| start.checked_add(u32_at(header, 20) as u64))
                .is_some_and(|end| end <= central_start),
            "APK ZIP member data escapes content area"
        );
        file.seek(SeekFrom::Current(u16_at(&local, 26) as i64))?;
        let mut local_extra = vec![0; u16_at(&local, 28) as usize];
        file.read_exact(&mut local_extra)?;
        validate_extra(&local_extra)?;
        let extra_start = at + name_length;
        let extra = central
            .get(extra_start..extra_start + extra_length)
            .context("APK ZIP extra field truncated")?;
        validate_extra(extra)?;
        at += name_length + extra_length + comment_length;
        ensure!(at <= central.len(), "APK ZIP central record truncated");
    }
    ensure!(
        at == central.len(),
        "APK ZIP central directory has unparsed bytes"
    );
    file.seek(SeekFrom::Start(0))?;
    Ok((count as usize, central_start))
}

pub(crate) fn verify(lock: &ApkLock, root: &Path) -> Result<Vec<ExtractionVerification>> {
    let artifacts: BTreeMap<_, _> = lock.artifacts.iter().map(|a| (&a.path, a)).collect();
    let mut groups: BTreeMap<_, Vec<_>> = BTreeMap::new();
    let mut results = Vec::with_capacity(lock.extractions.len());
    for extraction in &lock.extractions {
        if extraction.mode == ExtractionMode::ArchiveMember {
            groups
                .entry(&extraction.input_apk_path)
                .or_default()
                .push(extraction);
        } else {
            results.push(ExtractionVerification {
                output_path: extraction.output_path.clone(),
                input_apk_path: extraction.input_apk_path.clone(),
                archive_member: extraction.archive_member.clone(),
                status: ExtractionStatus::OperatorDeclared,
            });
        }
    }
    let mut compressed_total = 0_u64;
    for (input, extractions) in groups {
        let artifact = artifacts.get(input).context("missing locked source APK")?;
        let mut file = open_artifact(root, artifact)?;
        let (count, central_start) = preflight(&mut file)?;
        let config = zip::read::Config {
            archive_offset: zip::read::ArchiveOffset::Known(0),
        };
        let mut archive = zip::ZipArchive::with_config(config, file).context("reading APK ZIP")?;
        ensure!(
            archive.len() == count && archive.central_directory_start() == central_start,
            "APK ZIP parser inventory differs from bounded preflight"
        );
        archive
            .validate_header_consistency()
            .context("APK ZIP local/central header mismatch")?;
        let mut decoded_names = BTreeSet::new();
        let mut decoded_name_bytes = 0;
        for name in archive.file_names() {
            let name = name?;
            decoded_name_bytes += name.len() as u64;
            ensure!(
                decoded_name_bytes <= 4 * CENTRAL_LIMIT,
                "APK ZIP decoded names exceed budget"
            );
            ensure!(
                decoded_names.insert(name.into_owned()),
                "duplicate decoded APK ZIP member name"
            );
        }
        for extraction in extractions {
            let output = artifacts
                .get(&extraction.output_path)
                .context("missing locked extraction output")?;
            let mut member = archive
                .by_name(&extraction.archive_member)
                .context("reading selected APK ZIP member")?;
            ensure!(!member.encrypted(), "unsupported encrypted APK ZIP member");
            ensure!(
                member.is_file() && !member.is_symlink(),
                "selected APK ZIP member is not a regular file"
            );
            ensure!(
                member.name_raw() == extraction.archive_member.as_bytes()
                    && member.name()?.as_ref() == extraction.archive_member,
                "selected APK ZIP member name is ambiguous"
            );
            ensure!(
                matches!(
                    member.compression(),
                    CompressionMethod::Stored | CompressionMethod::Deflated
                ),
                "unsupported APK ZIP member compression"
            );
            ensure!(
                member.size() == output.size_bytes,
                "APK member/output size mismatch"
            );
            compressed_total = compressed_total
                .checked_add(member.compressed_size())
                .context("compressed member budget overflow")?;
            ensure!(
                compressed_total <= SET_LIMIT,
                "APK member compressed-read budget exceeded"
            );
            let (size, digest) = hash_reader(&mut member.by_ref().take(output.size_bytes + 1))?;
            ensure!(
                size == output.size_bytes && digest == output.sha256,
                "APK archive member/output bytes differ: {}",
                extraction.output_path
            );
            results.push(ExtractionVerification {
                output_path: extraction.output_path.clone(),
                input_apk_path: extraction.input_apk_path.clone(),
                archive_member: extraction.archive_member.clone(),
                status: ExtractionStatus::ArchiveMemberVerified,
            });
        }
    }
    results.sort_by(|a, b| a.output_path.cmp(&b.output_path));
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn repeated_local_metadata_cannot_exceed_aggregate_budget() {
        // Repeated offsets can force a parser to revisit the same local extras.
        let mut bytes = vec![0; 65600];
        bytes[..4].copy_from_slice(b"PK\x03\x04");
        bytes[28..30].copy_from_slice(&65532_u16.to_le_bytes());
        let central_start = bytes.len() as u32;
        for index in 0..257 {
            let name = format!("synthetic-{index}.dex");
            let mut header = [0; 46];
            header[..4].copy_from_slice(b"PK\x01\x02");
            header[28..30].copy_from_slice(&(name.len() as u16).to_le_bytes());
            bytes.extend_from_slice(&header);
            bytes.extend_from_slice(name.as_bytes());
        }
        let central_size = bytes.len() as u32 - central_start;
        let mut end = [0; 22];
        end[..4].copy_from_slice(b"PK\x05\x06");
        end[8..10].copy_from_slice(&257_u16.to_le_bytes());
        end[10..12].copy_from_slice(&257_u16.to_le_bytes());
        end[12..16].copy_from_slice(&central_size.to_le_bytes());
        end[16..20].copy_from_slice(&central_start.to_le_bytes());
        bytes.extend_from_slice(&end);
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&bytes).unwrap();
        let error = preflight(file.as_file_mut()).unwrap_err();
        assert!(error.to_string().contains("local metadata exceeds budget"));
    }
}
