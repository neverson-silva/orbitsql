use encoding_rs::{
    BIG5, EUC_KR, GBK, SHIFT_JIS, UTF_8, WINDOWS_874, WINDOWS_1250, WINDOWS_1251, WINDOWS_1252,
    WINDOWS_1253, WINDOWS_1254, WINDOWS_1255, WINDOWS_1256, WINDOWS_1257, WINDOWS_1258,
};

use crate::collation::{Collation, Encoding};
use crate::data_type::type_by_id;
use crate::errors::{NotEnoughDataError, ParseResult};
use crate::metadata_parser::Metadata;
use crate::token::helpers::{
    read_f32_le, read_f64_le, read_i16_le, read_i32_le, read_i64_le, read_u8, read_u16_le,
    read_u24_le, read_u32_le, read_u40_le,
};
use crate::value_codec::Value;
use yore::CodePage;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueParserError {
    NotEnoughData(NotEnoughDataError),
    Unsupported(String),
}
impl From<NotEnoughDataError> for ValueParserError {
    fn from(value: NotEnoughDataError) -> Self {
        Self::NotEnoughData(value)
    }
}

fn bytes(
    buf: &[u8],
    offset: usize,
    length: usize,
) -> Result<ParseResult<Vec<u8>>, ValueParserError> {
    if buf.len() < offset.saturating_add(length) {
        return Err(NotEnoughDataError::new(offset.saturating_add(length)).into());
    }
    Ok(ParseResult::new(
        buf[offset..offset + length].to_vec(),
        offset + length,
    ))
}

fn decode_text(value: &[u8], collation: &Collation) -> Result<String, ValueParserError> {
    if let Some(codepage) = match collation.codepage {
        Some(Encoding::Cp437) => Some(&yore::code_pages::CP437 as &dyn CodePage),
        Some(Encoding::Cp850) => Some(&yore::code_pages::CP850 as &dyn CodePage),
        _ => None,
    } {
        return codepage
            .decode(value)
            .map(|text| text.into_owned())
            .map_err(|_| ValueParserError::Unsupported("invalid text encoding".to_owned()));
    }
    let (decoded, _, _) = match collation.codepage {
        Some(Encoding::Utf8) => UTF_8.decode(value),
        Some(Encoding::Cp1250) => WINDOWS_1250.decode(value),
        Some(Encoding::Cp1251) => WINDOWS_1251.decode(value),
        Some(Encoding::Cp1252) => WINDOWS_1252.decode(value),
        Some(Encoding::Cp1253) => WINDOWS_1253.decode(value),
        Some(Encoding::Cp1254) => WINDOWS_1254.decode(value),
        Some(Encoding::Cp1255) => WINDOWS_1255.decode(value),
        Some(Encoding::Cp1256) => WINDOWS_1256.decode(value),
        Some(Encoding::Cp1257) => WINDOWS_1257.decode(value),
        Some(Encoding::Cp1258) => WINDOWS_1258.decode(value),
        Some(Encoding::Cp874) => WINDOWS_874.decode(value),
        Some(Encoding::Cp932) => SHIFT_JIS.decode(value),
        Some(Encoding::Cp936) => GBK.decode(value),
        Some(Encoding::Cp949) => EUC_KR.decode(value),
        Some(Encoding::Cp950) => BIG5.decode(value),
        Some(Encoding::Cp437 | Encoding::Cp850) => {
            return Err(ValueParserError::Unsupported(
                "unsupported codepage".to_owned(),
            ));
        }
        None => UTF_8.decode(value),
    };
    let decoded = decoded.into_owned();
    if matches!(
        collation.codepage,
        Some(
            Encoding::Cp874
                | Encoding::Cp1250
                | Encoding::Cp1251
                | Encoding::Cp1252
                | Encoding::Cp1253
                | Encoding::Cp1254
                | Encoding::Cp1255
                | Encoding::Cp1256
                | Encoding::Cp1257
                | Encoding::Cp1258
        )
    ) {
        return Ok(decoded
            .chars()
            .zip(value.iter())
            .map(|(character, byte)| {
                if iconv_replacement_byte(collation.codepage, *byte) {
                    '�'
                } else {
                    character
                }
            })
            .collect());
    }
    Ok(decoded)
}

fn iconv_replacement_byte(codepage: Option<Encoding>, byte: u8) -> bool {
    match codepage {
        Some(Encoding::Cp874) => {
            matches!(
                byte,
                0x81..=0x90 | 0x98..=0x9F | 0xDB..=0xDE | 0xFC..=0xFF
            ) && !matches!(byte, 0x85 | 0x91 | 0x92 | 0x93 | 0x94 | 0x95 | 0x96 | 0x97)
        }
        Some(Encoding::Cp1250) => matches!(byte, 0x81 | 0x83 | 0x88 | 0x90 | 0x98),
        Some(Encoding::Cp1251) => byte == 0x98,
        Some(Encoding::Cp1252) => matches!(byte, 0x81 | 0x8D | 0x8F | 0x90 | 0x9D),
        Some(Encoding::Cp1253) => matches!(
            byte,
            0x81 | 0x88 | 0x8A | 0x8C..=0x90 | 0x98 | 0x9A | 0x9C..=0x9F | 0xAA | 0xD2 | 0xFF
        ),
        Some(Encoding::Cp1254) => matches!(byte, 0x81 | 0x8D..=0x90 | 0x9D | 0x9E),
        Some(Encoding::Cp1255) => matches!(
            byte,
            0x81 | 0x8A | 0x8C..=0x90 | 0x9A | 0x9C..=0x9F | 0xD9..=0xDF | 0xFB | 0xFC | 0xFF
        ),
        Some(Encoding::Cp1257) => matches!(
            byte,
            0x81 | 0x83 | 0x88 | 0x8A | 0x8C | 0x90 | 0x98 | 0x9A | 0x9C | 0x9F | 0xA1 | 0xA5
        ),
        Some(Encoding::Cp1258) => matches!(byte, 0x81 | 0x8A | 0x8D..=0x90 | 0x9A | 0x9D | 0x9E),
        _ => false,
    }
}

