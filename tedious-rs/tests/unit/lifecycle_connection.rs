use std::future::pending;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tedious_rs::connection::{
    Connection, ConnectionConfig, ConnectionEvent, ConnectionState, attention_acknowledged,
};
use tedious_rs::token::parser::{DoneToken, Token};
use tedious_rs::token::stream::StreamToken;

fn done(attention: bool) -> StreamToken {
    StreamToken::Control(Token::Done(DoneToken {
        token: 0xFD,
        more: false,
        sql_error: false,
        attention,
        server_error: false,
        row_count: None,
        cur_cmd: 0,
    }))
}

#[tokio::test]
async fn external_close_handle_aborts_connect_and_finishes_once() {
    let mut config = ConnectionConfig::sql("localhost", "user", "password");
    config.connect_timeout = Duration::from_secs(5);
    config.max_retries_on_transient_errors = 0;
    let mut connection = Connection::new(config);
    connection.set_custom_connector(|| async { pending().await });
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    connection.on(move |event| seen.lock().unwrap().push(event.clone()));
    let handle = connection.close_handle();
    let task = tokio::spawn(async move { connection.connect().await });
    tokio::time::sleep(Duration::from_millis(10)).await;
    handle.close();
    let error = task.await.unwrap().unwrap_err();

    assert_eq!(error.code.as_deref(), Some("ECLOSE"));
    let events = events.lock().unwrap();
    let terminal_events = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                ConnectionEvent::ConnectError(_) | ConnectionEvent::End
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(terminal_events.len(), 2);
    assert!(matches!(
        terminal_events[0],
        ConnectionEvent::ConnectError(_)
    ));
    assert_eq!(*terminal_events[1], ConnectionEvent::End);
}

#[test]
fn attention_ack_requires_done_attention_flag() {
    assert!(attention_acknowledged(&[done(true)]));
    assert!(!attention_acknowledged(&[done(false)]));
    assert!(!attention_acknowledged(&[]));
}

#[tokio::test]
async fn close_is_idempotent_after_external_abort() {
    let mut config = ConnectionConfig::sql("localhost", "user", "password");
    config.connect_timeout = Duration::from_secs(5);
    let mut connection = Connection::new(config);
    connection.set_custom_connector(|| async { pending().await });
    let handle = connection.close_handle();
    handle.close();
    let first = connection.connect().await.unwrap_err();
    assert_eq!(first.code.as_deref(), Some("ECLOSE"));
    connection.close().await;
    connection.close().await;
    assert_eq!(connection.state, ConnectionState::Final);
}
