use tedious_rs::packet::packet_type;
use tedious_rs::transport::{MessageIo, TransportError};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn attention_message_has_one_complete_packet() {
    let (client, mut server) = tokio::io::duplex(32);
    let mut io = MessageIo::new(client, 16).unwrap();
    io.send_attention().await.unwrap();
    let mut packet = [0u8; 8];
    server.read_exact(&mut packet).await.unwrap();

    assert_eq!(packet[0], packet_type::ATTENTION);
    assert_eq!(packet[1] & tedious_rs::packet::status::EOM, 1);
    assert_eq!(u16::from_be_bytes([packet[2], packet[3]]), 8);
}

#[tokio::test]
async fn truncated_attention_response_is_not_accepted_as_a_message() {
    let (client, mut server) = tokio::io::duplex(32);
    let mut io = MessageIo::new(client, 16).unwrap();
    server
        .write_all(&[packet_type::TABULAR_RESULT, 1, 0, 8, 0, 0, 1])
        .await
        .unwrap();
    drop(server);

    let error = io.read_message().await.unwrap_err();
    assert!(
        matches!(error, TransportError::Io(value) if value.kind() == std::io::ErrorKind::UnexpectedEof)
    );
}
