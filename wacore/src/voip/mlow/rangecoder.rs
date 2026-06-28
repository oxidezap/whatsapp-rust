//! Opus/CELT range decoder (`ec_dec`, RFC 6716 §4.1), the entropy coder MLow's smpl_audio_codec
//! reuses verbatim. The symbols must match the WhatsApp WASM bit-for-bit. Range-coded symbols come
//! from the front of the buffer, raw bits from the back. `wrapping_*` is used wherever uint32
//! modular arithmetic is required.
//!
//! This is a COMPLETE ec_dec implementation: the raw-bits / icdf / uint primitives are validated by
//! the round-trip vector test but the mlow decode path itself only exercises a subset, so the unused
//! primitives are allowed rather than removed (keeping the entropy coder faithful and reusable).
#![allow(dead_code)]

const EC_SYM_BITS: u32 = 8;
const EC_CODE_BITS: u32 = 32;
const EC_SYM_MAX: u32 = (1 << EC_SYM_BITS) - 1; // 255
const EC_CODE_TOP: u32 = 1u32 << (EC_CODE_BITS - 1);
const EC_CODE_BOT: u32 = EC_CODE_TOP >> EC_SYM_BITS;
const EC_CODE_EXTRA: u32 = (EC_CODE_BITS - 2) % EC_SYM_BITS + 1; // 7
const EC_WINDOW_SIZE: u32 = 32;
const EC_UINT_BITS: i32 = 8;
const EC_CODE_SHIFT: u32 = EC_CODE_BITS - EC_SYM_BITS - 1; // 23

/// EC_ILOG: floor(log2(x))+1 for x>0, 0 for x==0.
#[inline]
fn ilog(x: u32) -> i32 {
    (EC_CODE_BITS - x.leading_zeros()) as i32
}

pub(crate) struct RangeDecoder<'a> {
    buf: &'a [u8],
    storage: u32,
    end_offs: u32,
    end_window: u32,
    nend_bits: i32,
    nbits_total: i32,
    offs: u32,
    rng: u32,
    val: u32,
    ext: u32,
    rem: i32,
    /// Sticky decode error (degenerate/malformed table or exhausted bits). Inspectable so the
    /// higher layers can fail loud instead of synthesizing from garbage.
    pub(crate) err: i32,
}

impl<'a> RangeDecoder<'a> {
    /// RFC 6716 `ec_dec_init`.
    pub(crate) fn new(buf: &'a [u8]) -> Self {
        let mut d = RangeDecoder {
            buf,
            storage: buf.len() as u32,
            end_offs: 0,
            end_window: 0,
            nend_bits: 0,
            nbits_total: EC_CODE_BITS as i32 + 1
                - (((EC_CODE_BITS - EC_CODE_EXTRA) / EC_SYM_BITS) * EC_SYM_BITS) as i32,
            offs: 0,
            rng: 1u32 << EC_CODE_EXTRA,
            val: 0,
            ext: 0,
            rem: 0,
            err: 0,
        };
        d.rem = d.read_byte() as i32;
        d.val = d.rng - 1 - ((d.rem >> (EC_SYM_BITS - EC_CODE_EXTRA)) as u32);
        d.normalize();
        d
    }

    fn read_byte(&mut self) -> u32 {
        if self.offs < self.storage {
            let b = self.buf[self.offs as usize];
            self.offs += 1;
            b as u32
        } else {
            0
        }
    }

    fn read_byte_from_end(&mut self) -> u32 {
        if self.end_offs < self.storage {
            self.end_offs += 1;
            self.buf[(self.storage - self.end_offs) as usize] as u32
        } else {
            0
        }
    }

