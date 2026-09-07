//! Validated integer widths shared by embind registration, ABI and memory reads.
use crate::Value;
use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IntegerType {
    bytes: u8,
    signed: bool,
}

impl IntegerType {
    pub(crate) fn new(bytes: u32, signed: bool) -> Result<Self> {
        ensure!(
            matches!(bytes, 1 | 2 | 4 | 8),
            "unsupported integer width {bytes}"
        );
        Ok(Self {
            bytes: bytes as u8,
            signed,
        })
    }
    pub(crate) fn named(name: &str) -> Option<Self> {
        let (bytes, signed) = match name {
            "char" | "signed char" => (1, true),
            "unsigned char" => (1, false),
            "short" => (2, true),
            "unsigned short" => (2, false),
            "int" | "long" => (4, true),
            "unsigned int" | "unsigned long" => (4, false),
            "int64_t" | "long long" => (8, true),
            "uint64_t" | "unsigned long long" => (8, false),
            _ => return None,
        };
        Some(Self { bytes, signed })
    }
    pub(crate) fn bytes(self) -> u8 {
        self.bytes
    }
    pub(crate) fn signed(self) -> bool {
        self.signed
    }
    pub(crate) fn decode(self, raw: u64) -> Value {
        let bits = u32::from(self.bytes) * 8;
        let raw = if bits == 64 {
            raw
        } else {
            raw & ((1_u64 << bits) - 1)
        };
        if self.signed {
            let shift = 64 - bits;
            Value::Int(((raw << shift) as i64) >> shift)
        } else if let Ok(value) = i64::try_from(raw) {
            Value::Int(value)
        } else {
            Value::UInt(raw)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_widths_and_unsigned_high_bits() {
        for width in [0, 3, 5, 16, u32::MAX] {
            assert!(IntegerType::new(width, false).is_err());
        }
        assert_eq!(
            IntegerType::named("uint64_t").unwrap().decode(u64::MAX),
            Value::UInt(u64::MAX)
        );
        assert_eq!(
            IntegerType::named("signed char").unwrap().decode(255),
            Value::Int(-1)
        );
    }
}
