use std::sync::{Arc, Mutex};
use std::time::Duration;

use tedious_rs::connection::{Connection, ConnectionConfig, ConnectionState};
use tedious_rs::message::{Message, packetize};
use tedious_rs::packet::packet_type;
use tedious_rs::prelogin_payload::{PreloginOptions, PreloginPayload, PreloginVersion};
use tedious_rs::request::{Request, RequestEvent, RequestState};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, timeout};

async fn read_message(stream: &mut TcpStream) -> std::io::Result<(u8, Vec<u8>)> {
    let mut data = Vec::new();
    loop {
        let mut header = [0; 8];
        stream.read_exact(&mut header).await?;
        let length = u16::from_be_bytes([header[2], header[3]]) as usize;
        let mut payload = vec![0; length.saturating_sub(8)];
        stream.read_exact(&mut payload).await?;
        data.extend(payload);
        if header[1] & 1 != 0 {
            return Ok((header[0], data));
        }
    }
}

async fn send_message(
    stream: &mut TcpStream,
    message_type: u8,
    data: Vec<u8>,
) -> std::io::Result<()> {
    let message = Message {
        message_type,
        reset_connection: false,
        ignore: false,
        data,
    };
    for packet in packetize(&message, 4096).map_err(|error| std::io::Error::other(error.0))? {
        stream.write_all(&packet.buffer).await?;
    }
    Ok(())
}

async fn send_fragmented(
    stream: &mut TcpStream,
    message_type: u8,
    data: Vec<u8>,
) -> std::io::Result<()> {
    let message = Message {
        message_type,
        reset_connection: false,
        ignore: false,
        data,
    };
    let packet = packetize(&message, 4096)
        .map_err(|error| std::io::Error::other(error.0))?
        .remove(0)
        .buffer;
    for part in packet.chunks(2) {
        stream.write_all(part).await?;
        tokio::task::yield_now().await;
    }
    Ok(())
}

async fn send_nonterminal(
    stream: &mut TcpStream,
    message_type: u8,
    data: Vec<u8>,
) -> std::io::Result<()> {
    let message = Message {
        message_type,
        reset_connection: false,
        ignore: false,
        data,
    };
    let mut packet = packetize(&message, 4096)
        .map_err(|error| std::io::Error::other(error.0))?
        .remove(0)
        .buffer;
    packet[1] &= !1;
    stream.write_all(&packet).await
}

fn prelogin_response() -> Vec<u8> {
    let mut data = PreloginPayload::new(PreloginOptions {
        encrypt: false,
        version: PreloginVersion {
            major: 1,
            minor: 2,
            build: 3,
            subbuild: 0,
        },
    })
    .unwrap()
    .data;
    let mut offset = 0;
    while data[offset] != 0xff {
        let token = data[offset];
        let start = u16::from_be_bytes([data[offset + 1], data[offset + 2]]) as usize;
        let length = u16::from_be_bytes([data[offset + 3], data[offset + 4]]) as usize;
        if token == 0x06 && length == 1 {
            data[start] = 0;
        }
        offset += 5;
    }
    data
}

fn login_ack() -> Vec<u8> {
    let name = "fake"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut token = vec![0xad, 0, 0, 1, 0x74, 0, 0, 4, name.len() as u8 / 2];
    token.extend(name);
    token.extend([0, 0, 0, 0]);
    let length = (token.len() - 3) as u16;
    token[1..3].copy_from_slice(&length.to_le_bytes());
    token
}

fn done(attention: bool) -> Vec<u8> {
    let mut token = vec![0xfd, 0x20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    if !attention {
        token[1] = 0;
    }
    token
}

#[tokio::test]
async fn authenticated_request_cancel_sends_attention_consumes_ack_and_returns_logged_in() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        assert_eq!(
            read_message(&mut stream).await.unwrap().0,
            packet_type::PRELOGIN
        );
        send_message(&mut stream, packet_type::PRELOGIN, prelogin_response())
            .await
            .unwrap();
        assert_eq!(
            read_message(&mut stream).await.unwrap().0,
            packet_type::LOGIN7
        );
        send_message(&mut stream, packet_type::TABULAR_RESULT, login_ack())
            .await
            .unwrap();
        assert_eq!(
            read_message(&mut stream).await.unwrap().0,
            packet_type::SQL_BATCH
        );
        send_message(&mut stream, packet_type::TABULAR_RESULT, done(false))
            .await
            .unwrap();
        assert_eq!(
            read_message(&mut stream).await.unwrap().0,
            packet_type::SQL_BATCH
        );
        send_nonterminal(&mut stream, packet_type::TABULAR_RESULT, done(false))
            .await
            .unwrap();
        assert_eq!(
            read_message(&mut stream).await.unwrap().0,
            packet_type::ATTENTION
        );
        send_fragmented(&mut stream, packet_type::TABULAR_RESULT, done(false))
            .await
            .unwrap();
        send_fragmented(&mut stream, packet_type::TABULAR_RESULT, done(true))
            .await
            .unwrap();
    });

    let mut config = ConnectionConfig::sql("127.0.0.1", "sa", "password");
    config.port = address.port();
    config.encrypt = false;
    config.request_timeout = Duration::from_secs(2);
    let mut connection = Connection::new(config);
    timeout(Duration::from_secs(2), connection.connect())
        .await
        .unwrap()
        .unwrap();

    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observed = Arc::clone(&events);
    let mut request = Request::new("SELECT WAITFOR DELAY '00:00:10'", |_| {});
    request.on(move |event| observed.lock().unwrap().push(event.clone()));
    let cancellation = request.cancellation_handle();
    let error = {
        let execution = connection.exec_sql_batch_request(&mut request);
        tokio::pin!(execution);
        tokio::select! {
            result = &mut execution => panic!("request completed before cancellation: {result:?}"),
            _ = sleep(Duration::from_millis(20)) => cancellation.cancel(),
        }
        execution.await.unwrap_err()
    };
    assert_eq!(error.code.as_deref(), Some("ECANCEL"));
    assert_eq!(connection.state, ConnectionState::LoggedIn, "{error:?}");
    assert!(matches!(
        request.state,
        RequestState::Canceled | RequestState::Completed
    ));
    assert!(
        events
            .lock()
            .unwrap()
            .iter()
            .any(|event| matches!(event, RequestEvent::Cancel))
    );
    assert!(
        events
            .lock()
            .unwrap()
            .iter()
            .any(|event| matches!(event, RequestEvent::RequestCompleted))
    );
    connection.close().await;
    server.await.unwrap();
}

