use crate::always_encrypted::types::SqlServerEncryptionType;
use crate::errors::{NotEnoughDataError, ParseResult};
use crate::metadata_parser::{Metadata, MetadataError, read_metadata};
use crate::token::helpers::{read_b_var_char, read_u8, read_u16_le, read_us_var_char};
use crate::value_codec::Value;
use crate::value_parser::{ValueParserError, read_value_with_options};

#[derive(Clone, PartialEq, Eq)]
pub enum TableName {
    Legacy(String),
    Parts(Vec<String>),
}
impl std::fmt::Debug for TableName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Legacy(value) => formatter
                .debug_tuple("Legacy")
                .field(&format_args!("[REDACTED; {} bytes]", value.len()))
                .finish(),
            Self::Parts(values) => formatter
                .debug_tuple("Parts")
                .field(&format_args!("[REDACTED; {} parts]", values.len()))
                .finish(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ColumnMetadata {
    pub metadata: Metadata,
    pub col_name: String,
    pub table_name: Option<TableName>,
    pub crypto_metadata: Option<ColumnEncryptionMetadata>,
}
impl std::fmt::Debug for ColumnMetadata {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ColumnMetadata")
            .field("metadata", &"[REDACTED]")
            .field(
                "col_name",
                &format_args!("[REDACTED; {} bytes]", self.col_name.len()),
            )
            .field("table_name", &self.table_name)
            .field("crypto_metadata", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ColumnEncryptionMetadata {
    pub ordinal: usize,
    pub cipher_algorithm_id: u8,
    pub encryption_type: SqlServerEncryptionType,
    pub normalization_rule_version: Vec<u8>,
}
impl std::fmt::Debug for ColumnEncryptionMetadata {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ColumnEncryptionMetadata")
            .field("ordinal", &self.ordinal)
            .field("cipher_algorithm_id", &self.cipher_algorithm_id)
            .field("encryption_type", &self.encryption_type)
            .field(
                "normalization_rule_version",
                &format_args!(
                    "[REDACTED; {} bytes]",
                    self.normalization_rule_version.len()
                ),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct RowColumn {
    pub value: Value,
    pub metadata: ColumnMetadata,
}
impl std::fmt::Debug for RowColumn {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RowColumn")
            .field("value", &"[REDACTED]")
            .field("metadata", &self.metadata)
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct Row {
    pub columns: Vec<RowColumn>,
}

impl std::fmt::Debug for Row {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Row")
            .field(
                "columns",
                &format_args!("[REDACTED; {} columns]", self.columns.len()),
            )
            .finish()
    }
}

impl Row {
    pub fn named_values(&self) -> std::collections::BTreeMap<String, Vec<Value>> {
        let mut result: std::collections::BTreeMap<String, Vec<Value>> =
            std::collections::BTreeMap::new();
        for column in &self.columns {
            result
                .entry(column.metadata.col_name.clone())
                .or_default()
                .push(column.value.clone());
        }
        result
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowParseError {
    NotEnoughData(NotEnoughDataError),
    Metadata(MetadataError),
    Value(ValueParserError),
    Invalid(String),
}
impl From<NotEnoughDataError> for RowParseError {
    fn from(value: NotEnoughDataError) -> Self {
        Self::NotEnoughData(value)
    }
}
impl From<MetadataError> for RowParseError {
    fn from(value: MetadataError) -> Self {
        Self::Metadata(value)
    }
}
impl From<ValueParserError> for RowParseError {
    fn from(value: ValueParserError) -> Self {
        Self::Value(value)
    }
}

fn table_name(
    buf: &[u8],
    offset: usize,
    metadata: &Metadata,
    tds_version: &str,
) -> Result<ParseResult<Option<TableName>>, RowParseError> {
    let has_table = matches!(metadata.data_type.id, 0x22 | 0x23 | 0x63);
    if !has_table {
        return Ok(ParseResult::new(None, offset));
    }
    if tds_version < "7_2" {
        let value = read_us_var_char(buf, offset)?;
        return Ok(ParseResult::new(
            Some(TableName::Legacy(value.value)),
            value.offset,
        ));
    }
    let count = read_u8(buf, offset)?;
    let mut parts = Vec::with_capacity(count.value as usize);
    let mut cursor = count.offset;
    for _ in 0..count.value {
        let value = read_us_var_char(buf, cursor)?;
        parts.push(value.value);
        cursor = value.offset;
    }
    Ok(ParseResult::new(Some(TableName::Parts(parts)), cursor))
}

pub fn parse_colmetadata_at(
    buf: &[u8],
    offset: usize,
    tds_version: &str,
    camel_case_columns: bool,
) -> Result<ParseResult<Vec<ColumnMetadata>>, RowParseError> {
    let count = read_u16_le(buf, offset)?;
    let mut cursor = count.offset;
    let mut columns = Vec::with_capacity(count.value as usize);
    for _ in 0..count.value {
        let metadata = read_metadata(buf, cursor, tds_version)?;
        let table = table_name(buf, metadata.offset, &metadata.value, tds_version)?;
        let name = read_b_var_char(buf, table.offset)?;
        let col_name = if camel_case_columns {
            name.value
                .chars()
                .enumerate()
                .map(|(i, c)| {
                    if i == 0 {
                        c.to_ascii_lowercase().to_string()
                    } else {
                        c.to_string()
                    }
                })
                .collect()
        } else {
            name.value
        };
        let (crypto_metadata, offset) = if metadata.value.flags & 0x0800 != 0 {
            let cek_ordinal = read_u16_le(buf, name.offset)?;
            let algorithm = read_u8(buf, cek_ordinal.offset)?;
            let encryption_type_byte = read_u8(buf, algorithm.offset)?;
            let encryption_type = match encryption_type_byte.value {
                0 => SqlServerEncryptionType::PlainText,
                1 => SqlServerEncryptionType::Deterministic,
                2 => SqlServerEncryptionType::Randomized,
                value => {
                    return Err(RowParseError::Invalid(format!(
                        "unknown column encryption type {value}"
                    )));
                }
            };
            let normalization = read_u8(buf, encryption_type_byte.offset)?;
            (
                Some(ColumnEncryptionMetadata {
                    ordinal: cek_ordinal.value as usize,
                    cipher_algorithm_id: algorithm.value,
                    encryption_type,
                    normalization_rule_version: vec![normalization.value],
                }),
                normalization.offset,
            )
        } else {
            (None, name.offset)
        };
        columns.push(ColumnMetadata {
            metadata: metadata.value,
            col_name,
            table_name: table.value,
            crypto_metadata,
        });
        cursor = offset;
    }
    Ok(ParseResult::new(columns, cursor))
}

pub fn parse_colmetadata(
    buf: &[u8],
    tds_version: &str,
    camel_case_columns: bool,
) -> Result<ParseResult<Vec<ColumnMetadata>>, RowParseError> {
    parse_colmetadata_at(buf, 0, tds_version, camel_case_columns)
}

pub fn parse_row(
    buf: &[u8],
    offset: usize,
    columns: &[ColumnMetadata],
) -> Result<ParseResult<Row>, RowParseError> {
    parse_row_with_options(buf, offset, columns, true)
}

pub fn parse_row_with_options(
    buf: &[u8],
    offset: usize,
    columns: &[ColumnMetadata],
    use_utc: bool,
) -> Result<ParseResult<Row>, RowParseError> {
    let mut cursor = offset;
    let mut values = Vec::with_capacity(columns.len());
    for metadata in columns {
        let value = read_value_with_options(buf, cursor, &metadata.metadata, use_utc)?;
        cursor = value.offset;
        values.push(RowColumn {
            value: value.value,
            metadata: metadata.clone(),
        });
    }
    Ok(ParseResult::new(Row { columns: values }, cursor))
}

pub fn parse_nbc_row(
    buf: &[u8],
    offset: usize,
    columns: &[ColumnMetadata],
) -> Result<ParseResult<Row>, RowParseError> {
    parse_nbc_row_with_options(buf, offset, columns, true)
}

pub fn parse_nbc_row_with_options(
    buf: &[u8],
    offset: usize,
    columns: &[ColumnMetadata],
    use_utc: bool,
) -> Result<ParseResult<Row>, RowParseError> {
    let bitmap_length = columns.len().div_ceil(8);
    if buf.len() < offset.saturating_add(bitmap_length) {
        return Err(NotEnoughDataError::new(offset + bitmap_length).into());
    }
    let bitmap = &buf[offset..offset + bitmap_length];
    let mut cursor = offset + bitmap_length;
    let mut values = Vec::with_capacity(columns.len());
    for (index, metadata) in columns.iter().enumerate() {
        if bitmap[index / 8] & (1 << (index % 8)) != 0 {
            values.push(RowColumn {
                value: Value::Null,
                metadata: metadata.clone(),
            });
        } else {
            let value = read_value_with_options(buf, cursor, &metadata.metadata, use_utc)?;
            cursor = value.offset;
            values.push(RowColumn {
                value: value.value,
                metadata: metadata.clone(),
            });
        }
    }
    Ok(ParseResult::new(Row { columns: values }, cursor))
}
