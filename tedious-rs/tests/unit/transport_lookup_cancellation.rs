use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tedious_rs::connector::{
    CancellationToken, ConnectorError, connect_in_sequence_with_options_and_lookup,
    lookup_all_addresses_with,
};
use tedious_rs::instance_lookup::lookup_instance_with_options;
use tedious_rs::message::{Message, packetize};
use tedious_rs::packet::packet_type;
use tedious_rs::sender::send_in_parallel_with_cancel;
use tedious_rs::transport::MessageIo;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, UdpSocket};

#[tokio::test]
async fn connector_uses_the_injected_lookup_for_hostnames() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let accepted = tokio::spawn(async move { listener.accept().await.unwrap() });
    let lookup = move |_host: String, _port: u16| async move { Ok(vec![address]) };
    let stream = connect_in_sequence_with_options_and_lookup(
        "sql.example",
        1433,
        Duration::from_secs(1),
        None,
        None,
        &lookup,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(stream.peer_addr().unwrap(), address);
    drop(stream);
    accepted.await.unwrap();
}

#[tokio::test]
async fn lookup_cancellation_interrupts_an_injected_resolver() {
    let cancellation = CancellationToken::new();
    let lookup = |_host: String, _port: u16| async move {
        std::future::pending::<io::Result<Vec<SocketAddr>>>().await
    };
    let token = cancellation.clone();
    let task = tokio::spawn(async move {
        lookup_all_addresses_with("sql.example", 1433, &lookup, &token).await
    });
    cancellation.cancel();
    assert!(matches!(
        task.await.unwrap(),
        Err(ConnectorError::Cancelled)
    ));
}

#[tokio::test]
async fn sender_cancellation_stops_before_opening_udp_sockets() {
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let result = send_in_parallel_with_cancel(
        &["127.0.0.1:1".parse().unwrap()],
        1434,
        &[2],
        Duration::from_secs(1),
        &cancellation,
    )
    .await;
    assert_eq!(result.unwrap_err(), "operation cancelled");
}

#[tokio::test]
async fn instance_lookup_uses_injected_lookup_and_custom_browser_port() {
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let server_task = tokio::spawn(async move {
        let mut request = [0u8; 8];
        let (length, peer) = server.recv_from(&mut request).await.unwrap();
        assert_eq!(&request[..length], &[2]);
        server
            .send_to(b"xyzServerName;host;InstanceName;MSSQL;tcp;1444;;", peer)
            .await
            .unwrap();
    });
    let lookup = move |_host: String, _port: u16| async move { Ok(vec![address]) };
    let port = lookup_instance_with_options(
        "sql.example",
        "mssql",
        Duration::from_secs(1),
        0,
        address.port(),
        &lookup,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(port, 1444);
    server_task.await.unwrap();
}

#[tokio::test]
async fn transport_callback_observes_each_packet_before_message_completion() {
    let (mut writer, reader) = tokio::io::duplex(256);
    let mut message = Message::new(packet_type::SQL_BATCH);
    message.data = (0..20).collect();
    let packets = packetize(&message, 16).unwrap();
    let wire = packets
        .iter()
        .flat_map(|packet| packet.buffer.clone())
        .collect::<Vec<_>>();
    let writer_task = tokio::spawn(async move {
        writer.write_all(&wire).await.unwrap();
    });
    let mut io = MessageIo::new(reader, 16).unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&seen);
    let result = io
        .read_message_with_packet_callback(|packet| {
            observed.lock().unwrap().push(packet.data().len());
        })
        .await
        .unwrap();
    writer_task.await.unwrap();
    assert_eq!(result, message);
    assert_eq!(seen.lock().unwrap().len(), packets.len());
}
