use tedious_rs::incoming_message_stream::{IncomingMessageStream, MessageStreamEvent};
use tedious_rs::message::{Message, packetize};
use tedious_rs::packet::packet_type;

#[test]
fn incremental_events_do_not_retain_the_complete_message_payload() {
    let mut message = Message::new(packet_type::TABULAR_RESULT);
    message.data = (0..64).collect();
    let mut packets = packetize(&message, 24).unwrap();
    packets[0].last(Some(false));
    let first = packets.remove(0).buffer;

    let mut stream = IncomingMessageStream::new();
    let events = stream.feed_incremental(&first).unwrap();

    assert_eq!(stream.buffered_len(), 0);
    assert_eq!(
        events,
        vec![
            MessageStreamEvent::MessageStarted(Message::new(packet_type::TABULAR_RESULT)),
            MessageStreamEvent::Data {
                message_type: packet_type::TABULAR_RESULT,
                data: (0..16).collect(),
            },
        ]
    );
}
