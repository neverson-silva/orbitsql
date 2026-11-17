use std::sync::{Arc, Mutex};

use tedious_rs::connection::{Connection, ConnectionConfig, ConnectionEvent};

#[test]
fn named_listener_lifecycle_matches_node_event_emitter_order() {
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
                .push(format!("new:{event_name}:{listener_id}"));
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
                .push(format!("remove:{event_name}:{listener_id}"));
        }
    });

    let first = connection.on_named("secure", |_| {});
    let second = connection.on_named("secure", |_| {});
    let once = connection.once_named("debug", |_| {});

    connection.emit(ConnectionEvent::Debug("message".to_owned()));
    assert!(connection.remove_listener(first));
    assert!(connection.remove_listener(second));
    assert_eq!(connection.remove_all_named_listeners("secure"), 0);

    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            "new:removeListener:1".to_owned(),
            format!("new:secure:{first}"),
            format!("new:secure:{second}"),
            format!("new:debug:{once}"),
            format!("remove:debug:{once}"),
            format!("remove:secure:{first}"),
            format!("remove:secure:{second}"),
        ]
    );
}

#[test]
fn remove_all_named_listeners_emits_removals_in_reverse_registration_order() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "user", "password"));
    let observed = Arc::new(Mutex::new(Vec::new()));
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
                .push((event_name.clone(), *listener_id));
        }
    });

    let first = connection.on_named("secure", |_| {});
    let second = connection.on_named("secure", |_| {});
    let third = connection.on_named("secure", |_| {});

    assert_eq!(connection.remove_all_named_listeners("secure"), 3);
    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            ("secure".to_owned(), third),
            ("secure".to_owned(), second),
            ("secure".to_owned(), first),
        ]
    );
}
