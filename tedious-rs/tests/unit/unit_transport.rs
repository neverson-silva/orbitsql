use std::time::Duration;

use futures_util::{StreamExt, stream};
use tedious_rs::bulk_load::{BulkLoad, BulkLoadOptions};
use tedious_rs::message::Message;
use tedious_rs::packet::packet_type;
use tedious_rs::transport::{MessageIo, TdsTlsStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn sends_and_reads_tds_messages_over_partial_tcp_io() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut header = [0u8; 8];
        socket.read_exact(&mut header).await.unwrap();
        let length = u16::from_be_bytes([header[2], header[3]]) as usize;
        let mut body = vec![0; length - 8];
        socket.read_exact(&mut body).await.unwrap();
        assert_eq!(body, b"select 1");
        let mut response = vec![packet_type::TABULAR_RESULT, 1, 0, 10, 0, 0, 1, 0];
        response.extend([0xAA, 0xBB]);
        socket.write_all(&response[..3]).await.unwrap();
        socket.write_all(&response[3..]).await.unwrap();
    });
    let stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let mut io = MessageIo::new(stream, 16).unwrap();
    let mut request = Message::new(packet_type::SQL_BATCH);
    request.data = b"select 1".to_vec();
    io.send_message(&request).await.unwrap();
    let response = io.read_message().await.unwrap();
    assert_eq!(response.message_type, packet_type::TABULAR_RESULT);
    assert_eq!(response.data, vec![0xAA, 0xBB]);
    tokio::time::timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn restarts_packet_ids_for_each_message() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut first_ids = Vec::new();
        for _ in 0..2 {
            let mut first_packet = true;
            loop {
                let mut header = [0u8; 8];
                socket.read_exact(&mut header).await.unwrap();
                let length = u16::from_be_bytes([header[2], header[3]]) as usize;
                let mut body = vec![0; length - 8];
                socket.read_exact(&mut body).await.unwrap();
                if first_packet {
                    first_ids.push(header[6]);
                    first_packet = false;
                }
                if header[1] & 1 != 0 {
                    break;
                }
            }
        }
        first_ids
    });
    let stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let mut io = MessageIo::new(stream, 16).unwrap();
    let mut first = Message::new(packet_type::SQL_BATCH);
    first.data = vec![1; 9];
    let mut second = Message::new(packet_type::SQL_BATCH);
    second.data = vec![2; 9];
    io.send_message(&first).await.unwrap();
    io.send_message(&second).await.unwrap();
    assert_eq!(server.await.unwrap(), vec![1, 1]);
}

#[tokio::test]
async fn sends_buffered_message_chunks_without_concatenating_the_payload() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut packets = Vec::new();
        loop {
            let mut header = [0u8; 8];
            socket.read_exact(&mut header).await.unwrap();
            let length = u16::from_be_bytes([header[2], header[3]]) as usize;
            let mut body = vec![0; length - 8];
            socket.read_exact(&mut body).await.unwrap();
            let last = header[1] & 1 != 0;
            packets.push((header[6], last, body));
            if last {
                break;
            }
        }
        packets
    });
    let stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let mut io = MessageIo::new(stream, 16).unwrap();
    let buffers = vec![vec![1, 2, 3, 4, 5], vec![6, 7, 8, 9, 10, 11]];
    io.send_message_buffers(packet_type::BULK_LOAD, &buffers)
        .await
        .unwrap();
    let packets = tokio::time::timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0], (1, false, vec![1, 2, 3, 4, 5, 6, 7, 8]));
    assert_eq!(packets[1], (2, true, vec![9, 10, 11]));
}

