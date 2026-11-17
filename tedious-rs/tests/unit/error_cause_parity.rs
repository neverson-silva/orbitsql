use std::error::Error;

use tedious_rs::connector::ConnectorError;
use tedious_rs::{CodecError, ErrorCause, NotEnoughDataError, UnexpectedTokenError};

fn cause_of(error: &(dyn Error + 'static)) -> ErrorCause {
    ErrorCause::from_error(error)
}

#[test]
fn maps_unexpected_token_error_category_and_message() {
    let error = UnexpectedTokenError {
        handler: "RequestTokenHandler",
        token: "DONE",
        category: "unexpected_token",
        message: None,
        close_connection: false,
    };

    let cause = cause_of(&error);

    assert_eq!(cause.kind, "UnexpectedTokenError");
    assert_eq!(
        cause.message,
        "Unexpected token `DONE` in `RequestTokenHandler`"
    );
    assert_eq!(cause.code, None);
    assert_eq!(cause.cause, None);
}

#[test]
fn maps_not_enough_data_error_category_and_message() {
    let error = NotEnoughDataError::new(7);
    let cause = cause_of(&error);

    assert_eq!(cause.kind, "NotEnoughDataError");
    assert_eq!(cause.message, "need 7 bytes");
    assert_eq!(cause.code, None);
    assert_eq!(cause.cause, None);
}

#[test]
fn maps_codec_error_category_and_message() {
    let error = CodecError("invalid number".to_owned());
    let cause = cause_of(&error);

    assert_eq!(cause.kind, "CodecError");
    assert_eq!(cause.message, "invalid number");
    assert_eq!(cause.code, None);
    assert_eq!(cause.cause, None);
}

#[test]
fn maps_connector_error_category_message_and_source() {
    let error = ConnectorError::Resolve(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "resolver failed",
    ));
    let cause = cause_of(&error);

    assert_eq!(cause.kind, "ConnectorError");
    assert_eq!(cause.message, "resolver failed");
    assert_eq!(cause.code, None);
    assert_eq!(
        cause.cause.as_ref().map(|value| value.kind.as_str()),
        Some("IoError")
    );
    assert_eq!(
        cause.cause.as_ref().map(|value| value.message.as_str()),
        Some("resolver failed")
    );
}
