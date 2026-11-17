use tedious_rs::parameter::{ParameterOptions, SqlParameterType};
use tedious_rs::request::Request;
use tedious_rs::value_codec::{EncodedParameter, Value};

#[test]
fn duplicate_parameters_preserve_position_type_value_and_declaration() {
    let mut request = Request::new("SELECT @value", |_| {});
    request
        .add_typed_parameter(
            "value",
            SqlParameterType::Int,
            Value::Integer(7),
            ParameterOptions::default(),
        )
        .unwrap();
    request.add_raw_parameter(
        "value",
        EncodedParameter {
            type_info: vec![0x26, 4],
            length: vec![4],
            data: 9_i32.to_le_bytes().to_vec(),
        },
    );
    request
        .add_typed_output_parameter(
            "value",
            SqlParameterType::NVarChar,
            Value::Text("ok".to_owned()),
            ParameterOptions {
                length: Some(2),
                ..ParameterOptions::default()
            },
        )
        .unwrap();

    request.validate_parameters(None).unwrap();

    assert_eq!(request.parameters[0].encoded.data, 7_i32.to_le_bytes());
    assert_eq!(request.parameters[1].encoded.data, 9_i32.to_le_bytes());
    assert_eq!(request.parameters[2].encoded.data, vec![b'o', 0, b'k', 0]);
    assert_eq!(
        request.make_params_declaration().unwrap(),
        "@value int, @value int, @value nvarchar(2) OUTPUT"
    );
}

#[test]
fn duplicate_parameters_validate_against_their_own_types() {
    let mut request = Request::new("SELECT @value", |_| {});
    request
        .add_typed_parameter(
            "value",
            SqlParameterType::NVarChar,
            Value::Text("text".to_owned()),
            ParameterOptions {
                length: Some(4),
                ..ParameterOptions::default()
            },
        )
        .unwrap();
    request
        .add_typed_parameter(
            "value",
            SqlParameterType::Int,
            Value::Integer(42),
            ParameterOptions::default(),
        )
        .unwrap();

    request.validate_parameters(None).unwrap();

    assert_eq!(
        request.parameters[0].encoded.data,
        vec![b't', 0, b'e', 0, b'x', 0, b't', 0]
    );
    assert_eq!(request.parameters[1].encoded.data, 42_i32.to_le_bytes());
    assert_eq!(
        request.make_params_declaration().unwrap(),
        "@value nvarchar(4), @value int"
    );
}

#[test]
fn updating_duplicate_parameters_reencodes_every_typed_occurrence() {
    let mut request = Request::new("SELECT @value", |_| {});
    request
        .add_typed_parameter(
            "value",
            SqlParameterType::Int,
            Value::Integer(7),
            ParameterOptions::default(),
        )
        .unwrap();
    request.add_raw_parameter(
        "value",
        EncodedParameter {
            type_info: vec![0x26, 4],
            length: vec![4],
            data: 9_i32.to_le_bytes().to_vec(),
        },
    );
    request
        .add_typed_parameter(
            "value",
            SqlParameterType::Int,
            Value::Integer(11),
            ParameterOptions::default(),
        )
        .unwrap();

    request
        .set_typed_parameter_value("value", Value::Integer(42))
        .unwrap();

    assert_eq!(request.parameters[0].encoded.data, 42_i32.to_le_bytes());
    assert_eq!(request.parameters[1].encoded.data, 9_i32.to_le_bytes());
    assert_eq!(request.parameters[2].encoded.data, 42_i32.to_le_bytes());
}

#[test]
fn force_encrypt_on_duplicate_parameter_targets_the_new_occurrence() {
    let mut request = Request::new("SELECT @value", |_| {});
    request
        .add_parameter_with_force_encrypt(
            "value",
            tedious_rs::data_type::type_by_name("Int").unwrap(),
            Value::Integer(7),
            ParameterOptions::default(),
            false,
        )
        .unwrap();
    request.add_raw_parameter(
        "value",
        EncodedParameter {
            type_info: vec![0x26, 4],
            length: vec![4],
            data: 9_i32.to_le_bytes().to_vec(),
        },
    );
    request
        .add_parameter_with_force_encrypt(
            "value",
            tedious_rs::data_type::type_by_name("Int").unwrap(),
            Value::Integer(11),
            ParameterOptions::default(),
            true,
        )
        .unwrap();

    assert_eq!(request.force_encrypt_parameters(), &[false, false, true]);
}
