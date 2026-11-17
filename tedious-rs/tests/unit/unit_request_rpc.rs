use tedious_rs::connection::{Connection, ConnectionConfig};
use tedious_rs::data_type::type_by_name;
use tedious_rs::parameter::ParameterOptions;
use tedious_rs::request::{Request, RequestResult};
use tedious_rs::value_codec::EncodedParameter;
use tedious_rs::value_codec::Value;

#[test]
fn keeps_rpc_parameters_in_insertion_order_and_preserves_output_status() {
    let mut request = Request::new(
        "sp_executesql",
        |_result: Result<RequestResult, tedious_rs::errors::RequestError>| {},
    );
    request.add_raw_parameter(
        "statement",
        EncodedParameter {
            type_info: vec![0xE7],
            length: vec![2, 0],
            data: vec![b'X', 0],
        },
    );
    request.add_raw_output_parameter(
        "result",
        EncodedParameter {
            type_info: vec![0x26, 4],
            length: vec![4],
            data: vec![1, 0, 0, 0],
        },
    );
    assert_eq!(request.parameters.len(), 2);
    assert_eq!(request.parameters[0].name, "statement");
    assert!(!request.parameters[0].output);
    assert_eq!(request.parameters[1].name, "result");
    assert!(request.parameters[1].output);
}

#[test]
fn adds_parameters_from_public_data_types_and_serializes_output_values() {
    let mut request = Request::new("sp_executesql", |_| {});
    request
        .add_parameter(
            "id",
            type_by_name("Int").unwrap(),
            Value::Integer(7),
            ParameterOptions::default(),
        )
        .unwrap();
    request
        .add_output_parameter(
            "name",
            type_by_name("NVarChar").unwrap(),
            Value::Text("linha".to_owned()),
            ParameterOptions {
                length: Some(10),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(request.parameters.len(), 2);
    assert!(request.parameters[0].encoded.data.is_empty());
    assert!(!request.parameters[0].output);
    assert!(request.parameters[1].output);
    assert_eq!(
        request.make_params_declaration().unwrap(),
        "@id int, @name nvarchar(10) OUTPUT"
    );
}

#[test]
fn rejects_a_public_data_type_without_parameter_encoding() {
    let mut request = Request::new("SELECT 1", |_| {});
    let error = request
        .add_parameter_value(
            "value",
            type_by_name("Null").unwrap(),
            Value::Null,
            ParameterOptions::default(),
        )
        .unwrap_err();
    assert!(error.0.contains("cannot be used as a parameter"));
}

#[tokio::test]
async fn rpc_request_fails_before_serialization_when_connection_is_not_logged_in() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    let mut request = Request::new("sp_executesql", |_| {});
    let error = connection.exec_rpc_request(&mut request).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    assert_eq!(request.state, tedious_rs::request::RequestState::Failed);
}

#[test]
fn prepare_without_parameters_uses_a_null_params_value() {
    let encoded = tedious_rs::value_codec::encode_nvarchar(&Value::Null, 4000).unwrap();
    assert_eq!(encoded.length, vec![0xFF, 0xFF]);
    assert!(encoded.data.is_empty());
    assert_eq!(encoded.type_info, vec![0xE7, 0x40, 0x1F, 0, 0, 0, 0, 0]);
}
