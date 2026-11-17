use tedious_rs::data_type::type_by_name;
use tedious_rs::parameter::ParameterOptions;
use tedious_rs::request::Request;
use tedious_rs::value_codec::Value;

#[test]
fn make_params_declaration_prefixes_parameter_names_without_normalizing_them() {
    let mut request = Request::new("SELECT @plain, @@already", |_| {});
    request
        .add_parameter(
            "plain",
            type_by_name("Int").unwrap(),
            Value::Integer(1),
            ParameterOptions::default(),
        )
        .unwrap();
    request
        .add_output_parameter(
            "@already",
            type_by_name("NVarChar").unwrap(),
            Value::Text("value".to_owned()),
            ParameterOptions {
                length: Some(10),
                ..ParameterOptions::default()
            },
        )
        .unwrap();

    assert_eq!(
        request.make_params_declaration().unwrap(),
        "@plain int, @@already nvarchar(10) OUTPUT"
    );
}
