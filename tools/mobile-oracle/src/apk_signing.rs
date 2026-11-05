//! Structural v2/v3 certificate extraction per source.android.com APK signing
//! scheme v2/v3 formats. Signatures, digests and rotation proofs are not verified.
use anyhow::{Context, Result, ensure};
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SigningScheme {
    V2,
    V3,
}
struct Fields<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Fields<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }
    fn word(&mut self) -> Result<u32> {
        let value = u32::from_le_bytes(
            self.bytes
                .get(self.pos..self.pos + 4)
                .context("truncated signing word")?
                .try_into()?,
        );
        self.pos += 4;
        Ok(value)
    }
    fn field(&mut self) -> Result<&'a [u8]> {
        let len = self.word()? as usize;
        let end = self
            .pos
            .checked_add(len)
            .context("signing length overflow")?;
        let value = self
            .bytes
            .get(self.pos..end)
            .context("truncated signing field")?;
        self.pos = end;
        Ok(value)
    }
    fn done(&self) -> Result<()> {
        ensure!(self.pos == self.bytes.len(), "trailing signing bytes");
        Ok(())
    }
}
pub(crate) fn certificates(
    block: &[u8],
    scheme: SigningScheme,
    platform_sdk: u32,
) -> Result<Vec<Vec<u8>>> {
    ensure!(
        block.len() >= 32 && block.len() <= 16 * 1024 * 1024,
        "signing block budget exceeded"
    );
    let size = u64::from_le_bytes(block[..8].try_into()?);
    ensure!(
        size == block.len() as u64 - 8
            && block[block.len() - 16..] == *b"APK Sig Block 42"
            && u64::from_le_bytes(block[block.len() - 24..block.len() - 16].try_into()?) == size,
        "invalid signing block footer"
    );
    let target = match scheme {
        SigningScheme::V2 => 0x7109871a,
        SigningScheme::V3 => 0xf05368c0,
    };
    let mut pos = 8;
    let end = block.len() - 24;
    let mut selected = None;
    let mut ids = std::collections::BTreeSet::new();
    while pos < end {
        ensure!(end - pos >= 8, "truncated signing pair length");
        let size = u64::from_le_bytes(
            block
                .get(pos..pos + 8)
                .context("truncated signing pair")?
                .try_into()?,
        );
        pos += 8;
        ensure!(
            size >= 4 && size <= (end - pos) as u64,
            "invalid signing pair length"
        );
        let next = pos + size as usize;
        let id = u32::from_le_bytes(block[pos..pos + 4].try_into()?);
        ensure!(ids.insert(id), "duplicate signing block ID");
        if id == target {
            selected = Some(&block[pos + 4..next]);
        }
        pos = next;
    }
    ensure!(pos == end, "signing pairs overlap footer");
    let mut container = Fields::new(selected.context("selected signing scheme absent")?);
    let mut signers = Fields::new(container.field()?);
    container.done()?;
    let mut output = Vec::new();
    let mut signer_count = 0;
    let mut total = 0;
    while signers.pos < signers.bytes.len() {
        signer_count += 1;
        ensure!(signer_count <= 32, "signer budget exceeded");
        let mut signer = Fields::new(signers.field()?);
        let mut signed = Fields::new(signer.field()?);
        signed.field()?;
        let cert_bytes = signed.field()?;
        let applicable = if scheme == SigningScheme::V3 {
            let low = signed.word()?;
            let high = signed.word()?;
            ensure!(low <= high, "invalid signer SDK range");
            ensure!(
                signer.word()? == low && signer.word()? == high,
                "signer SDK range mismatch"
            );
            (low..=high).contains(&platform_sdk)
        } else {
            true
        };
        signed.field()?;
        signed.done()?;
        signer.field()?;
        signer.field()?;
        signer.done()?;
        let mut certs = Fields::new(cert_bytes);
        while certs.pos < certs.bytes.len() {
            let cert = certs.field()?;
            ensure!(
                !cert.is_empty() && cert.len() <= 65536 && cert[0] == 0x30,
                "invalid certificate envelope"
            );
            crate::apk_der::validate_certificate_envelope(cert)?;
            if applicable {
                total += cert.len();
                ensure!(
                    output.len() < 32 && total <= 1024 * 1024,
                    "certificate budget exceeded"
                );
                output.push(cert.to_vec());
            }
        }
    }
    ensure!(
        !output.is_empty(),
        "no applicable certificates for explicit scheme/platform"
    );
    Ok(output)
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    fn field(b: &[u8]) -> Vec<u8> {
        let mut o = (b.len() as u32).to_le_bytes().to_vec();
        o.extend(b);
        o
    }
    pub(crate) fn fixture(scheme: SigningScheme) -> Vec<u8> {
        let certs = field(&[0x30, 1, 7]);
        let mut signed = field(&[]);
        signed.extend(field(&certs));
        if scheme == SigningScheme::V3 {
            signed.extend(28u32.to_le_bytes());
            signed.extend(35u32.to_le_bytes());
        }
        signed.extend(field(&[]));
        let mut signer = field(&signed);
        if scheme == SigningScheme::V3 {
            signer.extend(28u32.to_le_bytes());
            signer.extend(35u32.to_le_bytes());
        }
        signer.extend(field(&[]));
        signer.extend(field(&[]));
        let value = field(&field(&signer));
        let id: u32 = if scheme == SigningScheme::V2 {
            0x7109871a
        } else {
            0xf05368c0
        };
        let mut pair = ((value.len() + 4) as u64).to_le_bytes().to_vec();
        pair.extend(id.to_le_bytes());
        pair.extend(value);
        let size = (pair.len() + 24) as u64;
        let mut block = size.to_le_bytes().to_vec();
        block.extend(pair);
        block.extend(size.to_le_bytes());
        block.extend(b"APK Sig Block 42");
        block
    }
    #[test]
    fn synthetic_schemes_and_truncation() {
        for scheme in [SigningScheme::V2, SigningScheme::V3] {
            let b = fixture(scheme);
            assert_eq!(
                certificates(&b, scheme, 33).unwrap(),
                vec![vec![0x30, 1, 7]]
            );
            for end in 0..b.len() {
                assert!(certificates(&b[..end], scheme, 33).is_err());
            }
            if scheme == SigningScheme::V3 {
                assert!(certificates(&b, scheme, 27).is_err());
            }
        }
    }
}