    fn normalize(&mut self) {
        while self.rng <= EC_CODE_BOT {
            self.nbits_total += EC_SYM_BITS as i32;
            self.rng <<= EC_SYM_BITS;
            let sym0 = self.rem;
            self.rem = self.read_byte() as i32;
            let sym = (sym0 << EC_SYM_BITS | self.rem) >> (EC_SYM_BITS - EC_CODE_EXTRA);
            self.val = (self
                .val
                .wrapping_shl(EC_SYM_BITS)
                .wrapping_add(EC_SYM_MAX & !(sym as u32)))
                & (EC_CODE_TOP - 1);
        }
    }

    /// Cumulative frequency in [0, ft) for the next symbol; caller locates the symbol and calls
    /// `update`.
    pub(crate) fn decode(&mut self, ft: u32) -> u32 {
        if ft == 0 {
            self.err = 1;
            self.ext = 1;
            return 0;
        }
        self.ext = self.rng / ft;
        if self.ext == 0 {
            self.err = 1;
            self.ext = 1;
            return 0;
        }
        let s = self.val / self.ext;
        ft - (s + 1).min(ft)
    }

    fn decode_bin(&mut self, bits_n: u32) -> u32 {
        self.ext = self.rng >> bits_n;
        if self.ext == 0 {
            self.err = 1;
            self.ext = 1;
            return 0;
        }
        let s = self.val / self.ext;
        let ft = 1u32 << bits_n;
        ft - (s + 1).min(ft)
    }

    /// Uniform `nbits`-bit symbol decoded directly off the range stream (sign coder).
    pub(crate) fn decode_raw_symbol(&mut self, nbits: u32) -> u32 {
        let sym = self.decode_bin(nbits);
        self.update(sym, sym + 1, 1u32 << nbits);
        sym
    }

    /// Advance past the symbol with cumulative range [fl,fh) out of ft.
    pub(crate) fn update(&mut self, fl: u32, fh: u32, ft: u32) {
        let s = self.ext.wrapping_mul(ft - fh);
        self.val = self.val.wrapping_sub(s);
        if fl > 0 {
            self.rng = self.ext.wrapping_mul(fh - fl);
        } else {
            self.rng = self.rng.wrapping_sub(s);
        }
        self.normalize();
    }

    /// One bit with P(0) = 1/2^logp (`ec_dec_bit_logp`).
    pub(crate) fn bit_logp(&mut self, logp: u32) -> i32 {
        let r = self.rng;
        let dv = self.val;
        let s = r >> logp;
        let ret = if dv < s { 1 } else { 0 };
        if ret == 0 {
            self.val = dv - s;
            self.rng = r - s;
        } else {
            self.rng = s;
        }
        self.normalize();
        ret
    }

    /// Symbol against an inverse-CDF table (`ec_dec_icdf`); `ftb = log2(ft)`.
    pub(crate) fn decode_icdf(&mut self, icdf: &[u8], ftb: u32) -> i32 {
        if icdf.is_empty() {
            self.err = 1;
            return 0;
        }
        let s0 = self.rng;
        let dv = self.val;
        let r = s0 >> ftb;
        let mut ret: i32 = -1;
        let mut t;
        let mut s = s0;
        loop {
            t = s;
            ret += 1;
            s = r.wrapping_mul(icdf[ret as usize] as u32);
            if dv >= s || ret as usize >= icdf.len() - 1 {
                break;
            }
        }
        self.val = dv - s;
        self.rng = t - s;
        self.normalize();
        ret
    }

    /// Symbol against a u16 CUMULATIVE CDF table (the smpl primitive). Effective total is
    /// `cdf[n-1] - cdf[0]` (a non-zero base is subtracted out).
    pub(crate) fn decode_cdf(&mut self, cdf: &[u16]) -> i32 {
        let n = cdf.len();
        if n < 2 {
            self.err = 1;
            return 0;
        }
        let base = cdf[0] as u32;
        if cdf[n - 1] as u32 <= base {
            self.err = 1;
            return 0;
        }
        let ft = cdf[n - 1] as u32 - base;
        let fs = self.decode(ft);
        let target = base + fs;
        let mut k = 0usize;
        while k < n - 1 {
            if cdf[k + 1] as u32 > target {
                break;
            }
            k += 1;
        }
        self.update(cdf[k] as u32 - base, cdf[k + 1] as u32 - base, ft);
        k as i32
    }

