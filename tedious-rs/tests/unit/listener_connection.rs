use std::sync::{Arc, Mutex};

use tedious_rs::connection::{Connection, ConnectionConfig, ConnectionEvent};

#[test]
fn connection_listener_ids_support_count_and_removal() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "user", "password"));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let first_seen = Arc::clone(&seen);
    let first = connection.on(move |_| first_seen.lock().unwrap().push(1));
    let second_seen = Arc::clone(&seen);
    let second = connection.on(move |_| second_seen.lock().unwrap().push(2));

    assert_eq!(connection.listener_count(), 2);
    assert_eq!(connection.listeners(), vec![first, second]);
    assert!(connection.off(first));
    connection.emit(ConnectionEvent::Secure);

    assert_eq!(*seen.lock().unwrap(), vec![2]);
}

#[test]
fn connection_once_listener_is_removed_before_callback() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "user", "password"));
    let seen = Arc::new(Mutex::new(0));
    let callback_seen = Arc::clone(&seen);
    let id = connection.once(move |_| *callback_seen.lock().unwrap() += 1);

    connection.emit(ConnectionEvent::Connect);
    connection.emit(ConnectionEvent::End);

    assert_eq!(*seen.lock().unwrap(), 1);
    assert_eq!(connection.listener_count(), 0);
    assert!(!connection.remove_listener(id));
}

#[test]
fn connection_callbacks_run_after_listener_lock_is_released() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "user", "password"));
    let entered = Arc::new(Mutex::new(false));
    let callback_entered = Arc::clone(&entered);
    connection.on(move |_| *callback_entered.lock().unwrap() = true);

    connection.emit(ConnectionEvent::Secure);

    assert!(*entered.lock().unwrap());
}
