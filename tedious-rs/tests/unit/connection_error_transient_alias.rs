use tedious_rs::ConnectionError;

#[test]
fn connection_error_exposes_is_transient_node_alias() {
    let mut error = ConnectionError::new("connection failed", Some("ESOCKET".to_owned()));

    assert_eq!(error.is_transient, None);
    assert_eq!(error.isTransient, None);

    error.is_transient = Some(true);
    error.isTransient = error.is_transient;

    assert_eq!(error.is_transient, Some(true));
    assert_eq!(error.isTransient, Some(true));
}