    /// `decode_cdf` against a length-`n` window of `base` starting at `start`, without materializing
    /// the window. Out-of-range positions read 0 (the old `cdf_window` zero-fill), so the math is
    /// identical to `decode_cdf(&cdf_window(base, start, n))`.
    pub(crate) fn decode_cdf_window(&mut self, base: &[u16], start: usize, n: usize) -> i32 {
        if n < 2 {
            self.err = 1;
            return 0;
        }
        let at = |i: usize| -> u32 { base.get(start + i).copied().unwrap_or(0) as u32 };
        let cbase = at(0);
        let last = at(n - 1);
        if last <= cbase {
            self.err = 1;
            return 0;
        }
        let ft = last - cbase;
        let fs = self.decode(ft);
        let target = cbase + fs;
        let mut k = 0usize;
        while k < n - 1 {
            if at(k + 1) > target {
                break;
            }
            k += 1;
        }
        self.update(at(k) - cbase, at(k + 1) - cbase, ft);
        k as i32
    }

    /// `decode_cdf` reading the n-entry u16 CDF directly from a little-endian byte slice
    /// (`bytes.len() == 2*n`), avoiding a `Vec<u16>` for the in-region heap reads.
    pub(crate) fn decode_cdf_le16(&mut self, bytes: &[u8]) -> i32 {
        let n = bytes.len() / 2;
        let at = |i: usize| -> u32 { u16::from_le_bytes([bytes[2 * i], bytes[2 * i + 1]]) as u32 };
        if n < 2 {
            self.err = 1;
            return 0;
        }
        let base = at(0);
        if at(n - 1) <= base {
            self.err = 1;
            return 0;
        }
        let ft = at(n - 1) - base;
        let fs = self.decode(ft);
        let target = base + fs;
        let mut k = 0usize;
        while k < n - 1 {
            if at(k + 1) > target {
                break;
            }
            k += 1;
        }
        self.update(at(k) - base, at(k + 1) - base, ft);
        k as i32
    }

    /// Raw `n` bits from the BACK of the buffer (`ec_dec_bits`), LSB-first.
    pub(crate) fn bits_n(&mut self, n: u32) -> u32 {
        let mut window = self.end_window;
        let mut available = self.nend_bits;
        if (available as u32) < n {
            loop {
                window |= self.read_byte_from_end() << (available as u32);
                available += EC_SYM_BITS as i32;
                if available as u32 > EC_WINDOW_SIZE - EC_SYM_BITS {
                    break;
                }
            }
        }
        let ret = window & ((1u32 << n) - 1);
        window >>= n;
        available -= n as i32;
        self.end_window = window;
        self.nend_bits = available;
        self.nbits_total += n as i32;
        ret
    }

    /// Integer uniformly distributed in [0, ft) for ft>1 (`ec_dec_uint`).
    pub(crate) fn decode_uint(&mut self, ft0: u32) -> u32 {
        let ft = ft0 - 1;
        let mut ftb = ilog(ft);
        if ftb > EC_UINT_BITS {
            ftb -= EC_UINT_BITS;
            let t = (ft >> (ftb as u32)) + 1;
            let s = self.decode(t);
            self.update(s, s + 1, t);
            let v = (s << (ftb as u32)) | self.bits_n(ftb as u32);
            if v <= ft {
                return v;
            }
            self.err = 1;
            return ft;
        }
        let ft = ft + 1;
        let s = self.decode(ft);
        self.update(s, s + 1, ft);
        s
    }

