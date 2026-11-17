use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tedious_rs::connection::{
    Authentication, Connection, ConnectionConfig, ConnectionEvent, ConnectionState, TokenProvider,
};
use tedious_rs::errors::ConnectionError;
use tedious_rs::message::{Message, packetize};
use tedious_rs::packet::packet_type;
use tedious_rs::prelogin_payload::{PreloginOptions, PreloginPayload, PreloginVersion};
use tedious_rs::token::parser::{EnvChangeKind, Token, parse_token};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

async fn read_message(stream: &mut TcpStream) -> std::io::Result<(u8, Vec<u8>)> {
    let mut data = Vec::new();
    loop {
        let mut header = [0; 8];
        timeout(Duration::from_secs(2), stream.read_exact(&mut header))
            .await
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "fake server read timed out")
            })??;
        let length = u16::from_be_bytes([header[2], header[3]]) as usize;
        let mut payload = vec![0; length.saturating_sub(8)];
        timeout(Duration::from_secs(2), stream.read_exact(&mut payload))
            .await
            .map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "fake server payload read timed out",
                )
            })??;
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
    let mut token = vec![0xAD, 0, 0, 1, 0x74, 0, 0, 4, name.len() as u8 / 2];
    token.extend(name);
    token.extend([0, 0, 0, 0]);
    let length = (token.len() - 3) as u16;
    token[1..3].copy_from_slice(&length.to_le_bytes());
    token
}

fn ntlm_challenge_token() -> Vec<u8> {
    let mut challenge = vec![0u8; 56];
    challenge[0..8].copy_from_slice(b"NTLMSSP\0");
    challenge[8..12].copy_from_slice(&2u32.to_le_bytes());
    challenge[24..32].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    challenge[16..20].copy_from_slice(&56u32.to_le_bytes());
    challenge[44..48].copy_from_slice(&56u32.to_le_bytes());
    let mut token = vec![0xED];
    token.extend((challenge.len() as u16).to_le_bytes());
    token.extend(challenge);
    token
}

fn fed_auth_info_token() -> Vec<u8> {
    let sts_url = "https://login.example.test/";
    let spn = "https://database.example.test";
    let sts_url = sts_url
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let spn = spn
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let sts_offset = 4 + 18;
    let spn_offset = sts_offset + sts_url.len();
    let mut payload = Vec::new();
    payload.extend(2u32.to_le_bytes());
    payload.extend([1]);
    payload.extend((sts_url.len() as u32).to_le_bytes());
    payload.extend((sts_offset as u32).to_le_bytes());
    payload.extend([2]);
    payload.extend((spn.len() as u32).to_le_bytes());
    payload.extend((spn_offset as u32).to_le_bytes());
    payload.extend(sts_url);
    payload.extend(spn);
    let mut token = vec![0xEE];
    token.extend((payload.len() as u32).to_le_bytes());
    token.extend(payload);
    token
}

struct FixedFedAuthProvider;

impl TokenProvider for FixedFedAuthProvider {
    fn get_token(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<String, ConnectionError>> + Send + '_>> {
        Box::pin(async { Ok("fed-auth-token".to_owned()) })
    }
}

fn error_token(number: u32) -> Vec<u8> {
    let text = "transient"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut token = vec![0xAA, 0, 0];
    token.extend(number.to_le_bytes());
    token.extend([0, 20]);
    token.extend((text.len() as u16 / 2).to_le_bytes());
    token.extend(text);
    token.push(0);
    token.push(0);
    token.extend([0, 0, 0, 0]);
    let length = (token.len() - 3) as u16;
    token[1..3].copy_from_slice(&length.to_le_bytes());
    token
}

fn routing_token(server: &str, port: u16, instance: &str) -> Vec<u8> {
    let target = format!("{server}\\{instance}");
    let encoded = target
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut value = vec![0, (port & 0xff) as u8, (port >> 8) as u8];
    value.extend((target.encode_utf16().count() as u16).to_le_bytes());
    value.extend(encoded);
    let mut data = vec![20];
    data.extend((value.len() as u16).to_le_bytes());
    data.extend(value);
    data.extend([0, 0]);
    let mut token = vec![0xE3];
    token.extend((data.len() as u16).to_le_bytes());
    token.extend(data);
    token
}

