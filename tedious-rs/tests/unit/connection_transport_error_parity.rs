use std::time::Duration;

use tedious_rs::connection::{Connection, ConnectionConfig};

#[tokio::test]
async fn transport_failure_is_an_esocket_connection_error_with_transport_cause() {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        drop(socket);
    });

    let mut config = ConnectionConfig::sql("127.0.0.1", "user", "password");
    config.port = address.port();
    config.encrypt = false;
    config.connect_timeout = Duration::from_secs(2);
    let mut connection = Connection::new(config);

    let error = connection.connect().await.unwrap_err();

    server.await.unwrap();
    assert_eq!(error.code.as_deref(), Some("ESOCKET"));
    assert_eq!(
        error.cause().map(|cause| cause.kind.as_str()),
        Some("TransportError")
    );
    assert!(error.cause().is_some_and(|cause| !cause.message.is_empty()));
    assert_eq!(
        error
            .cause()
            .and_then(|cause| cause.cause.as_deref())
            .map(|cause| cause.kind.as_str()),
        Some("IoError")
    );
}