    /// 64-symbol uniform fine-lag read: `ext = rng>>6`,
    /// `sym = clamp(63 - val/ext, 0, 64)`, then `update(sym, sym+1, 64)`.
    pub(crate) fn decode_64_fine_sym(&mut self) -> i32 {
        self.ext = self.rng >> 6;
        if self.ext == 0 {
            self.err = 1;
            self.ext = 1;
            return 0;
        }
        let s = self.val / self.ext;
        let sym = (63i64 - s as i64).clamp(0, 64) as i32;
        self.update(sym as u32, sym as u32 + 1, 64);
        sym
    }

    /// Bits consumed so far, rounded up (`ec_tell`).
    pub(crate) fn tell(&self) -> i32 {
        self.nbits_total - ilog(self.rng)
    }
}

/// Opus/CELT range ENCODER (`ec_enc`), the exact inverse of `RangeDecoder`, used by the mlow
/// ENCODER. Writes range-coded symbols toward the front and raw bits toward the back; `done()`
/// flushes and merges them, after which `bytes()` is the finished payload.
pub(crate) struct RangeEncoder {
    buf: Vec<u8>,
    storage: u32,
    end_offs: u32,
    end_window: u32,
    nend_bits: i32,
    nbits_total: i32,
    offs: u32,
    rng: u32,
    val: u32,
    ext: u32,
    rem: i32,
    err: i32,
}

impl RangeEncoder {
    pub(crate) fn new(size: usize) -> Self {
        RangeEncoder {
            buf: vec![0u8; size],
            storage: size as u32,
            end_offs: 0,
            end_window: 0,
            nend_bits: 0,
            nbits_total: EC_CODE_BITS as i32 + 1,
            offs: 0,
            rng: EC_CODE_TOP,
            val: 0,
            ext: 0,
            rem: -1,
            err: 0,
        }
    }

    pub(crate) fn err(&self) -> i32 {
        self.err
    }

    fn write_byte(&mut self, b: u32) {
        if self.offs + self.end_offs < self.storage {
            self.buf[self.offs as usize] = b as u8;
            self.offs += 1;
        } else {
            self.err = -1;
        }
    }

    fn write_byte_at_end(&mut self, b: u32) {
        if self.offs + self.end_offs < self.storage {
            self.end_offs += 1;
            self.buf[(self.storage - self.end_offs) as usize] = b as u8;
        } else {
            self.err = -1;
        }
    }

    fn carry_out(&mut self, c: i32) {
        if c as u32 != EC_SYM_MAX {
            let carry = c >> EC_SYM_BITS;
            if self.rem >= 0 {
                self.write_byte((self.rem + carry) as u32);
            }
            if self.ext > 0 {
                let sym = ((EC_SYM_MAX as i32 + carry) & EC_SYM_MAX as i32) as u32;
                loop {
                    self.write_byte(sym);
                    self.ext -= 1;
                    if self.ext == 0 {
                        break;
                    }
                }
            }
            self.rem = c & EC_SYM_MAX as i32;
        } else {
            self.ext += 1;
        }
    }

    fn normalize(&mut self) {
        while self.rng <= EC_CODE_BOT {
            self.carry_out((self.val >> EC_CODE_SHIFT) as i32);
            self.val = self.val.wrapping_shl(EC_SYM_BITS) & (EC_CODE_TOP - 1);
            self.rng <<= EC_SYM_BITS;
            self.nbits_total += EC_SYM_BITS as i32;
        }
    }

    pub(crate) fn encode(&mut self, fl: u32, fh: u32, ft: u32) {
        if ft == 0 {
            self.err = -1;
            return;
        }
        let r = self.rng / ft;
        if fl > 0 {
            self.val = self
                .val
                .wrapping_add(self.rng.wrapping_sub(r.wrapping_mul(ft - fl)));
            self.rng = r.wrapping_mul(fh - fl);
        } else {
            self.rng = self.rng.wrapping_sub(r.wrapping_mul(ft - fh));
        }
        self.normalize();
    }

    pub(crate) fn bit_logp(&mut self, val: i32, logp: u32) {
        let r = self.rng;
        let l = self.val;
        let s = r >> logp;
        let r2 = r - s;
        if val != 0 {
            self.val = l.wrapping_add(r2);
            self.rng = s;
        } else {
            self.rng = r2;
        }
        self.normalize();
    }