#[test]
fn parses_routing_server_and_instance_without_lossy_normalization() {
    let token = routing_token("redirect.example", 1444, "instanceA");
    let parsed = parse_token(&token, 0, "7_4").unwrap();
    let Token::EnvChange(change) = parsed.value else {
        panic!("routing token was not parsed as ENVCHANGE");
    };
    let EnvChangeKind::Routing(route) = change.kind else {
        panic!("ENVCHANGE was not parsed as routing");
    };
    assert_eq!(
        route.server_and_instance(),
        ("redirect.example".to_owned(), Some("instanceA".to_owned()))
    );
    assert_eq!(route.port, 1444);
}

async fn login_sequence(stream: &mut TcpStream) -> std::io::Result<()> {
    let (kind, _) = read_message(stream).await?;
    assert_eq!(kind, packet_type::PRELOGIN);
    send_message(stream, packet_type::PRELOGIN, prelogin_response()).await?;
    let (kind, _) = read_message(stream).await?;
    assert_eq!(kind, packet_type::LOGIN7);
    Ok(())
}

async fn success_sequence(mut stream: TcpStream) -> std::io::Result<()> {
    login_sequence(&mut stream).await?;
    let mut response = login_ack();
    send_message(
        &mut stream,
        packet_type::TABULAR_RESULT,
        std::mem::take(&mut response),
    )
    .await?;
    let _ = read_message(&mut stream).await?;
    send_message(&mut stream, packet_type::TABULAR_RESULT, Vec::new()).await
}

#[tokio::test]
async fn ntlm_login_reads_challenge_before_sending_response_and_accepts_optional_domain() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (kind, _) = read_message(&mut stream).await.unwrap();
        assert_eq!(kind, packet_type::PRELOGIN);
        send_message(&mut stream, packet_type::PRELOGIN, prelogin_response())
            .await
            .unwrap();
        let (kind, _) = read_message(&mut stream).await.unwrap();
        assert_eq!(kind, packet_type::LOGIN7);
        send_message(
            &mut stream,
            packet_type::TABULAR_RESULT,
            ntlm_challenge_token(),
        )
        .await
        .unwrap();
        let (kind, response) = read_message(&mut stream).await.unwrap();
        assert_eq!(kind, packet_type::NTLMAUTH_PKT);
        assert!(response.starts_with(b"NTLMSSP\0"));
        send_message(&mut stream, packet_type::TABULAR_RESULT, login_ack())
            .await
            .unwrap();
        let (kind, _) = read_message(&mut stream).await.unwrap();
        assert_eq!(kind, packet_type::SQL_BATCH);
        send_message(&mut stream, packet_type::TABULAR_RESULT, Vec::new())
            .await
            .unwrap();
    });

    let mut config = ConnectionConfig::ntlm("127.0.0.1", "user", "password", Some(String::new()));
    config.port = address.port();
    config.encrypt = false;
    config.connect_timeout = Duration::from_secs(2);
    let mut connection = Connection::new(config);
    timeout(Duration::from_secs(5), connection.connect())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(connection.state, ConnectionState::LoggedIn);
    connection.close().await;
    server.await.unwrap();
}

