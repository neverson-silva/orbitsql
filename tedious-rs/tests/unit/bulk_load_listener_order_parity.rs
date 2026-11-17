use std::sync::{Arc, Mutex};

use tedious_rs::bulk_load::{BulkLoad, BulkLoadMetaEvent, BulkLoadOptions};

#[test]
fn once_meta_listener_emits_remove_listener_before_its_callback() {
    let mut bulk_load = BulkLoad::new("items", BulkLoadOptions::default()).unwrap();
    let observed = Arc::new(Mutex::new(Vec::new()));

    let remove_observed = Arc::clone(&observed);
    bulk_load.on_remove_listener(move |event| {
        remove_observed
            .lock()
            .unwrap()
            .push(format!("remove:{event:?}"));
    });
    let callback_observed = Arc::clone(&observed);
    bulk_load.once_new_listener(move |_| {
        callback_observed
            .lock()
            .unwrap()
            .push("callback".to_owned());
    });

    bulk_load.on_named("cancel", |_| {});

    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            "remove:RemoveListener { event: \"newListener\", listener_id: 1 }".to_owned(),
            "callback".to_owned(),
        ]
    );
}

#[test]
fn remove_all_listeners_removes_global_registration_order_in_lifo_order() {
    let mut bulk_load = BulkLoad::new("items", BulkLoadOptions::default()).unwrap();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let remove_observed = Arc::clone(&observed);
    bulk_load.on_remove_listener(move |event| {
        if let BulkLoadMetaEvent::RemoveListener { listener_id, .. } = event {
            remove_observed.lock().unwrap().push(*listener_id);
        }
    });

    let first = bulk_load.on_named("cancel", |_| {});
    let second = bulk_load.once_named("error", |_| {});
    let _third = bulk_load.on(|_| {});

    assert_eq!(bulk_load.remove_all_listeners(), 4);
    assert_eq!(*observed.lock().unwrap(), vec![second, first]);
}

#[test]
fn once_meta_listener_remains_until_its_event_is_emitted() {
    let mut bulk_load = BulkLoad::new("items", BulkLoadOptions::default()).unwrap();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let callback_observed = Arc::clone(&observed);
    bulk_load.once_remove_listener(move |event| {
        callback_observed
            .lock()
            .unwrap()
            .push(matches!(event, BulkLoadMetaEvent::RemoveListener { .. }));
    });

    bulk_load.on_named("cancel", |_| {});
    bulk_load.remove_listener(1);

    assert_eq!(*observed.lock().unwrap(), vec![true]);
}
