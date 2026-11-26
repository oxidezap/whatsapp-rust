#![forbid(unsafe_code)]

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Eof,
    Varint,
    Tag,
    Wire(u32),
    Mismatch {
        field: u32,
        expected: u8,
        actual: u8,
    },
    Utf8,
    Depth,
    EndGroup(u32),
    Size,
}

pub const MAX_SIZE: usize = 0x7fff_ffff;
pub const MAX_DEPTH: u32 = 100;

pub struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(input: &'a [u8]) -> Result<Self, Error> {
        if input.len() > MAX_SIZE {
            return Err(Error::Size);
        }
        Ok(Self { remaining: input })
    }

    pub fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    pub fn varint(&mut self) -> Result<u64, Error> {
        let mut value = 0;
        for shift in (0..70).step_by(7) {
            let (&byte, rest) = self.remaining.split_first().ok_or(Error::Eof)?;
            self.remaining = rest;
            if shift == 63 && byte > 1 {
                return Err(Error::Varint);
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte < 128 {
                return Ok(value);
            }
        }
        Err(Error::Varint)
    }

    pub fn tag(&mut self) -> Result<(u32, u8), Error> {
        let value = self.varint()?;
        let tag = u32::try_from(value).map_err(|_| Error::Tag)?;
        let wire = (tag & 7) as u8;
        if wire > 5 {
            return Err(Error::Wire(u32::from(wire)));
        }
        if tag >> 3 == 0 {
            return Err(Error::Tag);
        }
        Ok((tag >> 3, wire))
    }

    pub fn take(&mut self, len: usize) -> Result<&'a [u8], Error> {
        if len > self.remaining.len() {
            return Err(Error::Eof);
        }
        let (value, rest) = self.remaining.split_at(len);
        self.remaining = rest;
        Ok(value)
    }

    pub fn bytes(&mut self) -> Result<&'a [u8], Error> {
        let len = usize::try_from(self.varint()?).map_err(|_| Error::Size)?;
        self.take(len)
    }

    pub fn string(&mut self) -> Result<&'a str, Error> {
        std::str::from_utf8(self.bytes()?).map_err(|_| Error::Utf8)
    }

    pub fn fixed32(&mut self) -> Result<u32, Error> {
        let bytes: [u8; 4] = self.take(4)?.try_into().map_err(|_| Error::Eof)?;
        Ok(u32::from_le_bytes(bytes))
    }

    pub fn fixed64(&mut self) -> Result<u64, Error> {
        let bytes: [u8; 8] = self.take(8)?.try_into().map_err(|_| Error::Eof)?;
        Ok(u64::from_le_bytes(bytes))
    }

    pub fn skip(&mut self, field: u32, wire: u8, depth: u32) -> Result<(), Error> {
        match wire {
            0 => {
                self.varint()?;
            }
            1 => {
                self.take(8)?;
            }
            2 => {
                self.bytes()?;
            }
            3 => {
                let depth = depth.checked_sub(1).ok_or(Error::Depth)?;
                loop {
                    let (number, wire) = self.tag()?;
                    if wire == 4 {
                        if number != field {
                            return Err(Error::EndGroup(number));
                        }
                        break;
                    }
                    self.skip(number, wire, depth)?;
                }
            }
            5 => {
                self.take(4)?;
            }
            other => return Err(Error::Wire(u32::from(other))),
        }
        Ok(())
    }
}

pub fn check(field: u32, actual: u8, expected: u8) -> Result<(), Error> {
    if actual != expected {
        return Err(Error::Mismatch {
            field,
            actual,
            expected,
        });
    }
    Ok(())
}

pub fn varint(mut value: u64, output: &mut Vec<u8>) {
    while value >= 128 {
        output.push(value as u8 | 128);
        value >>= 7;
    }
    output.push(value as u8);
}

pub fn varint_len(value: u64) -> usize {
    ((64 - value.leading_zeros()).max(1) as usize).div_ceil(7)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_boundaries_and_overflow() {
        for value in [0, 127, 128, 16383, 16384, u32::MAX as u64, u64::MAX] {
            let mut output = Vec::new();
            varint(value, &mut output);
            assert_eq!(output.len(), varint_len(value));
            assert_eq!(Reader::new(&output).unwrap().varint(), Ok(value));
            if output.len() > 1 {
                output.pop();
                assert_eq!(Reader::new(&output).unwrap().varint(), Err(Error::Eof));
            }
        }
        assert_eq!(
            Reader::new(&[255; 10]).unwrap().varint(),
            Err(Error::Varint)
        );
        assert_eq!(Reader::new(&[128, 0]).unwrap().varint(), Ok(0));
    }
}
