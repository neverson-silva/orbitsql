use std::time::Duration;

use tedious_rs::packet::packet_type;
use tedious_rs::transport::{IncomingMessageChunkResult, MessageIo};
use tokio::io::{AsyncWriteExt, duplex};

fn packet(message_type: u8, status: u8, packet_id: u8, data: &[u8]) -> Vec<u8> {
    let length = (8 + data.len()) as u16;
    let mut wire = vec![message_type, status, 0, 0, 0, 0, packet_id, 0];
    wire[2..4].copy_from_slice(&length.to_be_bytes());
    wire.extend_from_slice(data);
    wire
}

#[tokio::test]
async fn accepts_fragmented_oversized_packets_and_preserves_message_boundaries() {
    let (mut writer, reader) = duplex(256);
    let first = packet(packet_type::TABULAR_RESULT, 0, 1, b"oversized");
    let second = packet(packet_type::TABULAR_RESULT, 1, 2, b"-message");
    let third = packet(packet_type::RPC_REQUEST, 1, 1, b"next");
    let mut wire = first;
    wire.extend_from_slice(&second);
    wire.extend_from_slice(&third);
    let writer_task = tokio::spawn(async move {
        for fragment in wire.chunks(3) {
            writer.write_all(fragment).await.unwrap();
            tokio::task::yield_now().await;
        }
    });

    let mut io = MessageIo::new(reader, 16).unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), io.next_message_chunk())
            .await
            .unwrap()
            .unwrap(),
        IncomingMessageChunkResult::Chunk(tedious_rs::transport::IncomingMessageChunk {
            message_type: packet_type::TABULAR_RESULT,
            reset_connection: false,
            ignore: false,
            data: b"oversized".to_vec(),
            end_of_message: false,
        })
    );
    assert_eq!(
        io.next_message_chunk().await.unwrap(),
        IncomingMessageChunkResult::Chunk(tedious_rs::transport::IncomingMessageChunk {
            message_type: packet_type::TABULAR_RESULT,
            reset_connection: false,
            ignore: false,
            data: b"-message".to_vec(),
            end_of_message: true,
        })
    );
    assert_eq!(
        io.next_message_chunk().await.unwrap(),
        IncomingMessageChunkResult::Chunk(tedious_rs::transport::IncomingMessageChunk {
            message_type: packet_type::RPC_REQUEST,
            reset_connection: false,
            ignore: false,
            data: b"next".to_vec(),
            end_of_message: true,
        })
    );
    writer_task.await.unwrap();
}
