use tedious_rs::data_type::type_by_name;
use tedious_rs::parameter::ParameterOptions;
use tedious_rs::request::{Request, RequestResult};
use tedious_rs::value_codec::Value;

#[test]
fn force_encrypt_is_stored_per_parameter_without_changing_parameter_options() {
    let mut request = Request::new(
        "select @plain, @encrypted",
        |_result: Result<RequestResult, tedious_rs::errors::RequestError>| {},
    );
    request
        .add_parameter(
            "plain",
            type_by_name("Int").unwrap(),
            Value::Integer(1),
            ParameterOptions::default(),
        )
        .unwrap();
    request
        .add_parameter_with_force_encrypt(
            "encrypted",
            type_by_name("Int").unwrap(),
            Value::Integer(2),
            ParameterOptions::default(),
            true,
        )
        .unwrap();
    assert_eq!(request.force_encrypt_parameters(), &[false, true]);
}

#[test]
fn force_encrypt_can_be_changed_after_parameter_creation() {
    let mut request = Request::new("select @value", |_| {});
    request
        .add_parameter(
            "value",
            type_by_name("Int").unwrap(),
            Value::Integer(1),
            ParameterOptions::default(),
        )
        .unwrap();
    request.set_parameter_force_encrypt("value", true).unwrap();
    assert_eq!(request.force_encrypt_parameters(), &[true]);
}
