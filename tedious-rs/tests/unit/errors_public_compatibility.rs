use std::error::Error;

use tedious_rs::{NotEnoughDataError, RequestError};

#[test]
fn request_error_exposes_node_error_field_names_without_removing_rust_names() {
    let mut error = RequestError::new("request failed", Some("EREQUEST".to_owned()));

    error.serverName = Some("server-a".to_owned());
    error.procName = Some("procedure-a".to_owned());
    error.lineNumber = Some(27);

    assert_eq!(error.serverName.as_deref(), Some("server-a"));
    assert_eq!(error.procName.as_deref(), Some("procedure-a"));
    assert_eq!(error.lineNumber, Some(27));

    error.server_name = Some("server-b".to_owned());
    error.proc_name = Some("procedure-b".to_owned());
    error.line_number = Some(28);

    assert_eq!(error.server_name.as_deref(), Some("server-b"));
    assert_eq!(error.proc_name.as_deref(), Some("procedure-b"));
    assert_eq!(error.line_number, Some(28));
}

#[test]
fn aggregate_request_error_preserves_node_field_names() {
    let mut first = RequestError::new("request failed", Some("EREQUEST".to_owned()));
    first.serverName = Some("server-a".to_owned());
    first.procName = Some("procedure-a".to_owned());
    first.lineNumber = Some(27);

    let aggregate = RequestError::aggregate(vec![first]);

    assert_eq!(aggregate.serverName, None);
    assert_eq!(aggregate.procName, None);
    assert_eq!(aggregate.lineNumber, None);
}

#[test]
fn not_enough_data_error_has_only_required_byte_count_behavior() {
    let error = NotEnoughDataError::new(42);

    assert_eq!(error.byte_count, 42);
    assert_eq!(error.to_string(), "need 42 bytes");
    assert!(Error::source(&error).is_none());
}
