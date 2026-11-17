use tedious_rs::Request;
use tedious_rs::connection::{Connection, ConnectionConfig, ConnectionState};
use tedious_rs::payloads::RpcRequestPayload;

fn rpc_payload() -> RpcRequestPayload {
    RpcRequestPayload {
        procedure: Some("dbo.write_item".to_owned()),
        procedure_id: None,
        parameters: Vec::new(),
        transaction_descriptor: vec![0; 8],
        tds_version: "7_4".to_owned(),
    }
}

#[tokio::test]
async fn read_only_exec_rpc_tokens_rejects_before_transport_state_changes() {
    let mut config = ConnectionConfig::sql("localhost", "sa", "password");
    config.read_only = true;
    let mut connection = Connection::new(config);
    connection.state = ConnectionState::LoggedIn;

    let error = connection
        .exec_rpc_tokens(&rpc_payload())
        .await
        .unwrap_err();

    assert_eq!(error.code.as_deref(), Some("EREADONLY"));
    assert_eq!(connection.state, ConnectionState::LoggedIn);
}

#[tokio::test]
async fn read_only_unprepare_rejects_before_handle_or_transport_validation() {
    let mut config = ConnectionConfig::sql("localhost", "sa", "password");
    config.read_only = true;
    let mut connection = Connection::new(config);
    connection.state = ConnectionState::LoggedIn;
    let mut request = Request::new("SELECT 1", |_| {});
    request.handle = Some(7);

    let error = connection.unprepare(&mut request).await.unwrap_err();

    assert_eq!(error.code.as_deref(), Some("EREADONLY"));
    assert_eq!(request.handle, Some(7));
    assert_eq!(connection.state, ConnectionState::LoggedIn);
}

#[tokio::test]
async fn mutable_sql_auth_does_not_receive_read_only_rpc_errors() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "password"));
    connection.state = ConnectionState::LoggedIn;
    let rpc_error = connection
        .exec_rpc_tokens(&rpc_payload())
        .await
        .unwrap_err();
    assert_ne!(rpc_error.code.as_deref(), Some("EREADONLY"));

    let mut request = Request::new("SELECT 1", |_| {});
    request.handle = Some(7);
    let unprepare_error = connection.unprepare(&mut request).await.unwrap_err();
    assert_ne!(unprepare_error.code.as_deref(), Some("EREADONLY"));
}
