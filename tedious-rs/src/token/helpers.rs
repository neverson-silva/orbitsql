use crate::errors::{NotEnoughDataError, ParseResult};

fn need(buf: &[u8], offset: usize, count: usize) -> Result<(), NotEnoughDataError> {
    if buf.len() < offset.saturating_add(count) {
        Err(NotEnoughDataError::new(offset.saturating_add(count)))
    } else {
        Ok(())
    }
}
pub fn read_u8(buf: &[u8], offset: usize) -> Result<ParseResult<u8>, NotEnoughDataError> {
    need(buf, offset, 1)?;
    Ok(ParseResult::new(buf[offset], offset + 1))
}
pub fn read_u16_le(buf: &[u8], offset: usize) -> Result<ParseResult<u16>, NotEnoughDataError> {
    need(buf, offset, 2)?;
    Ok(ParseResult::new(
        u16::from_le_bytes([buf[offset], buf[offset + 1]]),
        offset + 2,
    ))
}
pub fn read_i16_le(buf: &[u8], offset: usize) -> Result<ParseResult<i16>, NotEnoughDataError> {
    read_u16_le(buf, offset).map(|r| ParseResult::new(r.value as i16, r.offset))
}
pub fn read_u24_le(buf: &[u8], offset: usize) -> Result<ParseResult<u32>, NotEnoughDataError> {
    need(buf, offset, 3)?;
    Ok(ParseResult::new(
        u32::from(buf[offset])
            | (u32::from(buf[offset + 1]) << 8)
            | (u32::from(buf[offset + 2]) << 16),
        offset + 3,
    ))
}
pub fn read_u32_le(buf: &[u8], offset: usize) -> Result<ParseResult<u32>, NotEnoughDataError> {
    need(buf, offset, 4)?;
    Ok(ParseResult::new(
        u32::from_le_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ]),
        offset + 4,
    ))
}
pub fn read_u32_be(buf: &[u8], offset: usize) -> Result<ParseResult<u32>, NotEnoughDataError> {
    need(buf, offset, 4)?;
    Ok(ParseResult::new(
        u32::from_be_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ]),
        offset + 4,
    ))
}
pub fn read_u40_le(buf: &[u8], offset: usize) -> Result<ParseResult<u64>, NotEnoughDataError> {
    need(buf, offset, 5)?;
    Ok(ParseResult::new(
        u64::from(buf[offset])
            | (u64::from(buf[offset + 1]) << 8)
            | (u64::from(buf[offset + 2]) << 16)
            | (u64::from(buf[offset + 3]) << 24)
            | (u64::from(buf[offset + 4]) << 32),
        offset + 5,
    ))
}
pub fn read_i32_le(buf: &[u8], offset: usize) -> Result<ParseResult<i32>, NotEnoughDataError> {
    read_u32_le(buf, offset).map(|r| ParseResult::new(r.value as i32, r.offset))
}
pub fn read_u64_le(buf: &[u8], offset: usize) -> Result<ParseResult<u64>, NotEnoughDataError> {
    need(buf, offset, 8)?;
    Ok(ParseResult::new(
        u64::from_le_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
            buf[offset + 4],
            buf[offset + 5],
            buf[offset + 6],
            buf[offset + 7],
        ]),
        offset + 8,
    ))
}
pub fn read_i64_le(buf: &[u8], offset: usize) -> Result<ParseResult<i64>, NotEnoughDataError> {
    read_u64_le(buf, offset).map(|r| ParseResult::new(r.value as i64, r.offset))
}
pub fn read_f32_le(buf: &[u8], offset: usize) -> Result<ParseResult<f32>, NotEnoughDataError> {
    read_u32_le(buf, offset).map(|r| ParseResult::new(f32::from_bits(r.value), r.offset))
}
pub fn read_f64_le(buf: &[u8], offset: usize) -> Result<ParseResult<f64>, NotEnoughDataError> {
    read_u64_le(buf, offset).map(|r| ParseResult::new(f64::from_bits(r.value), r.offset))
}
pub fn read_b_var_char(
    buf: &[u8],
    offset: usize,
) -> Result<ParseResult<String>, NotEnoughDataError> {
    let count = read_u8(buf, offset)?;
    let bytes = count.value as usize * 2;
    need(buf, count.offset, bytes)?;
    let units: Vec<u16> = buf[count.offset..count.offset + bytes]
        .chunks_exact(2)
        .map(|x| u16::from_le_bytes([x[0], x[1]]))
        .collect();
    Ok(ParseResult::new(
        String::from_utf16_lossy(&units),
        count.offset + bytes,
    ))
}
pub fn read_b_var_byte(
    buf: &[u8],
    offset: usize,
) -> Result<ParseResult<Vec<u8>>, NotEnoughDataError> {
    let count = read_u8(buf, offset)?;
    need(buf, count.offset, count.value as usize)?;
    Ok(ParseResult::new(
        buf[count.offset..count.offset + count.value as usize].to_vec(),
        count.offset + count.value as usize,
    ))
}
pub fn read_us_var_char(
    buf: &[u8],
    offset: usize,
) -> Result<ParseResult<String>, NotEnoughDataError> {
    let count = read_u16_le(buf, offset)?;
    let bytes = count.value as usize * 2;
    need(buf, count.offset, bytes)?;
    let units: Vec<u16> = buf[count.offset..count.offset + bytes]
        .chunks_exact(2)
        .map(|x| u16::from_le_bytes([x[0], x[1]]))
        .collect();
    Ok(ParseResult::new(
        String::from_utf16_lossy(&units),
        count.offset + bytes,
    ))
}
pub fn read_us_var_byte(
    buf: &[u8],
    offset: usize,
) -> Result<ParseResult<Vec<u8>>, NotEnoughDataError> {
    let count = read_u16_le(buf, offset)?;
    need(buf, count.offset, count.value as usize)?;
    Ok(ParseResult::new(
        buf[count.offset..count.offset + count.value as usize].to_vec(),
        count.offset + count.value as usize,
    ))
}
pub fn read_u_numeric64_le(
    buf: &[u8],
    offset: usize,
) -> Result<ParseResult<u64>, NotEnoughDataError> {
    read_u64_le(buf, offset)
}
pub fn read_u_numeric96_le(
    buf: &[u8],
    offset: usize,
) -> Result<ParseResult<u128>, NotEnoughDataError> {
    need(buf, offset, 12)?;
    let a = u32::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ]) as u128;
    let b = u32::from_le_bytes([
        buf[offset + 4],
        buf[offset + 5],
        buf[offset + 6],
        buf[offset + 7],
    ]) as u128;
    let c = u32::from_le_bytes([
        buf[offset + 8],
        buf[offset + 9],
        buf[offset + 10],
        buf[offset + 11],
    ]) as u128;
    Ok(ParseResult::new(a | b << 32 | c << 64, offset + 12))
}
pub fn read_u_numeric128_le(
    buf: &[u8],
    offset: usize,
) -> Result<ParseResult<u128>, NotEnoughDataError> {
    need(buf, offset, 16)?;
    Ok(ParseResult::new(
        u128::from_le_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
            buf[offset + 4],
            buf[offset + 5],
            buf[offset + 6],
            buf[offset + 7],
            buf[offset + 8],
            buf[offset + 9],
            buf[offset + 10],
            buf[offset + 11],
            buf[offset + 12],
            buf[offset + 13],
            buf[offset + 14],
            buf[offset + 15],
        ]),
        offset + 16,
    ))
}
