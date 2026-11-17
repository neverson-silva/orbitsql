macro_rules! define_data_type_module {
    ($lookup:literal) => {
        pub fn data_type() -> crate::data_type::DataType {
            crate::data_type::type_by_name($lookup).expect("registered data type")
        }

        pub fn declaration(
            value: &crate::value_codec::Value,
            options: &crate::parameter::ParameterOptions,
            output: bool,
        ) -> Result<String, crate::value_codec::CodecError> {
            data_type().declaration(value, options, output)
        }

        pub fn generate_type_info(
            value: &crate::value_codec::Value,
            options: &crate::parameter::ParameterOptions,
        ) -> Result<Vec<u8>, crate::value_codec::CodecError> {
            Ok(data_type().encode(value, options)?.type_info)
        }

        pub fn generate_parameter_length(
            value: &crate::value_codec::Value,
            options: &crate::parameter::ParameterOptions,
        ) -> Result<Vec<u8>, crate::value_codec::CodecError> {
            Ok(data_type().encode(value, options)?.length)
        }

        pub fn generate_parameter_data(
            value: &crate::value_codec::Value,
            options: &crate::parameter::ParameterOptions,
        ) -> Result<Vec<u8>, crate::value_codec::CodecError> {
            Ok(data_type().encode(value, options)?.data)
        }

        pub fn validate(
            value: &crate::value_codec::Value,
            options: &crate::parameter::ParameterOptions,
        ) -> Result<(), crate::value_codec::CodecError> {
            data_type().encode(value, options).map(|_| ())
        }
    };
}

pub mod bigint;
pub mod binary;
pub mod bit;
pub mod bitn;
pub mod char;
pub mod date;
pub mod datetime;
pub mod datetime2;
pub mod datetimen;
pub mod datetimeoffset;
pub mod decimal;
pub mod decimaln;
pub mod float;
pub mod floatn;
pub mod image;
pub mod int;
pub mod intn;
pub mod money;
pub mod moneyn;
pub mod nchar;
pub mod ntext;
pub mod null;
pub mod numeric;
pub mod numericn;
pub mod nvarchar;
pub mod real;
pub mod smalldatetime;
pub mod smallint;
pub mod smallmoney;
pub mod sql_variant;
pub mod text;
pub mod time;
pub mod tinyint;
pub mod tvp;
pub mod udt;
pub mod uniqueidentifier;
pub mod varbinary;
pub mod varchar;
pub mod xml;
