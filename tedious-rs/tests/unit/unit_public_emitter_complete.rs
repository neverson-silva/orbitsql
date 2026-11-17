use std::sync::{Arc, Mutex};
use tedious_rs::{
    CONNECTION_EVENT_NAMES, EventEmitter, EventEmitterError, EventLifecycle, REQUEST_EVENT_NAMES,
};

#[test]
fn listeners_are_called_in_registration_order_and_once_is_removed_before_call() {
    let mut emitter = EventEmitter::<u8>::new();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let first_calls = Arc::clone(&calls);
    emitter.on("data", move |_| first_calls.lock().unwrap().push(1));
    let once_calls = Arc::clone(&calls);
    emitter.once("data", move |_| once_calls.lock().unwrap().push(2));
    let third_calls = Arc::clone(&calls);
    emitter.on("data", move |_| third_calls.lock().unwrap().push(3));
    assert_eq!(emitter.emit("data", 7), Ok(3));
    assert_eq!(emitter.emit("data", 8), Ok(2));
    assert_eq!(*calls.lock().unwrap(), vec![1, 2, 3, 1, 3]);
}

#[test]
fn off_and_remove_all_are_observable_and_preserve_remaining_order() {
    let mut emitter = EventEmitter::<u8>::new();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let first_calls = Arc::clone(&calls);
    let first = emitter.on("data", move |_| first_calls.lock().unwrap().push(1));
    let second_calls = Arc::clone(&calls);
    emitter.on("data", move |_| second_calls.lock().unwrap().push(2));
    assert!(emitter.off(first));
    assert!(!emitter.off(first));
    assert_eq!(emitter.emit("data", 1), Ok(1));
    assert_eq!(emitter.remove_all_listeners(Some("data")), 1);
    assert_eq!(emitter.listener_count(Some("data")), 0);
    assert_eq!(*calls.lock().unwrap(), vec![2]);
}

#[test]
fn remove_all_listeners_emits_removals_in_reverse_registration_order() {
    let mut emitter = EventEmitter::<u8>::new();
    let removed = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&removed);
    emitter.on("removeListener", move |event| {
        observed.lock().unwrap().push(event.listener_id.unwrap());
    });
    emitter.on("data", |_| {});
    emitter.on("data", |_| {});
    emitter.on("data", |_| {});
    assert_eq!(emitter.remove_all_listeners(Some("data")), 3);
    assert_eq!(*removed.lock().unwrap(), vec![3, 2, 1]);
}

#[test]
fn meta_events_report_addition_and_removal() {
    let mut emitter = EventEmitter::<u8>::new();
    let lifecycle = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&lifecycle);
    emitter.on("newListener", move |event| {
        observed
            .lock()
            .unwrap()
            .push((event.lifecycle, event.listener_id));
    });
    let observed = Arc::clone(&lifecycle);
    let id = emitter.on("data", move |_| {
        observed
            .lock()
            .unwrap()
            .push((EventLifecycle::Emitted, None));
    });
    let observed = Arc::clone(&lifecycle);
    emitter.on("removeListener", move |event| {
        observed
            .lock()
            .unwrap()
            .push((event.lifecycle, event.listener_id));
    });
    assert!(emitter.off(id));
    let values = lifecycle.lock().unwrap();
    assert_eq!(values[0].0, EventLifecycle::NewListener);
    assert_eq!(values[1].0, EventLifecycle::NewListener);
    assert_eq!(values[2].0, EventLifecycle::RemoveListener);
}

#[test]
fn error_without_listener_is_explicit_and_close_is_terminal() {
    let mut emitter = EventEmitter::<u8>::new();
    assert!(!emitter.emit_checked("data", 0));
    assert!(!emitter.emit_checked("error", 1));
    assert_eq!(
        emitter.emit("error", 1),
        Err(EventEmitterError::UnhandledError)
    );
    assert_eq!(emitter.close(), Ok(()));
    assert!(emitter.is_closed());
    assert_eq!(emitter.emit("data", 1), Err(EventEmitterError::Closed));
    assert_eq!(emitter.close(), Err(EventEmitterError::AlreadyClosed));
}

#[test]
fn emit_checked_reports_delivery_for_non_error_events_and_once_listeners() {
    let mut emitter = EventEmitter::<u8>::new();
    assert!(!emitter.emit_checked("data", 1));
    emitter.once("data", |_| {});
    assert!(emitter.emit_checked("data", 2));
    assert!(!emitter.emit_checked("data", 3));
}

#[test]
fn emit_checked_reports_error_delivery_without_hiding_emit_errors() {
    let mut emitter = EventEmitter::<u8>::new();
    emitter.on("error", |_| {});
    assert!(emitter.emit_checked("error", 1));
    assert_eq!(emitter.emit("error", 2), Ok(1));
    emitter.close().unwrap();
    assert!(!emitter.emit_checked("error", 3));
    assert_eq!(emitter.emit("error", 4), Err(EventEmitterError::Closed));
}

#[test]
fn end_is_emitted_before_closed_state_and_close_is_once() {
    let mut emitter = EventEmitter::<u8>::new();
    let states = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&states);
    emitter.on("end", move |event| {
        observed.lock().unwrap().push((event.lifecycle, false));
    });
    let observed = Arc::clone(&states);
    emitter.once("end", move |event| {
        observed.lock().unwrap().push((event.lifecycle, true));
    });
    assert_eq!(emitter.close(), Ok(()));
    assert_eq!(states.lock().unwrap().len(), 2);
    assert_eq!(emitter.close(), Err(EventEmitterError::AlreadyClosed));
}

#[test]
fn once_removal_emits_remove_listener_before_callback() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut emitter = EventEmitter::<u8>::new();
    let remove_observed = Arc::clone(&observed);
    emitter.on("removeListener", move |notification| {
        remove_observed.lock().unwrap().push((
            notification.lifecycle,
            notification.target_name.map(str::to_owned),
        ));
    });
    let callback_observed = Arc::clone(&observed);
    let listener_id = emitter.once("data", move |_| {
        callback_observed
            .lock()
            .unwrap()
            .push((EventLifecycle::Emitted, None));
    });

    assert_eq!(emitter.emit("data", 1), Ok(1));
    assert_eq!(emitter.listeners(Some("data")), Vec::<u64>::new());
    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            (EventLifecycle::RemoveListener, Some("data".to_owned())),
            (EventLifecycle::Emitted, None),
        ]
    );
    assert!(!emitter.remove_listener(listener_id));
}

#[test]
fn public_event_name_surfaces_expose_node_names_and_rust_extensions() {
    assert_eq!(CONNECTION_EVENT_NAMES.len(), 19);
    assert_eq!(REQUEST_EVENT_NAMES.len(), 13);
    assert!(CONNECTION_EVENT_NAMES.contains(&"newListener"));
    assert!(CONNECTION_EVENT_NAMES.contains(&"removeListener"));
    assert!(REQUEST_EVENT_NAMES.contains(&"doneProc"));
    assert!(REQUEST_EVENT_NAMES.contains(&"doneInProc"));
    assert!(REQUEST_EVENT_NAMES.contains(&"returnValue"));
}
