use std::sync::{Arc, Mutex};

use tedious_rs::errors::RequestError;
use tedious_rs::request::{Request, RequestEvent, RequestResult, RequestState};

#[test]
fn failure_calls_callback_before_request_completed_once() {
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
    request.on_named("requestCompleted", move |_| {
        event_observations.lock().unwrap().push("requestCompleted");
    });

    request.begin().unwrap();
    request.fail(RequestError::new("failure", Some("EFAIL".to_owned())));
    request.finish();

    assert_eq!(request.state, RequestState::Failed);
    assert_eq!(
        request
            .error
            .as_ref()
            .and_then(|error| error.code.as_deref()),
        Some("EFAIL")
    );
    assert_eq!(
        *observations.lock().unwrap(),
        vec!["callback", "requestCompleted"]
    );
}

#[test]
fn canceled_failure_completes_once_even_if_repeated_or_finished() {
    let callbacks = Arc::new(Mutex::new(0));
    let callback_count = Arc::clone(&callbacks);
    let completions = Arc::new(Mutex::new(0));
    let completion_count = Arc::clone(&completions);
    let mut request = Request::new("SELECT 1", move |_| {
        *callback_count.lock().unwrap() += 1;
    });
    request.on_named("requestCompleted", move |_| {
        *completion_count.lock().unwrap() += 1;
    });

    request.begin().unwrap();
    request.cancel();
    request.fail(RequestError::new("Canceled.", Some("ECANCEL".to_owned())));
    request.fail(RequestError::new("Canceled.", Some("ECANCEL".to_owned())));
    request.finish();

    assert_eq!(request.state, RequestState::Canceled);
    assert_eq!(*callbacks.lock().unwrap(), 1);
    assert_eq!(*completions.lock().unwrap(), 1);
}

#[test]
fn preparing_failure_emits_error_without_callback_or_completion() {
    let observations = Arc::new(Mutex::new(Vec::new()));
    let callback_observations = Arc::clone(&observations);
    let mut request = Request::new("SELECT 1", move |_| {
        callback_observations.lock().unwrap().push("callback");
    });
    let event_observations = Arc::clone(&observations);
    request.on(move |event| match event {
        RequestEvent::Error(_) => event_observations.lock().unwrap().push("error"),
        RequestEvent::RequestCompleted => {
            event_observations.lock().unwrap().push("requestCompleted")
        }
        _ => {}
    });

    request.preparing = true;
    request.fail(RequestError::new(
        "prepare failed",
        Some("EPREPARE".to_owned()),
    ));

    assert_eq!(request.state, RequestState::Created);
    assert!(!request.preparing);
    assert_eq!(*observations.lock().unwrap(), vec!["error"]);
}
