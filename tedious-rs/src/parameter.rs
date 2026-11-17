use crate::collation::Collation;
use crate::value_codec::{CodecError, EncodedParameter, Value};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParameterOptions {
    pub output: bool,
    pub length: Option<usize>,
    pub precision: Option<u8>,
    pub scale: Option<u8>,
    pub collation: Option<Collation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SqlParameterType {
    TinyInt,
    SmallInt,
    Int,
    BigInt,
    Bit,
    Real,
    Float,
    Decimal,
    Money,
    SmallMoney,
    Date,
    Time,
    DateTime,
    SmallDateTime,
    DateTime2,
    DateTimeOffset,
    UniqueIdentifier,
    NVarChar,
    VarChar,
    Char,
    NChar,
    VarBinary,
    Binary,
    Text,
    NText,
    Image,
    Xml,
    Variant,
    Tvp,
    Udt,
}

impl SqlParameterType {
    pub fn encode(
        &self,
        value: &Value,
        options: &ParameterOptions,
    ) -> Result<EncodedParameter, CodecError> {
        self.encode_with_type_id(value, options, None)
    }

    pub fn encode_with_type_id(
        &self,
        value: &Value,
        options: &ParameterOptions,
        type_id: Option<u8>,
    ) -> Result<EncodedParameter, CodecError> {
        let default_collation = Collation::new(0x0409, 0, 0, 0);
        let collation = options.collation.as_ref().unwrap_or(&default_collation);
        let length = options
            .length
            .filter(|length| *length != 0)
            .unwrap_or_else(|| default_length(self, value, collation));
        let precision =
            options
                .precision
                .or(Some(if matches!(value, Value::Null) { 1 } else { 18 }));
        let scale = options.scale.or(Some(0));
        let temporal_scale =
            options
                .scale
                .unwrap_or(if matches!(value, Value::Null) { 0 } else { 7 });
        let lob_length = if matches!(value, Value::Null)
            && matches!(self, Self::Text | Self::NText | Self::Image)
            && options.length.is_none()
        {
            -1
        } else {
            i32::try_from(length).map_err(|_| CodecError("LOB length exceeds i32".to_owned()))?
        };
        let mut encoded = match self {
            Self::TinyInt => crate::value_codec::encode_tinyint(value),
            Self::SmallInt => crate::value_codec::encode_smallint(value),
            Self::Int => crate::value_codec::encode_int(value),
            Self::BigInt => crate::value_codec::encode_bigint(value),
            Self::Bit => crate::value_codec::encode_bit(value),
            Self::Real => crate::value_codec::encode_real(value),
            Self::Float => crate::value_codec::encode_float(value),
            Self::Decimal => crate::value_codec::encode_decimal(
                value,
                precision.unwrap_or(18),
                scale.unwrap_or(0),
            ),
            Self::Money => crate::value_codec::encode_money(value),
            Self::SmallMoney => crate::value_codec::encode_smallmoney(value),
            Self::Date => crate::value_codec::encode_date(value),
            Self::Time => crate::value_codec::encode_time(value, temporal_scale),
            Self::DateTime => crate::value_codec::encode_datetime(value),
            Self::SmallDateTime => crate::value_codec::encode_smalldatetime(value),
            Self::DateTime2 => crate::value_codec::encode_datetime2(value, temporal_scale),
            Self::DateTimeOffset => {
                crate::value_codec::encode_datetimeoffset(value, temporal_scale)
            }
            Self::UniqueIdentifier => crate::value_codec::encode_uniqueidentifier(value),
            Self::NVarChar => crate::value_codec::encode_nvarchar(value, length),
            Self::VarChar => crate::value_codec::encode_varchar(value, length, collation),
            Self::Char => crate::value_codec::encode_char(value, length, collation),
            Self::NChar => crate::value_codec::encode_nchar(value, length),
            Self::VarBinary => crate::value_codec::encode_varbinary(value, length),
            Self::Binary => crate::value_codec::encode_binary(value, length),
            Self::Text => crate::value_codec::encode_text(value, lob_length, collation),
            Self::NText => {
                crate::value_codec::encode_ntext_with_collation(value, lob_length, collation)
            }
            Self::Image => crate::value_codec::encode_image(value, lob_length),
            Self::Xml => crate::value_codec::encode_xml(value),
            Self::Variant => crate::value_codec::encode_variant(value, collation),
            Self::Tvp => crate::value_codec::encode_tvp(value),
            Self::Udt => crate::value_codec::encode_udt(value),
        }?;
        if options.collation.is_some()
            && matches!(
                self,
                Self::NVarChar | Self::NChar | Self::VarChar | Self::Char
            )
            && encoded.type_info.len() >= 8
        {
            let mut collation_bytes = collation.clone();
            encoded.type_info[3..8].copy_from_slice(&collation_bytes.to_buffer());
        } else if options.collation.is_some()
            && matches!(self, Self::Text | Self::NText)
            && encoded.type_info.len() >= 10
        {
            let mut collation_bytes = collation.clone();
            encoded.type_info[5..10].copy_from_slice(&collation_bytes.to_buffer());
        }
        if let Some(type_id) = type_id
            && let Some(first) = encoded.type_info.first_mut()
        {
            *first = type_id;
        }
        Ok(encoded)
    }
}

fn default_length(kind: &SqlParameterType, value: &Value, collation: &Collation) -> usize {
    match value {
        Value::Text(text)
            if matches!(kind, SqlParameterType::NVarChar | SqlParameterType::NChar) =>
        {
            text.encode_utf16().count().max(1)
        }
        Value::Text(text) if matches!(kind, SqlParameterType::VarChar | SqlParameterType::Char) => {
            crate::value_codec::encoded_non_unicode_length(text, collation)
                .unwrap_or(0)
                .max(1)
        }
        Value::Text(text) if matches!(kind, SqlParameterType::Text | SqlParameterType::NText) => {
            if matches!(kind, SqlParameterType::NText) {
                text.encode_utf16().count()
            } else {
                crate::value_codec::encoded_non_unicode_length(text, collation).unwrap_or(0)
            }
        }
        Value::Binary(bytes) if matches!(kind, SqlParameterType::Image) => bytes.len(),
        Value::Binary(bytes) => bytes.len().max(1),
        _ => match kind {
            SqlParameterType::NVarChar | SqlParameterType::NChar => 1,
            SqlParameterType::VarChar
            | SqlParameterType::Char
            | SqlParameterType::VarBinary
            | SqlParameterType::Binary => 1,
            SqlParameterType::Text | SqlParameterType::NText | SqlParameterType::Image => {
                i32::MAX as usize
            }
            _ => 0,
        },
    }
}
