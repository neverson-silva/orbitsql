use tedious_rs::data_types::tvp;
use tedious_rs::parameter::{ParameterOptions, SqlParameterType};
use tedious_rs::payloads::{TvpColumnDefinition, TvpPayload, TvpTypedColumnDefinition};
use tedious_rs::value_codec::Value;

fn payload() -> TvpPayload {
    let columns = [TvpTypedColumnDefinition {
        name: "id".to_owned(),
        parameter_type: SqlParameterType::Int,
        options: ParameterOptions::default(),
    }];
    TvpPayload::from_typed_values(
        "ignored",
        "dbo",
        "items",
        &columns,
        &[vec![Value::Integer(7)]],
        None,
    )
    .unwrap()
}

#[test]
fn declaration_and_type_info_match_node_tvp() {
    let value = Value::Tvp(Box::new(payload()));
    assert_eq!(
        tvp::declaration(&value, &ParameterOptions::default(), false).unwrap(),
        "dbo.items readonly"
    );
    assert_eq!(
        tvp::generate_type_info(&value, &ParameterOptions::default()).unwrap(),
        vec![
            0xF3, 0, 3, b'd', 0, b'b', 0, b'o', 0, 5, b'i', 0, b't', 0, b'e', 0, b'm', 0, b's', 0
        ]
    );
}

#[test]
fn null_length_and_data_match_node_tvp() {
    let value = Value::Null;
    assert_eq!(
        tvp::generate_type_info(&value, &ParameterOptions::default()).unwrap(),
        vec![0xF3, 0, 0, 0]
    );
    assert_eq!(
        tvp::generate_parameter_length(&value, &ParameterOptions::default()).unwrap(),
        vec![0xFF, 0xFF]
    );
    assert_eq!(
        tvp::generate_parameter_data(&value, &ParameterOptions::default()).unwrap(),
        vec![0, 0]
    );
    assert!(tvp::validate(&value, &ParameterOptions::default()).is_ok());
}

#[test]
fn column_metadata_row_and_final_token_match_node_tvp() {
    let value = Value::Tvp(Box::new(payload()));
    assert_eq!(
        tvp::generate_parameter_length(&value, &ParameterOptions::default()).unwrap(),
        vec![1, 0]
    );
    assert_eq!(
        tvp::generate_parameter_data(&value, &ParameterOptions::default()).unwrap(),
        vec![0, 0, 0, 0, 0, 0, 0x26, 4, 0, 0, 1, 4, 7, 0, 0, 0, 0]
    );
}

#[test]
fn row_validation_wraps_column_errors_and_preserves_public_errors() {
    let columns = [TvpTypedColumnDefinition {
        name: "id".to_owned(),
        parameter_type: SqlParameterType::Int,
        options: ParameterOptions::default(),
    }];
    let error = TvpPayload::from_typed_values(
        "",
        "dbo",
        "items",
        &columns,
        &[vec![Value::Text("invalid".to_owned())]],
        None,
    )
    .unwrap_err();
    assert!(error.contains("TVP column 'id' has invalid data at row index 0"));

    let malformed = Value::Tvp(Box::new(TvpPayload {
        database: String::new(),
        schema: "dbo".to_owned(),
        type_name: "items".to_owned(),
        columns: vec![TvpColumnDefinition {
            name: "id".to_owned(),
            type_info: vec![0x26, 4],
        }],
        rows: vec![tedious_rs::payloads::TvpRow { values: Vec::new() }],
    }));
    assert!(tvp::validate(&malformed, &ParameterOptions::default()).is_ok());
    assert!(tvp::generate_parameter_data(&malformed, &ParameterOptions::default()).is_ok());
}
