use std::sync::{Arc, Mutex};

use tedious_rs::errors::RequestError;
use tedious_rs::request::{Request, RequestMetaEvent, RequestResult};

#[test]
fn request_listener_meta_events_cover_registration_and_removal_lifecycle() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut request = Request::new(
        "SELECT 1",
        |_result: Result<RequestResult, RequestError>| {},
    );
    let new_observed = Arc::clone(&observed);
    let new_listener_id = request.on_new_listener(move |event| {
        if let RequestMetaEvent::NewListener { event, listener_id } = event {
            new_observed
                .lock()
                .unwrap()
                .push(format!("new:{event}:{listener_id}"));
        }
    });
    let remove_observed = Arc::clone(&observed);
    let remove_listener_id = request.on_remove_listener(move |event| {
        if let RequestMetaEvent::RemoveListener { event, listener_id } = event {
            remove_observed
                .lock()
                .unwrap()
                .push(format!("remove:{event}:{listener_id}"));
        }
    });
    let request_listener_id = request.on_named("prepared", |_| {});

    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            format!("new:removeListener:{remove_listener_id}"),
            format!("new:prepared:{request_listener_id}"),
        ]
    );
    assert_eq!(request.listener_count(), 3);

    assert!(request.remove_listener(request_listener_id));
    assert!(request.remove_listener(new_listener_id));
    assert!(!request.remove_listener(new_listener_id));
    assert!(request.remove_listener(remove_listener_id));
    assert!(!request.remove_listener(remove_listener_id));
    assert_eq!(request.listener_count(), 0);
    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            format!("new:removeListener:{remove_listener_id}"),
            format!("new:prepared:{request_listener_id}"),
            format!("remove:prepared:{request_listener_id}"),
            format!("remove:newListener:{new_listener_id}"),
        ]
    );
}

#[test]
fn request_once_meta_listener_emits_its_removal_before_callback() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut request = Request::new(
        "SELECT 1",
        |_result: Result<RequestResult, RequestError>| {},
    );
    let new_observed = Arc::clone(&observed);
    request.on_new_listener(move |event| {
        if let RequestMetaEvent::NewListener { event, listener_id } = event {
            new_observed
                .lock()
                .unwrap()
                .push(format!("new:{event}:{listener_id}"));
        }
    });
    let remove_observed = Arc::clone(&observed);
    let remove_listener_id = request.on_remove_listener(move |event| {
        if let RequestMetaEvent::RemoveListener { event, listener_id } = event {
            remove_observed
                .lock()
                .unwrap()
                .push(format!("remove:{event}:{listener_id}"));
        }
    });
    let callback_observed = Arc::clone(&observed);
    let once_listener_id = request.once_new_listener(move |event| {
        if let RequestMetaEvent::NewListener { event, listener_id } = event {
            callback_observed
                .lock()
                .unwrap()
                .push(format!("once:{event}:{listener_id}"));
        }
    });
    let request_listener_id = request.on_named("prepared", |_| {});

    assert_eq!(request.listener_count(), 3);
    assert!(!request.listeners().contains(&once_listener_id));
    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            format!("new:removeListener:{remove_listener_id}"),
            format!("new:newListener:{once_listener_id}"),
            format!("new:prepared:{request_listener_id}"),
            format!("remove:newListener:{once_listener_id}"),
            format!("once:prepared:{request_listener_id}"),
        ]
    );
}
