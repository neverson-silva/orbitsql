use std::sync::{Arc, Mutex, OnceLock};

use tedious_rs::connection::{Connection, ConnectionConfig, ConnectionEvent};

#[test]
fn multiple_once_listeners_interleave_removal_and_callback_like_node() {
    let mut connection = Arc::new(Connection::new(ConnectionConfig::sql(
        "localhost",
        "user",
        "password",
    )));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let connection_reference = Arc::new(OnceLock::<std::sync::Weak<Connection>>::new());

    let remove_observed = Arc::clone(&observed);
    Arc::get_mut(&mut connection)
        .unwrap()
        .on_named("removeListener", move |event| {
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

    let first_id = Arc::get_mut(&mut connection)
        .unwrap()
        .once_named("secure", {
            let observed = Arc::clone(&observed);
            let connection_reference = Arc::clone(&connection_reference);
            move |_| {
                observed.lock().unwrap().push("callback:first".to_owned());
                connection_reference
                    .get()
                    .unwrap()
                    .upgrade()
                    .unwrap()
                    .emit(ConnectionEvent::Secure);
            }
        });
    let second_id = Arc::get_mut(&mut connection)
        .unwrap()
        .once_named("secure", {
            let observed = Arc::clone(&observed);
            move |_| {
                observed.lock().unwrap().push("callback:second".to_owned());
            }
        });
    connection_reference
        .set(Arc::downgrade(&connection))
        .unwrap();

    connection.emit(ConnectionEvent::Secure);

    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            format!("remove:secure:{first_id}"),
            "callback:first".to_owned(),
            format!("remove:secure:{second_id}"),
            "callback:second".to_owned(),
        ]
    );
    assert_eq!(connection.listener_count_named("secure"), 0);
}
