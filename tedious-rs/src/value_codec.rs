use crate::collation::{Collation, Encoding};
use crate::guid_parser::guid_to_array;
use encoding_rs::{
    BIG5, EUC_KR, GBK, SHIFT_JIS, UTF_8, WINDOWS_874, WINDOWS_1250, WINDOWS_1251, WINDOWS_1252,
    WINDOWS_1253, WINDOWS_1254, WINDOWS_1255, WINDOWS_1256, WINDOWS_1257, WINDOWS_1258,
};
use std::fmt;

#[derive(Clone, PartialEq)]
pub enum Value {
    Null,
    Integer(i64),
    BigInteger(String),
    Boolean(bool),
    Real(f32),
    Float(f64),
    Binary(Vec<u8>),
    Text(String),
    Guid(String),
    Date {
        year: i32,
        month: u8,
        day: u8,
    },
    LocalDate {
        year: i32,
        month: u8,
        day: u8,
    },
    Time {
        hour: u8,
        minute: u8,
        second: u8,
        nanosecond: u32,
    },
    LocalTime {
        hour: u8,
        minute: u8,
        second: u8,
        nanosecond: u32,
    },
    DateTime {
        date: Box<Value>,
        time: Box<Value>,
    },
    LocalDateTime {
        date: Box<Value>,
        time: Box<Value>,
    },
    DateTimeOffset {
        date: Box<Value>,
        time: Box<Value>,
        offset_minutes: i16,
    },
    Decimal {
        unscaled: i128,
        scale: u8,
    },
    Tvp(Box<crate::payloads::TvpPayload>),
    Udt {
        database: String,
        schema: String,
        type_name: String,
        data: Vec<u8>,
    },
}

impl fmt::Debug for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => formatter.write_str("Null"),
            Self::Integer(_) => formatter.write_str("Integer([REDACTED])"),
            Self::BigInteger(value) | Self::Text(value) | Self::Guid(value) => formatter
                .debug_struct(match self {
                    Self::BigInteger(_) => "BigInteger",
                    Self::Text(_) => "Text",
                    _ => "Guid",
                })
                .field("value", &format_args!("[REDACTED; {} bytes]", value.len()))
                .finish(),
            Self::Boolean(_) => formatter.write_str("Boolean([REDACTED])"),
            Self::Real(_) => formatter.write_str("Real([REDACTED])"),
            Self::Float(_) => formatter.write_str("Float([REDACTED])"),
            Self::Binary(value) => formatter
                .debug_struct("Binary")
                .field("value", &format_args!("[REDACTED; {} bytes]", value.len()))
                .finish(),
            Self::Date { .. } => redacted_fields(formatter, "Date", &["year", "month", "day"]),
            Self::LocalDate { .. } => {
                redacted_fields(formatter, "LocalDate", &["year", "month", "day"])
            }
            Self::Time { .. } => redacted_fields(
                formatter,
                "Time",
                &["hour", "minute", "second", "nanosecond"],
            ),
            Self::LocalTime { .. } => redacted_fields(
                formatter,
                "LocalTime",
                &["hour", "minute", "second", "nanosecond"],
            ),
            Self::DateTime { .. } => redacted_fields(formatter, "DateTime", &["date", "time"]),
            Self::LocalDateTime { .. } => {
                redacted_fields(formatter, "LocalDateTime", &["date", "time"])
            }
            Self::DateTimeOffset { .. } => redacted_fields(
                formatter,
                "DateTimeOffset",
                &["date", "time", "offset_minutes"],
            ),
            Self::Decimal { .. } => redacted_fields(formatter, "Decimal", &["unscaled", "scale"]),
            Self::Tvp(_) => formatter.write_str("Tvp([REDACTED])"),
            Self::Udt {
                database,
                schema,
                type_name,
                data,
            } => formatter
                .debug_struct("Udt")
                .field(
                    "database",
                    &format_args!("[REDACTED; {} bytes]", database.len()),
                )
                .field(
                    "schema",
                    &format_args!("[REDACTED; {} bytes]", schema.len()),
                )
                .field(
                    "type_name",
                    &format_args!("[REDACTED; {} bytes]", type_name.len()),
                )
                .field("data", &format_args!("[REDACTED; {} bytes]", data.len()))
                .finish(),
        }
    }
}

fn redacted_fields(formatter: &mut fmt::Formatter<'_>, name: &str, fields: &[&str]) -> fmt::Result {
    let mut debug = formatter.debug_struct(name);
    for field in fields {
        debug.field(field, &"[REDACTED]");
    }
    debug.finish()
}

#[derive(Clone, PartialEq, Eq)]
pub struct EncodedParameter {
    pub type_info: Vec<u8>,
    pub length: Vec<u8>,
    pub data: Vec<u8>,
}

impl fmt::Debug for EncodedParameter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EncodedParameter")
            .field(
                "type_info",
                &format_args!("[REDACTED; {} bytes]", self.type_info.len()),
            )
            .field(
                "length",
                &format_args!("[REDACTED; {} bytes]", self.length.len()),
            )
            .field(
                "data",
                &format_args!("[REDACTED; {} bytes]", self.data.len()),
            )
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodecError(pub String);

impl std::fmt::Display for CodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for CodecError {}

fn parse_float_prefix(value: &str) -> Option<f64> {
    let value = value.trim_start();
    let mut end = 0;
    let bytes = value.as_bytes();
    if bytes
        .get(end)
        .is_some_and(|byte| *byte == b'+' || *byte == b'-')
    {
        end += 1;
    }
    let digits_start = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    let integer_end = end;
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
    }
    if end == digits_start || (integer_end == digits_start && end == integer_end + 1) {
        return None;
    }
    if bytes
        .get(end)
        .is_some_and(|byte| *byte == b'e' || *byte == b'E')
    {
        let exponent_start = end;
        end += 1;
        if bytes
            .get(end)
            .is_some_and(|byte| *byte == b'+' || *byte == b'-')
        {
            end += 1;
        }
        let exponent_digits = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        if exponent_digits == end {
            end = exponent_start;
        }
    }
    value[..end].parse().ok()
}

