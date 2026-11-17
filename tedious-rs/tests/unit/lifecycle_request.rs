use std::sync::{Arc, Mutex};

use tedious_rs::errors::RequestError;
use tedious_rs::request::{Request, RequestEvent, RequestResult, RequestState};

#[test]
fn failure_cleanup_is_idempotent_and_completes_callback_once() {
    let callbacks = Arc::new(Mutex::new(0));
    let callback_count = Arc::clone(&callbacks);
    let mut request = Request::new(
        "SELECT 1",
        move |result: Result<RequestResult, RequestError>| {
            assert!(result.is_err());
            *callback_count.lock().unwrap() += 1;
        },
    );
    request.begin().unwrap();
    let error = RequestError::new("socket closed", Some("ECONNRESET".to_owned()));
    request.fail(error.clone());
    request.fail(error);
    request.finish();

    assert_eq!(request.state, RequestState::Failed);
    assert_eq!(*callbacks.lock().unwrap(), 1);
}

#[test]
fn preparing_failure_is_idempotent() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    let callbacks = Arc::new(Mutex::new(0));
    let callback_count = Arc::clone(&callbacks);
    let mut request = Request::new("SELECT 1", move |_| {
        *callback_count.lock().unwrap() += 1;
    });
    request.preparing = true;
    request.begin().unwrap();
    request.on(move |event| seen.lock().unwrap().push(event.clone()));
    let error = RequestError::new("prepare failed", Some("EPREPARE".to_owned()));
    request.fail_preparing(error.clone());
    request.fail_preparing(error.clone());

    assert_eq!(request.state, RequestState::Created);
    assert!(!request.preparing);
    assert_eq!(request.error, None);
    assert_eq!(*callbacks.lock().unwrap(), 0);
    assert_eq!(*events.lock().unwrap(), vec![RequestEvent::Error(error)]);
}

#[test]
fn preparing_failure_restores_created_and_preserves_callback_for_retry() {
    let callbacks = Arc::new(Mutex::new(0));
    let callback_count = Arc::clone(&callbacks);
    let mut request = Request::new("SELECT 1", move |result| {
        assert!(result.is_ok());
        *callback_count.lock().unwrap() += 1;
    });
    request.preparing = true;
    request.begin().unwrap();

    request.fail_preparing(RequestError::new(
        "prepare failed",
        Some("EPREPARE".to_owned()),
    ));
    request.begin().unwrap();
    request.finish();

    assert_eq!(request.state, RequestState::Completed);
    assert_eq!(*callbacks.lock().unwrap(), 1);
}

#[test]
fn timeout_cancellation_still_completes_the_request_once() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    let mut request = Request::new("SELECT 1", |_| {});
    request.on(move |event| seen.lock().unwrap().push(event.clone()));
    request.begin().unwrap();
    request.cancel();
    request.fail(RequestError::new(
        "Timeout: Request failed to complete in 1ms",
        Some("ETIMEOUT".to_owned()),
    ));
    request.fail(RequestError::new(
        "Timeout: Request failed to complete in 1ms",
        Some("ETIMEOUT".to_owned()),
    ));

    assert_eq!(request.state, RequestState::Failed);
    assert_eq!(
        request
            .error
            .as_ref()
            .and_then(|error| error.code.as_deref()),
        Some("ETIMEOUT")
    );
    assert_eq!(
        *events.lock().unwrap(),
        vec![RequestEvent::Cancel, RequestEvent::RequestCompleted]
    );
}