#[tokio::test]
async fn fedauth_login_reads_info_sends_token_and_completes_login() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (kind, _) = read_message(&mut stream).await.unwrap();
        assert_eq!(kind, packet_type::PRELOGIN);
        send_message(&mut stream, packet_type::PRELOGIN, prelogin_response())
            .await
            .unwrap();
        let (kind, _) = read_message(&mut stream).await.unwrap();
        assert_eq!(kind, packet_type::LOGIN7);
        send_message(
            &mut stream,
            packet_type::TABULAR_RESULT,
            fed_auth_info_token(),
        )
        .await
        .unwrap();
        let (kind, token) = read_message(&mut stream).await.unwrap();
        assert_eq!(kind, packet_type::FEDAUTH_TOKEN);
        let expected_token = "fed-auth-token"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert!(
            token
                .windows(expected_token.len())
                .any(|window| window == expected_token)
        );
        send_message(&mut stream, packet_type::TABULAR_RESULT, login_ack())
            .await
            .unwrap();
        let (kind, _) = read_message(&mut stream).await.unwrap();
        assert_eq!(kind, packet_type::SQL_BATCH);
        send_message(&mut stream, packet_type::TABULAR_RESULT, Vec::new())
            .await
            .unwrap();
    });

    let mut config = ConnectionConfig::sql("127.0.0.1", "user", "password");
    config.authentication = Authentication::AzureActiveDirectoryTokenCredential;
    config.port = address.port();
    config.encrypt = false;
    config.connect_timeout = Duration::from_secs(2);
    let mut connection = Connection::new(config);
    connection.set_token_provider(FixedFedAuthProvider);
    timeout(Duration::from_secs(5), connection.connect())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(connection.state, ConnectionState::LoggedIn);
    connection.close().await;
    server.await.unwrap();
}

#[tokio::test]
async fn retries_transient_login_failure_and_emits_one_retry_per_new_attempt() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let attempts = Arc::new(Mutex::new(0usize));
    let server_attempts = Arc::clone(&attempts);
    let server = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let attempt = {
                let mut value = server_attempts.lock().unwrap();
                *value += 1;
                *value
            };
            tokio::spawn(async move {
                if login_sequence(&mut stream).await.is_err() {
                    return;
                }
                let data = if attempt <= 2 {
                    error_token(4060)
                } else {
                    login_ack()
                };
                if send_message(&mut stream, packet_type::TABULAR_RESULT, data)
                    .await
                    .is_ok()
                    && attempt > 2
                {
                    let _ = read_message(&mut stream).await;
                    let _ =
                        send_message(&mut stream, packet_type::TABULAR_RESULT, Vec::new()).await;
                }
            });
        }
    });

    let mut config = ConnectionConfig::sql("127.0.0.1", "sa", "password");
    config.port = address.port();
    config.encrypt = false;
    config.connect_timeout = Duration::from_secs(2);
    config.connection_retry_interval = Duration::from_millis(1);
    config.max_retries_on_transient_errors = 2;
    config.debug.payload = true;
    let mut connection = Connection::new(config);
    let events = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&events);
    connection.on(move |event| observed.lock().unwrap().push(event.clone()));

    timeout(Duration::from_secs(5), connection.connect())
        .await
        .unwrap_or_else(|_| {
            panic!(
                "retry test timed out after {} attempts",
                attempts.lock().unwrap()
            )
        })
        .unwrap();

    let retry_count = events
        .lock()
        .unwrap()
        .iter()
        .filter(|event| matches!(event, ConnectionEvent::Retry))
        .count();
    assert_eq!(retry_count, 2);
    assert_eq!(*attempts.lock().unwrap(), 3);
    assert_eq!(connection.state, ConnectionState::LoggedIn);
    let state_changes = events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            ConnectionEvent::Debug(message) if message.starts_with("State change: ") => {
                Some(message.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let state_names = state_changes
        .iter()
        .map(|message| message.trim_start_matches("State change: ").to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        state_names,
        vec![
            "Initialized -> Connecting",
            "Connecting -> SentPrelogin",
            "SentPrelogin -> SentLogin7WithStandardLogin",
            "SentLogin7WithStandardLogin -> TRANSIENT_FAILURE_RETRY",
            "TRANSIENT_FAILURE_RETRY -> Connecting",
            "Connecting -> SentPrelogin",
            "SentPrelogin -> SentLogin7WithStandardLogin",
            "SentLogin7WithStandardLogin -> TRANSIENT_FAILURE_RETRY",
            "TRANSIENT_FAILURE_RETRY -> Connecting",
            "Connecting -> SentPrelogin",
            "SentPrelogin -> SentLogin7WithStandardLogin",
            "SentLogin7WithStandardLogin -> LoggedInSendingInitialSql",
            "LoggedInSendingInitialSql -> LoggedIn",
        ]
    );
    assert!(
        state_changes
            .iter()
            .any(|message| message == "State change: Connecting -> SentPrelogin")
    );
    assert_eq!(
        state_changes
            .iter()
            .filter(|message| message.ends_with("-> TRANSIENT_FAILURE_RETRY"))
            .count(),
        2
    );
    assert_eq!(
        state_changes
            .iter()
            .filter(|message| message.starts_with("State change: TRANSIENT_FAILURE_RETRY ->"))
            .count(),
        2
    );
    assert!(
        state_changes
            .iter()
            .any(|message| message == "State change: LoggedInSendingInitialSql -> LoggedIn")
    );
    assert!(!events.lock().unwrap().iter().any(|event| {
        matches!(event, ConnectionEvent::Debug(message) if message.contains("-> Initialized"))
    }));
    connection.close().await;
    server.abort();
}

#[tokio::test]
async fn stops_after_retry_budget_and_emits_error_then_end() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let attempts = Arc::new(Mutex::new(0usize));
    let server_attempts = Arc::clone(&attempts);
    let server = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            *server_attempts.lock().unwrap() += 1;
            tokio::spawn(async move {
                if login_sequence(&mut stream).await.is_ok() {
                    let _ =
                        send_message(&mut stream, packet_type::TABULAR_RESULT, error_token(4060))
                            .await;
                }
            });
        }
    });

    let mut config = ConnectionConfig::sql("127.0.0.1", "sa", "password");
    config.port = address.port();
    config.encrypt = false;
    config.connect_timeout = Duration::from_secs(2);
    config.connection_retry_interval = Duration::from_millis(1);
    config.max_retries_on_transient_errors = 1;
    let mut connection = Connection::new(config);
    let events = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&events);
    connection.on(move |event| observed.lock().unwrap().push(event.clone()));

    let error = connection.connect().await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ELOGIN"));
    assert_eq!(*attempts.lock().unwrap(), 2);
    let events = events.lock().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, ConnectionEvent::Retry))
            .count(),
        1
    );
    let retry_index = events
        .iter()
        .position(|event| matches!(event, ConnectionEvent::Retry))
        .unwrap();
    let error_index = events
        .iter()
        .position(|event| matches!(event, ConnectionEvent::ConnectError(_)))
        .unwrap();
    let end_index = events
        .iter()
        .position(|event| matches!(event, ConnectionEvent::End))
        .unwrap();
    assert!(retry_index < error_index);
    assert!(error_index < end_index);
    drop(events);
    server.abort();
}