fn parse_js_number(value: &str) -> Option<f64> {
    let value = value.trim();
    if value.is_empty() {
        return Some(0.0);
    }
    let (sign, digits) = match value.as_bytes().first() {
        Some(b'+') => (1.0, &value[1..]),
        Some(b'-') => (-1.0, &value[1..]),
        _ => (1.0, value),
    };
    let (radix, digits) = if digits.starts_with("0x") || digits.starts_with("0X") {
        (16, &digits[2..])
    } else if digits.starts_with("0b") || digits.starts_with("0B") {
        (2, &digits[2..])
    } else if digits.starts_with("0o") || digits.starts_with("0O") {
        (8, &digits[2..])
    } else {
        return value.parse::<f64>().ok();
    };
    if digits.is_empty() {
        return None;
    }
    u64::from_str_radix(digits, radix)
        .ok()
        .map(|number| sign * number as f64)
}

fn parse_js_bigint(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.is_empty() {
        return Some(0);
    }
    if value.starts_with('+') || value.starts_with('-') {
        let digits = &value[1..];
        if digits.starts_with("0x")
            || digits.starts_with("0X")
            || digits.starts_with("0b")
            || digits.starts_with("0B")
            || digits.starts_with("0o")
            || digits.starts_with("0O")
        {
            return None;
        }
    }
    let (radix, digits) = if value.starts_with("0x") || value.starts_with("0X") {
        (16, &value[2..])
    } else if value.starts_with("0b") || value.starts_with("0B") {
        (2, &value[2..])
    } else if value.starts_with("0o") || value.starts_with("0O") {
        (8, &value[2..])
    } else {
        return value.parse::<i64>().ok();
    };
    if digits.is_empty() {
        return None;
    }
    u64::from_str_radix(digits, radix).ok()?.try_into().ok()
}

fn parse_decimal_components(value: &str, prefix: bool) -> Option<(i128, i64)> {
    let value = value.trim_start();
    let bytes = value.as_bytes();
    let mut position = 0;
    let negative = match bytes.first() {
        Some(b'-') => {
            position = 1;
            true
        }
        Some(b'+') => {
            position = 1;
            false
        }
        _ => false,
    };
    let mut digits = 0_i128;
    let mut digit_count = 0_i64;
    while bytes.get(position).is_some_and(u8::is_ascii_digit) {
        digits = digits
            .checked_mul(10)?
            .checked_add(i128::from(bytes[position] - b'0'))?;
        digit_count += 1;
        position += 1;
    }
    let mut fractional_digits = 0_i64;
    if bytes.get(position) == Some(&b'.') {
        position += 1;
        while bytes.get(position).is_some_and(u8::is_ascii_digit) {
            digits = digits
                .checked_mul(10)?
                .checked_add(i128::from(bytes[position] - b'0'))?;
            digit_count += 1;
            fractional_digits += 1;
            position += 1;
        }
    }
    if digit_count == 0 {
        return None;
    }
    let mut exponent = 0_i64;
    if bytes
        .get(position)
        .is_some_and(|byte| *byte == b'e' || *byte == b'E')
    {
        let exponent_start = position;
        position += 1;
        let exponent_negative = match bytes.get(position) {
            Some(b'-') => {
                position += 1;
                true
            }
            Some(b'+') => {
                position += 1;
                false
            }
            _ => false,
        };
        let exponent_digits_start = position;
        while bytes.get(position).is_some_and(u8::is_ascii_digit) {
            exponent = exponent
                .checked_mul(10)?
                .checked_add(i64::from(bytes[position] - b'0'))?;
            position += 1;
        }
        if exponent_digits_start == position {
            if prefix {
                position = exponent_start;
            } else {
                return None;
            }
        } else if exponent_negative {
            exponent = -exponent;
        }
    }
    if !prefix && !value[position..].trim().is_empty() {
        return None;
    }
    let signed_digits = if negative { -digits } else { digits };
    Some((signed_digits, exponent - fractional_digits))
}

fn decimal_to_scaled(value: &str, scale: u32, prefix: bool) -> Option<i128> {
    let (digits, exponent) = parse_decimal_components(value, prefix)?;
    let shift = i64::from(scale).checked_add(exponent)?;
    if digits == 0 {
        return Some(0);
    }
    if shift >= 0 {
        let multiplier = 10_i128.checked_pow(u32::try_from(shift).ok()?)?;
        return digits.checked_mul(multiplier);
    }
    let divisor = 10_u128.checked_pow(u32::try_from(-shift).ok()?)?;
    let magnitude = digits.unsigned_abs();
    let quotient = magnitude / divisor;
    let remainder = magnitude % divisor;
    let rounded = if digits > 0 {
        quotient + u128::from(remainder.saturating_mul(2) >= divisor)
    } else {
        quotient + u128::from(remainder.saturating_mul(2) > divisor)
    };
    let rounded = i128::try_from(rounded).ok()?;
    Some(if digits < 0 { -rounded } else { rounded })
}

fn parse_js_integer(value: &str) -> Option<i128> {
    let value = value.trim();
    if value.is_empty() {
        return Some(0);
    }
    let (negative, digits) = match value.as_bytes().first() {
        Some(b'-') => (true, &value[1..]),
        Some(b'+') => (false, &value[1..]),
        _ => (false, value),
    };
    let radix = if digits.starts_with("0x") || digits.starts_with("0X") {
        Some(16)
    } else if digits.starts_with("0b") || digits.starts_with("0B") {
        Some(2)
    } else if digits.starts_with("0o") || digits.starts_with("0O") {
        Some(8)
    } else {
        None
    };
    if let Some(radix) = radix {
        let digits = &digits[2..];
        if digits.is_empty() {
            return None;
        }
        let magnitude = i128::from_str_radix(digits, radix).ok()?;
        return Some(if negative { -magnitude } else { magnitude });
    }
    let (digits, exponent) = parse_decimal_components(value, false)?;
    if exponent >= 0 {
        return digits.checked_mul(10_i128.checked_pow(u32::try_from(exponent).ok()?)?);
    }
    Some(digits / 10_i128.checked_pow(u32::try_from(-exponent).ok()?)?)
}

