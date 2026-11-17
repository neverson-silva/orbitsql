use std::sync::{Arc, Mutex};
use std::time::Duration;

use tedious_rs::connection::{
    Connection, ConnectionConfig, ConnectionEvent, ConnectionState, dispatch_request_tokens,
};
use tedious_rs::request::{Request, RequestEvent};
use tedious_rs::token::parser::{DoneToken, Token};
use tedious_rs::token::stream::StreamToken;

fn done(token: u8) -> StreamToken {
    StreamToken::Control(Token::Done(DoneToken {
        token,
        more: false,
        sql_error: false,
        attention: false,
        server_error: false,
        row_count: Some(0),
        cur_cmd: 0,
    }))
}

#[test]
fn done_events_use_empty_arrays_when_done_row_collection_is_disabled() {
    let mut request = Request::new("SELECT 1", |_| {});
    request.begin().unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&events);
    request.on(move |event| observed.lock().unwrap().push(event.clone()));

    dispatch_request_tokens(
        &mut request,
        vec![done(0xFE), done(0xFF), done(0xFD)],
        false,
        false,
        false,
    );

    let events = events.lock().unwrap();
    assert!(matches!(
        &events[0],
        RequestEvent::DoneProc { rows: Some(rows), .. } if rows.is_empty()
    ));
    assert!(matches!(
        &events[1],
        RequestEvent::DoneInProc { rows: Some(rows), .. } if rows.is_empty()
    ));
    assert!(matches!(
        &events[2],
        RequestEvent::Done { rows: Some(rows), .. } if rows.is_empty()
    ));
}

#[tokio::test]
async fn prepared_operations_without_handle_follow_connection_validation() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    let mut request = Request::new("SELECT 1", |_| {});

    let execute_error = connection.execute_prepared(&mut request).await.unwrap_err();
    assert_eq!(execute_error.code.as_deref(), Some("ENOTCONNECTED"));

    let unprepare_error = connection.unprepare(&mut request).await.unwrap_err();
    assert_eq!(unprepare_error.code.as_deref(), Some("ENOTCONNECTED"));
}

#[tokio::test]
async fn connection_failure_finishes_cleanup_before_callback() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let listener_observed = Arc::clone(&observed);
    let mut connection = Connection::new(ConnectionConfig::sql("127.0.0.1", "sa", "secret"));
    connection.config.port = 1;
    connection.config.connect_timeout = Duration::from_millis(20);
    connection.on(move |event| match event {
        ConnectionEvent::ConnectError(_) => listener_observed.lock().unwrap().push("connect"),
        ConnectionEvent::End => listener_observed.lock().unwrap().push("end"),
        _ => {}
    });
    let callback_observed = Arc::clone(&observed);

    let result = connection
        .connect_with_callback(move |_| callback_observed.lock().unwrap().push("callback"))
        .await;

    assert!(result.is_err());
    assert_eq!(connection.state, ConnectionState::Final);
    assert_eq!(
        *observed.lock().unwrap(),
        vec!["connect", "callback", "end"]
    );
}

#[tokio::test]
async fn connection_failure_emits_connect_event_with_error_payload() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let listener_observed = Arc::clone(&observed);
    let mut connection = Connection::new(ConnectionConfig::sql("127.0.0.1", "sa", "secret"));
    connection.config.port = 1;
    connection.config.connect_timeout = Duration::from_millis(20);
    connection.on_named("connect", move |event| {
        if let ConnectionEvent::ConnectError(error) = event {
            listener_observed
                .lock()
                .unwrap()
                .push((error.code.clone(), error.message.clone()));
        }
    });

    assert!(connection.connect().await.is_err());
    let observed = observed.lock().unwrap();
    assert_eq!(observed.len(), 1);
    assert!(observed[0].0.is_some());
    assert!(!observed[0].1.is_empty());
}
