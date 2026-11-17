use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tedious_rs::connection::{Authentication, Connection, ConnectionConfig, ConnectionEvent};
use tedious_rs::message::{Message, packetize};
use tedious_rs::packet::packet_type;
use tedious_rs::prelogin_payload::{PreloginOptions, PreloginPayload, PreloginVersion};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

async fn read_message(stream: &mut TcpStream) -> std::io::Result<(u8, Vec<u8>)> {
    let mut data = Vec::new();
    loop {
        let mut header = [0; 8];
        timeout(Duration::from_secs(2), stream.read_exact(&mut header))
            .await
            .map_err(|_| std::io::Error::other("message header timeout"))??;
        let length = u16::from_be_bytes([header[2], header[3]]) as usize;
        let mut payload = vec![0; length.saturating_sub(8)];
        timeout(Duration::from_secs(2), stream.read_exact(&mut payload))
            .await
            .map_err(|_| std::io::Error::other("message payload timeout"))??;
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

fn routing_token(server: &str, port: u16) -> Vec<u8> {
    let encoded = server
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut value = vec![0, (port & 0xff) as u8, (port >> 8) as u8];
    value.extend((server.encode_utf16().count() as u16).to_le_bytes());
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

async fn run_server(listener: TcpListener, observed: Arc<Mutex<Vec<u8>>>) {
    let Ok((mut stream, _)) = listener.accept().await else {
        return;
    };
    let Ok((prelogin, _)) = read_message(&mut stream).await else {
        return;
    };
    observed.lock().unwrap().push(prelogin);
    if send_message(&mut stream, packet_type::PRELOGIN, prelogin_response())
        .await
        .is_err()
    {
        return;
    }
    let Ok((login7, _)) = read_message(&mut stream).await else {
        return;
    };
    observed.lock().unwrap().push(login7);
    if send_message(&mut stream, packet_type::TABULAR_RESULT, login_ack())
        .await
        .is_err()
    {
        return;
    }
    if let Ok((initial_sql, _)) = read_message(&mut stream).await {
        observed.lock().unwrap().push(initial_sql);
        let _ = send_message(&mut stream, packet_type::TABULAR_RESULT, Vec::new()).await;
    }
}

async fn run_retry_server(listener: TcpListener, observed: Arc<Mutex<Vec<u8>>>) {
    for attempt in 1..=3 {
        let Ok(Ok((mut stream, _))) = timeout(Duration::from_secs(2), listener.accept()).await
        else {
            return;
        };
        let Ok((prelogin, _)) = read_message(&mut stream).await else {
            return;
        };
        observed.lock().unwrap().push(prelogin);
        if send_message(&mut stream, packet_type::PRELOGIN, prelogin_response())
            .await
            .is_err()
        {
            return;
        }
        let Ok((login7, _)) = read_message(&mut stream).await else {
            return;
        };
        observed.lock().unwrap().push(login7);
        let response = if attempt < 3 {
            error_token(4060)
        } else {
            login_ack()
        };
        if send_message(&mut stream, packet_type::TABULAR_RESULT, response)
            .await
            .is_err()
        {
            return;
        }
        if attempt == 3
            && let Ok((initial_sql, _)) = read_message(&mut stream).await
        {
            observed.lock().unwrap().push(initial_sql);
            let _ = send_message(&mut stream, packet_type::TABULAR_RESULT, Vec::new()).await;
        }
    }
}

async fn run_route_server(
    initial_listener: TcpListener,
    target_listener: TcpListener,
    target_port: u16,
    observed: Arc<Mutex<Vec<u8>>>,
) {
    let Ok(Ok((mut initial_stream, _))) =
        timeout(Duration::from_secs(2), initial_listener.accept()).await
    else {
        return;
    };
    let Ok((prelogin, _)) = read_message(&mut initial_stream).await else {
        return;
    };
    observed.lock().unwrap().push(prelogin);
    if send_message(
        &mut initial_stream,
        packet_type::PRELOGIN,
        prelogin_response(),
    )
    .await
    .is_err()
    {
        return;
    }
    let Ok((login7, _)) = read_message(&mut initial_stream).await else {
        return;
    };
    observed.lock().unwrap().push(login7);
    let mut response = login_ack();
    response.extend(routing_token("127.0.0.1", target_port));
    if send_message(&mut initial_stream, packet_type::TABULAR_RESULT, response)
        .await
        .is_err()
    {
        return;
    }
    drop(initial_stream);
    let Ok(Ok((mut target_stream, _))) =
        timeout(Duration::from_secs(2), target_listener.accept()).await
    else {
        return;
    };
    let Ok((prelogin, _)) = read_message(&mut target_stream).await else {
        return;
    };
    observed.lock().unwrap().push(prelogin);
    if send_message(
        &mut target_stream,
        packet_type::PRELOGIN,
        prelogin_response(),
    )
    .await
    .is_err()
    {
        return;
    }
    let Ok((login7, _)) = read_message(&mut target_stream).await else {
        return;
    };
    observed.lock().unwrap().push(login7);
    if send_message(&mut target_stream, packet_type::TABULAR_RESULT, login_ack())
        .await
        .is_err()
    {
        return;
    }
    if let Ok((initial_sql, _)) = read_message(&mut target_stream).await {
        observed.lock().unwrap().push(initial_sql);
        let _ = send_message(&mut target_stream, packet_type::TABULAR_RESULT, Vec::new()).await;
    }
}

async fn run_auth_server(listener: TcpListener, observed: Arc<Mutex<Vec<u8>>>, ntlm: bool) {
    let Ok((mut stream, _)) = listener.accept().await else {
        return;
    };
    let Ok((prelogin, _)) = read_message(&mut stream).await else {
        return;
    };
    observed.lock().unwrap().push(prelogin);
    if send_message(&mut stream, packet_type::PRELOGIN, prelogin_response())
        .await
        .is_err()
    {
        return;
    }
    let Ok((login7, _)) = read_message(&mut stream).await else {
        return;
    };
    observed.lock().unwrap().push(login7);
    if ntlm {
        if send_message(
            &mut stream,
            packet_type::TABULAR_RESULT,
            ntlm_challenge_token(),
        )
        .await
        .is_err()
        {
            return;
        }
        let Ok((ntlm_auth, _)) = read_message(&mut stream).await else {
            return;
        };
        observed.lock().unwrap().push(ntlm_auth);
    }
    if send_message(&mut stream, packet_type::TABULAR_RESULT, login_ack())
        .await
        .is_err()
    {
        return;
    }
    if let Ok((initial_sql, _)) = read_message(&mut stream).await {
        observed.lock().unwrap().push(initial_sql);
        let _ = send_message(&mut stream, packet_type::TABULAR_RESULT, Vec::new()).await;
    }
}

fn done_token() -> Vec<u8> {
    vec![0xFD, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
}

async fn run_request_server(listener: TcpListener, observed: Arc<Mutex<Vec<u8>>>) {
    let Ok((mut stream, _)) = listener.accept().await else {
        return;
    };
    let Ok((prelogin, _)) = read_message(&mut stream).await else {
        return;
    };
    observed.lock().unwrap().push(prelogin);
    if send_message(&mut stream, packet_type::PRELOGIN, prelogin_response())
        .await
        .is_err()
    {
        return;
    }
    let Ok((login7, _)) = read_message(&mut stream).await else {
        return;
    };
    observed.lock().unwrap().push(login7);
    if send_message(&mut stream, packet_type::TABULAR_RESULT, login_ack())
        .await
        .is_err()
    {
        return;
    }
    let Ok((initial_sql, _)) = read_message(&mut stream).await else {
        return;
    };
    observed.lock().unwrap().push(initial_sql);
    if send_message(&mut stream, packet_type::TABULAR_RESULT, done_token())
        .await
        .is_err()
    {
        return;
    }
    let Ok((request_sql, _)) = read_message(&mut stream).await else {
        return;
    };
    observed.lock().unwrap().push(request_sql);
    let _ = send_message(&mut stream, packet_type::TABULAR_RESULT, done_token()).await;
}

async fn run_cancel_server(listener: TcpListener, observed: Arc<Mutex<Vec<u8>>>) {
    let Ok((mut stream, _)) = listener.accept().await else {
        return;
    };
    let Ok((prelogin, _)) = read_message(&mut stream).await else {
        return;
    };
    observed.lock().unwrap().push(prelogin);
    if send_message(&mut stream, packet_type::PRELOGIN, prelogin_response())
        .await
        .is_err()
    {
        return;
    }
    let Ok((login7, _)) = read_message(&mut stream).await else {
        return;
    };
    observed.lock().unwrap().push(login7);
    if send_message(&mut stream, packet_type::TABULAR_RESULT, login_ack())
        .await
        .is_err()
    {
        return;
    }
    let Ok((initial_sql, _)) = read_message(&mut stream).await else {
        return;
    };
    observed.lock().unwrap().push(initial_sql);
    if send_message(&mut stream, packet_type::TABULAR_RESULT, done_token())
        .await
        .is_err()
    {
        return;
    }
    let Ok((request_sql, _)) = read_message(&mut stream).await else {
        return;
    };
    observed.lock().unwrap().push(request_sql);
    let Ok((attention, _)) = read_message(&mut stream).await else {
        return;
    };
    observed.lock().unwrap().push(attention);
    if send_message(&mut stream, packet_type::TABULAR_RESULT, done_token())
        .await
        .is_err()
    {
        return;
    }
    let mut acknowledged = done_token();
    acknowledged[1] = 0x20;
    if send_message(&mut stream, packet_type::TABULAR_RESULT, acknowledged)
        .await
        .is_err()
    {
        return;
    }
    let Ok((second_sql, _)) = read_message(&mut stream).await else {
        return;
    };
    observed.lock().unwrap().push(second_sql);
    let mut second_done = done_token();
    second_done[5..13].copy_from_slice(&7i64.to_le_bytes());
    let _ = send_message(&mut stream, packet_type::TABULAR_RESULT, second_done).await;
}

async fn run_close_server(listener: TcpListener, observed: Arc<Mutex<Vec<u8>>>) {
    let Ok((mut stream, _)) = listener.accept().await else {
        return;
    };
    let Ok((prelogin, _)) = read_message(&mut stream).await else {
        return;
    };
    observed.lock().unwrap().push(prelogin);
    if send_message(&mut stream, packet_type::PRELOGIN, prelogin_response())
        .await
        .is_err()
    {
        return;
    }
    let Ok((login7, _)) = read_message(&mut stream).await else {
        return;
    };
    observed.lock().unwrap().push(login7);
    if send_message(&mut stream, packet_type::TABULAR_RESULT, login_ack())
        .await
        .is_err()
    {
        return;
    }
    let Ok((initial_sql, _)) = read_message(&mut stream).await else {
        return;
    };
    observed.lock().unwrap().push(initial_sql);
    if send_message(&mut stream, packet_type::TABULAR_RESULT, done_token())
        .await
        .is_err()
    {
        return;
    }
    if let Ok((request_sql, _)) = read_message(&mut stream).await {
        observed.lock().unwrap().push(request_sql);
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
}

async fn run_node() -> (Vec<String>, Vec<u8>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let server = tokio::spawn(run_server(listener, Arc::clone(&observed)));
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/compatibility/node_connection_runtime_parity.js");
    let output = tokio::task::spawn_blocking(move || {
        Command::new("node")
            .arg(script)
            .arg(port.to_string())
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    server.await.unwrap();
    assert!(output.status.success());
    let events = serde_json::from_slice::<Vec<String>>(&output.stdout).unwrap();
    (events, observed.lock().unwrap().clone())
}

async fn run_node_retry() -> (Vec<String>, Vec<u8>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let server = tokio::spawn(run_retry_server(listener, Arc::clone(&observed)));
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/compatibility/node_connection_runtime_parity.js");
    let output = tokio::task::spawn_blocking(move || {
        Command::new("node")
            .arg(script)
            .arg(port.to_string())
            .arg("retry")
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    server.await.unwrap();
    assert!(
        output.status.success(),
        "Node retry failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let events = serde_json::from_slice::<Vec<String>>(&output.stdout).unwrap();
    (events, observed.lock().unwrap().clone())
}

async fn run_node_route() -> (Vec<String>, Vec<u8>) {
    let initial_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let target_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let initial_port = initial_listener.local_addr().unwrap().port();
    let target_port = target_listener.local_addr().unwrap().port();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let server = tokio::spawn(run_route_server(
        initial_listener,
        target_listener,
        target_port,
        Arc::clone(&observed),
    ));
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/compatibility/node_connection_runtime_parity.js");
    let output = tokio::task::spawn_blocking(move || {
        Command::new("node")
            .arg(script)
            .arg(initial_port.to_string())
            .arg("route")
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    server.await.unwrap();
    assert!(
        output.status.success(),
        "Node route failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let events = serde_json::from_slice::<Vec<String>>(&output.stdout).unwrap();
    (events, observed.lock().unwrap().clone())
}

async fn run_node_auth(mode: &str) -> (Vec<String>, Vec<u8>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let server = tokio::spawn(run_auth_server(
        listener,
        Arc::clone(&observed),
        mode == "ntlm",
    ));
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/compatibility/node_connection_runtime_parity.js");
    let mode = mode.to_owned();
    let output = tokio::task::spawn_blocking(move || {
        Command::new("node")
            .arg(script)
            .arg(port.to_string())
            .arg(mode)
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    server.await.unwrap();
    assert!(
        output.status.success(),
        "Node auth failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let events = serde_json::from_slice::<Vec<String>>(&output.stdout).unwrap();
    (events, observed.lock().unwrap().clone())
}

async fn run_node_request() -> (Vec<String>, Vec<u8>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let server = tokio::spawn(run_request_server(listener, Arc::clone(&observed)));
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/compatibility/node_connection_runtime_parity.js");
    let output = tokio::task::spawn_blocking(move || {
        Command::new("node")
            .arg(script)
            .arg(port.to_string())
            .arg("request")
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    server.await.unwrap();
    assert!(
        output.status.success(),
        "Node request failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let events = serde_json::from_slice::<Vec<String>>(&output.stdout).unwrap();
    (events, observed.lock().unwrap().clone())
}

async fn run_node_cancel() -> (Vec<String>, Vec<u8>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let server = tokio::spawn(run_cancel_server(listener, Arc::clone(&observed)));
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/compatibility/node_connection_runtime_parity.js");
    let output = tokio::task::spawn_blocking(move || {
        Command::new("node")
            .arg(script)
            .arg(port.to_string())
            .arg("cancel")
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    server.await.unwrap();
    assert!(
        output.status.success(),
        "Node cancel failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let events = serde_json::from_slice::<Vec<String>>(&output.stdout).unwrap();
    (events, observed.lock().unwrap().clone())
}

async fn run_node_close() -> (Vec<String>, Vec<u8>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let server = tokio::spawn(run_close_server(listener, Arc::clone(&observed)));
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/compatibility/node_connection_runtime_parity.js");
    let output = tokio::task::spawn_blocking(move || {
        Command::new("node")
            .arg(script)
            .arg(port.to_string())
            .arg("close")
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    server.await.unwrap();
    assert!(
        output.status.success(),
        "Node close failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let events = serde_json::from_slice::<Vec<String>>(&output.stdout).unwrap();
    (events, observed.lock().unwrap().clone())
}

async fn run_node_timeout() -> (Vec<String>, Vec<u8>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let server = tokio::spawn(run_close_server(listener, Arc::clone(&observed)));
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/compatibility/node_connection_runtime_parity.js");
    let output = tokio::task::spawn_blocking(move || {
        Command::new("node")
            .arg(script)
            .arg(port.to_string())
            .arg("timeout")
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    server.await.unwrap();
    assert!(
        output.status.success(),
        "Node timeout failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let events = serde_json::from_slice::<Vec<String>>(&output.stdout).unwrap();
    (events, observed.lock().unwrap().clone())
}

#[tokio::test]
async fn node_and_rust_connect_runtime_sequences_match() {
    let (node_events, node_packets) = run_node().await;
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let server = tokio::spawn(run_server(listener, Arc::clone(&observed)));
    let mut config = ConnectionConfig::sql("127.0.0.1", "sa", "password");
    config.port = port;
    config.encrypt = false;
    config.connect_timeout = Duration::from_secs(2);
    config.debug.token = true;
    let mut connection = Connection::new(config);
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    connection.on(move |event| match event {
        ConnectionEvent::Connect => seen.lock().unwrap().push("connect".to_owned()),
        ConnectionEvent::End => seen.lock().unwrap().push("end".to_owned()),
        ConnectionEvent::ConnectError(_) => seen.lock().unwrap().push("connectError".to_owned()),
        ConnectionEvent::Error(_) => seen.lock().unwrap().push("error".to_owned()),
        ConnectionEvent::Debug(message) if message.starts_with("State change: ") => {
            seen.lock().unwrap().push(message.clone())
        }
        _ => {}
    });
    connection.connect().await.unwrap();
    connection.close().await;
    server.await.unwrap();
    assert_eq!(node_events, *events.lock().unwrap());
    assert_eq!(node_packets, *observed.lock().unwrap());
    assert_eq!(
        node_packets,
        vec![
            packet_type::PRELOGIN,
            packet_type::LOGIN7,
            packet_type::SQL_BATCH
        ]
    );
}

#[tokio::test]
async fn node_and_rust_retry_runtime_sequences_match() {
    let (node_events, node_packets) = timeout(Duration::from_secs(20), run_node_retry())
        .await
        .expect("Node retry runtime parity timed out");
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let server = tokio::spawn(run_retry_server(listener, Arc::clone(&observed)));
    let mut config = ConnectionConfig::sql("127.0.0.1", "sa", "password");
    config.port = port;
    config.encrypt = false;
    config.connect_timeout = Duration::from_secs(2);
    config.connection_retry_interval = Duration::from_millis(1);
    config.max_retries_on_transient_errors = 2;
    config.debug.token = true;
    let mut connection = Connection::new(config);
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    connection.on(move |event| match event {
        ConnectionEvent::Retry => seen.lock().unwrap().push("retry".to_owned()),
        ConnectionEvent::Connect => seen.lock().unwrap().push("connect".to_owned()),
        ConnectionEvent::End => seen.lock().unwrap().push("end".to_owned()),
        ConnectionEvent::ConnectError(_) => seen.lock().unwrap().push("connectError".to_owned()),
        ConnectionEvent::Error(_) => seen.lock().unwrap().push("error".to_owned()),
        ConnectionEvent::Debug(message) if message.starts_with("State change: ") => {
            seen.lock().unwrap().push(message.clone())
        }
        _ => {}
    });
    connection.connect().await.unwrap();
    connection.close().await;
    server.await.unwrap();
    assert_eq!(node_events, *events.lock().unwrap());
    assert_eq!(node_packets, *observed.lock().unwrap());
    assert_eq!(
        node_packets,
        vec![
            packet_type::PRELOGIN,
            packet_type::LOGIN7,
            packet_type::PRELOGIN,
            packet_type::LOGIN7,
            packet_type::PRELOGIN,
            packet_type::LOGIN7,
            packet_type::SQL_BATCH,
        ]
    );
}

#[tokio::test]
async fn node_and_rust_route_runtime_sequences_match() {
    let (node_events, node_packets) = timeout(Duration::from_secs(10), run_node_route())
        .await
        .expect("Node route runtime parity timed out");
    let initial_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let target_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let initial_port = initial_listener.local_addr().unwrap().port();
    let target_port = target_listener.local_addr().unwrap().port();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let server = tokio::spawn(run_route_server(
        initial_listener,
        target_listener,
        target_port,
        Arc::clone(&observed),
    ));
    let mut config = ConnectionConfig::sql("127.0.0.1", "sa", "password");
    config.port = initial_port;
    config.encrypt = false;
    config.connect_timeout = Duration::from_secs(2);
    config.debug.token = true;
    let mut connection = Connection::new(config);
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    connection.on(move |event| match event {
        ConnectionEvent::Rerouting => seen.lock().unwrap().push("rerouting".to_owned()),
        ConnectionEvent::Connect => seen.lock().unwrap().push("connect".to_owned()),
        ConnectionEvent::End => seen.lock().unwrap().push("end".to_owned()),
        ConnectionEvent::ConnectError(_) => seen.lock().unwrap().push("connectError".to_owned()),
        ConnectionEvent::Error(_) => seen.lock().unwrap().push("error".to_owned()),
        ConnectionEvent::Debug(message) if message.starts_with("State change: ") => {
            seen.lock().unwrap().push(message.clone())
        }
        _ => {}
    });
    connection.connect().await.unwrap();
    connection.close().await;
    server.await.unwrap();
    assert_eq!(node_events, *events.lock().unwrap());
    assert_eq!(node_packets, *observed.lock().unwrap());
    assert_eq!(
        node_packets,
        vec![
            packet_type::PRELOGIN,
            packet_type::LOGIN7,
            packet_type::PRELOGIN,
            packet_type::LOGIN7,
            packet_type::SQL_BATCH,
        ]
    );
}

async fn auth_runtime_sequence_matches(mode: &str, authentication: Authentication) {
    let (node_events, node_packets) = run_node_auth(mode).await;
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let server = tokio::spawn(run_auth_server(
        listener,
        Arc::clone(&observed),
        mode == "ntlm",
    ));
    let mut config = ConnectionConfig::sql("127.0.0.1", "user", "password");
    config.authentication = authentication;
    config.port = port;
    config.encrypt = false;
    config.connect_timeout = Duration::from_secs(2);
    config.debug.token = true;
    let mut connection = Connection::new(config);
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    connection.on(move |event| match event {
        ConnectionEvent::Connect => seen.lock().unwrap().push("connect".to_owned()),
        ConnectionEvent::End => seen.lock().unwrap().push("end".to_owned()),
        ConnectionEvent::ConnectError(_) => seen.lock().unwrap().push("connectError".to_owned()),
        ConnectionEvent::Error(_) => seen.lock().unwrap().push("error".to_owned()),
        ConnectionEvent::Debug(message) if message.starts_with("State change: ") => {
            seen.lock().unwrap().push(message.clone())
        }
        _ => {}
    });
    connection.connect().await.unwrap();
    connection.close().await;
    server.await.unwrap();
    assert_eq!(node_events, *events.lock().unwrap());
    assert_eq!(node_packets, *observed.lock().unwrap());
}

#[tokio::test]
async fn node_and_rust_ntlm_runtime_sequences_match() {
    auth_runtime_sequence_matches(
        "ntlm",
        Authentication::Ntlm {
            user_name: "user".to_owned(),
            password: "password".to_owned(),
            domain: None,
        },
    )
    .await;
}

#[tokio::test]
async fn node_and_rust_fedauth_runtime_sequences_match() {
    auth_runtime_sequence_matches(
        "fedauth",
        Authentication::AzureActiveDirectoryAccessToken {
            token: "fed-auth-token".to_owned(),
        },
    )
    .await;
}

#[tokio::test]
async fn node_and_rust_request_runtime_sequences_match() {
    let (node_events, node_packets) = run_node_request().await;
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let server = tokio::spawn(run_request_server(listener, Arc::clone(&observed)));
    let mut config = ConnectionConfig::sql("127.0.0.1", "sa", "password");
    config.port = port;
    config.encrypt = false;
    config.connect_timeout = Duration::from_secs(2);
    config.request_timeout = Duration::from_secs(2);
    config.debug.token = true;
    let mut connection = Connection::new(config);
    let request_events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&request_events);
    let mut request = tedious_rs::request::Request::new("SELECT 1", move |_| {
        seen.lock().unwrap().push("requestCallback".to_owned())
    });
    let seen = Arc::clone(&request_events);
    request.on(move |event| match event {
        tedious_rs::request::RequestEvent::RequestCompleted => {
            seen.lock().unwrap().push("requestCompleted".to_owned())
        }
        tedious_rs::request::RequestEvent::Done { .. } => {
            seen.lock().unwrap().push("done".to_owned())
        }
        tedious_rs::request::RequestEvent::Error(_) => {
            seen.lock().unwrap().push("requestError".to_owned())
        }
        _ => {}
    });
    let seen = Arc::clone(&request_events);
    connection.on(move |event| match event {
        ConnectionEvent::Connect => seen.lock().unwrap().push("connect".to_owned()),
        ConnectionEvent::End => seen.lock().unwrap().push("end".to_owned()),
        ConnectionEvent::Debug(message) if message.starts_with("State change: ") => {
            seen.lock().unwrap().push(message.clone())
        }
        _ => {}
    });
    connection.connect().await.unwrap();
    let result = connection.exec_sql(&mut request).await;
    assert!(result.is_ok());
    connection.close().await;
    server.await.unwrap();
    assert_eq!(node_events, *request_events.lock().unwrap());
    assert_eq!(node_packets, *observed.lock().unwrap());
}

#[tokio::test]
async fn node_and_rust_cancel_runtime_sequences_match() {
    let (node_events, node_packets) = timeout(Duration::from_secs(10), run_node_cancel())
        .await
        .expect("Node cancel runtime parity timed out");
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let server = tokio::spawn(run_cancel_server(listener, Arc::clone(&observed)));
    let mut config = ConnectionConfig::sql("127.0.0.1", "sa", "password");
    config.port = port;
    config.encrypt = false;
    config.connect_timeout = Duration::from_secs(2);
    config.request_timeout = Duration::from_secs(2);
    config.cancel_timeout = Duration::from_secs(2);
    config.debug.token = true;
    let mut connection = Connection::new(config);
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    connection.on(move |event| match event {
        ConnectionEvent::Connect => seen.lock().unwrap().push("connect".to_owned()),
        ConnectionEvent::End => seen.lock().unwrap().push("end".to_owned()),
        ConnectionEvent::Debug(message) if message.starts_with("State change: ") => {
            seen.lock().unwrap().push(message.clone())
        }
        _ => {}
    });
    connection.connect().await.unwrap();
    let request_events = Arc::clone(&events);
    let mut request = tedious_rs::request::Request::new("SELECT 1", move |result| {
        let label = result
            .err()
            .and_then(|error| error.code)
            .map(|code| format!("requestCallbackError:{code}"))
            .unwrap_or_else(|| "requestCallback".to_owned());
        request_events.lock().unwrap().push(label);
    });
    let cancellation = request.cancellation_handle();
    let seen = Arc::clone(&events);
    request.on(move |event| {
        if matches!(event, tedious_rs::request::RequestEvent::Error(_)) {
            seen.lock().unwrap().push("requestError".to_owned())
        }
    });
    {
        let execution = connection.exec_sql_batch_request(&mut request);
        tokio::pin!(execution);
        tokio::select! {
            result = &mut execution => panic!("request completed before cancellation: {result:?}"),
            _ = tokio::time::sleep(Duration::from_millis(30)) => cancellation.cancel(),
        }
        assert_eq!(
            execution.await.unwrap_err().code.as_deref(),
            Some("ECANCEL")
        );
    }
    let mut second = tedious_rs::request::Request::new("SELECT 2", {
        let seen = Arc::clone(&events);
        move |result| {
            seen.lock().unwrap().push(if result.is_ok() {
                "secondCallback".to_owned()
            } else {
                "secondCallbackError".to_owned()
            });
        }
    });
    let seen = Arc::clone(&events);
    second.on(move |event| {
        if matches!(event, tedious_rs::request::RequestEvent::RequestCompleted) {
            seen.lock()
                .unwrap()
                .push("secondRequestCompleted".to_owned());
        }
    });
    connection
        .exec_sql_batch_request(&mut second)
        .await
        .unwrap();
    connection.close().await;
    server.await.unwrap();
    assert_eq!(node_events, *events.lock().unwrap());
    assert_eq!(node_packets, *observed.lock().unwrap());
    assert_eq!(
        node_packets,
        vec![
            packet_type::PRELOGIN,
            packet_type::LOGIN7,
            packet_type::SQL_BATCH,
            packet_type::SQL_BATCH,
            packet_type::ATTENTION,
            packet_type::SQL_BATCH,
        ]
    );
}

#[tokio::test]
async fn node_and_rust_close_runtime_sequences_match() {
    let (node_events, node_packets) = timeout(Duration::from_secs(10), run_node_close())
        .await
        .expect("Node close runtime parity timed out");
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let server = tokio::spawn(run_close_server(listener, Arc::clone(&observed)));
    let mut config = ConnectionConfig::sql("127.0.0.1", "sa", "password");
    config.port = port;
    config.encrypt = false;
    config.connect_timeout = Duration::from_secs(2);
    config.request_timeout = Duration::from_secs(2);
    config.debug.token = true;
    let mut connection = Connection::new(config);
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    connection.on(move |event| match event {
        ConnectionEvent::Connect => seen.lock().unwrap().push("connect".to_owned()),
        ConnectionEvent::End => seen.lock().unwrap().push("end".to_owned()),
        ConnectionEvent::Debug(message) if message.starts_with("State change: ") => {
            seen.lock().unwrap().push(message.clone())
        }
        _ => {}
    });
    connection.connect().await.unwrap();
    let seen = Arc::clone(&events);
    let mut request = tedious_rs::request::Request::new("SELECT 1", move |result| {
        seen.lock().unwrap().push(
            result
                .err()
                .and_then(|error| error.code)
                .map(|code| format!("requestCallbackError:{code}"))
                .unwrap_or_else(|| "requestCallback".to_owned()),
        );
    });
    let close_handle = connection.close_handle();
    {
        let execution = connection.exec_sql_batch_request(&mut request);
        tokio::pin!(execution);
        tokio::select! {
            result = &mut execution => panic!("request completed before close: {result:?}"),
            _ = tokio::time::sleep(Duration::from_millis(30)) => close_handle.close(),
        }
        assert_eq!(execution.await.unwrap_err().code.as_deref(), Some("ECLOSE"));
    }
    connection.close().await;
    server.await.unwrap();
    assert_eq!(node_events, *events.lock().unwrap());
    assert_eq!(node_packets, *observed.lock().unwrap());
    assert_eq!(
        node_packets,
        vec![
            packet_type::PRELOGIN,
            packet_type::LOGIN7,
            packet_type::SQL_BATCH,
            packet_type::SQL_BATCH,
        ]
    );
}

#[tokio::test]
async fn node_and_rust_request_timeout_runtime_sequences_match() {
    let (node_events, node_packets) = timeout(Duration::from_secs(10), run_node_timeout())
        .await
        .expect("Node timeout runtime parity timed out");
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let server = tokio::spawn(run_close_server(listener, Arc::clone(&observed)));
    let mut config = ConnectionConfig::sql("127.0.0.1", "sa", "password");
    config.port = port;
    config.encrypt = false;
    config.connect_timeout = Duration::from_secs(2);
    config.request_timeout = Duration::from_millis(200);
    config.cancel_timeout = Duration::from_millis(100);
    config.debug.token = true;
    let mut connection = Connection::new(config);
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    connection.on(move |event| match event {
        ConnectionEvent::Connect => seen.lock().unwrap().push("connect".to_owned()),
        ConnectionEvent::End => seen.lock().unwrap().push("end".to_owned()),
        ConnectionEvent::Debug(message) if message.starts_with("State change: ") => {
            seen.lock().unwrap().push(message.clone())
        }
        _ => {}
    });
    connection.connect().await.unwrap();
    let seen = Arc::clone(&events);
    let mut request = tedious_rs::request::Request::new("SELECT 1", move |result| {
        seen.lock().unwrap().push(
            result
                .err()
                .and_then(|error| error.code)
                .map(|code| format!("requestCallbackError:{code}"))
                .unwrap_or_else(|| "requestCallback".to_owned()),
        );
    });
    let result = connection.exec_sql_batch_request(&mut request).await;
    assert_eq!(result.unwrap_err().code.as_deref(), Some("ETIMEOUT"));
    connection.close().await;
    server.await.unwrap();
    assert_eq!(node_events, *events.lock().unwrap());
    assert_eq!(node_packets, *observed.lock().unwrap());
}