fn number(value: &Value, prefix: bool) -> Result<f64, CodecError> {
    let parsed = match value {
        Value::Integer(value) => Some(*value as f64),
        Value::BigInteger(value) => value.parse::<f64>().ok(),
        Value::Real(value) => Some(f64::from(*value)),
        Value::Float(value) => Some(*value),
        Value::Text(value) if prefix => parse_float_prefix(value),
        Value::Text(value) => parse_js_number(value),
        _ => None,
    };
    parsed
        .filter(|value| value.is_finite() && !value.is_nan())
        .ok_or_else(|| CodecError("invalid number".to_owned()))
}

fn integer(value: &Value, min: i64, max: i64, width: usize) -> Result<Option<i64>, CodecError> {
    if matches!(value, Value::Null) {
        return Ok(None);
    }
    let value = match value {
        Value::Boolean(value) => i128::from(u8::from(*value)),
        Value::Integer(value) => i128::from(*value),
        Value::BigInteger(value) | Value::Text(value) => {
            parse_js_integer(value).ok_or_else(|| CodecError("invalid number".to_owned()))?
        }
        Value::Real(value) => {
            if !value.is_finite() {
                return Err(CodecError("invalid number".to_owned()));
            }
            value.trunc() as i128
        }
        Value::Float(value) => {
            if !value.is_finite() {
                return Err(CodecError("invalid number".to_owned()));
            }
            value.trunc() as i128
        }
        _ => return Err(CodecError("invalid number".to_owned())),
    };
    if value < i128::from(min) || value > i128::from(max) {
        return Err(CodecError(format!("value must be between {min} and {max}")));
    }
    Ok(Some(value as i64 & ((1_i64 << (width * 8).min(63)) - 1)))
}

pub fn encode_tinyint(value: &Value) -> Result<EncodedParameter, CodecError> {
    let value = integer(value, 0, 255, 1)?;
    Ok(EncodedParameter {
        type_info: vec![0x26, 1],
        length: vec![value.map_or(0, |_| 1)],
        data: value.map_or_else(Vec::new, |v| vec![v as u8]),
    })
}

pub fn encode_smallint(value: &Value) -> Result<EncodedParameter, CodecError> {
    let value = integer(value, -32768, 32767, 2)?;
    Ok(EncodedParameter {
        type_info: vec![0x26, 2],
        length: vec![value.map_or(0, |_| 2)],
        data: value.map_or_else(Vec::new, |v| (v as i16).to_le_bytes().to_vec()),
    })
}

pub fn encode_int(value: &Value) -> Result<EncodedParameter, CodecError> {
    let value = integer(value, i64::from(i32::MIN), i64::from(i32::MAX), 4)?;
    Ok(EncodedParameter {
        type_info: vec![0x26, 4],
        length: vec![value.map_or(0, |_| 4)],
        data: value.map_or_else(Vec::new, |v| (v as i32).to_le_bytes().to_vec()),
    })
}

pub fn encode_bigint(value: &Value) -> Result<EncodedParameter, CodecError> {
    let value = match value {
        Value::Null => None,
        Value::BigInteger(value) => {
            Some(parse_js_bigint(value).ok_or_else(|| CodecError("invalid bigint".to_owned()))?)
        }
        Value::Integer(value) => Some(*value),
        Value::Text(value) => {
            Some(parse_js_bigint(value).ok_or_else(|| CodecError("invalid bigint".to_owned()))?)
        }
        Value::Real(value) if value.is_finite() && value.fract() == 0.0 => {
            Some(float_to_i64(f64::from(*value))?)
        }
        Value::Float(value) if value.is_finite() && value.fract() == 0.0 => {
            Some(float_to_i64(*value)?)
        }
        Value::Boolean(value) => Some(i64::from(*value)),
        _ => return Err(CodecError("invalid bigint".to_owned())),
    };
    Ok(EncodedParameter {
        type_info: vec![0x26, 8],
        length: vec![value.map_or(0, |_| 8)],
        data: value.map_or_else(Vec::new, |v| v.to_le_bytes().to_vec()),
    })
}

fn float_to_i64(value: f64) -> Result<i64, CodecError> {
    if !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&value) {
        return Err(CodecError("invalid bigint".to_owned()));
    }
    Ok(value as i64)
}

pub fn encode_bit(value: &Value) -> Result<EncodedParameter, CodecError> {
    let value = match value {
        Value::Null => None,
        Value::Boolean(value) => Some(*value),
        Value::Integer(value) => Some(*value != 0),
        Value::BigInteger(value) => Some(
            parse_js_bigint(value).ok_or_else(|| CodecError("invalid boolean".to_owned()))? != 0,
        ),
        Value::Real(value) => Some(*value != 0.0),
        Value::Float(value) => Some(*value != 0.0),
        Value::Binary(value) => Some(!value.is_empty()),
        Value::Text(value) => Some(!value.is_empty()),
        _ => return Err(CodecError("invalid boolean".to_owned())),
    };
    Ok(EncodedParameter {
        type_info: vec![0x68, 1],
        length: vec![value.map_or(0, |_| 1)],
        data: value.map_or_else(Vec::new, |v| vec![u8::from(v)]),
    })
}

pub fn encode_real(value: &Value) -> Result<EncodedParameter, CodecError> {
    let value = match value {
        Value::Null => None,
        _ => Some(number(value, true)? as f32),
    };
    Ok(EncodedParameter {
        type_info: vec![0x6D, 4],
        length: vec![value.map_or(0, |_| 4)],
        data: value.map_or_else(Vec::new, |v| v.to_le_bytes().to_vec()),
    })
}

pub fn encode_float(value: &Value) -> Result<EncodedParameter, CodecError> {
    let value = match value {
        Value::Null => None,
        _ => Some(number(value, true)?),
    };
    Ok(EncodedParameter {
        type_info: vec![0x6D, 8],
        length: vec![value.map_or(0, |_| 8)],
        data: value.map_or_else(Vec::new, |v| v.to_le_bytes().to_vec()),
    })
}

