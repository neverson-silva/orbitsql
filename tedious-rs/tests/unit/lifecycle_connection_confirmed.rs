use std::future::pending;
use std::time::Duration;

use tedious_rs::connection::{Connection, ConnectionConfig, ConnectionState, RoutingData};

#[tokio::test]
async fn connection_timeout_reports_rerouted_destination_and_origin() {
    let mut config = ConnectionConfig::sql("origin.example", "user", "");
    config.port = 1433;
    config.connect_timeout = Duration::from_millis(20);
    config.max_retries_on_transient_errors = 0;
    let mut connection = Connection::new(config);
    connection.routing = Some(RoutingData {
        protocol: 0,
        server: "target.example".to_owned(),
        port: 1444,
        instance: None,
        old_value: Vec::new(),
    });
    connection.set_custom_connector(|| async { pending().await });

    let error = connection.connect().await.unwrap_err();

    assert_eq!(error.code.as_deref(), Some("ETIMEOUT"));
    assert_eq!(
        error.message,
        "Failed to connect to target.example:1444 (redirected from origin.example:1433) in 20ms"
    );
    assert_eq!(connection.state, ConnectionState::Final);
}

#[tokio::test]
async fn close_signals_close_handle_after_terminal_state() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "user", ""));
    let handle = connection.close_handle();
    connection.transition_to(ConnectionState::Final);

    connection.close().await;

    assert!(handle.is_closed());
    assert_eq!(connection.state, ConnectionState::Final);
}