#[tokio::test]
async fn closing_during_an_active_request_completes_it_with_eclose() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        assert_eq!(
            read_message(&mut stream).await.unwrap().0,
            packet_type::PRELOGIN
        );
        send_message(&mut stream, packet_type::PRELOGIN, prelogin_response())
            .await
            .unwrap();
        assert_eq!(
            read_message(&mut stream).await.unwrap().0,
            packet_type::LOGIN7
        );
        send_message(&mut stream, packet_type::TABULAR_RESULT, login_ack())
            .await
            .unwrap();
        assert_eq!(
            read_message(&mut stream).await.unwrap().0,
            packet_type::SQL_BATCH
        );
        send_message(&mut stream, packet_type::TABULAR_RESULT, done(false))
            .await
            .unwrap();
        assert_eq!(
            read_message(&mut stream).await.unwrap().0,
            packet_type::SQL_BATCH
        );
        sleep(Duration::from_millis(100)).await;
    });

    let mut config = ConnectionConfig::sql("127.0.0.1", "sa", "password");
    config.port = address.port();
    config.encrypt = false;
    let mut connection = Connection::new(config);
    connection.connect().await.unwrap();
    let handle = connection.close_handle();
    let order = Arc::new(Mutex::new(Vec::new()));
    let connection_order = Arc::clone(&order);
    connection.on(move |event| {
        if matches!(event, tedious_rs::connection::ConnectionEvent::End) {
            connection_order.lock().unwrap().push("end");
        }
    });
    let request_order = Arc::clone(&order);
    let mut request = Request::new("SELECT WAITFOR DELAY '00:00:10'", move |result| {
        assert_eq!(result.unwrap_err().code.as_deref(), Some("ECLOSE"));
        request_order.lock().unwrap().push("callback");
    });
    let error = {
        let execution = connection.exec_sql_batch_request(&mut request);
        tokio::pin!(execution);
        tokio::select! {
            result = &mut execution => panic!("request completed before close: {result:?}"),
            _ = sleep(Duration::from_millis(20)) => handle.close(),
        }
        execution.await.unwrap_err()
    };
    assert_eq!(error.code.as_deref(), Some("ECLOSE"));
    assert_eq!(request.state, RequestState::Failed);
    assert_eq!(connection.state, ConnectionState::Final);
    assert_eq!(*order.lock().unwrap(), vec!["callback", "end"]);
    server.await.unwrap();
}

#[tokio::test]
async fn request_timeout_cancel_timeout_orders_callback_before_end() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        assert_eq!(
            read_message(&mut stream).await.unwrap().0,
            packet_type::PRELOGIN
        );
        send_message(&mut stream, packet_type::PRELOGIN, prelogin_response())
            .await
            .unwrap();
        assert_eq!(
            read_message(&mut stream).await.unwrap().0,
            packet_type::LOGIN7
        );
        send_message(&mut stream, packet_type::TABULAR_RESULT, login_ack())
            .await
            .unwrap();
        assert_eq!(
            read_message(&mut stream).await.unwrap().0,
            packet_type::SQL_BATCH
        );
        send_message(&mut stream, packet_type::TABULAR_RESULT, done(false))
            .await
            .unwrap();
        assert_eq!(
            read_message(&mut stream).await.unwrap().0,
            packet_type::SQL_BATCH
        );
        assert_eq!(
            timeout(Duration::from_secs(1), read_message(&mut stream))
                .await
                .unwrap()
                .unwrap()
                .0,
            packet_type::ATTENTION
        );
    });
    let mut config = ConnectionConfig::sql("127.0.0.1", "sa", "password");
    config.port = address.port();
    config.encrypt = false;
    config.request_timeout = Duration::from_millis(20);
    config.cancel_timeout = Duration::from_millis(20);
    let mut connection = Connection::new(config);
    connection.connect().await.unwrap();
    let order = Arc::new(Mutex::new(Vec::new()));
    let connection_order = Arc::clone(&order);
    connection.on(move |event| {
        if matches!(event, tedious_rs::connection::ConnectionEvent::End) {
            connection_order.lock().unwrap().push("end");
        }
    });
    let request_order = Arc::clone(&order);
    let mut request = Request::new("SELECT 1", move |result| {
        assert_eq!(result.unwrap_err().code.as_deref(), Some("ETIMEOUT"));
        request_order.lock().unwrap().push("callback");
    });
    let result = connection.exec_sql_batch_request(&mut request).await;
    assert_eq!(result.unwrap_err().code.as_deref(), Some("ETIMEOUT"));
    assert_eq!(*order.lock().unwrap(), vec!["callback", "end"]);
    assert_eq!(connection.state, ConnectionState::Final);
    server.await.unwrap();
}
