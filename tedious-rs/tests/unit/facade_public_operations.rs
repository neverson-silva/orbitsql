use tedious_rs::{BulkLoadOptions, ConnectionConfig, Request, connect};

#[tokio::test]
async fn facade_exposes_execution_and_bulk_operations_after_sync_connect() {
    let mut config = ConnectionConfig::sql("localhost", "user", "password");
    config.port = 1;
    let mut connection = connect(config);

    let bulk_load = connection.new_bulk_load("facade_table", BulkLoadOptions::default());
    assert!(bulk_load.is_ok());
    assert!(!connection.cancel());

    let error = connection.exec_sql_batch("SELECT 1").await.unwrap_err();
    assert!(error.code.is_some());

    let mut request = Request::new("SELECT 1", |_| {});
    let error = connection.exec_sql(&mut request).await.unwrap_err();
    assert!(error.code.is_some());
}