pub fn encode_binary(
    value: &Value,
    parameter_length: usize,
) -> Result<EncodedParameter, CodecError> {
    let value = match value {
        Value::Null => None,
        Value::Binary(value) => Some(value),
        _ => return Err(CodecError("invalid buffer".to_owned())),
    };
    if parameter_length > u16::MAX as usize {
        return Err(CodecError("binary length exceeds u16".to_owned()));
    }
    let length = value.map_or(vec![0xFF, 0xFF], |_| {
        (parameter_length as u16).to_le_bytes().to_vec()
    });
    let data = value.map_or_else(Vec::new, |v| {
        v[..v.len().min(parameter_length).min(8000)].to_vec()
    });
    let type_info = [vec![0xAD], (parameter_length as u16).to_le_bytes().to_vec()].concat();
    Ok(EncodedParameter {
        type_info,
        length,
        data,
    })
}

pub fn encode_uniqueidentifier(value: &Value) -> Result<EncodedParameter, CodecError> {
    let value = match value {
        Value::Null => None,
        Value::Guid(value) => Some(guid_to_array(value).map_err(CodecError)?),
        _ => return Err(CodecError("invalid GUID".to_owned())),
    };
    Ok(EncodedParameter {
        type_info: vec![0x24, 0x10],
        length: vec![value.as_ref().map_or(0, |_| 16)],
        data: value.map_or_else(Vec::new, |v| v.to_vec()),
    })
}

fn days_from_year_month_day(year: i32, month: u8, day: u8) -> Result<i64, CodecError> {
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) || day == 0 {
        return Err(CodecError("invalid date".to_owned()));
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if day > month_days {
        return Err(CodecError("invalid date".to_owned()));
    }
    fn civil_day_number(year: i32, month: u8, day: u8) -> i64 {
        let month_i = i64::from(month);
        let year_i = i64::from(year) - i64::from(month <= 2);
        let era = if year_i >= 0 { year_i } else { year_i - 399 } / 400;
        let year_of_era = year_i - era * 400;
        let month_prime = month_i + if month_i > 2 { -3 } else { 9 };
        let day_of_year = (153 * month_prime + 2) / 5 + i64::from(day) - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146097 + day_of_era
    }
    Ok(civil_day_number(year, month, day) - civil_day_number(1, 1, 1))
}

fn temporal(value: &Value) -> Result<(i64, i64), CodecError> {
    match value {
        Value::Date { year, month, day } | Value::LocalDate { year, month, day } => {
            Ok((days_from_year_month_day(*year, *month, *day)?, 0))
        }
        _ => Err(CodecError("invalid date value".to_owned())),
    }
}

fn datetime_parts(value: &Value) -> Result<(&Value, &Value), CodecError> {
    match value {
        Value::DateTime { date, time } | Value::LocalDateTime { date, time } => Ok((date, time)),
        _ => Err(CodecError("invalid datetime value".to_owned())),
    }
}

fn time_ticks(value: &Value, scale: u8) -> Result<i64, CodecError> {
    let (hour, minute, second, nanosecond) = match value {
        Value::Time {
            hour,
            minute,
            second,
            nanosecond,
        }
        | Value::LocalTime {
            hour,
            minute,
            second,
            nanosecond,
        } => (*hour, *minute, *second, *nanosecond),
        _ => return Err(CodecError("invalid time value".to_owned())),
    };
    if hour > 23 || minute > 59 || second > 59 || nanosecond >= 1_000_000_000 || scale > 7 {
        return Err(CodecError("invalid time".to_owned()));
    }
    let unit = 10_i64.pow(u32::from(9 - scale));
    let total_nanos = (i64::from(hour) * 3600 + i64::from(minute) * 60 + i64::from(second))
        * 1_000_000_000
        + i64::from(nanosecond);
    Ok((total_nanos + unit / 2) / unit)
}

fn legacy_datetime_parts(value: &Value) -> Result<(i64, i64), CodecError> {
    let (date, time) = datetime_parts(value)?;
    let year = match date {
        Value::Date { year, .. } | Value::LocalDate { year, .. } => *year,
        _ => return Err(CodecError("invalid datetime date".to_owned())),
    };
    if !(1753..=9999).contains(&year) {
        return Err(CodecError("datetime value is out of range".to_owned()));
    }
    let mut days = temporal(date)?.0 - days_from_year_month_day(1900, 1, 1)?;
    let mut ticks = (time_ticks(time, 3)? * 300 + 500) / 1000;
    if ticks == 25_920_000 {
        days = days
            .checked_add(1)
            .ok_or_else(|| CodecError("datetime value is out of range".to_owned()))?;
        ticks = 0;
    }
    Ok((days, ticks))
}

fn write_unsigned_le(value: i64, width: usize) -> Vec<u8> {
    value.to_le_bytes()[..width].to_vec()
}

fn temporal_length(scale: u8) -> Result<usize, CodecError> {
    match scale {
        0..=2 => Ok(3),
        3..=4 => Ok(4),
        5..=7 => Ok(5),
        _ => Err(CodecError("invalid scale".to_owned())),
    }
}

pub fn encode_date(value: &Value) -> Result<EncodedParameter, CodecError> {
    let data = match value {
        Value::Null => Vec::new(),
        _ => write_unsigned_le(temporal(value)?.0, 3),
    };
    Ok(EncodedParameter {
        type_info: vec![0x28],
        length: vec![u8::from(!data.is_empty()) * 3],
        data,
    })
}

pub fn encode_time(value: &Value, scale: u8) -> Result<EncodedParameter, CodecError> {
    let length = temporal_length(scale)?;
    let data = match value {
        Value::Null => Vec::new(),
        _ => write_unsigned_le(time_ticks(value, scale)?, length),
    };
    Ok(EncodedParameter {
        type_info: vec![0x29, scale],
        length: vec![u8::from(!data.is_empty()) * length as u8],
        data,
    })
}

