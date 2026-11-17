use std::error::Error;
use std::fmt::{Display, Formatter};

use tedious_rs::errors::{ConnectionError, ErrorCause, InputError, RequestError};
use tedious_rs::ntlm::NtlmError;
use tedious_rs::transport::TransportError;

#[derive(Debug)]
struct RootError;

impl Display for RootError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("root failure")
    }
}

impl Error for RootError {}

#[test]
fn connection_error_preserves_code_transient_and_cause() {
    let error =
        ConnectionError::with_cause("connection failed", Some("ESOCKET".to_owned()), RootError);

    assert_eq!(error.message, "connection failed");
    assert_eq!(error.code.as_deref(), Some("ESOCKET"));
    assert_eq!(error.is_transient, None);
    assert_eq!(
        error.cause().map(|cause| cause.kind.as_str()),
        Some("Error")
    );
    assert_eq!(
        error.cause().map(|cause| cause.message.as_str()),
        Some("root failure")
    );
    assert_eq!(
        error.source().map(|cause| cause.to_string()),
        Some("root failure".to_owned())
    );
}

#[test]
fn request_error_preserves_sql_server_fields_and_cause_chain() {
    let nested = ErrorCause::typed("TypeError", "invalid parameter")
        .with_code("ERR_INVALID")
        .with_cause(ErrorCause::new("value is outside the accepted range"));
    let mut error = RequestError::with_cause_value(
        "SQL Server rejected the request",
        Some("EREQUEST".to_owned()),
        nested,
    );
    error.number = Some(245);
    error.state = Some(1);
    error.class = Some(16);
    error.server_name = Some("server-a".to_owned());
    error.proc_name = Some("procedure-a".to_owned());
    error.line_number = Some(27);
    error.is_transient = Some(false);

    assert_eq!(error.code.as_deref(), Some("EREQUEST"));
    assert_eq!(error.number, Some(245));
    assert_eq!(error.state, Some(1));
    assert_eq!(error.class, Some(16));
    assert_eq!(error.server_name.as_deref(), Some("server-a"));
    assert_eq!(error.proc_name.as_deref(), Some("procedure-a"));
    assert_eq!(error.line_number, Some(27));
    assert_eq!(error.is_transient, Some(false));
    assert_eq!(
        error.cause().map(|cause| cause.kind.as_str()),
        Some("TypeError")
    );
    assert_eq!(
        error.cause().and_then(|cause| cause.code.as_deref()),
        Some("ERR_INVALID")
    );
    assert_eq!(
        error
            .cause()
            .and_then(|cause| cause.cause.as_deref())
            .map(|cause| cause.message.as_str()),
        Some("value is outside the accepted range")
    );
    assert_eq!(
        error.source().map(|cause| cause.to_string()),
        Some("invalid parameter".to_owned())
    );
}

#[test]
fn constructors_keep_optional_fields_absent_like_node_error_defaults() {
    let connection = ConnectionError::new("connection", None);
    let request = RequestError::new("request", None);

    assert_eq!(connection.code, None);
    assert_eq!(connection.cause, None);
    assert_eq!(request.code, None);
    assert_eq!(request.number, None);
    assert_eq!(request.state, None);
    assert_eq!(request.class, None);
    assert_eq!(request.server_name, None);
    assert_eq!(request.proc_name, None);
    assert_eq!(request.line_number, None);
    assert_eq!(request.cause, None);
}

#[test]
fn input_error_retains_type_error_message_and_identity() {
    let error = InputError("invalid input".to_owned());

    assert_eq!(error.0, "invalid input");
    assert_eq!(error.to_string(), "invalid input");
}

#[test]
fn transport_error_preserves_category_message_and_io_cause() {
    let transport = TransportError::Io(std::io::Error::new(
        std::io::ErrorKind::ConnectionReset,
        "socket reset",
    ));
    let error =
        ConnectionError::with_cause("Unable to connect", Some("ESOCKET".to_owned()), transport);

    assert_eq!(
        error.cause().map(|cause| cause.kind.as_str()),
        Some("TransportError")
    );
    assert_eq!(
        error.cause().map(|cause| cause.message.as_str()),
        Some("socket reset")
    );
    assert_eq!(
        error
            .cause()
            .and_then(|cause| cause.cause.as_deref())
            .map(|cause| cause.kind.as_str()),
        Some("IoError")
    );
    assert_eq!(error.to_string(), "Unable to connect");
}

#[test]
fn ntlm_error_preserves_category_and_message() {
    let error = ConnectionError::with_cause(
        "NTLM authentication failed",
        Some("ELOGIN".to_owned()),
        NtlmError("invalid NTLM challenge signature".to_owned()),
    );

    assert_eq!(
        error.cause().map(|cause| cause.kind.as_str()),
        Some("NtlmError")
    );
    assert_eq!(
        error.cause().map(|cause| cause.message.as_str()),
        Some("invalid NTLM challenge signature")
    );
    assert_eq!(
        error.source().map(ToString::to_string),
        Some("invalid NTLM challenge signature".to_owned())
    );
}
