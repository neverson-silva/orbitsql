use std::sync::{Arc, Mutex};

use tedious_rs::errors::RequestError;
use tedious_rs::request::{Request, RequestEvent, RequestResult, RequestState};

#[test]
fn prepare_failure_uses_special_callback_lifecycle_exactly_once() {
    let observations = Arc::new(Mutex::new(Vec::new()));
    let callback_observations = Arc::clone(&observations);
    let mut request = Request::new(
        "SELECT 1",
        move |result: Result<RequestResult, RequestError>| {
            assert!(result.is_err());
            callback_observations.lock().unwrap().push("callback");
        },
    );
    let event_observations = Arc::clone(&observations);
    request.on_named("error", move |event| {
        if matches!(event, RequestEvent::Error(_)) {
            event_observations.lock().unwrap().push("error");
        }
    });
    let event_observations = Arc::clone(&observations);
    request.on_named("prepared", move |event| {
        if matches!(event, RequestEvent::Prepared) {
            event_observations.lock().unwrap().push("prepared");
        }
    });
    let event_observations = Arc::clone(&observations);
    request.on_named("requestCompleted", move |event| {
        if matches!(event, RequestEvent::RequestCompleted) {
            event_observations.lock().unwrap().push("requestCompleted");
        }
    });

    request.preparing = true;
    let error = RequestError::new("prepare failed", Some("EPREPARE".to_owned()));
    request.fail_preparing(error.clone());
    request.fail_preparing(error);
    request.finish();
    request.finish();

    assert_eq!(request.state, RequestState::Created);
    assert!(!request.preparing);
    assert_eq!(*observations.lock().unwrap(), vec!["error"]);
}

#[test]
fn prepare_failure_preserves_callback_for_a_later_execution() {
    let callbacks = Arc::new(Mutex::new(0));
    let callback_count = Arc::clone(&callbacks);
    let mut request = Request::new(
        "SELECT 1",
        move |result: Result<RequestResult, RequestError>| {
            assert!(result.is_ok());
            *callback_count.lock().unwrap() += 1;
        },
    );

    request.preparing = true;
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
fn prepare_success_emits_prepared_once_and_closes_completion_lifecycle() {
    let observations = Arc::new(Mutex::new(Vec::new()));
    let callback_observations = Arc::clone(&observations);
    let mut request = Request::new("SELECT 1", move |result| {
        callback_observations
            .lock()
            .unwrap()
            .push(if result.is_ok() {
                "callback-ok"
            } else {
                "callback-err"
            });
    });
    let event_observations = Arc::clone(&observations);
    request.on_named("prepared", move |_| {
        event_observations.lock().unwrap().push("prepared");
    });
    let event_observations = Arc::clone(&observations);
    request.on_named("requestCompleted", move |_| {
        event_observations.lock().unwrap().push("requestCompleted");
    });

    request.preparing = true;
    request.finish();
    request.finish();

    assert_eq!(request.state, RequestState::Created);
    assert!(!request.preparing);
    assert_eq!(*observations.lock().unwrap(), vec!["prepared"]);
}

#[test]
fn unexpected_prepare_output_error_is_retained_before_special_error_event() {
    let mut request = Request::new("SELECT 1", |_| {});
    request.preparing = true;
    let error = RequestError::new(
        "Tedious > Unexpected output parameter unexpected from sp_prepare",
        Some("EPREPARE".to_owned()),
    );
    request.error = Some(error.clone());
    request.fail_preparing(error.clone());

    assert_eq!(request.error, Some(error));
    assert!(!request.preparing);
    assert_eq!(request.state, RequestState::Created);
}
