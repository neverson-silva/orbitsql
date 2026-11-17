use std::sync::{Arc, Mutex};

use tedious_rs::bulk_load::{
    BulkLoad, BulkLoadEmitError, BulkLoadEvent, BulkLoadMetaEvent, BulkLoadOptions,
};

#[test]
fn bulk_load_meta_events_report_registration_and_removal_order() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut bulk_load = BulkLoad::new("dbo.items", BulkLoadOptions::default()).unwrap();

    let new_observed = Arc::clone(&observed);
    bulk_load.on_new_listener(move |event| {
        if let BulkLoadMetaEvent::NewListener { event, .. } = event {
            new_observed.lock().unwrap().push(format!("new:{event}"));
        }
    });
    let remove_observed = Arc::clone(&observed);
    bulk_load.on_remove_listener(move |event| {
        if let BulkLoadMetaEvent::RemoveListener { event, .. } = event {
            remove_observed
                .lock()
                .unwrap()
                .push(format!("remove:{event}"));
        }
    });

    let id = bulk_load.on_named("cancel", |_| {});
    assert_eq!(
        *observed.lock().unwrap(),
        vec!["new:removeListener", "new:cancel"]
    );
    assert!(bulk_load.remove_listener(id));
    assert_eq!(
        *observed.lock().unwrap(),
        vec!["new:removeListener", "new:cancel", "remove:cancel"]
    );
}

#[test]
fn bulk_load_named_and_wildcard_listeners_preserve_event_delivery() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut bulk_load = BulkLoad::new("dbo.items", BulkLoadOptions::default()).unwrap();
    let wildcard_observed = Arc::clone(&observed);
    bulk_load.on(move |event| {
        if matches!(event, BulkLoadEvent::Cancel) {
            wildcard_observed.lock().unwrap().push("wildcard");
        }
    });
    let named_observed = Arc::clone(&observed);
    bulk_load.once_named("cancel", move |_| {
        named_observed.lock().unwrap().push("named");
    });

    assert!(bulk_load.emit_checked(BulkLoadEvent::Cancel));
    assert!(bulk_load.emit_checked(BulkLoadEvent::Cancel));
    assert_eq!(
        *observed.lock().unwrap(),
        vec!["wildcard", "named", "wildcard"]
    );
}

#[test]
fn bulk_load_checked_error_reports_missing_listener_without_lifecycle_error() {
    let completion_calls = Arc::new(Mutex::new(0));
    let completion_calls_seen = Arc::clone(&completion_calls);
    let mut bulk_load =
        BulkLoad::new_with_callback("dbo.items", BulkLoadOptions::default(), move |result| {
            assert_eq!(result, Err("failure".to_owned()));
            *completion_calls_seen.lock().unwrap() += 1;
        })
        .unwrap();

    assert_eq!(
        bulk_load.emit_error_checked("unhandled"),
        Err(BulkLoadEmitError::UnhandledError)
    );
    assert!(bulk_load.fail("failure"));
    assert_eq!(*completion_calls.lock().unwrap(), 1);
}

#[test]
fn bulk_load_error_checked_succeeds_with_error_listener_and_completion_is_once() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut bulk_load = BulkLoad::new("dbo.items", BulkLoadOptions::default()).unwrap();
    let error_observed = Arc::clone(&observed);
    bulk_load.on_named("error", move |event| {
        if let BulkLoadEvent::Error(message) = event {
            error_observed.lock().unwrap().push(message.clone());
        }
    });

    assert_eq!(bulk_load.emit_error_checked("server failure"), Ok(1));
    assert_eq!(bulk_load.emit_error_checked("second failure"), Ok(1));
    assert_eq!(
        *observed.lock().unwrap(),
        vec!["server failure", "second failure"]
    );
    assert!(bulk_load.complete(2));
    assert!(!bulk_load.complete(3));
}
