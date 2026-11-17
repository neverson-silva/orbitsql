use tedious_rs::message::{IncomingMessageStream, MessageStreamError};
use tedious_rs::outgoing_message_stream::OutgoingMessageStream;
use tedious_rs::packet::packet_type;

#[test]
fn incoming_finish_rejects_a_truncated_packet_or_message() {
    let mut stream = IncomingMessageStream::new();
    assert!(stream.feed(&[1, 0, 0, 12, 0]).unwrap().is_empty());
    assert_eq!(
        stream.finish(),
        Err(MessageStreamError(
            "incoming stream ended before the message was complete".to_owned()
        ))
    );
}

#[test]
fn outgoing_parts_packetize_without_materializing_one_message_buffer() {
    let mut stream = OutgoingMessageStream::new(12).unwrap();
    stream
        .write_message_parts(
            packet_type::SQL_BATCH,
            true,
            [Ok(vec![0, 1, 2]), Ok((3..25).collect())],
        )
        .unwrap();
    let mut payload = Vec::new();
    let mut packets = 0;
    while let Some(packet) = stream.read_packet() {
        payload.extend_from_slice(packet.data());
        packets += 1;
        if packet.is_last() {
            break;
        }
    }
    assert_eq!(packets, 7);
    assert_eq!(payload, (0..25).collect::<Vec<_>>());
}