    pub(crate) fn encode_icdf(&mut self, s: i32, icdf: &[u8], ftb: u32) {
        let r = self.rng >> ftb;
        if s > 0 {
            self.val = self.val.wrapping_add(
                self.rng
                    .wrapping_sub(r.wrapping_mul(icdf[(s - 1) as usize] as u32)),
            );
            self.rng = r.wrapping_mul(icdf[(s - 1) as usize].wrapping_sub(icdf[s as usize]) as u32);
        } else {
            self.rng = self
                .rng
                .wrapping_sub(r.wrapping_mul(icdf[s as usize] as u32));
        }
        self.normalize();
    }

    /// Inverse of `decode_cdf`: encode symbol `s` against a u16 cumulative CDF (`ft = cdf[n-1]-cdf[0]`).
    pub(crate) fn encode_cdf(&mut self, s: i32, cdf: &[u16]) {
        let n = cdf.len();
        if n < 2 || s < 0 || (s + 1) as usize >= n {
            self.err = -1;
            return;
        }
        let base = cdf[0] as u32;
        if cdf[n - 1] as u32 <= base {
            self.err = -1;
            return;
        }
        let ft = cdf[n - 1] as u32 - base;
        self.encode(
            cdf[s as usize] as u32 - base,
            cdf[(s + 1) as usize] as u32 - base,
            ft,
        );
    }

    /// `encode_cdf` against a length-`n` window of `base` starting at `start`, without materializing
    /// the window. Out-of-range positions read 0, matching `encode_cdf(&cdf_window(base, start, n))`.
    pub(crate) fn encode_cdf_window(&mut self, s: i32, base: &[u16], start: usize, n: usize) {
        if n < 2 || s < 0 || (s + 1) as usize >= n {
            self.err = -1;
            return;
        }
        let at = |i: usize| -> u32 { base.get(start + i).copied().unwrap_or(0) as u32 };
        let cbase = at(0);
        let last = at(n - 1);
        if last <= cbase {
            self.err = -1;
            return;
        }
        let ft = last - cbase;
        self.encode(at(s as usize) - cbase, at((s + 1) as usize) - cbase, ft);
    }

    /// Raw `n` bits toward the back of the buffer.
    pub(crate) fn bits_n(&mut self, fl: u32, n: u32) {
        let mut window = self.end_window;
        let mut used = self.nend_bits;
        if used + n as i32 > EC_WINDOW_SIZE as i32 {
            loop {
                self.write_byte_at_end(window & EC_SYM_MAX);
                window >>= EC_SYM_BITS;
                used -= EC_SYM_BITS as i32;
                if used < EC_SYM_BITS as i32 {
                    break;
                }
            }
        }
        window |= fl.wrapping_shl(used as u32);
        used += n as i32;
        self.end_window = window;
        self.nend_bits = used;
        self.nbits_total += n as i32;
    }

    pub(crate) fn encode_uint(&mut self, fl: u32, ft0: u32) {
        let ft = ft0 - 1;
        let ftb = ilog(ft);
        if ftb > EC_UINT_BITS {
            let ftb = (ftb - EC_UINT_BITS) as u32;
            let t = (ft >> ftb) + 1;
            self.encode(fl >> ftb, (fl >> ftb) + 1, t);
            self.bits_n(fl & ((1u32 << ftb) - 1), ftb);
        } else {
            self.encode(fl, fl + 1, ft + 1);
        }
    }

    /// Inverse of `decode_raw_symbol`: encode a uniform `nbits`-bit symbol on the range stream.
    pub(crate) fn encode_raw_symbol(&mut self, sym: u32, nbits: u32) {
        self.encode(sym, sym + 1, 1u32 << nbits);
    }

