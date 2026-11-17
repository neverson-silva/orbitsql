use std::sync::{Arc, Mutex};
use std::time::Duration;

use tedious_rs::connection::{Connection, ConnectionConfig, ConnectionEvent};
use tedious_rs::message::{Message, packetize};
use tedious_rs::packet::packet_type;
use tedious_rs::prelogin_payload::{PreloginOptions, PreloginPayload, PreloginVersion};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn read_message(stream: &mut tokio::net::TcpStream) -> std::io::Result<(u8, Vec<u8>)> {
    let mut header = [0; 8];
    tokio::time::timeout(Duration::from_secs(2), stream.read_exact(&mut header))
        .await
        .map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::TimedOut, "message header timeout")
        })??;
    let length = u16::from_be_bytes([header[2], header[3]]) as usize;
    let mut payload = vec![0; length.saturating_sub(8)];
    stream.read_exact(&mut payload).await?;
    Ok((header[0], payload))
}

async fn send_message(
    stream: &mut tokio::net::TcpStream,
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

fn prelogin_response(encryption: u8) -> Vec<u8> {
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
        if token == 0x01 && length == 1 {
            data[start] = encryption;
        }
        if token == 0x06 && length == 1 {
            data[start] = 0;
        }
        offset += 5;
    }
    data
}

#[tokio::test]
async fn encrypt_true_with_server_off_sends_login7_without_tls() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (message_type, payload) = read_message(&mut stream).await.unwrap();
        assert_eq!(message_type, packet_type::PRELOGIN);
        let prelogin = PreloginPayload::from_bytes(payload).unwrap();
        assert_eq!(
            prelogin.version,
            PreloginVersion {
                major: 0,
                minor: 0,
                build: 0,
                subbuild: 0,
            }
        );
        send_message(&mut stream, packet_type::PRELOGIN, prelogin_response(0))
            .await
            .unwrap();
        read_message(&mut stream).await.unwrap().0
    });

    let mut config = ConnectionConfig::sql("127.0.0.1", "sa", "password");
    config.port = address.port();
    config.connect_timeout = Duration::from_secs(2);
    config.max_retries_on_transient_errors = 0;
    let mut connection = Connection::new(config);
    let events = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&events);
    connection.on(move |event| observed.lock().unwrap().push(event.clone()));
    assert!(connection.connect().await.is_err());
    assert_eq!(server.await.unwrap(), packet_type::LOGIN7);
    assert!(
        !events
            .lock()
            .unwrap()
            .iter()
            .any(|event| matches!(event, ConnectionEvent::Secure))
    );
}
