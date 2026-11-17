use std::sync::{Arc, Mutex};

use tedious_rs::errors::RequestError;
use tedious_rs::request::{Request, RequestEvent, RequestResult};

#[test]
fn request_listener_ids_support_order_count_and_removal() {
    let mut request = Request::new(
        "SELECT 1",
        |_result: Result<RequestResult, RequestError>| {},
    );
    let seen = Arc::new(Mutex::new(Vec::new()));
    let first_seen = Arc::clone(&seen);
    let first = request.on(move |_| first_seen.lock().unwrap().push(1));
    let second_seen = Arc::clone(&seen);
    let second = request.on(move |_| second_seen.lock().unwrap().push(2));

    assert_eq!(request.listener_count(), 2);
    assert_eq!(request.listeners(), vec![first, second]);
    assert!(request.off(first));
    assert!(!request.remove_listener(first));
    request.emit(RequestEvent::Cancel);

    assert_eq!(*seen.lock().unwrap(), vec![2]);
}

#[test]
fn request_once_listener_is_removed_before_callback_and_does_not_repeat() {
    let mut request = Request::new(
        "SELECT 1",
        |_result: Result<RequestResult, RequestError>| {},
    );
    let seen = Arc::new(Mutex::new(0));
    let callback_seen = Arc::clone(&seen);
    let id = request.once(move |_| *callback_seen.lock().unwrap() += 1);

    assert_eq!(request.listeners(), vec![id]);
    request.emit(RequestEvent::Cancel);
    request.emit(RequestEvent::Cancel);

    assert_eq!(*seen.lock().unwrap(), 1);
    assert_eq!(request.listener_count(), 0);
}