    /// Inverse of `decode_64_fine_sym`: encode the 64-symbol uniform fine-lag value.
    pub(crate) fn encode_64_fine_sym(&mut self, sym: i32) {
        self.encode(sym as u32, sym as u32 + 1, 64);
    }

    /// Flush the range coder and merge the back raw-bit stream. After this, `bytes()` is the payload.
    pub(crate) fn done(&mut self) {
        let mut l = EC_CODE_BITS as i32 - ilog(self.rng);
        let mut msk = (EC_CODE_TOP - 1) >> (l as u32);
        let mut end = self.val.wrapping_add(msk) & !msk;
        if end | msk >= self.val.wrapping_add(self.rng) {
            l += 1;
            msk >>= 1;
            end = self.val.wrapping_add(msk) & !msk;
        }
        while l > 0 {
            self.carry_out((end >> EC_CODE_SHIFT) as i32);
            end = end.wrapping_shl(EC_SYM_BITS) & (EC_CODE_TOP - 1);
            l -= EC_SYM_BITS as i32;
        }
        if self.rem >= 0 || self.ext > 0 {
            self.carry_out(0);
        }
        let mut window = self.end_window;
        let mut used = self.nend_bits;
        while used >= EC_SYM_BITS as i32 {
            self.write_byte_at_end(window & EC_SYM_MAX);
            window >>= EC_SYM_BITS;
            used -= EC_SYM_BITS as i32;
        }
        if self.err == 0 {
            for i in self.offs..(self.storage - self.end_offs) {
                self.buf[i as usize] = 0;
            }
            if used > 0 {
                if self.end_offs >= self.storage - self.offs {
                    self.err = -1;
                } else {
                    self.buf[(self.storage - self.end_offs - 1) as usize] |= window as u8;
                }
            }
        }
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Meaningful body length = front (range) bytes + back (raw-bit) bytes; the gap between is
    /// zero-fill padding that `done()` wrote. The smpl encode path issues no back-bit ops, so
    /// `end_offs` stays 0 and this is just the front length; if a back-bit op is ever added, the
    /// front/back layout must be compacted (per the C `ec_enc_shrink`) before this stays correct.
    pub(crate) fn consumed_len(&self) -> usize {
        (self.offs + self.end_offs) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn vectors() -> Value {
        serde_json::from_str(include_str!("testdata/rc_vectors.json"))
            .expect("rc_vectors.json must parse")
    }

    // Replays a deterministic mixed script (icdf / raw back-bits / bit_logp / uint) from the
    // captured vectors, requiring identical decoded values; proves ec_dec parity bit-for-bit.
    #[test]
    fn range_decoder_matches_go_vectors() {
        let v = vectors();
        // The dumped `icdf` is the fixed table the captured script encoded against (ftb=8).
        let icdf: [u8; 6] = [255, 200, 150, 90, 30, 0];
        let bytes = hex::decode(v["bytesHex"].as_str().unwrap()).unwrap();
        let mut d = RangeDecoder::new(&bytes);
        for (i, op) in v["ops"].as_array().unwrap().iter().enumerate() {
            let kind = op["kind"].as_u64().unwrap();
            let a = op["a"].as_u64().unwrap() as u32;
            let b = op["b"].as_u64().unwrap() as u32;
            match kind {
                0 => assert_eq!(d.decode_icdf(&icdf, 8), a as i32, "op {i} icdf"),
                1 => assert_eq!(d.bits_n(a), b, "op {i} bits({a})"),
                2 => assert_eq!(d.bit_logp(a) as u32, b, "op {i} bit_logp({a})"),
                3 => assert_eq!(d.decode_uint(a), b, "op {i} uint(ft={a})"),
                _ => unreachable!("bad op kind {kind}"),
            }
        }
        assert_eq!(d.err, 0, "no decode error");
    }

    // Exercises decodeCDF against cumulative tables (including non-zero-base ones).
    #[test]
    fn range_decoder_cdf_matches_go_vectors() {
        let v = vectors();
        let tables: Vec<Vec<u16>> = v["cdfTables"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| {
                t.as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_u64().unwrap() as u16)
                    .collect()
            })
            .collect();
        let bytes = hex::decode(v["cdfBytesHex"].as_str().unwrap()).unwrap();
        let mut d = RangeDecoder::new(&bytes);
        for (i, op) in v["cdfOps"].as_array().unwrap().iter().enumerate() {
            let ti = op["kind"].as_u64().unwrap() as usize;
            let sym = op["a"].as_u64().unwrap() as i32;
            assert_eq!(d.decode_cdf(&tables[ti]), sym, "cdf op {i} table {ti}");
        }
        assert_eq!(d.err, 0, "no decode error");
    }

    // Re-encodes the captured script and requires byte-identical output; proves our ec_enc matches
    // the WASM range encoder bit-for-bit (the foundation of the mlow encoder).
    #[test]
    fn range_encoder_matches_go_bytes() {
        let v = vectors();
        let icdf: [u8; 6] = [255, 200, 150, 90, 30, 0];
        let want = hex::decode(v["bytesHex"].as_str().unwrap()).unwrap();
        let mut e = RangeEncoder::new(want.len());
        for op in v["ops"].as_array().unwrap() {
            let kind = op["kind"].as_u64().unwrap();
            let a = op["a"].as_u64().unwrap() as u32;
            let b = op["b"].as_u64().unwrap() as u32;
            match kind {
                0 => e.encode_icdf(a as i32, &icdf, 8),
                1 => e.bits_n(b, a),
                2 => e.bit_logp(b as i32, a),
                3 => e.encode_uint(b, a),
                _ => unreachable!(),
            }
        }
        e.done();
        assert_eq!(e.err(), 0, "encoder error");
        assert_eq!(
            e.bytes(),
            want.as_slice(),
            "encoder output differs from captured vectors"
        );
    }

    // Inverse property over a long LCG-random script mixing every primitive (icdf / cdf with zero
    // and non-zero base / raw back-bits / range-stream raw symbol / 64-fine / uint small+large): a
    // RangeEncoder run then a RangeDecoder run must recover every original symbol with err==0. A
    // broad self-contained complement to the fixed captured vectors that needs no external data.
    #[test]
    fn rangecoder_round_trips_random_scripts() {
        // ftb=8 icdf (strictly decreasing, terminating 0): symbols 0..=4.
        let icdf: [u8; 6] = [255, 200, 150, 90, 30, 0];
        // Cumulative u16 CDFs: a zero-base one (symbols 0..=3) and a non-zero-base one (0..=2).
        let cdf0: [u16; 5] = [0, 3, 7, 12, 20];
        let cdf1: [u16; 4] = [5, 8, 14, 30];

        #[derive(Clone)]
        enum Op {
            Icdf(i32),
            Cdf0(i32),
            Cdf1(i32),
            Bits { fl: u32, n: u32 },
            RawSym { sym: u32, nbits: u32 },
            Fine(i32),
            Uint { fl: u32, ft0: u32 },
        }

        // Build one deterministic script of mixed ops from an LCG seed.
        let build = |seed0: u32| -> Vec<Op> {
            let mut s = seed0;
            let mut next = || {
                s = s.wrapping_mul(1664525).wrapping_add(1013904223);
                s
            };
            let mut ops = Vec::with_capacity(256);
            for _ in 0..256 {
                let r = next();
                let op = match r % 7 {
                    0 => Op::Icdf((r >> 8) as i32 % 5),
                    1 => Op::Cdf0((r >> 8) as i32 % 4),
                    2 => Op::Cdf1((r >> 8) as i32 % 3),
                    3 => {
                        let n = 1 + (r >> 8) % 16; // 1..=16 raw back-bits
                        Op::Bits {
                            fl: next() & ((1u32 << n) - 1),
                            n,
                        }
                    }
                    4 => {
                        let nbits = 1 + (r >> 8) % 6; // range-stream uniform symbol
                        Op::RawSym {
                            sym: next() & ((1u32 << nbits) - 1),
                            nbits,
                        }
                    }
                    5 => Op::Fine((r >> 8) as i32 % 64), // 64-symbol uniform: 0..=63
                    _ => {
                        // Alternate small (ftb<=8) and large (ftb>8) uint branches.
                        let ft0 = if r & 1 == 0 { 200 } else { 2000 };
                        Op::Uint {
                            fl: next() % ft0,
                            ft0,
                        }
                    }
                };
                ops.push(op);
            }
            ops
        };

        for seed in [0x1357_9bdfu32, 0x2468_ace0, 0xdead_beef, 0x0badf00d] {
            let ops = build(seed);
            let mut e = RangeEncoder::new(4096);
            for op in &ops {
                match *op {
                    Op::Icdf(s) => e.encode_icdf(s, &icdf, 8),
                    Op::Cdf0(s) => e.encode_cdf(s, &cdf0),
                    Op::Cdf1(s) => e.encode_cdf(s, &cdf1),
                    Op::Bits { fl, n } => e.bits_n(fl, n),
                    Op::RawSym { sym, nbits } => e.encode_raw_symbol(sym, nbits),
                    Op::Fine(sym) => e.encode_64_fine_sym(sym),
                    Op::Uint { fl, ft0 } => e.encode_uint(fl, ft0),
                }
            }
            e.done();
            assert_eq!(e.err(), 0, "seed {seed:#x}: encoder error");
            // Decode the full buffer: the encoder lays raw back-bits at the physical end of `storage`,
            // so the decoder must see the same storage size to mirror the front/back read offsets.
            // Truncating to `consumed_len()` is only valid when no back-bit op is used.
            let bytes = e.bytes().to_vec();

            let mut d = RangeDecoder::new(&bytes);
            for (i, op) in ops.iter().enumerate() {
                match *op {
                    Op::Icdf(s) => assert_eq!(d.decode_icdf(&icdf, 8), s, "seed {seed:#x} op {i}"),
                    Op::Cdf0(s) => assert_eq!(d.decode_cdf(&cdf0), s, "seed {seed:#x} op {i}"),
                    Op::Cdf1(s) => assert_eq!(d.decode_cdf(&cdf1), s, "seed {seed:#x} op {i}"),
                    Op::Bits { fl, n } => assert_eq!(d.bits_n(n), fl, "seed {seed:#x} op {i}"),
                    Op::RawSym { sym, nbits } => {
                        assert_eq!(d.decode_raw_symbol(nbits), sym, "seed {seed:#x} op {i}")
                    }
                    Op::Fine(sym) => {
                        assert_eq!(d.decode_64_fine_sym(), sym, "seed {seed:#x} op {i}")
                    }
                    Op::Uint { fl, ft0 } => {
                        assert_eq!(d.decode_uint(ft0), fl, "seed {seed:#x} op {i}")
                    }
                }
            }
            assert_eq!(d.err, 0, "seed {seed:#x}: decoder error");
        }
    }

    #[test]
    fn range_encoder_cdf_matches_go_bytes() {
        let v = vectors();
        let tables: Vec<Vec<u16>> = v["cdfTables"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| {
                t.as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_u64().unwrap() as u16)
                    .collect()
            })
            .collect();
        let want = hex::decode(v["cdfBytesHex"].as_str().unwrap()).unwrap();
        let mut e = RangeEncoder::new(want.len());
        for op in v["cdfOps"].as_array().unwrap() {
            let ti = op["kind"].as_u64().unwrap() as usize;
            let sym = op["a"].as_u64().unwrap() as i32;
            e.encode_cdf(sym, &tables[ti]);
        }
        e.done();
        assert_eq!(e.err(), 0, "encoder error");
        assert_eq!(
            e.bytes(),
            want.as_slice(),
            "cdf encoder output differs from captured vectors"
        );
    }
}
