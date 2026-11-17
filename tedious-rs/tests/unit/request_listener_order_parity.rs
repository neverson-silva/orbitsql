use std::sync::{Arc, Mutex};

use tedious_rs::errors::RequestError;
use tedious_rs::request::{Request, RequestEvent, RequestMetaEvent, RequestResult};

#[test]
fn listeners_preserve_registration_order() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut request = Request::new(
        "SELECT 1",
        |_result: Result<RequestResult, RequestError>| {},
    );
    let first_observed = Arc::clone(&observed);
    let first = request.on_named("prepared", move |_| {
        first_observed.lock().unwrap().push("first");
    });
    let meta = request.on_new_listener(|_| {});
    let second_observed = Arc::clone(&observed);
    let second = request.on_named("prepared", move |_| {
        second_observed.lock().unwrap().push("second");
    });
    let third_observed = Arc::clone(&observed);
    let third = request.on_named("prepared", move |_| {
        third_observed.lock().unwrap().push("third");
    });

    assert_eq!(request.listeners(), vec![first, second, third, meta]);
    assert_eq!(
        request.listeners_named("prepared"),
        vec![first, second, third]
    );
    assert!(request.emit_named_checked("prepared", RequestEvent::Prepared));
    assert_eq!(*observed.lock().unwrap(), vec!["first", "second", "third"]);
}

#[test]
fn remove_all_listeners_removes_in_lifo_order() {
    let removed = Arc::new(Mutex::new(Vec::new()));
    let mut request = Request::new(
        "SELECT 1",
        |_result: Result<RequestResult, RequestError>| {},
    );
    let removed_observed = Arc::clone(&removed);
    request.on_remove_listener(move |event| {
        if let RequestMetaEvent::RemoveListener { event, listener_id } = event
            && event == "prepared"
        {
            removed_observed.lock().unwrap().push(*listener_id);
        }
    });
    let first = request.on_named("prepared", |_| {});
    let second = request.on_named("prepared", |_| {});
    let third = request.on_named("prepared", |_| {});

    assert_eq!(request.remove_all_listeners(Some("prepared")), 3);
    assert_eq!(*removed.lock().unwrap(), vec![third, second, first]);
    assert_eq!(request.listeners_named("prepared"), Vec::<u64>::new());
}

#[test]
fn once_is_removed_before_callback_and_reports_its_event() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut request = Request::new(
        "SELECT 1",
        |_result: Result<RequestResult, RequestError>| {},
    );
    let removal_observed = Arc::clone(&observed);
    request.on_remove_listener(move |event| {
        if let RequestMetaEvent::RemoveListener { event, listener_id } = event {
            removal_observed
                .lock()
                .unwrap()
                .push(format!("remove:{event}:{listener_id}"));
        }
    });
    let callback_observed = Arc::clone(&observed);
    let listener_id = request.once_named("prepared", move |_| {
        callback_observed
            .lock()
            .unwrap()
            .push("callback".to_owned());
    });

    assert!(request.emit_named_checked("prepared", RequestEvent::Prepared));
    assert_eq!(request.listeners_named("prepared"), Vec::<u64>::new());
    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            format!("remove:prepared:{listener_id}"),
            "callback".to_owned(),
        ]
    );
}
