#![forbid(unsafe_code)]

pub trait EncodeSink {
    fn put_u8(&mut self, value: u8);
    fn put_slice(&mut self, value: &[u8]);
    fn put_u32_le(&mut self, value: u32);
    fn put_u64_le(&mut self, value: u64);
    fn put_i32_le(&mut self, value: i32) {
        self.put_u32_le(value as u32);
    }
    fn put_i64_le(&mut self, value: i64) {
        self.put_u64_le(value as u64);
    }
    fn put_f32_le(&mut self, value: f32) {
        self.put_u32_le(value.to_bits());
    }
    fn put_f64_le(&mut self, value: f64) {
        self.put_u64_le(value.to_bits());
    }
    fn put_shared(&mut self, value: bytes::Bytes) {
        self.put_slice(&value);
    }
}

impl<B: bytes::BufMut> EncodeSink for B {
    fn put_u8(&mut self, value: u8) {
        bytes::BufMut::put_u8(self, value);
    }
    fn put_slice(&mut self, value: &[u8]) {
        bytes::BufMut::put_slice(self, value);
    }
    fn put_u32_le(&mut self, value: u32) {
        bytes::BufMut::put_u32_le(self, value);
    }
    fn put_u64_le(&mut self, value: u64) {
        bytes::BufMut::put_u64_le(self, value);
    }
}
