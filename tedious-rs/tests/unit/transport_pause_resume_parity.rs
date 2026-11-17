use std::time::Duration;

use tedious_rs::message::{Message, packetize};
use tedious_rs::packet::packet_type;
use tedious_rs::transport::MessageIo;
use tokio::io::{AsyncWriteExt, duplex};

fn packet(last: bool, data: &[u8]) -> Vec<u8> {
    let mut message = Message::new(packet_type::TABULAR_RESULT);
    message.data = data.to_vec();
    let mut packet = packetize(&message, 16).unwrap().remove(0);
    packet.last(Some(last));
    packet.buffer
}

#[tokio::test]
async fn pause_waits_for_resume_and_preserves_fragmented_message_bytes() {
    let (mut writer, reader) = duplex(64);
    let first = packet(false, b"left");
    let second = packet(true, b"right");
    let wire = [first, second].concat();
    let split = wire.len() - 3;
    writer.write_all(&wire[..split]).await.unwrap();
    let writer_task = tokio::spawn(async move {
        writer.write_all(&wire[split..]).await.unwrap();
    });

    let mut io = MessageIo::new(reader, 16).unwrap();
    let pause = io.incoming_pause_handle();
    pause.pause();
    let mut read = tokio::spawn(async move { io.read_message().await });
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut read)
            .await
            .is_err()
    );
    pause.resume();

    let result = tokio::time::timeout(Duration::from_secs(1), read)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(result.data, b"leftright");
    assert_eq!(result.message_type, packet_type::TABULAR_RESULT);
    writer_task.await.unwrap();
}