#[tokio::test]
async fn reroutes_to_the_reported_host_port_and_preserves_instance_for_login7() {
    let target_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let target_address = target_listener.local_addr().unwrap();
    let target_login_server = Arc::new(Mutex::new(None::<String>));
    let observed_target = Arc::clone(&target_login_server);
    let target = tokio::spawn(async move {
        let (mut stream, _) = target_listener.accept().await.unwrap();
        let _ = read_message(&mut stream).await.unwrap();
        send_message(&mut stream, packet_type::PRELOGIN, prelogin_response())
            .await
            .unwrap();
        let (_, login) = read_message(&mut stream).await.unwrap();
        *observed_target.lock().unwrap() = Some(String::from_utf8_lossy(&login).into_owned());
        send_message(&mut stream, packet_type::TABULAR_RESULT, login_ack())
            .await
            .unwrap();
        let _ = read_message(&mut stream).await.unwrap();
        let _ = send_message(&mut stream, packet_type::TABULAR_RESULT, Vec::new()).await;
    });

    let origin_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let origin_address = origin_listener.local_addr().unwrap();
    let target_port = target_address.port();
    let origin = tokio::spawn(async move {
        let (mut stream, _) = origin_listener.accept().await.unwrap();
        let _ = read_message(&mut stream).await.unwrap();
        send_message(&mut stream, packet_type::PRELOGIN, prelogin_response())
            .await
            .unwrap();
        let _ = read_message(&mut stream).await.unwrap();
        let mut response = login_ack();
        response.extend(routing_token("127.0.0.1", target_port, "instanceA"));
        send_message(&mut stream, packet_type::TABULAR_RESULT, response)
            .await
            .unwrap();
    });

    let mut config = ConnectionConfig::sql("127.0.0.1", "sa", "password");
    config.port = origin_address.port();
    config.encrypt = false;
    config.connect_timeout = Duration::from_secs(2);
    config.debug.payload = true;
    let mut connection = Connection::new(config);
    let reroutes = Arc::new(Mutex::new(0usize));
    let events = Arc::new(Mutex::new(Vec::new()));
    let observed_events = Arc::clone(&events);
    let seen = Arc::clone(&reroutes);
    connection.on(move |event| {
        observed_events.lock().unwrap().push(event.clone());
        if matches!(event, ConnectionEvent::Rerouting) {
            *seen.lock().unwrap() += 1;
        }
    });

    connection.connect().await.unwrap();

    assert_eq!(*reroutes.lock().unwrap(), 1);
    let route = connection.routing.as_ref().unwrap();
    assert_eq!(route.server, "127.0.0.1");
    assert_eq!(route.port, target_port);
    assert_eq!(route.instance.as_deref(), Some("instanceA"));
    assert_eq!(connection.state, ConnectionState::LoggedIn);
    assert!(target_login_server.lock().unwrap().is_some());
    let state_changes = events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            ConnectionEvent::Debug(message) if message.starts_with("State change: ") => {
                Some(message.trim_start_matches("State change: ").to_owned())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        state_changes,
        vec![
            "Initialized -> Connecting",
            "Connecting -> SentPrelogin",
            "SentPrelogin -> SentLogin7WithStandardLogin",
            "SentLogin7WithStandardLogin -> ReRouting",
            "ReRouting -> Connecting",
            "Connecting -> SentPrelogin",
            "SentPrelogin -> SentLogin7WithStandardLogin",
            "SentLogin7WithStandardLogin -> LoggedInSendingInitialSql",
            "LoggedInSendingInitialSql -> LoggedIn",
        ]
    );
    assert!(!events.lock().unwrap().iter().any(|event| {
        matches!(event, ConnectionEvent::Debug(message) if message.contains("-> Initialized"))
    }));
    connection.close().await;
    origin.abort();
    target.abort();
}

