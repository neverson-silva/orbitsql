use crate::parameter::{ParameterOptions, SqlParameterType};
use crate::value_codec::{CodecError, EncodedParameter, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DataType {
    pub id: u8,
    pub type_name: &'static str,
    pub name: &'static str,
    pub maximum_length: Option<usize>,
    pub has_table_name: bool,
    pub parameter_type: Option<SqlParameterType>,
}

macro_rules! datatype {
    ($id:expr, $type_name:expr, $name:expr, $maximum_length:expr, $has_table_name:expr, $parameter_type:expr) => {
        DataType {
            id: $id,
            type_name: $type_name,
            name: $name,
            maximum_length: $maximum_length,
            has_table_name: $has_table_name,
            parameter_type: $parameter_type,
        }
    };
}

pub const TYPES: [DataType; 39] = [
    datatype!(0x1F, "NULL", "Null", None, false, None),
    datatype!(
        0x30,
        "INT1",
        "TinyInt",
        None,
        false,
        Some(SqlParameterType::TinyInt)
    ),
    datatype!(0x32, "BIT", "Bit", None, false, Some(SqlParameterType::Bit)),
    datatype!(
        0x34,
        "INT2",
        "SmallInt",
        None,
        false,
        Some(SqlParameterType::SmallInt)
    ),
    datatype!(
        0x37,
        "DECIMAL",
        "Decimal",
        None,
        false,
        Some(SqlParameterType::Decimal)
    ),
    datatype!(
        0x38,
        "INT4",
        "Int",
        None,
        false,
        Some(SqlParameterType::Int)
    ),
    datatype!(
        0x3A,
        "DATETIM4",
        "SmallDateTime",
        None,
        false,
        Some(SqlParameterType::SmallDateTime)
    ),
    datatype!(
        0x3B,
        "FLT4",
        "Real",
        None,
        false,
        Some(SqlParameterType::Real)
    ),
    datatype!(
        0x3C,
        "MONEY",
        "Money",
        None,
        false,
        Some(SqlParameterType::Money)
    ),
    datatype!(
        0x3D,
        "DATETIME",
        "DateTime",
        None,
        false,
        Some(SqlParameterType::DateTime)
    ),
    datatype!(
        0x3E,
        "FLT8",
        "Float",
        None,
        false,
        Some(SqlParameterType::Float)
    ),
    datatype!(
        0x3F,
        "NUMERIC",
        "Numeric",
        None,
        false,
        Some(SqlParameterType::Decimal)
    ),
    datatype!(
        0x22,
        "IMAGE",
        "Image",
        None,
        true,
        Some(SqlParameterType::Image)
    ),
    datatype!(
        0x23,
        "TEXT",
        "Text",
        None,
        true,
        Some(SqlParameterType::Text)
    ),
    datatype!(
        0x24,
        "GUIDN",
        "UniqueIdentifier",
        None,
        false,
        Some(SqlParameterType::UniqueIdentifier)
    ),
    datatype!(
        0x26,
        "INTN",
        "IntN",
        None,
        false,
        Some(SqlParameterType::Int)
    ),
    datatype!(
        0x28,
        "DATEN",
        "Date",
        None,
        false,
        Some(SqlParameterType::Date)
    ),
    datatype!(
        0x29,
        "TIMEN",
        "Time",
        None,
        false,
        Some(SqlParameterType::Time)
    ),
    datatype!(
        0x2A,
        "DATETIME2N",
        "DateTime2",
        None,
        false,
        Some(SqlParameterType::DateTime2)
    ),
    datatype!(
        0x2B,
        "DATETIMEOFFSETN",
        "DateTimeOffset",
        None,
        false,
        Some(SqlParameterType::DateTimeOffset)
    ),
    datatype!(
        0x63,
        "NTEXT",
        "NText",
        None,
        true,
        Some(SqlParameterType::NText)
    ),
    datatype!(
        0x62,
        "SSVARIANTTYPE",
        "Variant",
        None,
        false,
        Some(SqlParameterType::Variant)
    ),
    datatype!(
        0x68,
        "BITN",
        "BitN",
        None,
        false,
        Some(SqlParameterType::Bit)
    ),
    datatype!(
        0x6A,
        "DECIMALN",
        "DecimalN",
        None,
        false,
        Some(SqlParameterType::Decimal)
    ),
    datatype!(
        0x6C,
        "NUMERICN",
        "NumericN",
        None,
        false,
        Some(SqlParameterType::Decimal)
    ),
    datatype!(
        0x6D,
        "FLTN",
        "FloatN",
        None,
        false,
        Some(SqlParameterType::Float)
    ),
    datatype!(
        0x6E,
        "MONEYN",
        "MoneyN",
        None,
        false,
        Some(SqlParameterType::Money)
    ),
    datatype!(
        0x6F,
        "DATETIMN",
        "DateTimeN",
        None,
        false,
        Some(SqlParameterType::DateTime)
    ),
    datatype!(
        0x7A,
        "MONEY4",
        "SmallMoney",
        None,
        false,
        Some(SqlParameterType::SmallMoney)
    ),
    datatype!(
        0x7F,
        "INT8",
        "BigInt",
        None,
        false,
        Some(SqlParameterType::BigInt)
    ),
    datatype!(
        0xA5,
        "BIGVARBIN",
        "VarBinary",
        Some(8000),
        false,
        Some(SqlParameterType::VarBinary)
    ),
    datatype!(
        0xA7,
        "BIGVARCHR",
        "VarChar",
        Some(8000),
        false,
        Some(SqlParameterType::VarChar)
    ),
    datatype!(
        0xAD,
        "BIGBinary",
        "Binary",
        Some(8000),
        false,
        Some(SqlParameterType::Binary)
    ),
    datatype!(
        0xAF,
        "BIGCHAR",
        "Char",
        Some(8000),
        false,
        Some(SqlParameterType::Char)
    ),
    datatype!(
        0xE7,
        "NVARCHAR",
        "NVarChar",
        Some(4000),
        false,
        Some(SqlParameterType::NVarChar)
    ),
    datatype!(
        0xEF,
        "NCHAR",
        "NChar",
        Some(4000),
        false,
        Some(SqlParameterType::NChar)
    ),
    datatype!(
        0xF0,
        "UDTTYPE",
        "UDT",
        None,
        false,
        Some(SqlParameterType::Udt)
    ),
    datatype!(0xF1, "XML", "Xml", None, false, Some(SqlParameterType::Xml)),
    datatype!(
        0xF3,
        "TVPTYPE",
        "TVP",
        None,
        false,
        Some(SqlParameterType::Tvp)
    ),
];

impl DataType {
    pub fn declaration(
        &self,
        value: &Value,
        options: &ParameterOptions,
        output: bool,
    ) -> Result<String, CodecError> {
        let length = options
            .length
            .filter(|length| *length != 0)
            .unwrap_or_else(|| {
                usize::try_from(self.resolve_length_with_options(value, output, options))
                    .unwrap_or(0)
            });
        let length = if matches!(self.name, "Binary" | "VarBinary") && length == 0 {
            1
        } else {
            length
        };
        let precision = self.resolve_precision_for_value(value, options);
        let scale = self.resolve_scale_for_value(value, options);
        let temporal_scale = self.resolve_temporal_scale_for_value(value, options);
        let declaration = match self.name {
            "Null" => "NULL".to_owned(),
            "TinyInt" => "tinyint".to_owned(),
            "SmallInt" => "smallint".to_owned(),
            "Int" => "int".to_owned(),
            "IntN" => "int".to_owned(),
            "BigInt" => "bigint".to_owned(),
            "Bit" => "bit".to_owned(),
            "BitN" => "bit".to_owned(),
            "Real" => "real".to_owned(),
            "FloatN" => "float".to_owned(),
            "Float" => "float".to_owned(),
            "Money" => "money".to_owned(),
            "MoneyN" => "money".to_owned(),
            "SmallMoney" => "smallmoney".to_owned(),
            "Date" => "date".to_owned(),
            "Time" => format!("time({temporal_scale})"),
            "DateTime" => "datetime".to_owned(),
            "DateTimeN" => "datetime".to_owned(),
            "SmallDateTime" => "smalldatetime".to_owned(),
            "DateTime2" => format!("datetime2({temporal_scale})"),
            "DateTimeOffset" => format!("datetimeoffset({temporal_scale})"),
            "Decimal" => format!("decimal({}, {})", precision, scale),
            "Numeric" => format!("numeric({}, {})", precision, scale),
            "DecimalN" => format!("decimal({}, {})", precision, scale),
            "NumericN" => format!("numeric({}, {})", precision, scale),
            "UniqueIdentifier" => "uniqueidentifier".to_owned(),
            "NVarChar" => string_declaration("nvarchar", length, 4000),
            "VarChar" => string_declaration("varchar", length, 8000),
            "NChar" => format!("nchar({})", length.min(4000)),
            "Char" => format!("char({})", length.min(8000)),
            "Text" => "text".to_owned(),
            "NText" => "ntext".to_owned(),
            "Image" => "image".to_owned(),
            "VarBinary" => string_declaration("varbinary", length, 8000),
            "Binary" => format!("binary({length})"),
            "Xml" => "xml".to_owned(),
            "Variant" => "sql_variant".to_owned(),
            "TVP" => {
                let payload = match value {
                    Value::Tvp(payload) => payload,
                    Value::Null => {
                        return Err(CodecError(
                            "TVP declaration requires a named table type".to_owned(),
                        ));
                    }
                    _ => return Err(CodecError("invalid table value".to_owned())),
                };
                let qualified_name = if payload.schema.is_empty() {
                    payload.type_name.clone()
                } else {
                    format!("{}.{}", payload.schema, payload.type_name)
                };
                format!("{qualified_name} readonly")
            }
            "UDT" => match value {
                Value::Udt {
                    database,
                    schema,
                    type_name,
                    ..
                } => {
                    if database.is_empty() {
                        format!("{schema}.{type_name}")
                    } else {
                        format!("{database}.{schema}.{type_name}")
                    }
                }
                Value::Null => {
                    return Err(CodecError(
                        "UDT declaration requires type metadata".to_owned(),
                    ));
                }
                _ => return Err(CodecError("invalid UDT value".to_owned())),
            },
            _ => return Err(CodecError("unsupported data type".to_owned())),
        };
        Ok(declaration)
    }

    pub fn resolve_length(&self, value: &Value, output: bool) -> i32 {
        if let Some(length) = self.maximum_length {
            return match value {
                Value::Text(text) if !text.is_empty() => {
                    i32::try_from(text.encode_utf16().count()).unwrap_or(i32::MAX)
                }
                Value::Binary(bytes) => i32::try_from(bytes.len()).unwrap_or(i32::MAX),
                _ if output || matches!(value, Value::Null) => {
                    i32::try_from(length).unwrap_or(i32::MAX)
                }
                _ => 1,
            };
        }
        match self.name {
            "Text" => match value {
                Value::Text(text) => i32::try_from(
                    crate::value_codec::encoded_non_unicode_length(
                        text,
                        &crate::collation::Collation::new(0x0409, 0, 0, 0),
                    )
                    .unwrap_or(0),
                )
                .unwrap_or(i32::MAX),
                _ if output || matches!(value, Value::Null) => -1,
                _ => 1,
            },
            "NText" => match value {
                Value::Text(text) => i32::try_from(text.encode_utf16().count()).unwrap_or(i32::MAX),
                _ if output || matches!(value, Value::Null) => -1,
                _ => 1,
            },
            "Image" => match value {
                Value::Binary(bytes) => i32::try_from(bytes.len()).unwrap_or(i32::MAX),
                _ if output || matches!(value, Value::Null) => -1,
                _ => 1,
            },
            _ => 0,
        }
    }

    pub fn resolve_length_with_options(
        &self,
        value: &Value,
        output: bool,
        options: &ParameterOptions,
    ) -> i32 {
        if matches!(self.name, "VarChar" | "Char")
            && let Value::Text(text) = value
        {
            let collation = options
                .collation
                .as_ref()
                .cloned()
                .unwrap_or_else(|| crate::collation::Collation::new(0x0409, 0, 0, 0));
            return i32::try_from(
                crate::value_codec::encoded_non_unicode_length(text, &collation)
                    .unwrap_or(0)
                    .max(1),
            )
            .unwrap_or(i32::MAX);
        }
        if self.name == "Text"
            && let Value::Text(text) = value
        {
            let collation = options
                .collation
                .as_ref()
                .cloned()
                .unwrap_or_else(|| crate::collation::Collation::new(0x0409, 0, 0, 0));
            return i32::try_from(
                crate::value_codec::encoded_non_unicode_length(text, &collation).unwrap_or(0),
            )
            .unwrap_or(i32::MAX);
        }
        self.resolve_length(value, output)
    }

    pub fn resolve_precision(&self, options: &ParameterOptions) -> u8 {
        options.precision.unwrap_or(18)
    }

    pub fn resolve_scale(&self, options: &ParameterOptions) -> u8 {
        options.scale.unwrap_or(0)
    }

    pub fn resolve_precision_for_value(&self, value: &Value, options: &ParameterOptions) -> u8 {
        options
            .precision
            .unwrap_or(if matches!(value, Value::Null) { 1 } else { 18 })
    }

    pub fn resolve_scale_for_value(&self, _value: &Value, options: &ParameterOptions) -> u8 {
        options.scale.unwrap_or(0)
    }

    pub fn resolve_temporal_scale_for_value(
        &self,
        value: &Value,
        options: &ParameterOptions,
    ) -> u8 {
        options
            .scale
            .unwrap_or(if matches!(value, Value::Null) { 0 } else { 7 })
    }

    pub fn encode(
        &self,
        value: &Value,
        options: &ParameterOptions,
    ) -> Result<EncodedParameter, CodecError> {
        if self.name == "Null" {
            if !matches!(value, Value::Null) {
                return Err(CodecError("invalid null value".to_owned()));
            }
            return Ok(EncodedParameter {
                type_info: vec![0x1F],
                length: Vec::new(),
                data: Vec::new(),
            });
        }
        let (parameter_type, type_id) = (
            self.parameter_type
                .ok_or_else(|| CodecError(format!("{} cannot encode parameters", self.name)))?,
            self.numeric_parameter_type_id(),
        );
        parameter_type.encode_with_type_id(value, options, type_id)
    }

    fn numeric_parameter_type_id(&self) -> Option<u8> {
        match self.id {
            0x26 => Some(0x26),
            0x68 => Some(0x68),
            0x6D => Some(0x6D),
            0x6E => Some(0x6E),
            0x6F => Some(0x6F),
            0x6A => Some(0x6A),
            0x3F | 0x6C => Some(0x6C),
            _ => None,
        }
    }
}

pub fn type_for_parameter_type(parameter_type: SqlParameterType) -> Option<DataType> {
    TYPES.iter().copied().find(|data_type| {
        data_type.parameter_type == Some(parameter_type) && !data_type.name.ends_with('N')
    })
}

fn string_declaration(name: &str, length: usize, maximum_length: usize) -> String {
    if length <= maximum_length {
        format!("{name}({length})")
    } else {
        format!("{name}(max)")
    }
}

pub fn type_by_id(id: u8) -> Option<DataType> {
    TYPES.iter().copied().find(|value| value.id == id)
}
pub fn type_by_name(name: &str) -> Option<DataType> {
    TYPES.iter().copied().find(|value| value.name == name)
}
