use std::sync::{Arc, Mutex};

use tedious_rs::public_api::RequestMetaEvent;
use tedious_rs::{Request, RequestError};

#[test]
fn request_cancel_returns_transition_result_and_emits_once() {
    let events = Arc::new(Mutex::new(0));
    let observed = Arc::clone(&events);
    let mut request = Request::new("select 1", |_| {});
    request.on_named("cancel", move |_| *observed.lock().unwrap() += 1);

    assert!(request.cancel());
    assert!(!request.cancel());
    assert_eq!(*events.lock().unwrap(), 1);
}

#[test]
fn request_meta_listeners_are_scoped_to_their_event() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let new_events = Arc::clone(&events);
    let remove_events = Arc::clone(&events);
    let mut request = Request::new("select 1", |_| {});
    request.on_new_listener(move |event| {
        if let RequestMetaEvent::NewListener { .. } = event {
            new_events.lock().unwrap().push("new");
        }
    });
    request.on_remove_listener(move |event| {
        if let RequestMetaEvent::RemoveListener { .. } = event {
            remove_events.lock().unwrap().push("remove");
        }
    });
    let id = request.on_named("prepared", |_| {});
    request.remove_listener(id);

    assert_eq!(*events.lock().unwrap(), vec!["new", "new", "remove"]);
}

#[test]
fn node_callback_adapter_maps_success_and_error_results() {
    let success = Arc::new(Mutex::new(None));
    let observed_success = Arc::clone(&success);
    let mut request = Request::new_optional(Some("select 1"), move |error, count, rows| {
        *observed_success.lock().unwrap() = Some((error, count, rows.len()));
    });
    request.finish();
    assert_eq!(
        success.lock().unwrap().as_ref().map(|value| value.1),
        Some(0)
    );

    let failure = Arc::new(Mutex::new(None));
    let observed_failure = Arc::clone(&failure);
    let mut request = Request::new_optional(None::<String>, move |error, count, rows| {
        *observed_failure.lock().unwrap() = Some((error, count, rows.len()));
    });
    request.fail(RequestError::new("failure", Some("EFAIL".to_owned())));
    let result = failure.lock().unwrap();
    assert_eq!(
        result
            .as_ref()
            .map(|value| value.0.as_ref().unwrap().code.as_deref()),
        Some(Some("EFAIL"))
    );
}
