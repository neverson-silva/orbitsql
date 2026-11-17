use tedious_rs::data_type::{type_by_id, type_by_name};
use tedious_rs::parameter::{ParameterOptions, SqlParameterType};
use tedious_rs::value_codec::Value;

fn numeric_value() -> Value {
    Value::Decimal {
        unscaled: -12345,
        scale: 2,
    }
}

#[test]
fn numeric_keeps_the_node_numericn_wire_type_id() {
    let options = ParameterOptions {
        precision: Some(9),
        scale: Some(2),
        ..Default::default()
    };
    let value = numeric_value();

    let encoded = type_by_id(0x3F).unwrap().encode(&value, &options).unwrap();
    assert_eq!(encoded.type_info, vec![0x6C, 5, 9, 2]);
    assert_eq!(encoded.length, vec![5]);
    assert_eq!(encoded.data, vec![0, 0x39, 0x30, 0, 0]);
}

#[test]
fn decimal_parameter_codec_keeps_decimaln_wire_type_id() {
    let options = ParameterOptions {
        precision: Some(9),
        scale: Some(2),
        ..Default::default()
    };
    let encoded = SqlParameterType::Decimal
        .encode(&numeric_value(), &options)
        .unwrap();
    assert_eq!(encoded.type_info, vec![0x6A, 5, 9, 2]);
}

#[test]
fn numeric_defaults_and_null_match_node_parameter_metadata() {
    let numeric = type_by_name("Numeric").unwrap();
    assert_eq!(numeric.id, 0x3F);
    assert_eq!(
        numeric
            .encode(&Value::Null, &ParameterOptions::default())
            .unwrap()
            .type_info,
        vec![0x6C, 5, 1, 0]
    );
}
