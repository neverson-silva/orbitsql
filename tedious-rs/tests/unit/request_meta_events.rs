use std::sync::{Arc, Mutex};

use tedious_rs::errors::RequestError;
use tedious_rs::request::{Request, RequestEvent, RequestMetaEvent, RequestResult};

#[test]
fn request_meta_events_report_registration_and_removal_in_observable_order() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut request = Request::new(
        "SELECT 1",
        |_result: Result<RequestResult, RequestError>| {},
    );
    let new_listener_observed = Arc::clone(&observed);
    request.on_new_listener(move |event| {
        if let RequestMetaEvent::NewListener { event, listener_id } = event {
            new_listener_observed
                .lock()
                .unwrap()
                .push(format!("new:{event}:{listener_id}"));
        }
    });
    let remove_listener_observed = Arc::clone(&observed);
    request.on_remove_listener(move |event| {
        if let RequestMetaEvent::RemoveListener { event, listener_id } = event {
            remove_listener_observed
                .lock()
                .unwrap()
                .push(format!("remove:{event}:{listener_id}"));
        }
    });
    let listener_id = request.on_named("prepared", |_| {});

    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            "new:removeListener:1".to_owned(),
            format!("new:prepared:{listener_id}")
        ]
    );
    assert!(request.remove_listener(listener_id));
    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            "new:removeListener:1".to_owned(),
            format!("new:prepared:{listener_id}"),
            format!("remove:prepared:{listener_id}")
        ]
    );
    assert!(!request.remove_listener(listener_id));
}

#[test]
fn request_checked_error_emission_reports_missing_listener_without_panicking() {
    let mut request = Request::new("SELECT 1", |_| {});
    let error = RequestError::new("failure", Some("EFAIL".to_owned()));

    assert!(!request.emit_checked(RequestEvent::Error(error.clone())));

    let observed = Arc::new(Mutex::new(Vec::new()));
    let error_observed = Arc::clone(&observed);
    request.on_named("error", move |event| {
        if let RequestEvent::Error(error) = event {
            error_observed.lock().unwrap().push(error.code.clone());
        }
    });
    assert!(request.emit_checked(RequestEvent::Error(error)));
    assert_eq!(*observed.lock().unwrap(), vec![Some("EFAIL".to_owned())]);
}

#[test]
fn request_failure_completes_callback_and_completion_once() {
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
    request.on(move |event| match event {
        RequestEvent::Error(_) => event_observations.lock().unwrap().push("error"),
        RequestEvent::RequestCompleted => event_observations.lock().unwrap().push("completed"),
        _ => {}
    });

    request.fail(RequestError::new("failure", Some("EFAIL".to_owned())));
    request.fail(RequestError::new("duplicate", Some("EFAIL".to_owned())));
    request.finish();

    assert_eq!(*observations.lock().unwrap(), vec!["callback", "completed"]);
}
