use crate::errors::InputError;

pub mod writable_tracking_buffer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Ucs2,
    Ascii,
}

pub struct WritableTrackingBuffer {
    pub initial_size: usize,
    pub encoding: Encoding,
    pub double_size_growth: bool,
    buffer: Vec<u8>,
    composite_buffer: Vec<u8>,
    pub position: usize,
}

impl WritableTrackingBuffer {
    pub fn new(initial_size: usize, encoding: Option<Encoding>, double_size_growth: bool) -> Self {
        Self {
            initial_size,
            encoding: encoding.unwrap_or(Encoding::Ucs2),
            double_size_growth,
            buffer: vec![0; initial_size],
            composite_buffer: Vec::new(),
            position: 0,
        }
    }
    pub fn data(&mut self) -> Vec<u8> {
        self.new_buffer(0);
        self.composite_buffer.clone()
    }
    pub fn length(&self) -> usize {
        self.composite_buffer.len() + self.position
    }
    pub fn copy_from(&mut self, value: &[u8]) {
        self.make_room_for(value.len());
        self.buffer[self.position..self.position + value.len()].copy_from_slice(value);
        self.position += value.len();
    }
    pub fn make_room_for(&mut self, required: usize) {
        if self.buffer.len() - self.position < required {
            if self.double_size_growth {
                let mut size = self.buffer.len().saturating_mul(2).max(128);
                while size < required {
                    size = size.saturating_mul(2);
                }
                self.new_buffer(size)
            } else {
                self.new_buffer(required)
            }
        }
    }
    fn new_buffer(&mut self, size: usize) {
        self.composite_buffer
            .extend_from_slice(&self.buffer[..self.position]);
        self.buffer = vec![0; size];
        self.position = 0;
    }
    fn put(&mut self, bytes: &[u8]) {
        self.copy_from(bytes)
    }
    pub fn write_u8(&mut self, value: u8) {
        self.put(&[value])
    }
    pub fn write_u16_le(&mut self, value: u16) {
        self.put(&value.to_le_bytes())
    }
    pub fn write_ushort(&mut self, value: u16) {
        self.write_u16_le(value)
    }
    pub fn write_u16_be(&mut self, value: u16) {
        self.put(&value.to_be_bytes())
    }
    pub fn write_u24_le(&mut self, value: u32) {
        self.put(&[
            (value & 255) as u8,
            ((value >> 8) & 255) as u8,
            ((value >> 16) & 255) as u8,
        ])
    }
    pub fn write_u32_le(&mut self, value: u32) {
        self.put(&value.to_le_bytes())
    }
    pub fn patch_u32_le(&mut self, offset: usize, value: u32) {
        let composite_length = self.composite_buffer.len();
        if offset.saturating_add(4) <= composite_length {
            self.composite_buffer[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        } else if offset >= composite_length
            && offset.saturating_add(4) <= composite_length.saturating_add(self.position)
        {
            let local_offset = offset - composite_length;
            self.buffer[local_offset..local_offset + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    pub fn write_i64_le(&mut self, value: i64) {
        self.put(&value.to_le_bytes())
    }
    pub fn write_u64_le(&mut self, value: u64) {
        self.put(&value.to_le_bytes())
    }

    pub fn write_big_i64_le(&mut self, value: i64) {
        self.write_i64_le(value)
    }

    pub fn write_big_u64_le(&mut self, value: u64) {
        self.write_u64_le(value)
    }
    pub fn write_u32_be(&mut self, value: u32) {
        self.put(&value.to_be_bytes())
    }
    pub fn write_u40_le(&mut self, value: u64) {
        self.write_u32_le(value as u32);
        self.write_u8((value >> 32) as u8)
    }
    pub fn write_i8(&mut self, value: i8) {
        self.put(&value.to_le_bytes())
    }
    pub fn write_i16_le(&mut self, value: i16) {
        self.put(&value.to_le_bytes())
    }
    pub fn write_i16_be(&mut self, value: i16) {
        self.put(&value.to_be_bytes())
    }
    pub fn write_i32_le(&mut self, value: i32) {
        self.put(&value.to_le_bytes())
    }
    pub fn write_i32_be(&mut self, value: i32) {
        self.put(&value.to_be_bytes())
    }
    pub fn write_f32_le(&mut self, value: f32) {
        self.put(&value.to_le_bytes())
    }
    pub fn write_f64_le(&mut self, value: f64) {
        self.put(&value.to_le_bytes())
    }
    pub fn write_string(
        &mut self,
        value: &str,
        encoding: Option<Encoding>,
    ) -> Result<(), InputError> {
        let encoding = encoding.unwrap_or(self.encoding);
        let bytes = encode(value, encoding)?;
        self.put(&bytes);
        Ok(())
    }
    pub fn write_b_varchar(
        &mut self,
        value: &str,
        encoding: Option<Encoding>,
    ) -> Result<(), InputError> {
        let length = value.encode_utf16().count();
        if length > usize::from(u8::MAX) {
            return Err(InputError("B_VARCHAR length exceeds u8".to_owned()));
        }
        self.write_u8(length as u8);
        self.write_string(value, encoding)
    }
    pub fn write_us_varchar(
        &mut self,
        value: &str,
        encoding: Option<Encoding>,
    ) -> Result<(), InputError> {
        let length = value.encode_utf16().count();
        if length > usize::from(u16::MAX) {
            return Err(InputError("US_VARCHAR length exceeds u16".to_owned()));
        }
        self.write_u16_le(length as u16);
        self.write_string(value, encoding)
    }
    pub fn write_us_varbyte(&mut self, value: &[u8]) -> Result<(), InputError> {
        if value.len() > usize::from(u16::MAX) {
            return Err(InputError("US_VARBYTE length exceeds u16".to_owned()));
        }
        self.write_u16_le(value.len() as u16);
        self.put(value);
        Ok(())
    }
    pub fn write_plp_body(&mut self, value: &[u8]) {
        self.put(&[0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
        if !value.is_empty() {
            self.write_u32_le(value.len() as u32);
            self.put(value)
        }
        self.write_u32_le(0)
    }
    pub fn write_buffer(&mut self, value: &[u8]) {
        self.put(value)
    }
    pub fn write_money(&mut self, value: f64) {
        self.write_i32_le((value / 4294967296.0).floor() as i32);
        self.write_i32_le(value as i32)
    }
}

fn encode(value: &str, encoding: Encoding) -> Result<Vec<u8>, InputError> {
    match encoding {
        Encoding::Utf8 => Ok(value.as_bytes().to_vec()),
        Encoding::Ascii => Ok(value.bytes().map(|b| b & 0x7f).collect()),
        Encoding::Ucs2 => Ok(value.encode_utf16().flat_map(u16::to_le_bytes).collect()),
    }
}
