use std::sync::{Arc, Mutex};

use tedious_rs::bulk_load::{BulkLoad, BulkLoadEvent, BulkLoadOptions};
use tedious_rs::errors::RequestError;
use tedious_rs::request::{Request, RequestEvent, RequestResult};

#[test]
fn request_listener_registration_and_removal_match_event_emitter_order() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut request = Request::new("SELECT 1", |_| {});
    let first_events = Arc::clone(&events);
    let first = request.on(move |_| first_events.lock().unwrap().push("first"));
    let second_events = Arc::clone(&events);
    let second = request.on(move |_| second_events.lock().unwrap().push("second"));

    assert_eq!(request.listener_count(), 2);
    assert_eq!(request.listeners(), vec![first, second]);
    assert!(request.emit_checked(RequestEvent::Prepared));
    assert_eq!(*events.lock().unwrap(), vec!["first", "second"]);
    assert!(request.remove_listener(first));
    assert!(!request.remove_listener(first));
    assert!(request.off(second));
    assert_eq!(request.listener_count(), 0);
    assert!(!request.emit_checked(RequestEvent::Prepared));
}

#[test]
fn request_once_listener_is_removed_before_reentrant_emission() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut request = Request::new("SELECT 1", |_| {});
    let seen = Arc::clone(&events);
    request.once(move |_| seen.lock().unwrap().push(1));
    assert!(request.emit_checked(RequestEvent::Prepared));
    assert!(!request.emit_checked(RequestEvent::Prepared));
    assert_eq!(*events.lock().unwrap(), vec![1]);
    assert_eq!(request.listener_count(), 0);
}

#[test]
fn request_failure_callback_and_completion_have_stable_order_and_single_completion() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let callback_observed = Arc::clone(&observed);
    let mut request = Request::new(
        "SELECT 1",
        move |result: Result<RequestResult, RequestError>| {
            assert!(result.is_err());
            callback_observed.lock().unwrap().push("callback");
        },
    );
    let listener_observed = Arc::clone(&observed);
    request.on(move |event| {
        let name = match event {
            RequestEvent::Error(_) => "error",
            RequestEvent::RequestCompleted => "completed",
            _ => "other",
        };
        listener_observed.lock().unwrap().push(name);
    });

    request.fail(RequestError::new("failure", Some("EFAIL".to_owned())));
    request.finish();

    assert_eq!(*observed.lock().unwrap(), vec!["callback", "completed"]);
}

#[test]
fn request_emit_without_listener_is_non_panicking_and_reports_false() {
    let mut request = Request::new("SELECT 1", |_| {});
    assert!(!request.emit_checked(RequestEvent::Error(RequestError::new(
        "failure",
        Some("EFAIL".to_owned()),
    ))));
}

#[test]
fn bulk_load_listener_contract_and_once_semantics_are_deterministic() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut bulk_load = BulkLoad::new("dbo.items", BulkLoadOptions::default()).unwrap();
    let persistent_events = Arc::clone(&events);
    let persistent = bulk_load.on(move |_| persistent_events.lock().unwrap().push("persistent"));
    let once_events = Arc::clone(&events);
    let once = bulk_load.once(move |_| once_events.lock().unwrap().push("once"));

    assert!(bulk_load.emit_checked(BulkLoadEvent::Cancel));
    assert!(bulk_load.emit_checked(BulkLoadEvent::Cancel));
    assert_eq!(
        *events.lock().unwrap(),
        vec!["persistent", "once", "persistent"]
    );
    assert!(bulk_load.remove_listener(persistent));
    assert!(!bulk_load.remove_listener(once));
    assert!(!bulk_load.emit_checked(BulkLoadEvent::Cancel));
}

#[test]
fn bulk_load_failure_reports_completion_without_error_event() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let callback_observed = Arc::clone(&observed);
    let mut bulk_load =
        BulkLoad::new_with_callback("dbo.items", BulkLoadOptions::default(), move |result| {
            assert_eq!(result, Err("failure".to_owned()));
            callback_observed.lock().unwrap().push("callback");
        })
        .unwrap();
    let listener_observed = Arc::clone(&observed);
    bulk_load.on(move |event| {
        if matches!(event, BulkLoadEvent::Error(_)) {
            listener_observed.lock().unwrap().push("error");
        }
    });

    assert!(bulk_load.fail("failure"));
    assert!(!bulk_load.fail("second failure"));
    assert_eq!(*observed.lock().unwrap(), vec!["callback"]);
}

#[test]
fn bulk_load_completion_callback_is_called_once_for_success() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let callback_calls = Arc::clone(&calls);
    let mut bulk_load =
        BulkLoad::new_with_callback("dbo.items", BulkLoadOptions::default(), move |result| {
            callback_calls.lock().unwrap().push(result.unwrap())
        })
        .unwrap();

    assert!(bulk_load.complete(3));
    assert!(!bulk_load.complete(4));
    assert_eq!(*calls.lock().unwrap(), vec![3]);
}
