use tedious_rs::connection::{Connection, ConnectionConfig};

#[tokio::test]
async fn read_only_reset_is_rejected_before_transport() {
    let mut config = ConnectionConfig::sql("localhost", "sa", "password");
    config.read_only = true;
    let mut connection = Connection::new(config);
    let error = connection.reset().await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EREADONLY"));
}
