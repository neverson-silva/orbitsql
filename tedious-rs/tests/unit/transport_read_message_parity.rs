use std::time::Duration;

use tedious_rs::message::{Message, packetize};
use tedious_rs::packet::packet_type;
use tedious_rs::transport::{IncomingMessageChunkResult, MessageIo};
use tokio::io::{AsyncWriteExt, duplex};

fn packet(message_type: u8, end_of_message: bool, data: &[u8]) -> Vec<u8> {
    let mut message = Message::new(message_type);
    message.data = data.to_vec();
    let mut packet = packetize(&message, 64).unwrap().remove(0);
    packet.last(Some(end_of_message));
    packet.buffer
}

#[tokio::test]
async fn incremental_read_exposes_the_first_packet_before_message_completion() {
    let (mut writer, reader) = duplex(256);
    let first = packet(packet_type::TABULAR_RESULT, false, b"first");
    let second = packet(packet_type::TABULAR_RESULT, true, b"second");
    let writer_task = tokio::spawn(async move {
        writer.write_all(&first).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        writer.write_all(&second).await.unwrap();
    });

    let mut io = MessageIo::new(reader, 64).unwrap();
    let first_chunk = tokio::time::timeout(Duration::from_millis(50), io.next_message_chunk())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        first_chunk,
        IncomingMessageChunkResult::Chunk(tedious_rs::transport::IncomingMessageChunk {
            message_type: packet_type::TABULAR_RESULT,
            reset_connection: false,
            ignore: false,
            data: b"first".to_vec(),
            end_of_message: false,
        })
    );
    let second_chunk = io.next_message_chunk().await.unwrap();
    assert_eq!(
        second_chunk,
        IncomingMessageChunkResult::Chunk(tedious_rs::transport::IncomingMessageChunk {
            message_type: packet_type::TABULAR_RESULT,
            reset_connection: false,
            ignore: false,
            data: b"second".to_vec(),
            end_of_message: true,
        })
    );
    writer_task.await.unwrap();
}
