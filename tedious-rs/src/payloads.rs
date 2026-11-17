use crate::all_headers::write_to_tracking_buffer;
use crate::parameter::{ParameterOptions, SqlParameterType};
use crate::tracking_buffer::{Encoding, WritableTrackingBuffer};
use crate::value_codec::EncodedParameter;
use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct SqlBatchPayload {
    pub sql_text: String,
    pub transaction_descriptor: Vec<u8>,
    pub tds_version: String,
}

impl fmt::Debug for SqlBatchPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqlBatchPayload")
            .field("sql_text", &"[REDACTED]")
            .field("transaction_descriptor", &"[REDACTED]")
            .field("tds_version", &self.tds_version)
            .finish()
    }
}
impl SqlBatchPayload {
    pub fn buffers(&self) -> Result<Vec<Vec<u8>>, String> {
        let mut result = Vec::new();
        if self.tds_version.as_str() >= "7_2" {
            let mut header = WritableTrackingBuffer::new(18, Some(Encoding::Ucs2), false);
            write_to_tracking_buffer(&mut header, &self.transaction_descriptor, 1);
            result.push(header.data());
        }
        result.push(
            self.sql_text
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect(),
        );
        Ok(result)
    }

    pub fn to_string(&self, indent: &str) -> String {
        format!("{indent}SQL Batch - {}", self.sql_text)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RpcParameter {
    pub name: String,
    pub output: bool,
    pub encoded: EncodedParameter,
    pub encrypted_metadata: Option<EncryptedParameterMetadata>,
}

impl fmt::Debug for RpcParameter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RpcParameter")
            .field("name", &self.name)
            .field("output", &self.output)
            .field("encoded", &"[REDACTED]")
            .field(
                "encrypted_metadata",
                &self.encrypted_metadata.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct EncryptedParameterMetadata {
    pub base_type_info: Vec<u8>,
    pub algorithm_id: u8,
    pub encryption_type: u8,
    pub normalization_rule_version: u8,
    pub database_id: u32,
    pub cek_id: u32,
    pub cek_version: u32,
    pub cek_md_version: u64,
}
impl fmt::Debug for EncryptedParameterMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EncryptedParameterMetadata")
            .field("base_type_info", &"[REDACTED]")
            .field("algorithm_id", &self.algorithm_id)
            .field("encryption_type", &self.encryption_type)
            .field(
                "normalization_rule_version",
                &self.normalization_rule_version,
            )
            .field("database_id", &self.database_id)
            .field("cek_id", &self.cek_id)
            .field("cek_version", &self.cek_version)
            .field("cek_md_version", &self.cek_md_version)
            .finish()
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct RpcRequestPayload {
    pub procedure: Option<String>,
    pub procedure_id: Option<u16>,
    pub parameters: Vec<RpcParameter>,
    pub transaction_descriptor: Vec<u8>,
    pub tds_version: String,
}

impl fmt::Debug for RpcRequestPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RpcRequestPayload")
            .field("procedure", &self.procedure.as_ref().map(|_| "[REDACTED]"))
            .field("procedure_id", &self.procedure_id)
            .field(
                "parameters",
                &format_args!("[REDACTED; {} parameters]", self.parameters.len()),
            )
            .field("transaction_descriptor", &"[REDACTED]")
            .field("tds_version", &self.tds_version)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TvpColumnDefinition {
    pub name: String,
    pub type_info: Vec<u8>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct TvpRow {
    pub values: Vec<EncodedParameter>,
}

impl fmt::Debug for TvpRow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TvpRow")
            .field(
                "values",
                &format_args!("[REDACTED; {} values]", self.values.len()),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TvpPayload {
    pub database: String,
    pub schema: String,
    pub type_name: String,
    pub columns: Vec<TvpColumnDefinition>,
    pub rows: Vec<TvpRow>,
}

impl fmt::Debug for TvpPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TvpPayload")
            .field("database", &self.database)
            .field("schema", &self.schema)
            .field("type_name", &self.type_name)
            .field(
                "columns",
                &format_args!("[REDACTED; {} columns]", self.columns.len()),
            )
            .field(
                "rows",
                &format_args!("[REDACTED; {} rows]", self.rows.len()),
            )
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TvpTypedColumnDefinition {
    pub name: String,
    pub parameter_type: SqlParameterType,
    pub options: ParameterOptions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BulkLoadColumn {
    pub name: String,
    pub table_name: String,
    pub type_info: Vec<u8>,
    pub nullable: bool,
    pub legacy_lob: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct BulkLoadRow {
    pub values: Vec<EncodedParameter>,
}

impl fmt::Debug for BulkLoadRow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BulkLoadRow")
            .field(
                "values",
                &format_args!("[REDACTED; {} values]", self.values.len()),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct BulkLoadPayload {
    pub columns: Vec<BulkLoadColumn>,
    pub rows: Vec<BulkLoadRow>,
    pub tds_version: String,
}

impl fmt::Debug for BulkLoadPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BulkLoadPayload")
            .field(
                "columns",
                &format_args!("[REDACTED; {} columns]", self.columns.len()),
            )
            .field(
                "rows",
                &format_args!("[REDACTED; {} rows]", self.rows.len()),
            )
            .field("tds_version", &self.tds_version)
            .finish()
    }
}

impl BulkLoadPayload {
    pub fn column_metadata(&self) -> Result<Vec<u8>, String> {
        self.column_metadata_with_nullable_unknown(&[])
    }

    pub fn column_metadata_with_nullable_unknown(
        &self,
        nullable_unknown_columns: &[usize],
    ) -> Result<Vec<u8>, String> {
        let count = u16::try_from(self.columns.len())
            .map_err(|_| "bulk column count exceeds u16".to_owned())?;
        if nullable_unknown_columns
            .iter()
            .any(|index| *index >= self.columns.len())
        {
            return Err("bulk nullable-unknown column index is out of range".to_owned());
        }
        let mut result = vec![0x81, count as u8, (count >> 8) as u8];
        for (index, column) in self.columns.iter().enumerate() {
            if self.tds_version.as_str() < "7_2" {
                result.extend([0, 0]);
            } else {
                result.extend([0, 0, 0, 0]);
            }
            let flags: u16 = 0x0004
                | if column.nullable { 0x0001 } else { 0 }
                | if self.tds_version.as_str() >= "7_2" && nullable_unknown_columns.contains(&index)
                {
                    0x8000
                } else {
                    0
                };
            result.extend(flags.to_le_bytes());
            result.extend(&column.type_info);
            if column.legacy_lob {
                let mut table = WritableTrackingBuffer::new(
                    2 + column.table_name.encode_utf16().count() * 2,
                    Some(Encoding::Ucs2),
                    false,
                );
                table
                    .write_us_varchar(&column.table_name, Some(Encoding::Ucs2))
                    .map_err(|error| error.0)?;
                result.extend(table.data());
            }
            let units = column.name.encode_utf16().collect::<Vec<_>>();
            if units.len() > usize::from(u8::MAX) {
                return Err("bulk column name exceeds u8 character count".to_owned());
            }
            result.push(units.len() as u8);
            for unit in units {
                result.extend(unit.to_le_bytes());
            }
        }
        Ok(result)
    }

    pub fn buffers(&self) -> Result<Vec<Vec<u8>>, String> {
        if self
            .rows
            .iter()
            .any(|row| row.values.len() != self.columns.len())
        {
            return Err("bulk row value count does not match column count".to_owned());
        }
        let mut buffers = vec![self.column_metadata()?];
        for row in &self.rows {
            buffers.extend(self.row_buffers(row)?);
        }
        let mut done = vec![0xFD, 0, 0, 0, 0];
        done.extend([0; 8]);
        if self.tds_version.as_str() < "7_2" {
            done.truncate(done.len() - 4);
        }
        buffers.push(done);
        Ok(buffers)
    }

    pub fn row_buffers(&self, row: &BulkLoadRow) -> Result<Vec<Vec<u8>>, String> {
        if row.values.len() != self.columns.len() {
            return Err("bulk row value count does not match column count".to_owned());
        }
        let mut buffers = vec![vec![0xD1]];
        for (column, value) in self.columns.iter().zip(&row.values) {
            if column.legacy_lob || matches!(column.type_info.first(), Some(0x22 | 0x23 | 0x63)) {
                if value.length.is_empty() || value.length == [0] {
                    buffers.push(vec![0]);
                } else {
                    let mut pointer = vec![0x10];
                    pointer.extend([0; 15]);
                    pointer.extend([0; 8]);
                    buffers.push(pointer);
                }
            }
            buffers.push(value.length.clone());
            if !value.data.is_empty() {
                buffers.push(value.data.clone());
            }
        }
        Ok(buffers)
    }

    pub fn done_token(&self) -> Result<Vec<u8>, String> {
        self.buffers()?
            .last()
            .cloned()
            .ok_or_else(|| "bulk payload has no DONE token".to_owned())
    }

    pub fn data(&self) -> Result<Vec<u8>, String> {
        let buffers = self.buffers()?;
        let length = buffers.iter().map(Vec::len).sum();
        let mut result = Vec::with_capacity(length);
        for buffer in buffers {
            result.extend(buffer);
        }
        Ok(result)
    }

    pub fn to_string(&self, indent: &str) -> String {
        format!("{indent}BulkLoad")
    }
}

impl TvpPayload {
    pub fn from_typed_values(
        database: impl Into<String>,
        schema: impl Into<String>,
        type_name: impl Into<String>,
        columns: &[TvpTypedColumnDefinition],
        rows: &[Vec<crate::value_codec::Value>],
        collation: Option<&crate::collation::Collation>,
    ) -> Result<Self, String> {
        let mut typed_columns = Vec::with_capacity(columns.len());
        for column in columns {
            let mut options = column.options.clone();
            if let Some(collation) = collation {
                options.collation = Some(collation.clone());
            }
            let encoded = column
                .parameter_type
                .encode(&crate::value_codec::Value::Null, &options)
                .map_err(|error| {
                    format!("TVP column '{}' has invalid type: {error}", column.name)
                })?;
            typed_columns.push((column.name.clone(), column.parameter_type, options, encoded));
        }
        let mut encoded_rows = Vec::with_capacity(rows.len());
        for (row_index, row) in rows.iter().enumerate() {
            let mut values = Vec::with_capacity(row.len());
            for (column_index, value) in row.iter().enumerate() {
                let (name, parameter_type, options, _) =
                    typed_columns.get(column_index).ok_or_else(|| {
                        format!("TVP row value count exceeds column count at row index {row_index}")
                    })?;
                let encoded = parameter_type.encode(value, options).map_err(|error| {
                    format!(
                        "TVP column '{name}' has invalid data at row index {row_index}: {error}"
                    )
                })?;
                values.push(encoded);
            }
            encoded_rows.push(TvpRow { values });
        }
        Ok(Self {
            database: database.into(),
            schema: schema.into(),
            type_name: type_name.into(),
            columns: typed_columns
                .into_iter()
                .map(|(_, _, _, encoded)| TvpColumnDefinition {
                    name: String::new(),
                    type_info: encoded.type_info,
                })
                .collect(),
            rows: encoded_rows,
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [("schema", &self.schema), ("type name", &self.type_name)] {
            if value.encode_utf16().count() > usize::from(u8::MAX) {
                return Err(format!("TVP {name} exceeds u8 character count"));
            }
        }
        if self.columns.len() > usize::from(u16::MAX) {
            return Err("TVP column count exceeds u16".to_owned());
        }
        for column in &self.columns {
            if column.name.encode_utf16().count() > usize::from(u8::MAX) {
                return Err("TVP column name exceeds u8 character count".to_owned());
            }
        }
        Ok(())
    }

    pub fn type_info(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut buffer = WritableTrackingBuffer::new(64, Some(Encoding::Ucs2), false);
        buffer.write_u8(0xF3);
        buffer
            .write_b_varchar("", Some(Encoding::Ucs2))
            .map_err(|error| error.0)?;
        buffer
            .write_b_varchar(&self.schema, Some(Encoding::Ucs2))
            .map_err(|error| error.0)?;
        buffer
            .write_b_varchar(&self.type_name, Some(Encoding::Ucs2))
            .map_err(|error| error.0)?;
        Ok(buffer.data())
    }

    pub fn parameter_length(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let count = u16::try_from(self.columns.len())
            .map_err(|_| "TVP column count exceeds u16".to_owned())?;
        Ok(count.to_le_bytes().to_vec())
    }

    pub fn data(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut result = Vec::new();
        for column in &self.columns {
            result.extend([0, 0, 0, 0, 0, 0]);
            result.extend(&column.type_info);
            let mut name = WritableTrackingBuffer::new(1, Some(Encoding::Ucs2), false);
            name.write_b_varchar("", Some(Encoding::Ucs2))
                .map_err(|error| error.0)?;
            result.extend(name.data());
        }
        result.push(0);
        for row in &self.rows {
            if row.values.len() > self.columns.len() {
                return Err("TVP row value count exceeds column count".to_owned());
            }
            result.push(1);
            for value in &row.values {
                result.extend(&value.length);
                result.extend(&value.data);
            }
        }
        result.push(0);
        Ok(result)
    }

    pub fn buffers(&self) -> Result<Vec<Vec<u8>>, String> {
        self.validate()?;
        Ok(vec![
            self.type_info()?,
            self.parameter_length()?,
            self.data()?,
        ])
    }
}
impl RpcRequestPayload {
    pub fn buffers(&self) -> Result<Vec<Vec<u8>>, String> {
        let mut first = WritableTrackingBuffer::new(500, Some(Encoding::Ucs2), false);
        if self.tds_version.as_str() >= "7_2" {
            write_to_tracking_buffer(&mut first, &self.transaction_descriptor, 1);
        }
        match (&self.procedure, self.procedure_id) {
            (Some(procedure), _) => first
                .write_us_varchar(procedure, Some(Encoding::Ucs2))
                .map_err(|error| error.0)?,
            (None, Some(id)) => {
                first.write_u16_le(u16::MAX);
                first.write_u16_le(id);
            }
            (None, None) => return Err("RPC procedure is missing".to_owned()),
        }
        first.write_u16_le(0);
        let mut result = vec![first.data()];
        for parameter in &self.parameters {
            let mut prefix = WritableTrackingBuffer::new(
                3 + parameter.name.encode_utf16().count() * 2,
                Some(Encoding::Ucs2),
                false,
            );
            let parameter_name = if parameter.name.is_empty() {
                String::new()
            } else {
                format!("@{}", parameter.name)
            };
            prefix
                .write_b_varchar(&parameter_name, Some(Encoding::Ucs2))
                .map_err(|error| error.0)?;
            let status = if parameter.output { 0x01 } else { 0 };
            prefix.write_u8(status);
            result.push(prefix.data());
            result.push(parameter.encoded.type_info.clone());
            result.push(parameter.encoded.length.clone());
            result.push(parameter.encoded.data.clone());
        }
        Ok(result)
    }
}
