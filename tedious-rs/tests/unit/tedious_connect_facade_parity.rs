use std::future::Future;
use std::pin::Pin;

use tedious_rs::{Connection, ConnectionConfig, ConnectionState, connect};

#[test]
fn connect_facade_exposes_the_node_connection_symbols() {
    let config = ConnectionConfig::sql("localhost", "user", "password");
    let connection = Connection::new(config.clone());

    assert_eq!(connection.state, ConnectionState::Initialized);
    assert_eq!(connection.config, config);
    drop(Box::pin(connect(config))
        as Pin<
            Box<dyn Future<Output = Result<Connection, tedious_rs::ConnectionError>>>,
        >);
}

#[test]
fn connect_facade_documents_async_return_divergence() {
    let config = ConnectionConfig::sql("localhost", "user", "password");
    let future = connect(config);

    drop(Box::pin(future)
        as Pin<
            Box<dyn Future<Output = Result<Connection, tedious_rs::ConnectionError>>>,
        >);
}
