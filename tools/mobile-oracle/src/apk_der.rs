//! Bounded structural extraction of v1 PKCS#7 certificate bytes, not trust validation.
use anyhow::{Result, bail, ensure};

struct Der<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Der<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn next(&mut self) -> Result<(u8, &'a [u8], &'a [u8])> {
        let start = self.offset;
        let tag = *self
            .bytes
            .get(start)
            .ok_or_else(|| anyhow::anyhow!("truncated DER tag"))?;
        ensure!(tag & 31 != 31, "unsupported DER high tag");
        let first = *self
            .bytes
            .get(start + 1)
            .ok_or_else(|| anyhow::anyhow!("truncated DER length"))?;
        let mut offset = start + 2;
        let length = if first & 128 == 0 {
            usize::from(first)
        } else {
            let count = usize::from(first & 127);
            ensure!(
                (1..=4).contains(&count),
                "indefinite or excessive DER length"
            );
            let bytes = self
                .bytes
                .get(offset..offset + count)
                .ok_or_else(|| anyhow::anyhow!("truncated DER length"))?;
            ensure!(bytes[0] != 0, "nonminimal DER length");
            offset += count;
            let length = bytes.iter().fold(0usize, |a, b| (a << 8) | usize::from(*b));
            ensure!(length >= 128, "nonminimal DER long length");
            length
        };
        let end = offset
            .checked_add(length)
            .ok_or_else(|| anyhow::anyhow!("DER length overflow"))?;
        let content = self
            .bytes
            .get(offset..end)
            .ok_or_else(|| anyhow::anyhow!("truncated DER content"))?;
        self.offset = end;
        Ok((tag, content, &self.bytes[start..end]))
    }
    fn expect(&mut self, expected: u8) -> Result<&'a [u8]> {
        let (tag, content, _) = self.next()?;
        ensure!(tag == expected, "unexpected DER tag");
        Ok(content)
    }
    fn finish(&self) -> Result<()> {
        ensure!(self.offset == self.bytes.len(), "trailing DER data");
        Ok(())
    }
}

pub(crate) fn validate_certificate_envelope(bytes: &[u8]) -> Result<()> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= 65_536,
        "certificate budget exceeded"
    );
    let mut der = Der::new(bytes);
    der.expect(0x30)?;
    der.finish()
}

/// Extract ordered certificate choices from ContentInfo/SignedData.
/// SignerInfos and certificate signatures are deliberately not authenticated.
/// APK v2/v3 signing blocks cannot substitute for this v1 material.
pub(crate) fn certificates(bytes: &[u8]) -> Result<Vec<Vec<u8>>> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= 4 * 1024 * 1024,
        "v1 block exceeds budget"
    );
    let mut outer = Der::new(bytes);
    let mut content = Der::new(outer.expect(0x30)?);
    outer.finish()?;
    ensure!(
        content.expect(6)? == [0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 1, 7, 2],
        "not PKCS7 SignedData"
    );
    let mut explicit = Der::new(content.expect(0xa0)?);
    content.finish()?;
    let mut signed = Der::new(explicit.expect(0x30)?);
    explicit.finish()?;
    let version = signed.expect(2)?;
    ensure!(
        !version.is_empty() && version.len() <= 4,
        "invalid SignedData version"
    );
    signed.expect(0x31)?; // digestAlgorithms
    signed.expect(0x30)?; // encapContentInfo
    let (tag, collection, _) = signed.next()?;
    ensure!(
        tag == 0xa0,
        "no v1 certificate collection; v2/v3 extraction unsupported"
    );
    let mut collection = Der::new(collection);
    let mut certificates = Vec::new();
    let mut total = 0;
    while collection.offset < collection.bytes.len() {
        let (tag, _, encoded) = collection.next()?;
        ensure!(tag == 0x30, "unsupported PKCS7 certificate choice");
        ensure!(
            encoded.len() <= 65_536 && certificates.len() < 32,
            "certificate budget exceeded"
        );
        total += encoded.len();
        ensure!(total <= 1_048_576, "certificate set budget exceeded");
        certificates.push(encoded.to_vec());
    }
    ensure!(!certificates.is_empty(), "empty v1 certificate collection");
    let (mut tag, _, _) = signed.next()?;
    if tag == 0xa1 {
        tag = signed.next()?.0;
    }
    if tag != 0x31 {
        bail!("expected PKCS7 signerInfos");
    }
    signed.finish()?;
    Ok(certificates)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    fn tlv(tag: u8, bytes: &[u8]) -> Vec<u8> {
        assert!(bytes.len() < 128);
        let mut out = vec![tag, bytes.len() as u8];
        out.extend(bytes);
        out
    }
    pub(crate) fn fixture(certificates: &[u8]) -> Vec<u8> {
        let mut signed = tlv(2, &[1]);
        signed.extend(tlv(0x31, &[]));
        signed.extend(tlv(0x30, &[]));
        signed.extend(tlv(0xa0, certificates));
        signed.extend(tlv(0x31, &[]));
        let mut content = tlv(6, &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 1, 7, 2]);
        content.extend(tlv(0xa0, &tlv(0x30, &signed)));
        tlv(0x30, &content)
    }
    #[test]
    fn synthetic_structure_retains_order_without_claiming_x509_validity() {
        let certs = [tlv(0x30, &[1]), tlv(0x30, &[2])].concat();
        assert_eq!(
            certificates(&fixture(&certs)).unwrap(),
            vec![tlv(0x30, &[1]), tlv(0x30, &[2])]
        );
        assert!(certificates(&fixture(&tlv(0xa1, &[1]))).is_err());
        assert!(certificates(&fixture(&[])).is_err());
    }
    #[test]
    fn rejects_truncation_trailing_noncanonical_and_indefinite() {
        let good = fixture(&tlv(0x30, &[1]));
        for end in 0..good.len() {
            assert!(certificates(&good[..end]).is_err());
        }
        let mut trailing = good.clone();
        trailing.push(0);
        assert!(certificates(&trailing).is_err());
        assert!(certificates(&[0x30, 0x80, 0, 0]).is_err());
        assert!(certificates(&[0x30, 0x81, 0]).is_err());
    }
}