#[tokio::test]
async fn sends_exact_multiple_payloads_without_an_empty_packet() {
    let (client, mut server) = tokio::io::duplex(128);
    let mut io = MessageIo::new(client, 16).unwrap();
    let server_task = tokio::spawn(async move {
        let mut packets = Vec::new();
        for _ in 0..2 {
            let mut header = [0u8; 8];
            server.read_exact(&mut header).await.unwrap();
            let length = u16::from_be_bytes([header[2], header[3]]) as usize;
            let mut body = vec![0; length - 8];
            server.read_exact(&mut body).await.unwrap();
            packets.push((header[6], header[1], body));
        }
        packets
    });
    io.send_message_buffers(packet_type::BULK_LOAD, &[vec![1; 8], vec![2; 8]])
        .await
        .unwrap();
    let packets = server_task.await.unwrap();
    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0], (1, 0, vec![1; 8]));
    assert_eq!(packets[1], (2, 1, vec![2; 8]));
}

#[tokio::test]
async fn sends_async_message_chunks_without_collecting_the_stream() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut packets = Vec::new();
        loop {
            let mut header = [0u8; 8];
            socket.read_exact(&mut header).await.unwrap();
            let length = u16::from_be_bytes([header[2], header[3]]) as usize;
            let mut body = vec![0; length - 8];
            socket.read_exact(&mut body).await.unwrap();
            let last = header[1] & 1 != 0;
            packets.push((header[6], last, body));
            if last {
                break;
            }
        }
        packets
    });
    let stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let mut io = MessageIo::new(stream, 16).unwrap();
    let parts = stream::iter(vec![Ok(vec![1, 2, 3, 4, 5]), Ok(vec![6, 7, 8, 9, 10, 11])]);
    io.send_message_stream(packet_type::BULK_LOAD, parts)
        .await
        .unwrap();
    let packets = tokio::time::timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0], (1, false, vec![1, 2, 3, 4, 5, 6, 7, 8]));
    assert_eq!(packets[1], (2, true, vec![9, 10, 11]));
}

#[tokio::test]
async fn cancellation_finishes_stream_message_with_residual_payload_and_ignore() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut header = [0u8; 8];
        socket.read_exact(&mut header).await.unwrap();
        let length = u16::from_be_bytes([header[2], header[3]]) as usize;
        let mut body = vec![0; length - 8];
        socket.read_exact(&mut body).await.unwrap();
        (header[1], header[6], body)
    });
    let bulk_load = BulkLoad::new("table", BulkLoadOptions::default()).unwrap();
    let cancellation = bulk_load.cancellation_handle();
    let cancellation_for_task = cancellation.clone();
    let (release_sender, release_receiver) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let _ = release_receiver.await;
        cancellation_for_task.cancel();
    });
    let stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let mut io = MessageIo::new(stream, 16).unwrap();
    let parts =
        stream::once(async { Ok(vec![1, 2, 3, 4, 5, 6, 7, 8]) }).chain(stream::once(async move {
            let _ = release_sender.send(());
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok(vec![9])
        }));
    let canceled = tokio::time::timeout(
        Duration::from_secs(1),
        io.send_message_stream_with_cancellation(packet_type::BULK_LOAD, parts, &cancellation),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(canceled);
    let (status, packet_id, body) = server.await.unwrap();
    assert_eq!(status & 0x03, 0x03);
    assert_eq!(packet_id, 1);
    assert_eq!(body, vec![1, 2, 3, 4, 5, 6, 7, 8]);
}

#[tokio::test]
async fn producer_error_finishes_stream_message_with_residual_payload_and_ignore() {
    let (client, mut server) = tokio::io::duplex(128);
    let server_task = tokio::spawn(async move {
        let mut header = [0u8; 8];
        server.read_exact(&mut header).await.unwrap();
        let length = u16::from_be_bytes([header[2], header[3]]) as usize;
        let mut body = vec![0; length - 8];
        server.read_exact(&mut body).await.unwrap();
        (header[1], header[6], body)
    });
    let mut io = MessageIo::new(client, 16).unwrap();
    let parts = stream::iter(vec![
        Ok(vec![1, 2, 3]),
        Err(tedious_rs::transport::TransportError::InvalidPacket(
            "producer failed".to_owned(),
        )),
    ]);
    let error = io
        .send_message_stream_with_cancellation(
            packet_type::BULK_LOAD,
            parts,
            &BulkLoad::new("table", BulkLoadOptions::default())
                .unwrap()
                .cancellation_handle(),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        tedious_rs::transport::TransportError::InvalidPacket(message)
            if message == "producer failed"
    ));
    let (status, packet_id, body) = server_task.await.unwrap();
    assert_eq!(status & 0x03, 0x03);
    assert_eq!(packet_id, 1);
    assert_eq!(body, vec![1, 2, 3]);
}

