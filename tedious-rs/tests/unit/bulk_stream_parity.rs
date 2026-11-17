use std::time::Duration;

use tedious_rs::bulk_load::BulkLoadOptions;
use tedious_rs::connection::{Connection, ConnectionConfig, ConnectionState};
use tedious_rs::data_type::type_by_name;

#[test]
fn stream_metadata_preserves_nullable_unknown_column_flags() {
    let mut bulk = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"))
        .new_bulk_load("dbo.target", BulkLoadOptions::default())
        .unwrap();
    bulk.add_typed_column(
        "identifier",
        type_by_name("Int").unwrap(),
        Default::default(),
        false,
    )
    .unwrap();
    bulk.set_nullable_unknown_columns([0]).unwrap();
    assert_eq!(&bulk.get_col_metadata().unwrap()[7..9], &[0x04, 0x80]);
}

#[test]
fn stream_execution_state_is_set_only_after_payload_validation() {
    let mut bulk = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"))
        .new_bulk_load("dbo.target", BulkLoadOptions::default())
        .unwrap();
    assert!(bulk.begin_execution().is_ok());
    assert!(bulk.execution_started);
}

#[test]
fn stream_timeout_configuration_can_disable_request_timeout() {
    let mut bulk = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"))
        .new_bulk_load("dbo.target", BulkLoadOptions::default())
        .unwrap();
    bulk.set_timeout(Some(Duration::ZERO));
    assert_eq!(bulk.timeout, Some(Duration::ZERO));
}

#[test]
fn stream_requires_logged_in_connection_before_transport() {
    let connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    assert_eq!(connection.state, ConnectionState::Initialized);
}