pub fn encode_datetime2(value: &Value, scale: u8) -> Result<EncodedParameter, CodecError> {
    let length = temporal_length(scale)?;
    let data = match value {
        Value::Null => Vec::new(),
        Value::DateTime { date, time } | Value::LocalDateTime { date, time } => {
            let mut result = write_unsigned_le(time_ticks(time, scale)?, length);
            result.extend(write_unsigned_le(temporal(date)?.0, 3));
            result
        }
        _ => return Err(CodecError("invalid datetime2 value".to_owned())),
    };
    Ok(EncodedParameter {
        type_info: vec![0x2A, scale],
        length: vec![u8::from(!data.is_empty()) * data.len() as u8],
        data,
    })
}

pub fn encode_datetimeoffset(value: &Value, scale: u8) -> Result<EncodedParameter, CodecError> {
    let length = temporal_length(scale)?;
    let data = match value {
        Value::Null => Vec::new(),
        Value::DateTimeOffset {
            date,
            time,
            offset_minutes,
        } => {
            if !(-14 * 60..=14 * 60).contains(offset_minutes) {
                return Err(CodecError("invalid timezone offset".to_owned()));
            }
            let mut result = write_unsigned_le(time_ticks(time, scale)?, length);
            result.extend(write_unsigned_le(temporal(date)?.0, 3));
            result.extend(offset_minutes.to_le_bytes());
            result
        }
        _ => return Err(CodecError("invalid datetimeoffset value".to_owned())),
    };
    Ok(EncodedParameter {
        type_info: vec![0x2B, scale],
        length: vec![u8::from(!data.is_empty()) * data.len() as u8],
        data,
    })
}

pub fn encode_decimal(
    value: &Value,
    precision: u8,
    scale: u8,
) -> Result<EncodedParameter, CodecError> {
    if precision == 0 || precision > 38 || scale > precision {
        return Err(CodecError("invalid decimal precision or scale".to_owned()));
    }
    let width = if precision <= 9 {
        4
    } else if precision <= 19 {
        8
    } else if precision <= 28 {
        12
    } else {
        16
    };
    let scaled = match value {
        Value::Null => None,
        Value::Decimal {
            unscaled,
            scale: actual_scale,
        } if *actual_scale == scale => Some(*unscaled),
        Value::Decimal { .. } => return Err(CodecError("invalid decimal value".to_owned())),
        _ => numeric_to_scaled(value, u32::from(scale))?,
    };
    let data = match scaled {
        None => Vec::new(),
        Some(unscaled) => {
            let limit = 10_i128
                .checked_pow(u32::from(precision))
                .ok_or_else(|| CodecError("decimal precision overflow".to_owned()))?;
            let magnitude = unscaled.unsigned_abs();
            if magnitude >= limit as u128 {
                return Err(CodecError("decimal value exceeds precision".to_owned()));
            }
            let mut result = vec![u8::from(unscaled >= 0)];
            result.extend(magnitude.to_le_bytes()[..width].to_vec());
            result
        }
    };
    Ok(EncodedParameter {
        type_info: vec![0x6A, (width + 1) as u8, precision, scale],
        length: vec![u8::from(!data.is_empty()) * (width + 1) as u8],
        data,
    })
}

pub fn encode_datetime(value: &Value) -> Result<EncodedParameter, CodecError> {
    let data = match value {
        Value::Null => Vec::new(),
        _ => {
            let (days, ticks) = legacy_datetime_parts(value)?;
            let mut result = (days as i32).to_le_bytes().to_vec();
            result.extend((ticks as i32).to_le_bytes());
            result
        }
    };
    Ok(EncodedParameter {
        type_info: vec![0x6F, 8],
        length: vec![u8::from(!data.is_empty()) * 8],
        data,
    })
}

pub fn encode_smalldatetime(value: &Value) -> Result<EncodedParameter, CodecError> {
    let data = match value {
        Value::Null => Vec::new(),
        _ => {
            let (date, time) = datetime_parts(value)?;
            let (year, month, day) = match date {
                Value::Date { year, month, day } | Value::LocalDate { year, month, day } => {
                    (*year, *month, *day)
                }
                _ => return Err(CodecError("invalid datetime date".to_owned())),
            };
            if !(crate::data_types::smalldatetime::EPOCH_YEAR
                ..=crate::data_types::smalldatetime::MAX_YEAR)
                .contains(&year)
                || (year == crate::data_types::smalldatetime::MAX_YEAR
                    && (month > 6 || (month == 6 && day > 6)))
            {
                return Err(CodecError("smalldatetime value is out of range".to_owned()));
            }
            let days = temporal(date)?.0 - days_from_year_month_day(1900, 1, 1)?;
            let minutes = match time {
                Value::Time { hour, minute, .. } | Value::LocalTime { hour, minute, .. }
                    if *hour <= 23 && *minute <= 59 =>
                {
                    i64::from(*hour) * 60 + i64::from(*minute)
                }
                _ => return Err(CodecError("invalid time".to_owned())),
            };
            if !(0..=u16::MAX as i64).contains(&days) || !(0..=u16::MAX as i64).contains(&minutes) {
                return Err(CodecError("smalldatetime value is out of range".to_owned()));
            }
            [
                ((days as u16).to_le_bytes()).as_slice(),
                &((minutes as u16).to_le_bytes()),
            ]
            .concat()
        }
    };
    Ok(EncodedParameter {
        type_info: vec![0x6F, 4],
        length: vec![u8::from(!data.is_empty()) * 4],
        data,
    })
}