#[tokio::test]
async fn preserves_reset_connection_on_every_incremental_sql_batch_packet() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut packets = Vec::new();
        loop {
            let mut header = [0u8; 8];
            socket.read_exact(&mut header).await.unwrap();
            let length = u16::from_be_bytes([header[2], header[3]]) as usize;
            let mut body = vec![0; length - 8];
            socket.read_exact(&mut body).await.unwrap();
            let last = header[1] & 1 != 0;
            packets.push((header[1], body));
            if last {
                break;
            }
        }
        packets
    });
    let stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let mut io = MessageIo::new(stream, 16).unwrap();
    io.send_message_buffers_with_flags(
        packet_type::SQL_BATCH,
        true,
        &[vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]],
    )
    .await
    .unwrap();
    let packets = tokio::time::timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(packets.len(), 2);
    assert!(packets.iter().all(|(status, _)| status & 0x08 == 0x08));
    assert_eq!(packets[0].1, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(packets[1].1, vec![9, 10, 11]);
}

#[tokio::test]
async fn rejects_invalid_packet_size_before_io() {
    let stream = tokio::io::duplex(64).0;
    assert!(MessageIo::new(stream, 8).is_err());
}

#[tokio::test]
async fn accepts_received_packets_larger_than_configured_packet_size() {
    let (client, mut server) = tokio::io::duplex(64);
    let server_task = tokio::spawn(async move {
        server
            .write_all(&[
                packet_type::TABULAR_RESULT,
                1,
                0,
                17,
                0,
                0,
                1,
                0,
                1,
                2,
                3,
                4,
                5,
                6,
                7,
                8,
                9,
            ])
            .await
            .unwrap();
    });
    let mut io = MessageIo::new(client, 16).unwrap();
    let message = io.read_message().await.unwrap();
    assert_eq!(message.data, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    server_task.await.unwrap();
}

#[tokio::test]
async fn wraps_tls_records_in_tds_prelogin_packets() {
    let (client, mut server) = tokio::io::duplex(128);
    let mut tls_stream = TdsTlsStream::new(client, 32).unwrap();
    let server_task = tokio::spawn(async move {
        let mut header = [0u8; 8];
        server.read_exact(&mut header).await.unwrap();
        assert_eq!(header[0], packet_type::PRELOGIN);
        assert_eq!(header[1], 1);
        assert_eq!(header[6], 1);
        let length = u16::from_be_bytes([header[2], header[3]]) as usize;
        let mut body = vec![0; length - 8];
        server.read_exact(&mut body).await.unwrap();
        assert_eq!(body, b"tls-record");
        server
            .write_all(&[packet_type::PRELOGIN, 1, 0, 9, 0, 0, 1, 0, 0])
            .await
            .unwrap();
    });
    tls_stream.write_all(b"tls-record").await.unwrap();
    let mut response = [0; 1];
    tls_stream.read_exact(&mut response).await.unwrap();
    server_task.await.unwrap();
}

#[tokio::test]
async fn fragments_tls_handshake_data_with_eom_only_on_the_last_packet() {
    let (client, mut server) = tokio::io::duplex(128);
    let mut tls_stream = TdsTlsStream::new(client, 16).unwrap();
    let server_task = tokio::spawn(async move {
        let mut packets = Vec::new();
        for _ in 0..2 {
            let mut header = [0u8; 8];
            server.read_exact(&mut header).await.unwrap();
            let length = u16::from_be_bytes([header[2], header[3]]) as usize;
            let mut body = vec![0; length - 8];
            server.read_exact(&mut body).await.unwrap();
            packets.push((header[6], header[1], body));
        }
        server
            .write_all(&[packet_type::PRELOGIN, 1, 0, 9, 0, 0, 1, 0, 0])
            .await
            .unwrap();
        packets
    });
    tls_stream.write_all(&[0xAB; 12]).await.unwrap();
    let mut response = [0; 1];
    tls_stream.read_exact(&mut response).await.unwrap();
    let packets = server_task.await.unwrap();
    assert_eq!(packets[0], (1, 0, vec![0xAB; 8]));
    assert_eq!(packets[1], (2, 1, vec![0xAB; 4]));
}

#[tokio::test]
async fn restarts_tls_packet_ids_for_each_handshake_message() {
    let (client, mut server) = tokio::io::duplex(128);
    let mut tls_stream = TdsTlsStream::new(client, 16).unwrap();
    let server_task = tokio::spawn(async move {
        let mut ids = Vec::new();
        for _ in 0..2 {
            let mut header = [0u8; 8];
            server.read_exact(&mut header).await.unwrap();
            let length = u16::from_be_bytes([header[2], header[3]]) as usize;
            let mut body = vec![0; length - 8];
            server.read_exact(&mut body).await.unwrap();
            ids.push(header[6]);
            server
                .write_all(&[packet_type::PRELOGIN, 1, 0, 9, 0, 0, 1, 0, 0])
                .await
                .unwrap();
        }
        ids
    });
    let mut response = [0; 1];
    tls_stream.write_all(&[0xAB; 4]).await.unwrap();
    tls_stream.read_exact(&mut response).await.unwrap();
    tls_stream.write_all(&[0xCD; 4]).await.unwrap();
    tls_stream.read_exact(&mut response).await.unwrap();
    assert_eq!(server_task.await.unwrap(), vec![1, 1]);
}

#[tokio::test]
async fn changes_tls_record_packet_size_after_negotiation() {
    let (client, mut server) = tokio::io::duplex(128);
    let mut tls_stream = TdsTlsStream::new(client, 32).unwrap();
    tls_stream.set_packet_size(16).unwrap();
    let server_task = tokio::spawn(async move {
        let mut header = [0u8; 8];
        server.read_exact(&mut header).await.unwrap();
        assert_eq!(u16::from_be_bytes([header[2], header[3]]), 16);
        let mut body = vec![0; 8];
        server.read_exact(&mut body).await.unwrap();
        assert_eq!(body, vec![0xAB; 8]);
        server
            .write_all(&[packet_type::PRELOGIN, 1, 0, 9, 0, 0, 1, 0, 0])
            .await
            .unwrap();
    });
    tls_stream.write_all(&[0xAB; 8]).await.unwrap();
    let mut response = [0; 1];
    tls_stream.read_exact(&mut response).await.unwrap();
    server_task.await.unwrap();
}

#[tokio::test]
async fn unwraps_tds_prelogin_packets_for_tls() {
    let (client, mut server) = tokio::io::duplex(128);
    let mut tls_stream = TdsTlsStream::new(client, 32).unwrap();
    let server_task = tokio::spawn(async move {
        let mut packet = vec![packet_type::PRELOGIN, 1, 0, 0, 0, 0, 1, 0];
        packet.extend_from_slice(b"tls-record");
        let length = packet.len() as u16;
        packet[2..4].copy_from_slice(&length.to_be_bytes());
        server.write_all(&packet).await.unwrap();
    });
    let mut data = vec![0; 10];
    tls_stream.read_exact(&mut data).await.unwrap();
    assert_eq!(&data, b"tls-record");
    server_task.await.unwrap();
}
