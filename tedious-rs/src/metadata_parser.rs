use crate::collation::Collation;
use crate::data_type::{DataType, type_by_id};
use crate::errors::{InputError, NotEnoughDataError, ParseResult};
use crate::token::helpers::{read_b_var_char, read_u8, read_u16_le, read_u32_le, read_us_var_char};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmlSchema {
    pub dbname: String,
    pub owning_schema: String,
    pub xml_schema_collection: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UdtInfo {
    pub max_byte_size: u16,
    pub dbname: String,
    pub owning_schema: String,
    pub type_name: String,
    pub assembly_name: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub user_type: u32,
    pub flags: u16,
    pub data_type: DataType,
    pub collation: Option<Collation>,
    pub precision: Option<u8>,
    pub scale: Option<u8>,
    pub data_length: Option<u32>,
    pub schema: Option<XmlSchema>,
    pub udt_info: Option<UdtInfo>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetadataError {
    NotEnoughData(NotEnoughDataError),
    Invalid(InputError),
}
impl From<NotEnoughDataError> for MetadataError {
    fn from(value: NotEnoughDataError) -> Self {
        Self::NotEnoughData(value)
    }
}

fn read_collation(buf: &[u8], offset: usize) -> Result<ParseResult<Collation>, MetadataError> {
    if buf.len() < offset.saturating_add(5) {
        return Err(NotEnoughDataError::new(offset.saturating_add(5)).into());
    }
    let value = Collation::from_buffer(buf, offset)
        .ok_or_else(|| MetadataError::Invalid(InputError("invalid collation".to_owned())))?;
    Ok(ParseResult::new(value, offset + 5))
}
fn read_schema(buf: &[u8], offset: usize) -> Result<ParseResult<Option<XmlSchema>>, MetadataError> {
    let present = read_u8(buf, offset)?;
    if present.value != 1 {
        return Ok(ParseResult::new(None, present.offset));
    }
    let dbname = read_b_var_char(buf, present.offset)?;
    let owning_schema = read_b_var_char(buf, dbname.offset)?;
    let collection = read_us_var_char(buf, owning_schema.offset)?;
    Ok(ParseResult::new(
        Some(XmlSchema {
            dbname: dbname.value,
            owning_schema: owning_schema.value,
            xml_schema_collection: collection.value,
        }),
        collection.offset,
    ))
}
fn read_udt_info(buf: &[u8], offset: usize) -> Result<ParseResult<UdtInfo>, MetadataError> {
    let max_byte_size = read_u16_le(buf, offset)?;
    let dbname = read_b_var_char(buf, max_byte_size.offset)?;
    let owning_schema = read_b_var_char(buf, dbname.offset)?;
    let type_name = read_b_var_char(buf, owning_schema.offset)?;
    let assembly_name = read_us_var_char(buf, type_name.offset)?;
    Ok(ParseResult::new(
        UdtInfo {
            max_byte_size: max_byte_size.value,
            dbname: dbname.value,
            owning_schema: owning_schema.value,
            type_name: type_name.value,
            assembly_name: assembly_name.value,
        },
        assembly_name.offset,
    ))
}
pub fn read_metadata(
    buf: &[u8],
    offset: usize,
    tds_version: &str,
) -> Result<ParseResult<Metadata>, MetadataError> {
    let old = tds_version < "7_2";
    let user_type = if old {
        u32::from(read_u16_le(buf, offset)?.value)
    } else {
        read_u32_le(buf, offset)?.value
    };
    let base_offset = if old { offset + 2 } else { offset + 4 };
    let flags = read_u16_le(buf, base_offset)?;
    let type_number = read_u8(buf, flags.offset)?;
    let data_type = type_by_id(type_number.value).ok_or_else(|| {
        MetadataError::Invalid(InputError(format!(
            "Unrecognised data type 0x{:02X}",
            type_number.value
        )))
    })?;
    let mut cursor = type_number.offset;
    let mut collation = None;
    let mut precision = None;
    let mut scale = None;
    let mut data_length = None;
    let mut schema = None;
    let mut udt_info = None;
    match type_number.value {
        0x1F | 0x30 | 0x34 | 0x38 | 0x3A | 0x3B | 0x3E | 0x3C | 0x32 | 0x3D | 0x28 | 0x7A
        | 0x7F => {}
        0x26 | 0x6D | 0x6E | 0x68 | 0x24 | 0x6F => {
            let value = read_u8(buf, cursor)?;
            data_length = Some(u32::from(value.value));
            cursor = value.offset;
        }
        0x62 => {
            let value = read_u32_le(buf, cursor)?;
            data_length = Some(value.value);
            cursor = value.offset;
        }
        0xA7 | 0xAF | 0xE7 | 0xEF => {
            let value = read_u16_le(buf, cursor)?;
            data_length = Some(u32::from(value.value));
            cursor = value.offset;
            let value = read_collation(buf, cursor)?;
            collation = Some(value.value);
            cursor = value.offset;
        }
        0x23 | 0x63 => {
            let value = read_u32_le(buf, cursor)?;
            data_length = Some(value.value);
            cursor = value.offset;
            let value = read_collation(buf, cursor)?;
            collation = Some(value.value);
            cursor = value.offset;
        }
        0xA5 | 0xAD => {
            let value = read_u16_le(buf, cursor)?;
            data_length = Some(u32::from(value.value));
            cursor = value.offset;
        }
        0x22 => {
            let value = read_u32_le(buf, cursor)?;
            data_length = Some(value.value);
            cursor = value.offset;
        }
        0xF1 => {
            let value = read_schema(buf, cursor)?;
            schema = value.value;
            cursor = value.offset;
        }
        0x29..=0x2B => {
            let value = read_u8(buf, cursor)?;
            scale = Some(value.value);
            cursor = value.offset;
        }
        0x3F | 0x37 | 0x6C | 0x6A => {
            let length = read_u8(buf, cursor)?;
            let value = read_u8(buf, length.offset)?;
            let factor = read_u8(buf, value.offset)?;
            data_length = Some(u32::from(length.value));
            precision = Some(value.value);
            scale = Some(factor.value);
            cursor = factor.offset;
        }
        0xF0 => {
            let value = read_udt_info(buf, cursor)?;
            udt_info = Some(value.value);
            cursor = value.offset;
        }
        _ => {
            return Err(MetadataError::Invalid(InputError(format!(
                "Unrecognised type {}",
                data_type.name
            ))));
        }
    }
    Ok(ParseResult::new(
        Metadata {
            user_type,
            flags: flags.value,
            data_type,
            collation,
            precision,
            scale,
            data_length,
            schema,
            udt_info,
        },
        cursor,
    ))
}
pub fn parse_metadata(buf: &[u8], tds_version: &str) -> Result<Vec<Metadata>, MetadataError> {
    let mut offset = 0;
    let mut result = Vec::new();
    while offset < buf.len() {
        let value = read_metadata(buf, offset, tds_version)?;
        offset = value.offset;
        result.push(value.value);
    }
    Ok(result)
}
