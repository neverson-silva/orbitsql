use std::sync::{Arc, Mutex};

use tedious_rs::errors::RequestError;
use tedious_rs::request::{Request, RequestEvent, RequestResult, RequestState};

#[test]
fn named_listeners_receive_only_their_event_and_preserve_order() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut request = Request::new("select 1", |_| {});
    let first_observed = Arc::clone(&observed);
    request.on_named("row", move |_| first_observed.lock().unwrap().push(1));
    let second_observed = Arc::clone(&observed);
    request.once_named("done", move |_| second_observed.lock().unwrap().push(2));
    let third_observed = Arc::clone(&observed);
    request.on_named("row", move |_| third_observed.lock().unwrap().push(3));

    assert!(request.emit_checked(RequestEvent::Row(transparent_row())));
    assert!(request.emit_checked(RequestEvent::Done {
        row_count: Some(1),
        more: false,
        rows: None,
    }));
    assert!(!request.emit_checked(RequestEvent::Done {
        row_count: Some(2),
        more: false,
        rows: None,
    }));
    assert_eq!(*observed.lock().unwrap(), vec![1, 3, 2]);
    assert_eq!(request.listener_count_named("done"), 0);
    assert_eq!(request.listener_count_named("row"), 2);
}

#[test]
fn named_dispatch_keeps_unfiltered_listeners_and_supports_named_removal() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut request = Request::new("select 1", |_| {});
    let all_observed = Arc::clone(&observed);
    request.on(move |_| all_observed.lock().unwrap().push(1));
    let named_observed = Arc::clone(&observed);
    let named_id = request.on_named("prepared", move |_| named_observed.lock().unwrap().push(2));

    assert!(request.emit_named_checked("prepared", RequestEvent::Prepared));
    assert_eq!(*observed.lock().unwrap(), vec![1, 2]);
    assert!(request.remove_listener(named_id));
    assert_eq!(request.remove_all_listeners(Some("prepared")), 0);
    assert!(request.emit_named_checked("prepared", RequestEvent::Prepared));
    assert_eq!(*observed.lock().unwrap(), vec![1, 2, 1]);
}

#[test]
fn validation_failure_calls_back_before_request_completed_once() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let callback_observed = Arc::clone(&observed);
    let mut request = Request::new(
        "select @value",
        move |result: Result<RequestResult, RequestError>| {
            assert!(result.is_err());
            callback_observed.lock().unwrap().push("callback");
        },
    );
    let event_observed = Arc::clone(&observed);
    request.on_named("requestCompleted", move |_| {
        event_observed.lock().unwrap().push("requestCompleted")
    });
    request.fail_validation(RequestError::new(
        "Validation failed for parameter 'value'. invalid",
        Some("EPARAM".to_owned()),
    ));
    request.fail_validation(RequestError::new("duplicate", Some("EPARAM".to_owned())));

    assert_eq!(request.state, RequestState::Failed);
    assert_eq!(
        *observed.lock().unwrap(),
        vec!["callback", "requestCompleted"]
    );
}

fn transparent_row() -> tedious_rs::token::rows::Row {
    tedious_rs::token::rows::Row {
        columns: Vec::new(),
    }
}
