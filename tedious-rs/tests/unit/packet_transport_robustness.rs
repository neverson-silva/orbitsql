use tedious_rs::packet::{HEADER_LENGTH, Packet, packet_type};
use tedious_rs::transport::TdsTlsStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

#[test]
fn packet_accessors_reject_short_buffers_without_panicking() {
    let mut packet = Packet { buffer: vec![1] };
    assert_eq!(packet.length(), 0);
    assert_eq!(packet.packet_type(), 1);
    assert_eq!(packet.status(), 0);
    assert_eq!(packet.packet_id(None), 0);
    assert!(packet.data().is_empty());
    assert!(!packet.is_last());
    assert!(packet.set_length().is_err());
    assert!(packet.add_data(&[1]).is_err());
    assert_eq!(packet.buffer, vec![1]);
    assert!(Packet::from_buffer(vec![0; HEADER_LENGTH - 1]).is_err());
}

#[test]
fn packet_ignore_accepts_short_buffers_without_panicking() {
    for buffer in [Vec::new(), vec![1]] {
        let mut packet = Packet { buffer };
        let original = packet.buffer.clone();
        packet.ignore(true);
        packet.ignore(false);
        assert_eq!(packet.buffer, original);
    }
}

#[test]
fn packet_length_mutations_reject_u16_overflow_without_mutating() {
    let mut packet = Packet::new(packet_type::SQL_BATCH);
    packet.buffer.resize(u16::MAX as usize, 0);
    let original = packet.buffer.len();
    assert!(packet.add_data(&[1]).is_err());
    assert_eq!(packet.buffer.len(), original);
    packet.buffer.push(1);
    assert!(packet.set_length().is_err());
}

#[tokio::test]
async fn prelogin_tls_packets_advance_ids_without_changing_end_of_message() {
    let (client, mut server) = duplex(256);
    let mut stream = TdsTlsStream::new(client, 16).unwrap();
    let server_task = tokio::spawn(async move {
        let mut packets = Vec::new();
        for _ in 0..2 {
            let mut header = [0; HEADER_LENGTH];
            server.read_exact(&mut header).await.unwrap();
            let length = u16::from_be_bytes([header[2], header[3]]) as usize;
            let mut data = vec![0; length - HEADER_LENGTH];
            server.read_exact(&mut data).await.unwrap();
            packets.push((header[6], header[1], data));
        }
        server
            .write_all(&[packet_type::PRELOGIN, 1, 0, 9, 0, 0, 1, 0, 0])
            .await
            .unwrap();
        packets
    });
    stream.write_all(b"first").await.unwrap();
    stream.write_all(b"second").await.unwrap();
    let mut response = [0; 1];
    stream.read_exact(&mut response).await.unwrap();
    let packets = server_task.await.unwrap();
    assert_eq!(packets[0], (1, 0, b"firstsec".to_vec()));
    assert_eq!(packets[1], (2, 1, b"ond".to_vec()));
}

#[tokio::test]
async fn prelogin_tls_reports_truncated_packet_at_eof() {
    let (client, mut server) = duplex(64);
    let mut stream = TdsTlsStream::new(client, 32).unwrap();
    server
        .write_all(&[packet_type::PRELOGIN, 1, 0, 16, 0, 0, 1, 0, 1])
        .await
        .unwrap();
    drop(server);
    let mut output = [0; 1];
    let error = stream.read(&mut output).await.unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
}
