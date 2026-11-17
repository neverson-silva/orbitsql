use tedious_rs::data_type::{type_by_id, type_by_name};
use tedious_rs::metadata_parser::read_metadata;
use tedious_rs::parameter::ParameterOptions;
use tedious_rs::value_codec::Value;
use tedious_rs::value_parser::{ValueParserError, read_value};

fn metadata(id: u8, extra: &[u8]) -> tedious_rs::metadata_parser::Metadata {
    let mut bytes = vec![0, 0, 0, 0, 0, 0, id];
    bytes.extend_from_slice(extra);
    read_metadata(&bytes, 0, "7_4").unwrap().value
}

fn assert_executable(result: Result<(), tedious_rs::CodecError>) {
    assert!(result.is_ok());
}

#[test]
fn source_modules_preserve_node_identity_and_executable_operations() {
    let cases = [
        ("IntN", 0x26, tedious_rs::data_types::intn::data_type().name),
        (
            "NumericN",
            0x6C,
            tedious_rs::data_types::numericn::data_type().name,
        ),
        (
            "DecimalN",
            0x6A,
            tedious_rs::data_types::decimaln::data_type().name,
        ),
    ];
    for (name, id, module_name) in cases {
        assert_eq!(module_name, name);
        assert_eq!(type_by_id(id), type_by_name(name));
    }

    let values = [Value::Null, Value::Integer(-7)];
    let options = [
        ParameterOptions::default(),
        ParameterOptions {
            length: Some(8),
            precision: Some(19),
            scale: Some(4),
            ..Default::default()
        },
    ];
    for value in &values {
        for option in &options {
            assert_executable(
                tedious_rs::data_types::intn::declaration(value, option, false).map(|_| ()),
            );
            assert_executable(
                tedious_rs::data_types::numericn::declaration(value, option, false).map(|_| ()),
            );
            assert_executable(
                tedious_rs::data_types::decimaln::declaration(value, option, false).map(|_| ()),
            );
            for operation in [
                tedious_rs::data_types::intn::generate_type_info(value, option).map(|_| ()),
                tedious_rs::data_types::intn::generate_parameter_length(value, option).map(|_| ()),
                tedious_rs::data_types::intn::generate_parameter_data(value, option).map(|_| ()),
                tedious_rs::data_types::intn::validate(value, option),
                tedious_rs::data_types::numericn::generate_type_info(value, option).map(|_| ()),
                tedious_rs::data_types::numericn::generate_parameter_length(value, option)
                    .map(|_| ()),
                tedious_rs::data_types::numericn::generate_parameter_data(value, option)
                    .map(|_| ()),
                tedious_rs::data_types::numericn::validate(value, option),
                tedious_rs::data_types::decimaln::generate_type_info(value, option).map(|_| ()),
                tedious_rs::data_types::decimaln::generate_parameter_length(value, option)
                    .map(|_| ()),
                tedious_rs::data_types::decimaln::generate_parameter_data(value, option)
                    .map(|_| ()),
                tedious_rs::data_types::decimaln::validate(value, option),
            ] {
                assert_executable(operation);
            }
        }
    }
}

#[test]
fn nullable_n_serialization_preserves_wire_operations() {
    let options = ParameterOptions {
        precision: Some(19),
        scale: Some(4),
        ..Default::default()
    };
    for name in ["IntN", "NumericN", "DecimalN"] {
        let result = type_by_name(name).unwrap().encode(&Value::Null, &options);
        assert!(result.is_ok());
    }
}

#[test]
fn n_parsing_preserves_null_values_offsets_and_numeric_scale() {
    let int = read_value(&[0], 0, &metadata(0x26, &[4])).unwrap();
    assert_eq!(int.value, Value::Null);
    assert_eq!(int.offset, 1);

    let int = read_value(&[4, 0xFE, 0xFF, 0xFF, 0xFF], 0, &metadata(0x26, &[4])).unwrap();
    assert_eq!(int.value, Value::Integer(-2));
    assert_eq!(int.offset, 5);

    let numeric = read_value(&[5, 1, 0x39, 0x30, 0, 0], 0, &metadata(0x6C, &[5, 9, 2])).unwrap();
    assert_eq!(
        numeric.value,
        Value::Decimal {
            unscaled: 12345,
            scale: 2
        }
    );
    assert_eq!(numeric.offset, 6);

    let decimal = read_value(&[0], 0, &metadata(0x6A, &[5, 9, 2])).unwrap();
    assert_eq!(decimal.value, Value::Null);
    assert_eq!(decimal.offset, 1);
}

#[test]
fn n_parsing_rejects_invalid_lengths_and_truncated_values() {
    let invalid_int = read_value(&[3, 1, 2, 3], 0, &metadata(0x26, &[4]));
    assert_eq!(
        invalid_int,
        Err(ValueParserError::Unsupported(
            "Unsupported dataLength 3 for IntN".to_owned()
        ))
    );

    let truncated_int = read_value(&[4, 1, 2], 0, &metadata(0x26, &[4]));
    assert!(matches!(
        truncated_int,
        Err(ValueParserError::NotEnoughData(_))
    ));

    let truncated_decimal = read_value(&[5, 1, 1, 2], 0, &metadata(0x6A, &[5, 9, 2]));
    assert!(matches!(
        truncated_decimal,
        Err(ValueParserError::NotEnoughData(_))
    ));
}
