use std::time::Duration;

use tedious_rs::connection::{Connection, ConnectionConfig};
use tedious_rs::message::{Message, packetize};
use tedious_rs::packet::packet_type;
use tedious_rs::prelogin_payload::{PreloginOptions, PreloginPayload, PreloginVersion};
use tedious_rs::request::Request;
use tedious_rs::token::stream::{TokenStreamParser, TokenStreamState};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, timeout};

async fn read_message(stream: &mut TcpStream) -> std::io::Result<u8> {
    loop {
        let mut header = [0; 8];
        stream.read_exact(&mut header).await?;
        let length = u16::from_be_bytes([header[2], header[3]]) as usize;
        let mut payload = vec![0; length.saturating_sub(8)];
        stream.read_exact(&mut payload).await?;
        if header[1] & 1 != 0 {
            return Ok(header[0]);
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

fn done() -> Vec<u8> {
    vec![0xfd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
}

#[test]
fn request_pause_resume_reprocesses_buffered_parser_bytes() {
    let mut parser = TokenStreamParser::new("7_4", false);
    parser.pause();
    let payload = done();

    assert!(parser.feed(&payload).unwrap().is_empty());
    assert_eq!(parser.state(), &TokenStreamState::Paused);

    let tokens = parser.resume().unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(parser.state(), &TokenStreamState::Reading);
}

#[tokio::test]
async fn paused_request_suspends_connection_read_without_epaused() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        assert_eq!(
            read_message(&mut stream).await.unwrap(),
            packet_type::PRELOGIN
        );
        send_message(&mut stream, packet_type::PRELOGIN, prelogin_response())
            .await
            .unwrap();
        assert_eq!(
            read_message(&mut stream).await.unwrap(),
            packet_type::LOGIN7
        );
        send_message(&mut stream, packet_type::TABULAR_RESULT, login_ack())
            .await
            .unwrap();
        assert_eq!(
            read_message(&mut stream).await.unwrap(),
            packet_type::SQL_BATCH
        );
        send_message(&mut stream, packet_type::TABULAR_RESULT, done())
            .await
            .unwrap();
        assert_eq!(
            read_message(&mut stream).await.unwrap(),
            packet_type::SQL_BATCH
        );
        send_message(&mut stream, packet_type::TABULAR_RESULT, done())
            .await
            .unwrap();
        sleep(Duration::from_millis(100)).await;
    });

    let mut config = ConnectionConfig::sql("127.0.0.1", "sa", "password");
    config.port = address.port();
    config.encrypt = false;
    config.request_timeout = Duration::from_secs(2);
    let mut connection = Connection::new(config);
    connection.connect().await.unwrap();

    let mut request = Request::new("SELECT 1", |_| {});
    request.pause();
    let close_handle = connection.close_handle();
    let error = {
        let execution = connection.exec_sql_batch_request(&mut request);
        tokio::pin!(execution);
        tokio::select! {
            result = &mut execution => panic!("paused request completed unexpectedly: {result:?}"),
            _ = sleep(Duration::from_millis(20)) => close_handle.close(),
        }
        timeout(Duration::from_secs(1), execution)
            .await
            .unwrap()
            .unwrap_err()
    };

    assert_eq!(error.code.as_deref(), Some("ECLOSE"));
    assert_ne!(error.code.as_deref(), Some("EPAUSED"));
    connection.close().await;
    server.await.unwrap();
}
