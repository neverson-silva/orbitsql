use tedious_rs::metadata_parser::read_metadata;
use tedious_rs::parameter::ParameterOptions;
use tedious_rs::value_codec::Value;
use tedious_rs::value_parser::read_value_with_options;

fn metadata(id: u8, extra: &[u8]) -> tedious_rs::metadata_parser::Metadata {
    let mut bytes = vec![0, 0, 0, 0, 0, 0, id];
    bytes.extend_from_slice(extra);
    read_metadata(&bytes, 0, "7_4").unwrap().value
}

fn assert_executable(result: Result<(), tedious_rs::value_codec::CodecError>) {
    assert!(result.is_ok());
}

#[test]
fn nullable_numeric_type_modules_preserve_node_identity_and_executable_contract() {
    let options = ParameterOptions::default();
    let value = Value::Null;
    let modules = [
        (tedious_rs::data_types::bitn::data_type(), 0x68),
        (tedious_rs::data_types::moneyn::data_type(), 0x6E),
        (tedious_rs::data_types::datetimen::data_type(), 0x6F),
    ];

    for (data_type, id) in modules {
        assert_eq!(data_type.id, id);
        assert!(data_type.declaration(&value, &options, false).is_ok());
    }

    for operation in [
        tedious_rs::data_types::bitn::generate_type_info(&value, &options).map(|_| ()),
        tedious_rs::data_types::bitn::generate_parameter_length(&value, &options).map(|_| ()),
        tedious_rs::data_types::bitn::generate_parameter_data(&value, &options).map(|_| ()),
        tedious_rs::data_types::bitn::validate(&value, &options),
        tedious_rs::data_types::moneyn::generate_type_info(&value, &options).map(|_| ()),
        tedious_rs::data_types::moneyn::generate_parameter_length(&value, &options).map(|_| ()),
        tedious_rs::data_types::moneyn::generate_parameter_data(&value, &options).map(|_| ()),
        tedious_rs::data_types::moneyn::validate(&value, &options),
        tedious_rs::data_types::datetimen::generate_type_info(&value, &options).map(|_| ()),
        tedious_rs::data_types::datetimen::generate_parameter_length(&value, &options).map(|_| ()),
        tedious_rs::data_types::datetimen::generate_parameter_data(&value, &options).map(|_| ()),
        tedious_rs::data_types::datetimen::validate(&value, &options),
    ] {
        assert_executable(operation);
    }
}

#[test]
fn bitn_parser_matches_node_nullable_and_boolean_payloads() {
    let bitn = metadata(0x68, &[1]);
    assert_eq!(
        read_value_with_options(&[0], 0, &bitn, true).unwrap().value,
        Value::Null
    );
    assert_eq!(
        read_value_with_options(&[1, 0], 0, &bitn, true)
            .unwrap()
            .value,
        Value::Boolean(false)
    );
    assert_eq!(
        read_value_with_options(&[1, 2], 0, &bitn, true)
            .unwrap()
            .value,
        Value::Boolean(true)
    );
    assert!(read_value_with_options(&[2, 1, 0], 0, &bitn, true).is_err());
    assert!(read_value_with_options(&[1], 0, &bitn, true).is_err());
}

#[test]
fn moneyn_parser_matches_node_nullable_small_and_full_payloads() {
    let moneyn = metadata(0x6E, &[8]);
    assert_eq!(
        read_value_with_options(&[0], 0, &moneyn, true)
            .unwrap()
            .value,
        Value::Null
    );
    assert_eq!(
        read_value_with_options(&[4, 0x20, 0x4E, 0, 0], 0, &moneyn, true)
            .unwrap()
            .value,
        Value::Decimal {
            unscaled: 20_000,
            scale: 4
        }
    );
    assert_eq!(
        read_value_with_options(&[8, 0x20, 0x4B, 0xBC, 0, 0, 0, 0, 0, 0], 0, &moneyn, true)
            .unwrap()
            .value,
        Value::Decimal {
            unscaled: 12_340_000,
            scale: 4
        }
    );
    assert!(read_value_with_options(&[3, 0, 0, 0], 0, &moneyn, true).is_err());
    assert!(read_value_with_options(&[8, 0, 0], 0, &moneyn, true).is_err());
}

#[test]
fn datetimen_parser_matches_node_nullable_small_and_full_payloads() {
    let datetimen = metadata(0x6F, &[8]);
    assert_eq!(
        read_value_with_options(&[0], 0, &datetimen, true)
            .unwrap()
            .value,
        Value::Null
    );
    assert_eq!(
        read_value_with_options(&[4, 0, 0, 0, 0], 0, &datetimen, true)
            .unwrap()
            .value,
        Value::DateTime {
            date: Box::new(Value::Date {
                year: 1900,
                month: 1,
                day: 1
            }),
            time: Box::new(Value::Time {
                hour: 0,
                minute: 0,
                second: 0,
                nanosecond: 0
            })
        }
    );
    assert!(read_value_with_options(&[5, 0, 0, 0, 0, 0], 0, &datetimen, true).is_err());
    assert!(read_value_with_options(&[8, 0, 0, 0], 0, &datetimen, true).is_err());
}
