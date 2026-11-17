use std::io::ErrorKind;
use std::time::Duration;

use tedious_rs::message::{Message, packetize};
use tedious_rs::packet::packet_type;
use tedious_rs::transport::{IncomingMessageChunkResult, MessageIo, TransportError};
use tokio::io::{AsyncWriteExt, duplex};

fn packet(message_type: u8, last: bool, data: &[u8]) -> Vec<u8> {
    let mut message = Message::new(message_type);
    message.data = data.to_vec();
    let mut packet = packetize(&message, 64).unwrap().remove(0);
    packet.last(Some(last));
    packet.buffer
}

#[tokio::test]
async fn next_message_chunk_returns_each_packet_before_eom() {
    let (mut writer, reader) = duplex(256);
    let first = packet(packet_type::TABULAR_RESULT, false, b"first");
    let second = packet(packet_type::TABULAR_RESULT, true, b"second");
    let task = tokio::spawn(async move {
        writer.write_all(&first).await.unwrap();
        tokio::time::sleep(Duration::from_millis(150)).await;
        writer.write_all(&second).await.unwrap();
    });

    let mut io = MessageIo::new(reader, 64).unwrap();
    let result = tokio::time::timeout(Duration::from_millis(80), io.next_message_chunk())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        result,
        IncomingMessageChunkResult::Chunk(tedious_rs::transport::IncomingMessageChunk {
            message_type: packet_type::TABULAR_RESULT,
            reset_connection: false,
            ignore: false,
            data: b"first".to_vec(),
            end_of_message: false,
        })
    );
    assert!(io.next_message_chunk().await.is_ok());
    task.await.unwrap();
}

#[tokio::test]
async fn pause_prevents_reads_and_resume_continues_the_message() {
    let (mut writer, reader) = duplex(128);
    let bytes = packet(packet_type::SQL_BATCH, true, b"select 1");
    writer.write_all(&bytes).await.unwrap();
    let mut io = MessageIo::new(reader, 64).unwrap();
    io.pause_incoming();
    assert!(io.incoming_is_paused());
    assert_eq!(
        io.next_message_chunk().await.unwrap(),
        IncomingMessageChunkResult::Paused
    );
    io.resume_incoming();
    assert_eq!(
        io.next_message_chunk().await.unwrap(),
        IncomingMessageChunkResult::Chunk(tedious_rs::transport::IncomingMessageChunk {
            message_type: packet_type::SQL_BATCH,
            reset_connection: false,
            ignore: false,
            data: b"select 1".to_vec(),
            end_of_message: true,
        })
    );
}

#[tokio::test]
async fn partial_header_and_payload_reads_are_reassembled() {
    let (mut writer, reader) = duplex(128);
    let bytes = packet(packet_type::RPC_REQUEST, true, b"payload");
    let first = bytes[..3].to_vec();
    let second = bytes[3..].to_vec();
    let task = tokio::spawn(async move {
        writer.write_all(&first).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        writer.write_all(&second[..4]).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        writer.write_all(&second[4..]).await.unwrap();
    });
    let mut io = MessageIo::new(reader, 64).unwrap();
    let result = io.next_message_chunk().await.unwrap();
    match result {
        IncomingMessageChunkResult::Chunk(chunk) => {
            assert_eq!(chunk.message_type, packet_type::RPC_REQUEST);
            assert_eq!(chunk.data, b"payload");
            assert!(chunk.end_of_message);
        }
        other => panic!("unexpected result: {other:?}"),
    }
    task.await.unwrap();
}

#[tokio::test]
async fn clean_eof_is_terminal_and_truncated_message_is_an_error() {
    let (writer, reader) = duplex(64);
    drop(writer);
    let mut io = MessageIo::new(reader, 64).unwrap();
    assert_eq!(
        io.next_message_chunk().await.unwrap(),
        IncomingMessageChunkResult::EndOfStream
    );
    assert_eq!(
        io.next_message_chunk().await.unwrap(),
        IncomingMessageChunkResult::EndOfStream
    );

    let (mut writer, reader) = duplex(64);
    let bytes = packet(packet_type::TABULAR_RESULT, true, b"payload");
    writer.write_all(&bytes[..bytes.len() - 2]).await.unwrap();
    drop(writer);
    let mut io = MessageIo::new(reader, 64).unwrap();
    let error = io.next_message_chunk().await.unwrap_err();
    assert!(matches!(
        error,
        TransportError::Io(value) if value.kind() == ErrorKind::UnexpectedEof
    ));
    assert_eq!(
        io.next_message_chunk().await.unwrap(),
        IncomingMessageChunkResult::EndOfStream
    );
}

#[tokio::test]
async fn legacy_read_message_uses_incremental_path_and_preserves_eom_metadata() {
    let (mut writer, reader) = duplex(128);
    let first = packet(packet_type::TABULAR_RESULT, false, b"left");
    let second = packet(packet_type::TABULAR_RESULT, true, b"right");
    writer.write_all(&first).await.unwrap();
    writer.write_all(&second).await.unwrap();
    let mut io = MessageIo::new(reader, 64).unwrap();
    let message = io.read_message().await.unwrap();
    assert_eq!(message.message_type, packet_type::TABULAR_RESULT);
    assert_eq!(message.data, b"leftright");
}