#[tokio::test]
async fn destination_connector_receives_rerouted_server_and_port() {
    let target_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let target_address = target_listener.local_addr().unwrap();
    let target = tokio::spawn(async move {
        let (stream, _) = target_listener.accept().await.unwrap();
        success_sequence(stream).await.unwrap();
    });
    let origin_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let origin_address = origin_listener.local_addr().unwrap();
    let target_port = target_address.port();
    let origin = tokio::spawn(async move {
        let (mut stream, _) = origin_listener.accept().await.unwrap();
        let _ = read_message(&mut stream).await.unwrap();
        send_message(&mut stream, packet_type::PRELOGIN, prelogin_response())
            .await
            .unwrap();
        let _ = read_message(&mut stream).await.unwrap();
        let mut response = login_ack();
        response.extend(routing_token("127.0.0.1", target_port, "instanceB"));
        send_message(&mut stream, packet_type::TABULAR_RESULT, response)
            .await
            .unwrap();
    });
    let destinations = Arc::new(Mutex::new(Vec::<(String, u16)>::new()));
    let observed = Arc::clone(&destinations);
    let mut config = ConnectionConfig::sql("127.0.0.1", "sa", "password");
    config.port = origin_address.port();
    config.encrypt = false;
    config.connect_timeout = Duration::from_secs(2);
    let mut connection = Connection::new(config);
    connection.set_destination_connector(move |server, port| {
        let observed = Arc::clone(&observed);
        async move {
            observed.lock().unwrap().push((server, port));
            TcpStream::connect(("127.0.0.1", port)).await
        }
    });

    connection.connect().await.unwrap();
    let destination_list = destinations.lock().unwrap().clone();
    assert_eq!(destination_list.len(), 2);
    assert_eq!(destination_list[1], ("127.0.0.1".to_owned(), target_port));
    connection.close().await;
    origin.abort();
    target.abort();
}