fn numeric_to_scaled(value: &Value, scale: u32) -> Result<Option<i128>, CodecError> {
    let multiplier = 10_i128
        .checked_pow(scale)
        .ok_or_else(|| CodecError("numeric scale overflow".to_owned()))?;
    match value {
        Value::Null => Ok(None),
        Value::Decimal {
            unscaled,
            scale: value_scale,
        } => {
            if u32::from(*value_scale) == scale {
                Ok(Some(*unscaled))
            } else if u32::from(*value_scale) < scale {
                let factor = 10_i128
                    .checked_pow(scale - u32::from(*value_scale))
                    .ok_or_else(|| CodecError("numeric scale overflow".to_owned()))?;
                Ok(Some(unscaled.checked_mul(factor).ok_or_else(|| {
                    CodecError("numeric value overflow".to_owned())
                })?))
            } else {
                let divisor = 10_i128
                    .checked_pow(u32::from(*value_scale) - scale)
                    .ok_or_else(|| CodecError("numeric scale overflow".to_owned()))?;
                if *unscaled % divisor != 0 {
                    return Err(CodecError("numeric scale would lose precision".to_owned()));
                }
                Ok(Some(*unscaled / divisor))
            }
        }
        Value::Integer(number) => Ok(Some(
            i128::from(*number)
                .checked_mul(multiplier)
                .ok_or_else(|| CodecError("numeric value overflow".to_owned()))?,
        )),
        Value::BigInteger(number) => Ok(Some(
            number
                .parse::<i128>()
                .map_err(|_| CodecError("invalid number".to_owned()))?
                .checked_mul(multiplier)
                .ok_or_else(|| CodecError("numeric value overflow".to_owned()))?,
        )),
        Value::Real(number) => decimal_to_scaled(&number.to_string(), scale, true)
            .ok_or_else(|| CodecError("numeric value overflow".to_owned()))
            .map(Some),
        Value::Float(number) => decimal_to_scaled(&number.to_string(), scale, true)
            .ok_or_else(|| CodecError("numeric value overflow".to_owned()))
            .map(Some),
        Value::Text(number) => decimal_to_scaled(number, scale, true)
            .ok_or_else(|| CodecError("invalid number".to_owned()))
            .map(Some),
        _ => Err(CodecError("invalid number".to_owned())),
    }
}

pub fn encode_money(value: &Value) -> Result<EncodedParameter, CodecError> {
    let data = match numeric_to_scaled(value, 4)? {
        None => Vec::new(),
        Some(value) if value >= i128::from(i64::MIN) && value <= i128::from(i64::MAX) => {
            (value as i64).to_le_bytes().to_vec()
        }
        Some(_) => return Err(CodecError("money value is out of range".to_owned())),
    };
    Ok(EncodedParameter {
        type_info: vec![0x6E, 8],
        length: vec![u8::from(!data.is_empty()) * 8],
        data,
    })
}

pub fn encode_smallmoney(value: &Value) -> Result<EncodedParameter, CodecError> {
    let data = match numeric_to_scaled(value, 4)? {
        None => Vec::new(),
        Some(value) if value >= i128::from(i32::MIN) && value <= i128::from(i32::MAX) => {
            (value as i32).to_le_bytes().to_vec()
        }
        Some(_) => return Err(CodecError("smallmoney value is out of range".to_owned())),
    };
    Ok(EncodedParameter {
        type_info: vec![0x6E, 4],
        length: vec![u8::from(!data.is_empty()) * 4],
        data,
    })
}

fn encode_non_unicode(value: &str, collation: &Collation) -> Result<Vec<u8>, CodecError> {
    if let Some(bytes) = match collation.codepage {
        Some(Encoding::Cp437) => Some(yore::code_pages::CP437.encode_lossy(value, b'?')),
        Some(Encoding::Cp850) => Some(yore::code_pages::CP850.encode_lossy(value, b'?')),
        _ => None,
    } {
        return Ok(bytes.into_owned());
    }
    let encoding = match collation.codepage {
        Some(Encoding::Utf8) => UTF_8,
        Some(Encoding::Cp1250) => WINDOWS_1250,
        Some(Encoding::Cp1251) => WINDOWS_1251,
        Some(Encoding::Cp1252) => WINDOWS_1252,
        Some(Encoding::Cp1253) => WINDOWS_1253,
        Some(Encoding::Cp1254) => WINDOWS_1254,
        Some(Encoding::Cp1255) => WINDOWS_1255,
        Some(Encoding::Cp1256) => WINDOWS_1256,
        Some(Encoding::Cp1257) => WINDOWS_1257,
        Some(Encoding::Cp1258) => WINDOWS_1258,
        Some(Encoding::Cp874) => WINDOWS_874,
        Some(Encoding::Cp932) => SHIFT_JIS,
        Some(Encoding::Cp936) => GBK,
        Some(Encoding::Cp949) => return Ok(encode_cp949(value)),
        Some(Encoding::Cp950) => BIG5,
        Some(Encoding::Cp437 | Encoding::Cp850) => {
            return Err(CodecError("unsupported collation codepage".to_owned()));
        }
        None => {
            return Err(CodecError("unsupported collation codepage".to_owned()));
        }
    };
    let mut bytes = Vec::with_capacity(value.len());
    for character in value.chars() {
        let character = character.to_string();
        let (encoded, _, had_errors) = encoding.encode(&character);
        if had_errors {
            bytes.push(b'?');
        } else {
            bytes.extend_from_slice(&encoded);
        }
    }
    Ok(bytes)
}

fn encode_cp949(value: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(value.len());
    for character in value.chars() {
        let character = character.to_string();
        let (encoded, _, had_errors) = EUC_KR.encode(&character);
        if had_errors {
            bytes.push(b'?');
        } else {
            bytes.extend_from_slice(&encoded);
        }
    }
    bytes
}

pub fn encoded_non_unicode_length(value: &str, collation: &Collation) -> Result<usize, CodecError> {
    Ok(encode_non_unicode(value, collation)?.len())
}

fn plp_payload(value: Option<Vec<u8>>, max: bool) -> Result<(Vec<u8>, Vec<u8>), CodecError> {
    match (value, max) {
        (None, false) => Ok((vec![0xFF, 0xFF], Vec::new())),
        (None, true) => Ok((vec![0xFF; 8], Vec::new())),
        (Some(value), false) => {
            let length = u16::try_from(value.len())
                .map_err(|_| CodecError("value length exceeds u16".to_owned()))?;
            Ok((length.to_le_bytes().to_vec(), value))
        }
        (Some(value), true) => {
            let mut data = Vec::new();
            if !value.is_empty() {
                data.extend((value.len() as u32).to_le_bytes());
                data.extend(value);
            }
            data.extend([0, 0, 0, 0]);
            Ok((vec![0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF], data))
        }
    }
}