fn read_length_prefixed(
    buf: &[u8],
    offset: usize,
    width: usize,
) -> Result<ParseResult<Option<Vec<u8>>>, ValueParserError> {
    let length = if width == 1 {
        u32::from(read_u8(buf, offset)?.value)
    } else {
        u32::from(read_u16_le(buf, offset)?.value)
    };
    let next = offset + width;
    if width == 2 && length == u32::from(u16::MAX) {
        return Ok(ParseResult::new(None, next));
    }
    let value = bytes(buf, next, length as usize)?;
    Ok(ParseResult::new(Some(value.value), value.offset))
}

fn read_plp_chunks(
    buf: &[u8],
    offset: usize,
) -> Result<ParseResult<Option<Vec<Vec<u8>>>>, ValueParserError> {
    let mut parser = PlpStreamParser::new();
    let input = buf
        .get(offset..)
        .ok_or_else(|| NotEnoughDataError::new(offset))?;
    let mut cursor = 0;
    let mut chunks = Vec::new();
    while cursor < input.len()
        && !matches!(
            parser.state,
            PlpStreamState::Completed | PlpStreamState::Null
        )
    {
        let result = parser.feed(&input[cursor..])?;
        cursor += result.consumed;
        if let Some(chunk) = result.chunk {
            chunks.push(chunk);
        }
        if result.consumed == 0 {
            break;
        }
    }
    parser.finish()?;
    let value = if matches!(parser.state, PlpStreamState::Null) {
        None
    } else {
        Some(chunks)
    };
    Ok(ParseResult::new(value, offset + cursor))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlpStreamState {
    ReadingTotalLength,
    ReadingChunkLength,
    ReadingChunk,
    Completed,
    Null,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlpStreamOutput {
    pub consumed: usize,
    pub chunk: Option<Vec<u8>>,
}

impl PlpStreamOutput {
    fn empty() -> Self {
        Self {
            consumed: 0,
            chunk: None,
        }
    }
}

pub struct PlpStreamParser {
    state: PlpStreamState,
    header: Vec<u8>,
    chunk: Vec<u8>,
    chunk_remaining: usize,
    total_length: Option<u64>,
    emitted_length: u64,
    paused: bool,
    terminal_error: Option<ValueParserError>,
}

impl std::fmt::Debug for PlpStreamParser {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PlpStreamParser")
            .field("state", &self.state)
            .field("header_length", &self.header.len())
            .field("chunk_length", &self.chunk.len())
            .field("chunk_remaining", &self.chunk_remaining)
            .field("total_length", &self.total_length)
            .field("emitted_length", &self.emitted_length)
            .field("paused", &self.paused)
            .field("terminal_error", &self.terminal_error)
            .finish()
    }
}

impl PlpStreamParser {
    const UNKNOWN_PLP_LENGTH: u64 = 0xFFFFFFFFFFFFFFFE;

    pub fn new() -> Self {
        Self {
            state: PlpStreamState::ReadingTotalLength,
            header: Vec::with_capacity(8),
            chunk: Vec::new(),
            chunk_remaining: 0,
            total_length: None,
            emitted_length: 0,
            paused: false,
            terminal_error: None,
        }
    }

    pub fn state(&self) -> &PlpStreamState {
        &self.state
    }

    pub fn total_length(&self) -> Option<u64> {
        self.total_length
    }

    pub fn emitted_length(&self) -> u64 {
        self.emitted_length
    }

    pub fn pause(&mut self) {
        if !matches!(
            self.state,
            PlpStreamState::Completed | PlpStreamState::Null | PlpStreamState::Failed
        ) {
            self.paused = true;
        }
    }

    pub fn resume(&mut self) {
        if !matches!(
            self.state,
            PlpStreamState::Completed | PlpStreamState::Null | PlpStreamState::Failed
        ) {
            self.paused = false;
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    fn terminal_result(&self) -> Result<PlpStreamOutput, ValueParserError> {
        match &self.terminal_error {
            Some(error) => Err(error.clone()),
            None => Ok(PlpStreamOutput::empty()),
        }
    }

    fn fail(&mut self, error: ValueParserError) -> Result<PlpStreamOutput, ValueParserError> {
        self.state = PlpStreamState::Failed;
        self.paused = false;
        self.terminal_error = Some(error.clone());
        Err(error)
    }

    fn validate_total(&mut self) -> Result<(), ValueParserError> {
        let total =
            u64::from_le_bytes(self.header[..8].try_into().map_err(|_| {
                ValueParserError::Unsupported("invalid PLP total length".to_owned())
            })?);
        self.header.clear();
        if total == u64::MAX {
            self.total_length = None;
            self.state = PlpStreamState::Null;
            return Ok(());
        }
        self.total_length = if total == Self::UNKNOWN_PLP_LENGTH {
            None
        } else {
            Some(total)
        };
        self.state = PlpStreamState::ReadingChunkLength;
        Ok(())
    }

    fn validate_chunk_length(&mut self) -> Result<bool, ValueParserError> {
        let length =
            u32::from_le_bytes(self.header[..4].try_into().map_err(|_| {
                ValueParserError::Unsupported("invalid PLP chunk length".to_owned())
            })?);
        self.header.clear();
        if length == 0 {
            let total = self.total_length.unwrap_or(Self::UNKNOWN_PLP_LENGTH);
            if total != Self::UNKNOWN_PLP_LENGTH && total != self.emitted_length {
                return Err(ValueParserError::Unsupported(format!(
                    "PLP length mismatch: expected {}, got {} bytes",
                    total, self.emitted_length
                )));
            }
            self.state = PlpStreamState::Completed;
            return Ok(true);
        }
        self.chunk_remaining = usize::try_from(length)
            .map_err(|_| ValueParserError::Unsupported("PLP chunk length overflow".to_owned()))?;
        self.chunk.clear();
        self.state = PlpStreamState::ReadingChunk;
        Ok(false)
    }

    pub fn feed(&mut self, input: &[u8]) -> Result<PlpStreamOutput, ValueParserError> {
        if self.terminal_error.is_some() {
            return self.terminal_result();
        }
        if self.paused || matches!(self.state, PlpStreamState::Completed | PlpStreamState::Null) {
            return Ok(PlpStreamOutput::empty());
        }
        let mut output = PlpStreamOutput::empty();
        let mut cursor = 0;
        while cursor < input.len() {
            match self.state {
                PlpStreamState::ReadingTotalLength => {
                    let needed = 8 - self.header.len();
                    let count = needed.min(input.len() - cursor);
                    self.header
                        .extend_from_slice(&input[cursor..cursor + count]);
                    cursor += count;
                    if self.header.len() == 8
                        && let Err(error) = self.validate_total()
                    {
                        return self.fail(error);
                    }
                }
                PlpStreamState::ReadingChunkLength => {
                    let needed = 4 - self.header.len();
                    let count = needed.min(input.len() - cursor);
                    self.header
                        .extend_from_slice(&input[cursor..cursor + count]);
                    cursor += count;
                    if self.header.len() == 4 {
                        match self.validate_chunk_length() {
                            Ok(true) => break,
                            Ok(false) => {}
                            Err(error) => return self.fail(error),
                        }
                    }
                }
                PlpStreamState::ReadingChunk => {
                    let count = self.chunk_remaining.min(input.len() - cursor);
                    self.chunk.extend_from_slice(&input[cursor..cursor + count]);
                    self.chunk_remaining -= count;
                    cursor += count;
                    if self.chunk_remaining == 0 {
                        let chunk = std::mem::take(&mut self.chunk);
                        self.emitted_length =
                            match self.emitted_length.checked_add(chunk.len() as u64) {
                                Some(length) => length,
                                None => {
                                    return self.fail(ValueParserError::Unsupported(
                                        "PLP length overflow".to_owned(),
                                    ));
                                }
                            };
                        output.chunk = Some(chunk);
                        self.state = PlpStreamState::ReadingChunkLength;
                        break;
                    }
                }
                PlpStreamState::Completed | PlpStreamState::Null | PlpStreamState::Failed => break,
            }
        }
        output.consumed = cursor;
        Ok(output)
    }

    pub fn finish(&mut self) -> Result<PlpStreamOutput, ValueParserError> {
        if self.terminal_error.is_some() {
            return self.terminal_result();
        }
        if matches!(self.state, PlpStreamState::Completed | PlpStreamState::Null) {
            return Ok(PlpStreamOutput::empty());
        }
        let needed = match self.state {
            PlpStreamState::ReadingTotalLength => 8 - self.header.len(),
            PlpStreamState::ReadingChunkLength => 4 - self.header.len(),
            PlpStreamState::ReadingChunk => self.chunk_remaining,
            PlpStreamState::Completed | PlpStreamState::Null | PlpStreamState::Failed => 0,
        };
        self.fail(NotEnoughDataError::new(needed.max(1)).into())
    }
}

impl Default for PlpStreamParser {
    fn default() -> Self {
        Self::new()
    }
}

fn read_plp(buf: &[u8], offset: usize) -> Result<ParseResult<Option<Vec<u8>>>, ValueParserError> {
    let chunks = read_plp_chunks(buf, offset)?;
    Ok(ParseResult::new(
        chunks
            .value
            .map(|chunks| chunks.into_iter().flatten().collect()),
        chunks.offset,
    ))
}

pub fn is_plp_stream(metadata: &Metadata) -> bool {
    match metadata.data_type.id {
        0xA5 | 0xA7 | 0xE7 => metadata.data_length == Some(u32::from(u16::MAX)),
        0xF0 | 0xF1 => true,
        _ => false,
    }
}

pub fn read_plp_stream(
    buf: &[u8],
    offset: usize,
) -> Result<ParseResult<Option<Vec<Vec<u8>>>>, ValueParserError> {
    read_plp_chunks(buf, offset)
}

fn read_long_value(buf: &[u8], offset: usize) -> Result<ParseResult<Vec<u8>>, ValueParserError> {
    let length = read_u32_le(buf, offset)?;
    let value = bytes(buf, length.offset, length.value as usize)?;
    Ok(value)
}

fn read_legacy_lob(
    buf: &[u8],
    offset: usize,
    id: u8,
    metadata: &Metadata,
) -> Result<ParseResult<Value>, ValueParserError> {
    let pointer_length = read_u8(buf, offset)?;
    if pointer_length.value == 0 {
        return Ok(ParseResult::new(Value::Null, pointer_length.offset));
    }
    let pointer = bytes(
        buf,
        pointer_length.offset,
        usize::from(pointer_length.value),
    )?;
    let timestamp = bytes(buf, pointer.offset, 8)?;
    let value = read_long_value(buf, timestamp.offset)?;
    let raw = value.value;
    if id == 0x22 {
        return Ok(ParseResult::new(Value::Binary(raw), value.offset));
    }
    if id == 0x63 {
        let units = raw
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        return Ok(ParseResult::new(
            Value::Text(String::from_utf16_lossy(&units)),
            value.offset,
        ));
    }
    let collation = metadata
        .collation
        .as_ref()
        .ok_or_else(|| ValueParserError::Unsupported("missing collation".to_owned()))?;
    Ok(ParseResult::new(
        Value::Text(decode_text(&raw, collation)?),
        value.offset,
    ))
}

fn date_from_days(days: i64, use_utc: bool) -> Value {
    let z = days - 719_162 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    if use_utc {
        Value::Date {
            year: year as i32,
            month: month as u8,
            day: day as u8,
        }
    } else {
        Value::LocalDate {
            year: year as i32,
            month: month as u8,
            day: day as u8,
        }
    }
}

fn time_from_ticks(ticks: u64, scale: u8, use_utc: bool) -> Value {
    let nanos = ticks.saturating_mul(10_u64.pow(u32::from(9 - scale)));
    let seconds = nanos / 1_000_000_000;
    if use_utc {
        Value::Time {
            hour: (seconds / 3600) as u8,
            minute: ((seconds / 60) % 60) as u8,
            second: (seconds % 60) as u8,
            nanosecond: (nanos % 1_000_000_000) as u32,
        }
    } else {
        Value::LocalTime {
            hour: (seconds / 3600) as u8,
            minute: ((seconds / 60) % 60) as u8,
            second: (seconds % 60) as u8,
            nanosecond: (nanos % 1_000_000_000) as u32,
        }
    }
}

fn datetime_from_parts(date: Value, time: Value, use_utc: bool) -> Value {
    if use_utc {
        Value::DateTime {
            date: Box::new(date),
            time: Box::new(time),
        }
    } else {
        Value::LocalDateTime {
            date: Box::new(date),
            time: Box::new(time),
        }
    }
}

fn legacy_datetime_from_parts(days: i64, ticks: i64, use_utc: bool) -> Value {
    let milliseconds = if ticks >= 0 {
        (ticks * 1_000 + 150) / 300
    } else {
        -((-ticks * 1_000) / 300)
    };
    let day_milliseconds = 86_400_000_i64;
    let day_delta = milliseconds.div_euclid(day_milliseconds);
    let remainder = milliseconds.rem_euclid(day_milliseconds);
    let date = date_from_days(days + 693_595 + day_delta, use_utc);
    let time = if use_utc {
        Value::Time {
            hour: (remainder / 3_600_000) as u8,
            minute: ((remainder / 60_000) % 60) as u8,
            second: ((remainder / 1_000) % 60) as u8,
            nanosecond: ((remainder % 1_000) * 1_000_000) as u32,
        }
    } else {
        Value::LocalTime {
            hour: (remainder / 3_600_000) as u8,
            minute: ((remainder / 60_000) % 60) as u8,
            second: ((remainder / 1_000) % 60) as u8,
            nanosecond: ((remainder % 1_000) * 1_000_000) as u32,
        }
    };
    datetime_from_parts(date, time, use_utc)
}

fn read_temporal_time(
    buf: &[u8],
    offset: usize,
    scale: u8,
    use_utc: bool,
) -> Result<ParseResult<Value>, ValueParserError> {
    let width = match scale {
        0..=2 => 3,
        3..=4 => 4,
        5..=7 => 5,
        _ => {
            return Err(ValueParserError::Unsupported(
                "invalid temporal scale".to_owned(),
            ));
        }
    };
    let ticks = match width {
        3 => read_u24_le(buf, offset)?.value as u64,
        4 => read_u32_le(buf, offset)?.value as u64,
        _ => read_u40_le(buf, offset)?.value,
    };
    Ok(ParseResult::new(
        time_from_ticks(ticks, scale, use_utc),
        offset + width,
    ))
}

fn temporal_data_length(scale: u8) -> Result<usize, ValueParserError> {
    match scale {
        0..=2 => Ok(3),
        3..=4 => Ok(4),
        5..=7 => Ok(5),
        _ => Err(ValueParserError::Unsupported(
            "invalid temporal scale".to_owned(),
        )),
    }
}

fn read_decimal(
    buf: &[u8],
    offset: usize,
    width: usize,
    scale: u8,
) -> Result<ParseResult<Value>, ValueParserError> {
    if !matches!(width, 4 | 8 | 12 | 16) {
        return Err(ValueParserError::Unsupported(format!(
            "Unsupported numeric dataLength {}",
            width + 1
        )));
    }
    let sign = read_u8(buf, offset)?;
    let raw = bytes(buf, sign.offset, width)?;
    let mut magnitude_bytes = [0u8; 16];
    magnitude_bytes[..width].copy_from_slice(&raw.value);
    let magnitude = u128::from_le_bytes(magnitude_bytes);
    let unscaled = if sign.value == 1 {
        i128::try_from(magnitude).map_err(|_| {
            ValueParserError::Unsupported(
                "decimal magnitude exceeds signed i128 positive range".to_owned(),
            )
        })?
    } else if magnitude == 1u128 << 127 {
        i128::MIN
    } else {
        i128::try_from(magnitude)
            .map_err(|_| {
                ValueParserError::Unsupported(
                    "decimal magnitude exceeds signed i128 negative range".to_owned(),
                )
            })?
            .checked_neg()
            .ok_or_else(|| {
                ValueParserError::Unsupported(
                    "decimal magnitude exceeds signed i128 negative range".to_owned(),
                )
            })?
    };
    Ok(ParseResult::new(
        Value::Decimal { unscaled, scale },
        raw.offset,
    ))
}

fn decimal_from_scaled(value: i128, scale: u8) -> Value {
    Value::Decimal {
        unscaled: value,
        scale,
    }
}

pub fn read_value(
    buf: &[u8],
    offset: usize,
    metadata: &Metadata,
) -> Result<ParseResult<Value>, ValueParserError> {
    read_value_with_options(buf, offset, metadata, true)
}

pub fn read_value_with_options(
    buf: &[u8],
    offset: usize,
    metadata: &Metadata,
    use_utc: bool,
) -> Result<ParseResult<Value>, ValueParserError> {
    let id = metadata.data_type.id;
    match id {
        0x1F => Ok(ParseResult::new(Value::Null, offset)),
        0x24 => {
            let length = read_u8(buf, offset)?;
            match length.value {
                0 => Ok(ParseResult::new(Value::Null, length.offset)),
                16 => {
                    let raw = bytes(buf, length.offset, 16)?;
                    let guid = crate::guid_parser::buffer_to_upper_case_guid(&raw.value)
                        .ok_or_else(|| ValueParserError::Unsupported("invalid GUID".to_owned()))?;
                    Ok(ParseResult::new(Value::Guid(guid), raw.offset))
                }
                value => Err(ValueParserError::Unsupported(format!(
                    "invalid uniqueidentifier length {value}"
                ))),
            }
        }
        0x30 => read_u8(buf, offset)
            .map(|v| ParseResult::new(Value::Integer(i64::from(v.value)), v.offset))
            .map_err(Into::into),
        0x34 => read_i16_le(buf, offset)
            .map(|v| ParseResult::new(Value::Integer(i64::from(v.value)), v.offset))
            .map_err(Into::into),
        0x38 => read_i32_le(buf, offset)
            .map(|v| ParseResult::new(Value::Integer(i64::from(v.value)), v.offset))
            .map_err(Into::into),
        0x7F => read_i64_le(buf, offset)
            .map(|v| ParseResult::new(Value::BigInteger(v.value.to_string()), v.offset))
            .map_err(Into::into),
        0x3B => read_f32_le(buf, offset)
            .map(|v| ParseResult::new(Value::Real(v.value), v.offset))
            .map_err(Into::into),
        0x3E => read_f64_le(buf, offset)
            .map(|v| ParseResult::new(Value::Float(v.value), v.offset))
            .map_err(Into::into),
        0x32 => read_u8(buf, offset)
            .map(|v| ParseResult::new(Value::Boolean(v.value != 0), v.offset))
            .map_err(Into::into),
        0x26 => {
            let length = read_u8(buf, offset)?;
            match length.value {
                0 => Ok(ParseResult::new(Value::Null, length.offset)),
                1 => read_u8(buf, length.offset)
                    .map(|v| ParseResult::new(Value::Integer(i64::from(v.value)), v.offset))
                    .map_err(Into::into),
                2 => read_i16_le(buf, length.offset)
                    .map(|v| ParseResult::new(Value::Integer(i64::from(v.value)), v.offset))
                    .map_err(Into::into),
                4 => read_i32_le(buf, length.offset)
                    .map(|v| ParseResult::new(Value::Integer(i64::from(v.value)), v.offset))
                    .map_err(Into::into),
                8 => read_i64_le(buf, length.offset)
                    .map(|v| ParseResult::new(Value::BigInteger(v.value.to_string()), v.offset))
                    .map_err(Into::into),
                _ => Err(ValueParserError::Unsupported(format!(
                    "Unsupported dataLength {} for IntN",
                    length.value
                ))),
            }
        }
        0x68 => {
            let length = read_u8(buf, offset)?;
            match length.value {
                0 => Ok(ParseResult::new(Value::Null, length.offset)),
                1 => read_u8(buf, length.offset)
                    .map(|v| ParseResult::new(Value::Boolean(v.value != 0), v.offset))
                    .map_err(Into::into),
                _ => Err(ValueParserError::Unsupported(format!(
                    "Unsupported dataLength {} for BitN",
                    length.value
                ))),
            }
        }
        0x6D => {
            let length = read_u8(buf, offset)?;
            match length.value {
                0 => Ok(ParseResult::new(Value::Null, length.offset)),
                4 => read_f32_le(buf, length.offset)
                    .map(|v| ParseResult::new(Value::Real(v.value), v.offset))
                    .map_err(Into::into),
                8 => read_f64_le(buf, length.offset)
                    .map(|v| ParseResult::new(Value::Float(v.value), v.offset))
                    .map_err(Into::into),
                _ => Err(ValueParserError::Unsupported(format!(
                    "Unsupported dataLength {} for FloatN",
                    length.value
                ))),
            }
        }
        0x28 => {
            let length = read_u8(buf, offset)?;
            if length.value == 0 {
                return Ok(ParseResult::new(Value::Null, length.offset));
            }
            if length.value != 3 {
                return Err(ValueParserError::Unsupported(format!(
                    "invalid date length {}",
                    length.value
                )));
            }
            let value = read_u24_le(buf, length.offset)?;
            Ok(ParseResult::new(
                date_from_days(i64::from(value.value), use_utc),
                value.offset,
            ))
        }
        0x29 => {
            let scale = metadata.scale.unwrap_or(0);
            let length = read_u8(buf, offset)?;
            if length.value == 0 {
                return Ok(ParseResult::new(Value::Null, length.offset));
            }
            let expected = temporal_data_length(scale)?;
            if usize::from(length.value) != expected {
                return Err(ValueParserError::Unsupported(format!(
                    "invalid time length {}",
                    length.value
                )));
            }
            read_temporal_time(buf, length.offset, scale, use_utc)
        }
        0x2A => {
            let scale = metadata.scale.unwrap_or(0);
            let length = read_u8(buf, offset)?;
            if length.value == 0 {
                return Ok(ParseResult::new(Value::Null, length.offset));
            }
            let expected = temporal_data_length(scale)? + 3;
            if usize::from(length.value) != expected {
                return Err(ValueParserError::Unsupported(format!(
                    "invalid datetime2 length {}",
                    length.value
                )));
            }
            let time = read_temporal_time(buf, length.offset, scale, use_utc)?;
            let date = read_u24_le(buf, time.offset)?;
            Ok(ParseResult::new(
                datetime_from_parts(
                    date_from_days(i64::from(date.value), use_utc),
                    time.value,
                    use_utc,
                ),
                date.offset,
            ))
        }
        0x2B => {
            let scale = metadata.scale.unwrap_or(0);
            let length = read_u8(buf, offset)?;
            if length.value == 0 {
                return Ok(ParseResult::new(Value::Null, length.offset));
            }
            let expected = temporal_data_length(scale)? + 5;
            if usize::from(length.value) != expected {
                return Err(ValueParserError::Unsupported(format!(
                    "invalid datetimeoffset length {}",
                    length.value
                )));
            }
            let time = read_temporal_time(buf, length.offset, scale, true)?;
            let date = read_u24_le(buf, time.offset)?;
            let offset_minutes = read_i16_le(buf, date.offset)?;
            Ok(ParseResult::new(
                Value::DateTimeOffset {
                    date: Box::new(date_from_days(i64::from(date.value), true)),
                    time: Box::new(time.value),
                    offset_minutes: offset_minutes.value,
                },
                offset_minutes.offset,
            ))
        }
        0x3A => {
            let days = read_u16_le(buf, offset)?;
            let minutes = read_u16_le(buf, days.offset)?;
            let date = date_from_days(i64::from(days.value) + 693595, use_utc);
            let time = time_from_ticks(u64::from(minutes.value) * 60, 0, use_utc);
            Ok(ParseResult::new(
                datetime_from_parts(date, time, use_utc),
                minutes.offset,
            ))
        }
        0x6F => {
            let length = read_u8(buf, offset)?;
            match length.value {
                0 => Ok(ParseResult::new(Value::Null, length.offset)),
                4 => {
                    let days = read_u16_le(buf, length.offset)?;
                    let minutes = read_u16_le(buf, days.offset)?;
                    let date = date_from_days(i64::from(days.value) + 693595, use_utc);
                    let time = time_from_ticks(u64::from(minutes.value) * 60, 0, use_utc);
                    Ok(ParseResult::new(
                        datetime_from_parts(date, time, use_utc),
                        minutes.offset,
                    ))
                }
                8 => {
                    let days = read_i32_le(buf, length.offset)?;
                    let ticks = read_i32_le(buf, days.offset)?;
                    Ok(ParseResult::new(
                        legacy_datetime_from_parts(
                            i64::from(days.value),
                            i64::from(ticks.value),
                            use_utc,
                        ),
                        ticks.offset,
                    ))
                }
                value => Err(ValueParserError::Unsupported(format!(
                    "Unsupported dataLength {value} for DateTimeN"
                ))),
            }
        }
        0x3D => {
            let days = read_i32_le(buf, offset)?;
            let ticks = read_i32_le(buf, days.offset)?;
            Ok(ParseResult::new(
                legacy_datetime_from_parts(i64::from(days.value), i64::from(ticks.value), use_utc),
                ticks.offset,
            ))
        }
        0x3C => {
            let raw = bytes(buf, offset, 8)?;
            let bytes: [u8; 8] = raw.value.as_slice().try_into().map_err(|_| {
                ValueParserError::Unsupported("invalid money payload length".to_owned())
            })?;
            let value = i128::from(i64::from_le_bytes(bytes));
            Ok(ParseResult::new(decimal_from_scaled(value, 4), raw.offset))
        }
        0x6E => {
            let length = read_u8(buf, offset)?;
            if length.value == 0 {
                return Ok(ParseResult::new(Value::Null, length.offset));
            }
            let value_length = usize::from(length.value);
            if value_length != 4 && value_length != 8 {
                return Err(ValueParserError::Unsupported(format!(
                    "Unsupported dataLength {value_length} for MoneyN"
                )));
            }
            let raw = bytes(buf, length.offset, value_length)?;
            let value = if value_length == 4 {
                let bytes: [u8; 4] = raw.value.as_slice().try_into().map_err(|_| {
                    ValueParserError::Unsupported("invalid moneyN payload length".to_owned())
                })?;
                i128::from(i32::from_le_bytes(bytes))
            } else {
                let bytes: [u8; 8] = raw.value.as_slice().try_into().map_err(|_| {
                    ValueParserError::Unsupported("invalid moneyN payload length".to_owned())
                })?;
                i128::from(i64::from_le_bytes(bytes))
            };
            Ok(ParseResult::new(decimal_from_scaled(value, 4), raw.offset))
        }
        0x7A => {
            let raw = read_i32_le(buf, offset)?;
            Ok(ParseResult::new(
                decimal_from_scaled(i128::from(raw.value), 4),
                raw.offset,
            ))
        }
        0x37 | 0x3F | 0x6A | 0x6C => {
            let length = read_u8(buf, offset)?;
            if length.value == 0 {
                return Ok(ParseResult::new(Value::Null, length.offset));
            }
            read_decimal(
                buf,
                length.offset,
                usize::from(length.value - 1),
                metadata.scale.unwrap_or(0),
            )
        }
        0x22 | 0x23 | 0x63 => read_legacy_lob(buf, offset, id, metadata),
        0xF0 => {
            let value = read_plp(buf, offset)?;
            Ok(ParseResult::new(
                value.value.map_or(Value::Null, Value::Binary),
                value.offset,
            ))
        }
        0xF1 => {
            let value = read_plp(buf, offset)?;
            let raw = match value.value {
                None => return Ok(ParseResult::new(Value::Null, value.offset)),
                Some(raw) => raw,
            };
            if raw.len() % 2 != 0 {
                return Err(ValueParserError::Unsupported(
                    "XML payload has an odd UTF-16 byte length".to_owned(),
                ));
            }
            Ok(ParseResult::new(
                Value::Text(String::from_utf16_lossy(
                    &raw.chunks_exact(2)
                        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                        .collect::<Vec<_>>(),
                )),
                value.offset,
            ))
        }
        0x62 => {
            let length = read_u32_le(buf, offset)?;
            if length.value == 0 {
                return Ok(ParseResult::new(Value::Null, length.offset));
            }
            read_variant(buf, length.offset, length.value as usize, use_utc)
        }
        0xA5 | 0xAD => {
            let value = if metadata.data_length == Some(u32::from(u16::MAX)) {
                read_plp(buf, offset)?
            } else {
                read_length_prefixed(buf, offset, 2)?
            };
            Ok(ParseResult::new(
                value.value.map_or(Value::Null, Value::Binary),
                value.offset,
            ))
        }
        0xA7 | 0xAF => {
            let value = if metadata.data_length == Some(u32::from(u16::MAX)) {
                read_plp(buf, offset)?
            } else {
                read_length_prefixed(buf, offset, 2)?
            };
            let raw = match value.value {
                None => return Ok(ParseResult::new(Value::Null, value.offset)),
                Some(raw) => raw,
            };
            let collation = metadata
                .collation
                .as_ref()
                .ok_or_else(|| ValueParserError::Unsupported("missing collation".to_owned()))?;
            Ok(ParseResult::new(
                Value::Text(decode_text(&raw, collation)?),
                value.offset,
            ))
        }
        0xE7 | 0xEF => {
            let value = if metadata.data_length == Some(u32::from(u16::MAX)) {
                read_plp(buf, offset)?
            } else {
                read_length_prefixed(buf, offset, 2)?
            };
            let raw = match value.value {
                None => return Ok(ParseResult::new(Value::Null, value.offset)),
                Some(raw) => raw,
            };
            let units: Vec<u16> = raw
                .chunks_exact(2)
                .map(|x| u16::from_le_bytes([x[0], x[1]]))
                .collect();
            Ok(ParseResult::new(
                Value::Text(String::from_utf16_lossy(&units)),
                value.offset,
            ))
        }
        _ => Err(ValueParserError::Unsupported(format!(
            "unsupported value type {}",
            metadata.data_type.name
        ))),
    }
}

fn read_variant(
    buf: &[u8],
    offset: usize,
    variant_length: usize,
    use_utc: bool,
) -> Result<ParseResult<Value>, ValueParserError> {
    let end = offset
        .checked_add(variant_length)
        .ok_or_else(|| ValueParserError::Unsupported("sql_variant length overflow".to_owned()))?;
    if variant_length < 2 {
        return Err(ValueParserError::Unsupported(
            "invalid sql_variant length".to_owned(),
        ));
    }
    if end > buf.len() {
        return Err(NotEnoughDataError::new(end).into());
    }
    let base_type = read_u8(buf, offset)?;
    if base_type.value == 0x62 {
        return Err(ValueParserError::Unsupported(
            "nested sql_variant is not valid".to_owned(),
        ));
    }
    let properties = read_u8(buf, base_type.offset)?;
    let mut cursor = properties.offset;
    let value_length = variant_length
        .checked_sub(2)
        .and_then(|value| value.checked_sub(usize::from(properties.value)))
        .ok_or_else(|| {
            ValueParserError::Unsupported("invalid sql_variant properties".to_owned())
        })?;
    let data_type = type_by_id(base_type.value)
        .ok_or_else(|| ValueParserError::Unsupported("unknown sql_variant base type".to_owned()))?;
    let mut metadata = Metadata {
        user_type: 0,
        flags: 0,
        data_type,
        collation: None,
        precision: None,
        scale: None,
        data_length: Some(value_length as u32),
        schema: None,
        udt_info: None,
    };
    match base_type.value {
        0x29..=0x2B => {
            if properties.value != 1 {
                return Err(ValueParserError::Unsupported(
                    "invalid temporal sql_variant properties".to_owned(),
                ));
            }
            metadata.scale = Some(read_u8(buf, cursor)?.value);
            cursor += 1;
        }
        0x6A | 0x6C => {
            if properties.value != 2 {
                return Err(ValueParserError::Unsupported(
                    "invalid numeric sql_variant properties".to_owned(),
                ));
            }
            metadata.precision = Some(read_u8(buf, cursor)?.value);
            metadata.scale = Some(read_u8(buf, cursor + 1)?.value);
            cursor += 2;
        }
        0xA5 | 0xAD | 0xA7 | 0xAF | 0xE7 | 0xEF => {
            if properties.value != 7 && matches!(base_type.value, 0xA7 | 0xAF | 0xE7 | 0xEF) {
                return Err(ValueParserError::Unsupported(
                    "invalid character sql_variant properties".to_owned(),
                ));
            }
            if properties.value < 2 {
                return Err(ValueParserError::Unsupported(
                    "invalid binary sql_variant properties".to_owned(),
                ));
            }
            let max_length = read_u16_le(buf, cursor)?;
            metadata.data_length = Some(max_length.value.into());
            cursor += 2;
            if matches!(base_type.value, 0xA7 | 0xAF | 0xE7 | 0xEF) {
                if cursor + 5 > end {
                    return Err(NotEnoughDataError::new(cursor + 5).into());
                }
                metadata.collation = Collation::from_buffer(buf, cursor);
                if metadata.collation.is_none() {
                    return Err(ValueParserError::Unsupported(
                        "invalid character sql_variant collation".to_owned(),
                    ));
                }
                cursor += 5;
            }
        }
        _ if properties.value != 0 => {
            return Err(ValueParserError::Unsupported(
                "invalid sql_variant properties".to_owned(),
            ));
        }
        _ => {}
    }
    let parsed = match base_type.value {
        0x24 => {
            if value_length != 16 {
                return Err(ValueParserError::Unsupported(
                    "invalid uniqueidentifier sql_variant length".to_owned(),
                ));
            }
            let raw = bytes(buf, cursor, value_length)?;
            let guid = crate::guid_parser::buffer_to_upper_case_guid(&raw.value)
                .ok_or_else(|| ValueParserError::Unsupported("invalid GUID".to_owned()))?;
            ParseResult::new(Value::Guid(guid), raw.offset)
        }
        0xA5 | 0xAD => {
            let raw = bytes(buf, cursor, value_length)?;
            ParseResult::new(Value::Binary(raw.value), raw.offset)
        }
        0xA7 | 0xAF => {
            let raw = bytes(buf, cursor, value_length)?;
            let collation = metadata
                .collation
                .as_ref()
                .ok_or_else(|| ValueParserError::Unsupported("missing collation".to_owned()))?;
            ParseResult::new(Value::Text(decode_text(&raw.value, collation)?), raw.offset)
        }
        0xE7 | 0xEF => {
            let raw = bytes(buf, cursor, value_length)?;
            let units = raw
                .value
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>();
            ParseResult::new(Value::Text(String::from_utf16_lossy(&units)), raw.offset)
        }
        0x28 => {
            if value_length != 3 {
                return Err(ValueParserError::Unsupported(
                    "invalid date sql_variant length".to_owned(),
                ));
            }
            let raw = read_u24_le(buf, cursor)?;
            ParseResult::new(date_from_days(i64::from(raw.value), use_utc), raw.offset)
        }
        0x29 => {
            let scale = metadata.scale.unwrap_or(0);
            let expected = temporal_data_length(scale)?;
            if value_length != expected {
                return Err(ValueParserError::Unsupported(
                    "invalid time sql_variant length".to_owned(),
                ));
            }
            read_temporal_time(buf, cursor, scale, use_utc)?
        }
        0x2A => {
            let scale = metadata.scale.unwrap_or(0);
            let time_length = temporal_data_length(scale)?;
            if value_length != time_length + 3 {
                return Err(ValueParserError::Unsupported(
                    "invalid datetime2 sql_variant length".to_owned(),
                ));
            }
            let time = read_temporal_time(buf, cursor, scale, use_utc)?;
            let date = read_u24_le(buf, time.offset)?;
            ParseResult::new(
                datetime_from_parts(
                    date_from_days(i64::from(date.value), use_utc),
                    time.value,
                    use_utc,
                ),
                date.offset,
            )
        }
        0x2B => {
            let scale = metadata.scale.unwrap_or(0);
            let time_length = temporal_data_length(scale)?;
            if value_length != time_length + 5 {
                return Err(ValueParserError::Unsupported(
                    "invalid datetimeoffset sql_variant length".to_owned(),
                ));
            }
            let time = read_temporal_time(buf, cursor, scale, true)?;
            let date = read_u24_le(buf, time.offset)?;
            let offset_minutes = read_i16_le(buf, date.offset)?;
            ParseResult::new(
                Value::DateTimeOffset {
                    date: Box::new(date_from_days(i64::from(date.value), true)),
                    time: Box::new(time.value),
                    offset_minutes: offset_minutes.value,
                },
                offset_minutes.offset,
            )
        }
        0x6A | 0x6C => read_decimal(
            buf,
            cursor,
            value_length.saturating_sub(1),
            metadata.scale.unwrap_or(0),
        )?,
        _ => read_value_with_options(buf, cursor, &metadata, use_utc)?,
    };
    if parsed.offset != end {
        return Err(ValueParserError::Unsupported(
            "sql_variant value does not match declared length".to_owned(),
        ));
    }
    Ok(parsed)
}
