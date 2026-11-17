use std::sync::{Arc, Mutex};
use std::time::Duration;

use tedious_rs::{
    ConnectionConfig, ConnectionEvent, ConnectionState, connect, connect_with_callback,
};

#[test]
fn connect_returns_facade_before_connection_attempt_finishes() {
    let mut config = ConnectionConfig::sql("localhost", "user", "password");
    config.port = 1;
    let connection = connect(config.clone());

    assert_eq!(connection.config(), config);
    assert!(matches!(
        connection.state(),
        ConnectionState::Initialized | ConnectionState::Connecting | ConnectionState::Final
    ));
}

#[test]
fn callback_receives_connection_error_once() {
    let mut config = ConnectionConfig::sql("localhost", "user", "password");
    config.port = 1;
    let observed = Arc::new(Mutex::new(Vec::new()));
    let callback_observed = Arc::clone(&observed);
    let connection = connect_with_callback(config, move |error| {
        callback_observed
            .lock()
            .unwrap()
            .push(error.map(|value| value.code));
    });

    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(observed.lock().unwrap().len(), 1);
    drop(connection);
}

#[test]
fn close_is_synchronous_and_end_is_emitted_after_close() {
    let mut config = ConnectionConfig::sql("localhost", "user", "password");
    config.port = 1;
    let order = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&order);
    let mut connection = connect(config);
    connection.on_named("end", move |_| observed.lock().unwrap().push("end"));

    connection.close();
    assert!(connection.is_closed());
    assert_eq!(connection.state(), ConnectionState::Final);
    assert_eq!(*order.lock().unwrap(), vec!["end"]);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(*order.lock().unwrap(), vec!["end"]);
}

#[test]
fn wildcard_facade_listener_observes_connect_error_and_end() {
    let mut config = ConnectionConfig::sql("localhost", "user", "password");
    config.port = 1;
    let observed = Arc::new(Mutex::new(Vec::new()));
    let listener_observed = Arc::clone(&observed);
    let mut connection = connect(config);
    connection.on(move |event| {
        let name = match event {
            ConnectionEvent::ConnectError(_) => "connect",
            ConnectionEvent::Error(_) => "error",
            ConnectionEvent::End => "end",
            _ => return,
        };
        listener_observed.lock().unwrap().push(name);
    });

    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(*observed.lock().unwrap(), vec!["connect", "end"]);
}

#[test]
fn listener_registered_immediately_after_connect_is_not_lost() {
    for _ in 0..20 {
        let mut config = ConnectionConfig::sql("localhost", "user", "password");
        config.port = 1;
        let observed = Arc::new(Mutex::new(Vec::new()));
        let listener_observed = Arc::clone(&observed);
        let mut connection = connect(config);
        connection.on_named("connect", move |_| {
            listener_observed.lock().unwrap().push("connect");
        });
        for _ in 0..500 {
            if observed.lock().unwrap().len() == 1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(observed.lock().unwrap().as_slice(), ["connect"]);
    }
}
