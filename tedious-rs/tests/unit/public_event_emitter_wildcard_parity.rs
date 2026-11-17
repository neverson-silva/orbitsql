use std::sync::{Arc, Mutex};

use tedious_rs::EventEmitter;

#[test]
fn wildcard_is_a_literal_event_name() {
    let mut emitter = EventEmitter::<u8>::new();
    let observed = Arc::new(Mutex::new(Vec::new()));

    let wildcard_observed = Arc::clone(&observed);
    emitter.on("*", move |event| {
        wildcard_observed
            .lock()
            .unwrap()
            .push((event.name.to_owned(), event.payload.copied()));
    });
    let named_observed = Arc::clone(&observed);
    emitter.on("event", move |event| {
        named_observed
            .lock()
            .unwrap()
            .push((event.name.to_owned(), event.payload.copied()));
    });

    assert_eq!(emitter.emit("event", 1), Ok(1));
    assert_eq!(emitter.emit("*", 2), Ok(1));
    assert_eq!(
        *observed.lock().unwrap(),
        vec![("event".to_owned(), Some(1)), ("*".to_owned(), Some(2))]
    );
}
