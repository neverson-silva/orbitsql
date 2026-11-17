use std::error::Error;
use std::fmt::{Display, Formatter};

use tedious_rs::public_api::{
    ConnectorError, ErrorCause, InputError, NodeConnectionError, NodeInputError,
    NodeNotEnoughDataError, NodeRequestError, NodeUnexpectedTokenError, NotEnoughData,
    NotEnoughDataError, NtlmError, RequestError, TransportError, UnexpectedTokenError,
};
use tedious_rs::{CodecError, ConnectionError};

#[derive(Debug)]
struct CauseError;

impl Display for CauseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("transport cause")
    }
}

impl Error for CauseError {}

#[test]
fn root_and_public_facade_export_the_error_contract() {
    let _: NodeConnectionError = ConnectionError::new("connection", Some("ECONNRESET".to_owned()));
    let _: NodeRequestError = RequestError::new("request", Some("EREQUEST".to_owned()));
    let _: NodeInputError = InputError("input".to_owned());
    let _: NodeUnexpectedTokenError = UnexpectedTokenError {
        handler: "handler",
        token: "token",
        category: "category",
        message: None,
        close_connection: false,
    };
    let _: NodeNotEnoughDataError = NotEnoughDataError::new(4);
    let _: NotEnoughData = NotEnoughDataError::new(5);
    let _: Option<ConnectorError> = None;
    let _: Option<TransportError> = None;
    let _: Option<NtlmError> = None;
    let _: Option<CodecError> = None;
}

#[test]
fn public_errors_preserve_codes_sql_fields_and_cause_chain() {
    let cause = ErrorCause::typed("TypeError", "bad value")
        .with_code("EINVAL")
        .with_cause(ErrorCause::new("out of range"));
    let mut error = RequestError::with_cause_value(
        "server rejected request",
        Some("EREQUEST".to_owned()),
        cause,
    );
    error.number = Some(245);
    error.state = Some(1);
    error.class = Some(16);
    error.server_name = Some("server".to_owned());
    error.proc_name = Some("proc".to_owned());
    error.line_number = Some(9);

    assert_eq!(error.code.as_deref(), Some("EREQUEST"));
    assert_eq!(error.number, Some(245));
    assert_eq!(error.state, Some(1));
    assert_eq!(error.class, Some(16));
    assert_eq!(error.server_name.as_deref(), Some("server"));
    assert_eq!(error.proc_name.as_deref(), Some("proc"));
    assert_eq!(error.line_number, Some(9));
    assert_eq!(
        error.cause().map(|value| value.kind.as_str()),
        Some("TypeError")
    );
    assert_eq!(
        error.cause().and_then(|value| value.code.as_deref()),
        Some("EINVAL")
    );
    assert_eq!(
        error
            .cause()
            .and_then(|value| value.cause.as_deref())
            .map(|value| value.message.as_str()),
        Some("out of range")
    );
}

#[test]
fn connection_cause_and_transport_categories_remain_observable() {
    let error =
        ConnectionError::with_cause("connection failed", Some("ESOCKET".to_owned()), CauseError);

    assert_eq!(error.code.as_deref(), Some("ESOCKET"));
    assert_eq!(
        error.cause().map(|value| value.message.as_str()),
        Some("transport cause")
    );
    assert!(matches!(TransportError::Timeout, TransportError::Timeout));
    assert!(matches!(
        ConnectorError::Cancelled,
        ConnectorError::Cancelled
    ));
}
