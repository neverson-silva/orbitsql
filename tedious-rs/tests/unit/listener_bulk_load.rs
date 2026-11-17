use std::sync::{Arc, Mutex};

use tedious_rs::bulk_load::{BulkLoad, BulkLoadEvent, BulkLoadOptions};

#[test]
fn bulk_load_once_listener_is_removed_after_cancel() {
    let mut bulk_load = BulkLoad::new("test_table", BulkLoadOptions::default()).unwrap();
    let seen = Arc::new(Mutex::new(0));
    let callback_seen = Arc::clone(&seen);
    let id = bulk_load.once(move |_| *callback_seen.lock().unwrap() += 1);

    assert_eq!(bulk_load.listeners(), vec![id]);
    bulk_load.cancel();
    bulk_load.cancel();

    assert_eq!(*seen.lock().unwrap(), 1);
    assert_eq!(bulk_load.listener_count(), 0);
}

#[test]
fn bulk_load_listener_can_be_removed_without_emitting() {
    let mut bulk_load = BulkLoad::new("test_table", BulkLoadOptions::default()).unwrap();
    let seen = Arc::new(Mutex::new(0));
    let callback_seen = Arc::clone(&seen);
    let id = bulk_load.on(move |event| {
        if matches!(event, BulkLoadEvent::Cancel) {
            *callback_seen.lock().unwrap() += 1;
        }
    });

    assert!(bulk_load.remove_listener(id));
    bulk_load.cancel();

    assert_eq!(*seen.lock().unwrap(), 0);
}