pub fn encode_varchar(
    value: &Value,
    length: usize,
    collation: &Collation,
) -> Result<EncodedParameter, CodecError> {
    let max = length > 8000;
    let raw = match value {
        Value::Null => None,
        Value::Text(text) => Some(encode_non_unicode(text, collation)?),
        _ => return Err(CodecError("invalid varchar value".to_owned())),
    };
    let (parameter_length, data) = plp_payload(raw, max)?;
    let wire_length = if max { u16::MAX } else { length as u16 }.to_le_bytes();
    let mut type_info = vec![0xA7, wire_length[0], wire_length[1]];
    let mut collation_copy = collation.clone();
    type_info.extend(collation_copy.to_buffer());
    Ok(EncodedParameter {
        type_info,
        length: parameter_length,
        data,
    })
}

pub fn encode_nvarchar(value: &Value, length: usize) -> Result<EncodedParameter, CodecError> {
    let max = length > 4000;
    let raw = match value {
        Value::Null => None,
        Value::Text(text) => Some(text.encode_utf16().flat_map(u16::to_le_bytes).collect()),
        _ => return Err(CodecError("invalid nvarchar value".to_owned())),
    };
    let (parameter_length, data) = plp_payload(raw, max)?;
    let units = length.min(u16::MAX as usize / 2) as u16;
    let bytes = if max {
        u16::MAX
    } else {
        units.saturating_mul(2)
    };
    Ok(EncodedParameter {
        type_info: vec![0xE7, bytes as u8, (bytes >> 8) as u8, 0, 0, 0, 0, 0],
        length: parameter_length,
        data,
    })
}

pub fn encode_char(
    value: &Value,
    length: usize,
    collation: &Collation,
) -> Result<EncodedParameter, CodecError> {
    let raw = match value {
        Value::Null => None,
        Value::Text(text) => Some(encode_non_unicode(text, collation)?),
        _ => return Err(CodecError("invalid char value".to_owned())),
    };
    if length > u16::MAX as usize {
        return Err(CodecError("char length exceeds u16".to_owned()));
    }
    if raw
        .as_ref()
        .is_some_and(|value: &Vec<u8>| value.len() > usize::from(u16::MAX))
    {
        return Err(CodecError("char value length exceeds u16".to_owned()));
    }
    let bytes = (length as u16).to_le_bytes();
    Ok(EncodedParameter {
        type_info: vec![0xAF, bytes[0], bytes[1], 0, 0, 0, 0, 0],
        length: raw.as_ref().map_or_else(
            || vec![0xFF, 0xFF],
            |value| (value.len() as u16).to_le_bytes().to_vec(),
        ),
        data: raw.unwrap_or_default(),
    })
}

pub fn encode_nchar(value: &Value, length: usize) -> Result<EncodedParameter, CodecError> {
    let raw = match value {
        Value::Null => None,
        Value::Text(text) => Some(text.encode_utf16().flat_map(u16::to_le_bytes).collect()),
        _ => return Err(CodecError("invalid nchar value".to_owned())),
    };
    let byte_length = length
        .checked_mul(2)
        .ok_or_else(|| CodecError("nchar length exceeds usize".to_owned()))?;
    if byte_length > u16::MAX as usize {
        return Err(CodecError("nchar length exceeds u16".to_owned()));
    }
    if raw
        .as_ref()
        .is_some_and(|value: &Vec<u8>| value.len() > usize::from(u16::MAX))
    {
        return Err(CodecError("nchar value length exceeds u16".to_owned()));
    }
    let bytes = (byte_length as u16).to_le_bytes();
    Ok(EncodedParameter {
        type_info: vec![0xEF, bytes[0], bytes[1], 0, 0, 0, 0, 0],
        length: raw.as_ref().map_or_else(
            || vec![0xFF, 0xFF],
            |value| (value.len() as u16).to_le_bytes().to_vec(),
        ),
        data: raw.unwrap_or_default(),
    })
}

pub fn encode_variant(
    value: &Value,
    collation: &Collation,
) -> Result<EncodedParameter, CodecError> {
    let mut body = Vec::new();
    match value {
        Value::Integer(value) if i32::try_from(*value).is_ok() => {
            body.extend([0x38, 0]);
            body.extend((*value as i32).to_le_bytes());
        }
        Value::Text(value) => {
            let raw = value
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>();
            let length = u16::try_from(raw.len())
                .map_err(|_| CodecError("sql_variant text is too long".to_owned()))?;
            body.extend([0xE7, 7]);
            body.extend(length.to_le_bytes());
            let mut collation = collation.clone();
            body.extend(collation.to_buffer());
            body.extend(raw);
        }
        Value::Decimal { unscaled, scale } => {
            let precision = decimal_precision(*unscaled);
            let width = if precision <= 9 {
                4
            } else if precision <= 19 {
                8
            } else if precision <= 28 {
                12
            } else {
                16
            };
            let magnitude = unscaled.unsigned_abs();
            body.extend([0x6A, 2, precision, *scale]);
            body.push(u8::from(*unscaled >= 0));
            body.extend(magnitude.to_le_bytes()[..width].to_vec());
        }
        Value::Null => {
            return Err(CodecError(
                "sql_variant cannot encode null without a base type".to_owned(),
            ));
        }
        _ => return Err(CodecError("unsupported sql_variant value".to_owned())),
    }
    let length = u32::try_from(body.len())
        .map_err(|_| CodecError("sql_variant value is too long".to_owned()))?;
    Ok(EncodedParameter {
        type_info: vec![0x62],
        length: length.to_le_bytes().to_vec(),
        data: body,
    })
}

pub fn encode_tvp(value: &Value) -> Result<EncodedParameter, CodecError> {
    let payload = match value {
        Value::Tvp(payload) => payload,
        Value::Null => {
            return Ok(EncodedParameter {
                type_info: vec![0xF3, 0, 0, 0],
                length: vec![0xFF, 0xFF],
                data: vec![0, 0],
            });
        }
        _ => return Err(CodecError("invalid table value".to_owned())),
    };
    let buffers = payload.buffers().map_err(CodecError)?;
    Ok(EncodedParameter {
        type_info: buffers[0].clone(),
        length: buffers[1].clone(),
        data: buffers[2].clone(),
    })
}

