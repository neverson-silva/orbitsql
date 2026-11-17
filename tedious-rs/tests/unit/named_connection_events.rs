use std::sync::{Arc, Mutex};

use tedious_rs::ConnectionEventName;
use tedious_rs::EventEmitterError;
use tedious_rs::connection::{Connection, ConnectionConfig, ConnectionEvent};

#[test]
fn named_listeners_receive_only_their_event() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "user", "password"));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let secure_observed = Arc::clone(&observed);
    connection.on_named(ConnectionEventName::Secure, move |_| {
        secure_observed.lock().unwrap().push("secure");
    });
    let debug_observed = Arc::clone(&observed);
    connection.on_named("debug", move |_| {
        debug_observed.lock().unwrap().push("debug");
    });

    connection.emit(ConnectionEvent::Secure);
    connection.emit(ConnectionEvent::Debug("message".to_owned()));

    assert_eq!(*observed.lock().unwrap(), vec!["secure", "debug"]);
}

#[test]
fn generic_listeners_remain_wildcards_and_named_once_is_removed_before_callback() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "user", "password"));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let wildcard_observed = Arc::clone(&observed);
    connection.on(move |_| wildcard_observed.lock().unwrap().push("wildcard"));
    let once_observed = Arc::clone(&observed);
    let once_id = connection.once_named("end", move |_| {
        once_observed.lock().unwrap().push("end");
    });

    assert_eq!(connection.listener_count_named(ConnectionEventName::End), 1);
    assert_eq!(
        connection.listeners_named(ConnectionEventName::End),
        vec![once_id]
    );
    connection.emit(ConnectionEvent::Connect);
    connection.emit(ConnectionEvent::End);
    connection.emit(ConnectionEvent::End);

    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            "wildcard", "wildcard", "wildcard", "wildcard", "end", "wildcard"
        ]
    );
    assert_eq!(connection.listener_count_named(ConnectionEventName::End), 0);
}

#[test]
fn named_listener_removal_and_unknown_event_are_deterministic() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "user", "password"));
    let id = connection.on_named("secure", |_| {});
    assert_eq!(connection.listener_count_named("secure"), 1);
    assert_eq!(connection.remove_all_named_listeners("secure"), 1);
    assert!(!connection.off(id));
    assert_eq!(connection.remove_all_named_listeners("not-an-event"), 0);
    connection.emit(ConnectionEvent::Secure);
}

#[test]
fn named_listener_meta_events_report_registration_and_removal_order() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "user", "password"));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let new_observed = Arc::clone(&observed);
    connection.on_named("newListener", move |event| {
        if let ConnectionEvent::NewListener {
            event_name,
            listener_id,
        } = event
        {
            new_observed
                .lock()
                .unwrap()
                .push(("new", event_name.clone(), *listener_id));
        }
    });
    let remove_observed = Arc::clone(&observed);
    connection.on_named("removeListener", move |event| {
        if let ConnectionEvent::RemoveListener {
            event_name,
            listener_id,
        } = event
        {
            remove_observed
                .lock()
                .unwrap()
                .push(("remove", event_name.clone(), *listener_id));
        }
    });
    let listener_id = connection.on_named("secure", |_| {});
    assert!(connection.off(listener_id));

    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            ("new", "removeListener".to_owned(), 1),
            ("new", "secure".to_owned(), 2),
            ("remove", "secure".to_owned(), 2),
        ]
    );
}

#[test]
fn checked_error_emission_reports_missing_listener_without_suppressing_event() {
    let connection = Connection::new(ConnectionConfig::sql("localhost", "user", "password"));
    let error = tedious_rs::ConnectionError::new("failure", Some("EFAIL".to_owned()));
    assert_eq!(
        connection.emit_checked(ConnectionEvent::Error(error)),
        Err(EventEmitterError::UnhandledError)
    );
}

#[test]
fn checked_error_emission_counts_named_and_wildcard_listeners() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "user", "password"));
    connection.on_named("error", |_| {});
    assert_eq!(
        connection.emit_checked(ConnectionEvent::Error(tedious_rs::ConnectionError::new(
            "failure", None
        ),)),
        Ok(1)
    );
}

#[test]
fn once_named_listener_removal_is_reported_before_its_callback() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "user", "password"));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let removal_observed = Arc::clone(&observed);
    connection.on_named("removeListener", move |event| {
        if let ConnectionEvent::RemoveListener { event_name, .. } = event {
            removal_observed
                .lock()
                .unwrap()
                .push(format!("remove:{event_name}"));
        }
    });
    let callback_observed = Arc::clone(&observed);
    connection.once_named("secure", move |_| {
        callback_observed
            .lock()
            .unwrap()
            .push("callback".to_owned());
    });
    connection.emit(ConnectionEvent::Secure);

    assert_eq!(
        *observed.lock().unwrap(),
        vec!["remove:secure".to_owned(), "callback".to_owned()]
    );
}
