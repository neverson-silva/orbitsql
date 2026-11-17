use std::sync::{Arc, Mutex};

use tedious_rs::bulk_load::{BulkLoad, BulkLoadMetaEvent, BulkLoadOptions};

#[test]
fn named_listener_registration_and_removal_follow_event_emitter_order() {
    let mut bulk_load = BulkLoad::new("items", BulkLoadOptions::default()).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let registration_events = Arc::clone(&events);
    bulk_load.on_new_listener(move |event| {
        registration_events.lock().unwrap().push(event.clone());
    });
    let removal_events = Arc::clone(&events);
    bulk_load.on_remove_listener(move |event| {
        removal_events.lock().unwrap().push(event.clone());
    });

    let listener_id = bulk_load.on_named("cancel", |_| {});

    assert_eq!(
        *events.lock().unwrap(),
        vec![
            BulkLoadMetaEvent::NewListener {
                event: "removeListener".to_owned(),
                listener_id: 1,
            },
            BulkLoadMetaEvent::NewListener {
                event: "cancel".to_owned(),
                listener_id,
            }
        ]
    );

    assert!(bulk_load.remove_listener(listener_id));
    assert_eq!(
        events.lock().unwrap().last(),
        Some(&BulkLoadMetaEvent::RemoveListener {
            event: "cancel".to_owned(),
            listener_id,
        })
    );
}

#[test]
fn generic_listener_registration_and_removal_have_no_named_meta_event() {
    let mut bulk_load = BulkLoad::new("items", BulkLoadOptions::default()).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    bulk_load.on_new_listener(move |event| seen.lock().unwrap().push(event.clone()));
    let seen = Arc::clone(&events);
    bulk_load.on_remove_listener(move |event| seen.lock().unwrap().push(event.clone()));
    let listener_id = bulk_load.on(|_| {});

    assert_eq!(events.lock().unwrap().len(), 1);
    assert!(bulk_load.remove_listener(listener_id));
    assert_eq!(events.lock().unwrap().len(), 1);
}

#[test]
fn once_meta_listener_is_removed_before_the_next_registration_callback() {
    let mut bulk_load = BulkLoad::new("items", BulkLoadOptions::default()).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    bulk_load.once_new_listener(move |event| seen.lock().unwrap().push(event.clone()));

    let first = bulk_load.on_named("cancel", |_| {});
    let second = bulk_load.on_named("error", |_| {});

    assert_eq!(
        *events.lock().unwrap(),
        vec![BulkLoadMetaEvent::NewListener {
            event: "cancel".to_owned(),
            listener_id: first,
        }]
    );
    assert!(bulk_load.remove_listener(first));
    assert!(bulk_load.remove_listener(second));
}