pub fn encode_udt(value: &Value) -> Result<EncodedParameter, CodecError> {
    let (database, schema, type_name, data) = match value {
        Value::Udt {
            database,
            schema,
            type_name,
            data,
        } => (database, schema, type_name, Some(data.clone())),
        Value::Null => return Err(CodecError("UDT null requires type metadata".to_owned())),
        _ => return Err(CodecError("invalid UDT value".to_owned())),
    };
    let mut type_info = vec![0xF0];
    type_info.extend(
        u16::try_from(data.as_ref().map_or(0, Vec::len))
            .map_err(|_| CodecError("UDT value is too large".to_owned()))?
            .to_le_bytes(),
    );
    append_b_var_char(&mut type_info, database)?;
    append_b_var_char(&mut type_info, schema)?;
    append_b_var_char(&mut type_info, type_name)?;
    type_info.extend([0, 0]);
    let (length, payload) = plp_payload(data, true)?;
    Ok(EncodedParameter {
        type_info,
        length,
        data: payload,
    })
}

fn append_b_var_char(target: &mut Vec<u8>, value: &str) -> Result<(), CodecError> {
    let units = value.encode_utf16().collect::<Vec<_>>();
    let length =
        u8::try_from(units.len()).map_err(|_| CodecError("UDT name is too long".to_owned()))?;
    target.push(length);
    target.extend(units.into_iter().flat_map(u16::to_le_bytes));
    Ok(())
}

fn decimal_precision(value: i128) -> u8 {
    value.unsigned_abs().to_string().len().clamp(1, 38) as u8
}

pub fn encode_varbinary(value: &Value, length: usize) -> Result<EncodedParameter, CodecError> {
    let max = length > 8000;
    let raw = match value {
        Value::Null => None,
        Value::Binary(value) => Some(value.clone()),
        _ => return Err(CodecError("invalid varbinary value".to_owned())),
    };
    let (parameter_length, data) = plp_payload(raw, max)?;
    let bytes = if max { u16::MAX } else { length as u16 }.to_le_bytes();
    Ok(EncodedParameter {
        type_info: vec![0xA5, bytes[0], bytes[1]],
        length: parameter_length,
        data,
    })
}

pub fn encode_text(
    value: &Value,
    length: i32,
    collation: &Collation,
) -> Result<EncodedParameter, CodecError> {
    let raw = match value {
        Value::Null => None,
        Value::Text(text) => Some(encode_non_unicode(text, collation)?),
        _ => return Err(CodecError("invalid text value".to_owned())),
    };
    let (length_bytes, data) = match raw {
        None => (u32::MAX.to_le_bytes().to_vec(), Vec::new()),
        Some(value) => (
            u32::try_from(value.len())
                .map_err(|_| CodecError("text value length exceeds u32".to_owned()))?
                .to_le_bytes()
                .to_vec(),
            value,
        ),
    };
    if length < -1 {
        return Err(CodecError("text length is out of range".to_owned()));
    }
    let mut type_info = vec![0x23];
    type_info.extend(length.to_le_bytes());
    let mut collation_copy = collation.clone();
    type_info.extend(collation_copy.to_buffer());
    Ok(EncodedParameter {
        type_info,
        length: length_bytes,
        data,
    })
}

pub fn encode_ntext(value: &Value, length: i32) -> Result<EncodedParameter, CodecError> {
    encode_ntext_with_collation(value, length, &Collation::new(0x0409, 0, 0, 0))
}

pub fn encode_ntext_with_collation(
    value: &Value,
    length: i32,
    collation: &Collation,
) -> Result<EncodedParameter, CodecError> {
    let raw = match value {
        Value::Null => None,
        Value::Text(text) => Some(
            text.encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        ),
        _ => return Err(CodecError("invalid ntext value".to_owned())),
    };
    let (length_bytes, data) = match raw {
        None => (u32::MAX.to_le_bytes().to_vec(), Vec::new()),
        Some(value) => (
            u32::try_from(value.len())
                .map_err(|_| CodecError("ntext value length exceeds u32".to_owned()))?
                .to_le_bytes()
                .to_vec(),
            value,
        ),
    };
    if length < -1 {
        return Err(CodecError("ntext length is out of range".to_owned()));
    }
    let mut type_info = vec![0x63];
    type_info.extend(length.to_le_bytes());
    let mut collation_copy = collation.clone();
    type_info.extend(collation_copy.to_buffer());
    Ok(EncodedParameter {
        type_info,
        length: length_bytes,
        data,
    })
}

pub fn encode_image(value: &Value, length: i32) -> Result<EncodedParameter, CodecError> {
    let raw = match value {
        Value::Null => None,
        Value::Binary(value) => Some(value.clone()),
        _ => return Err(CodecError("invalid image value".to_owned())),
    };
    let (length_bytes, data) = match raw {
        None => (u32::MAX.to_le_bytes().to_vec(), Vec::new()),
        Some(value) => (
            u32::try_from(value.len())
                .map_err(|_| CodecError("image value length exceeds u32".to_owned()))?
                .to_le_bytes()
                .to_vec(),
            value,
        ),
    };
    if length < -1 {
        return Err(CodecError("image length is out of range".to_owned()));
    }
    let mut type_info = vec![0x22];
    type_info.extend(length.to_le_bytes());
    Ok(EncodedParameter {
        type_info,
        length: length_bytes,
        data,
    })
}

pub fn encode_xml(value: &Value) -> Result<EncodedParameter, CodecError> {
    let raw = match value {
        Value::Null => None,
        Value::Text(value) => Some(value.encode_utf16().flat_map(u16::to_le_bytes).collect()),
        _ => return Err(CodecError("invalid XML value".to_owned())),
    };
    let (length, data) = plp_payload(raw, true)?;
    Ok(EncodedParameter {
        type_info: vec![0xF1, 0],
        length,
        data,
    })
}
