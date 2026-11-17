use std::sync::{Arc, Mutex};

use tedious_rs::bulk_load::{BulkLoad, BulkLoadEvent, BulkLoadOptions};

#[test]
fn failure_completes_callback_without_emitting_error_event() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let callback_observed = Arc::clone(&observed);
    let mut bulk_load =
        BulkLoad::new_with_callback("dbo.items", BulkLoadOptions::default(), move |result| {
            callback_observed
                .lock()
                .unwrap()
                .push(format!("callback:{result:?}"));
        })
        .unwrap();
    let event_observed = Arc::clone(&observed);
    bulk_load.on(move |event| {
        event_observed
            .lock()
            .unwrap()
            .push(format!("event:{event:?}"));
    });

    assert!(bulk_load.fail("schema mismatch"));
    assert!(!bulk_load.fail("second failure"));
    assert_eq!(
        *observed.lock().unwrap(),
        vec!["callback:Err(\"schema mismatch\")"]
    );
}

#[test]
fn cancellation_emits_cancel_before_later_failure_callback_once() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let callback_observed = Arc::clone(&observed);
    let mut bulk_load =
        BulkLoad::new_with_callback("dbo.items", BulkLoadOptions::default(), move |result| {
            callback_observed
                .lock()
                .unwrap()
                .push(format!("callback:{result:?}"));
        })
        .unwrap();
    let event_observed = Arc::clone(&observed);
    bulk_load.on(move |event| {
        if matches!(event, BulkLoadEvent::Cancel) {
            event_observed
                .lock()
                .unwrap()
                .push("event:cancel".to_owned());
        }
    });

    bulk_load.cancel();
    bulk_load.cancel();
    assert_eq!(*observed.lock().unwrap(), vec!["event:cancel"]);
    assert!(bulk_load.fail("Canceled."));
    assert_eq!(
        *observed.lock().unwrap(),
        vec!["event:cancel", "callback:Err(\"Canceled.\")"]
    );
    assert!(bulk_load.canceled);
    assert!(!bulk_load.complete(1));
}

#[test]
fn finish_selects_one_terminal_callback_result() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let callback_calls = Arc::clone(&calls);
    let mut bulk_load =
        BulkLoad::new_with_callback("dbo.items", BulkLoadOptions::default(), move |result| {
            callback_calls.lock().unwrap().push(result)
        })
        .unwrap();

    assert!(bulk_load.finish(Ok(4)));
    assert!(!bulk_load.fail("late failure"));
    assert!(!bulk_load.finish(Ok(5)));
    assert_eq!(*calls.lock().unwrap(), vec![Ok(4)]);
}

#[tokio::test]
async fn cancellation_handle_wakes_the_active_bulk_lifecycle() {
    let mut bulk_load = BulkLoad::new("dbo.items", BulkLoadOptions::default()).unwrap();
    let cancellation = bulk_load.cancellation_handle();
    let waiter = cancellation.clone();
    let waiting = tokio::spawn(async move {
        waiter.canceled().await;
        waiter.is_canceled()
    });
    tokio::task::yield_now().await;
    bulk_load.cancel();
    assert!(waiting.await.unwrap());
    assert!(cancellation.is_canceled());
}
