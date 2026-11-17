use tedious_rs::collation::Collation;
use tedious_rs::errors::RequestError;
use tedious_rs::parameter::ParameterOptions;
use tedious_rs::request::Request;
use tedious_rs::token::handler::{RequestTokenHandler, TokenHandler};
use tedious_rs::token::parser::MessageToken;
use tedious_rs::value_codec::Value;

fn message(number: u32, text: &str) -> MessageToken {
    MessageToken {
        token: 0xAA,
        number,
        state: 1,
        class: 16,
        message: text.to_owned(),
        server_name: "server".to_owned(),
        proc_name: "procedure".to_owned(),
        line_number: 3,
    }
}

#[test]
fn aggregate_request_error_keeps_each_sql_error_in_wire_order() {
    let mut handler = RequestTokenHandler::new(false, false);

    handler.on_error(&message(208, "missing object")).unwrap();
    handler.on_error(&message(2627, "duplicate key")).unwrap();

    let aggregate = handler.error.as_ref().unwrap();
    assert_eq!(aggregate.errors.len(), 2);
    assert_eq!(aggregate.errors[0].number, Some(208));
    assert_eq!(aggregate.errors[1].number, Some(2627));
    assert_eq!(aggregate.errors, handler.errors);
}

#[test]
fn validation_error_preserves_codec_error_as_cause() {
    let mut request = Request::new("SELECT @value", |_| {});
    request
        .add_parameter(
            "value",
            tedious_rs::data_type::type_by_name("VarChar").unwrap(),
            Value::Integer(7),
            ParameterOptions::default(),
        )
        .unwrap();

    let error = request
        .validate_parameters(Some(&Collation::new(0x0409, 0, 0, 0)))
        .unwrap_err();

    assert_eq!(error.code.as_deref(), Some("EPARAM"));
    assert!(
        error
            .message
            .starts_with("Validation failed for parameter 'value'.")
    );
    assert_eq!(
        error.cause().map(|cause| cause.kind.as_str()),
        Some("CodecError")
    );
    assert_eq!(
        error.cause().map(|cause| cause.message.as_str()),
        Some("invalid varchar value")
    );
}

#[test]
fn aggregate_error_constructor_does_not_change_member_errors() {
    let first = RequestError::new("first", Some("EREQUEST".to_owned()));
    let second = RequestError::new("second", Some("EREQUEST".to_owned()));
    let aggregate = RequestError::aggregate(vec![first.clone(), second.clone()]);

    assert_eq!(aggregate.message, "");
    assert_eq!(aggregate.code, None);
    assert_eq!(aggregate.errors, vec![first, second]);
}

#[test]
fn request_sql_error_does_not_expose_login_retry_state() {
    let mut handler = RequestTokenHandler::new(false, false);
    handler.on_error(&message(4060, "request error")).unwrap();
    assert_eq!(handler.error.unwrap().is_transient, None);
}
