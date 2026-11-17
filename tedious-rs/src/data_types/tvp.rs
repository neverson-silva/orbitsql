use crate::parameter::ParameterOptions;
use crate::value_codec::{CodecError, Value};

pub fn data_type() -> crate::data_type::DataType {
    crate::data_type::type_by_name("TVP").expect("registered data type")
}

pub fn declaration(value: &Value, _: &ParameterOptions, _: bool) -> Result<String, CodecError> {
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
    Ok(format!("{qualified_name} readonly"))
}

pub fn generate_type_info(value: &Value, _: &ParameterOptions) -> Result<Vec<u8>, CodecError> {
    match value {
        Value::Null => Ok(vec![0xF3, 0, 0, 0]),
        Value::Tvp(payload) => payload.type_info().map_err(CodecError),
        _ => Err(CodecError("invalid table value".to_owned())),
    }
}

pub fn generate_parameter_length(
    value: &Value,
    _: &ParameterOptions,
) -> Result<Vec<u8>, CodecError> {
    match value {
        Value::Null => Ok(vec![0xFF, 0xFF]),
        Value::Tvp(payload) => payload.parameter_length().map_err(CodecError),
        _ => Err(CodecError("invalid table value".to_owned())),
    }
}

pub fn generate_parameter_data(value: &Value, _: &ParameterOptions) -> Result<Vec<u8>, CodecError> {
    match value {
        Value::Null => Ok(vec![0, 0]),
        Value::Tvp(payload) => payload.data().map_err(CodecError),
        _ => Err(CodecError("invalid table value".to_owned())),
    }
}

pub fn validate(value: &Value, _: &ParameterOptions) -> Result<(), CodecError> {
    match value {
        Value::Null => Ok(()),
        Value::Tvp(payload) => payload.validate().map_err(CodecError),
        _ => Err(CodecError("invalid table value".to_owned())),
    }
}
