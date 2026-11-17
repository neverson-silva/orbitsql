use std::sync::{Arc, Mutex};
use tedious_rs::bulk_load::{BulkLoad, BulkLoadOptions};
use tedious_rs::connection::{Connection, ConnectionConfig};
use tedious_rs::payloads::BulkLoadPayload;

#[tokio::test]
async fn bulk_load_rejects_read_only_before_request_lifecycle_starts() {
    let mut config = ConnectionConfig::sql("localhost", "sa", "password");
    config.read_only = true;
    let mut connection = Connection::new(config);
    let payload = BulkLoadPayload {
        columns: Vec::new(),
        rows: Vec::new(),
        tds_version: "7_4".to_owned(),
    };
    let error = connection.exec_bulk_load(&payload).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EREADONLY"));
    assert!(!connection.cancel());
}

#[tokio::test]
async fn bulk_load_rejects_without_logged_in_transport() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "password"));
    let mut bulk = connection
        .new_bulk_load("dbo.items", BulkLoadOptions::default())
        .unwrap();
    bulk.add_typed_column(
        "id",
        tedious_rs::data_type::type_by_name("Int").unwrap(),
        Default::default(),
        false,
    )
    .unwrap();
    let error = connection
        .exec_bulk_load_object(&mut bulk)
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    assert!(!connection.cancel());
}

#[tokio::test]
async fn bulk_load_completion_callback_is_called_once_after_preflight_failure() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let callback_calls = Arc::clone(&calls);
    let mut bulk =
        BulkLoad::new_with_callback("dbo.items", BulkLoadOptions::default(), move |result| {
            callback_calls.lock().unwrap().push(result);
        })
        .unwrap();
    bulk.add_typed_column(
        "id",
        tedious_rs::data_type::type_by_name("Int").unwrap(),
        Default::default(),
        false,
    )
    .unwrap();
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "password"));

    let error = connection
        .exec_bulk_load_values(
            &mut bulk,
            [vec![tedious_rs::value_codec::Value::Integer(1)]],
        )
        .await
        .unwrap_err();

    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert!(!connection.cancel());
}

#[tokio::test]
async fn canceled_materialized_bulk_load_does_not_start_request_lifecycle() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "password"));
    let mut bulk = connection
        .new_bulk_load("dbo.items", BulkLoadOptions::default())
        .unwrap();
    bulk.cancel();

    let error = connection
        .exec_bulk_load_values(
            &mut bulk,
            [vec![tedious_rs::value_codec::Value::Integer(1)]],
        )
        .await
        .unwrap_err();

    assert_eq!(error.code.as_deref(), Some("ECANCEL"));
    assert!(!connection.cancel());
}
