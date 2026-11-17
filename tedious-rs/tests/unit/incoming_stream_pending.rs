use tedious_rs::incoming_message_stream::{IncomingMessageStream, MessageStreamEvent};
use tedious_rs::message::{Message, packetize};
use tedious_rs::packet::packet_type;

#[test]
fn public_stream_recovers_complete_message_after_error() {
    let first = Message {
        message_type: packet_type::TABULAR_RESULT,
        reset_connection: false,
        ignore: false,
        data: b"first".to_vec(),
    };
    let mut wire = packetize(&first, 64).unwrap().remove(0).buffer;
    wire.extend_from_slice(&[packet_type::SQL_BATCH, 0, 0, 7, 0, 0, 1, 0]);
    let mut stream = IncomingMessageStream::new();

    assert!(stream.feed(&wire).is_err());
    assert_eq!(stream.take_pending_messages(), vec![first]);
    assert!(stream.take_pending_messages().is_empty());
}

#[test]
fn public_stream_recovers_partial_events_after_error() {
    let mut message = Message::new(packet_type::TABULAR_RESULT);
    message.data = b"partial".to_vec();
    let mut packet = packetize(&message, 64).unwrap().remove(0);
    packet.last(Some(false));
    let mut wire = packet.buffer;
    wire.extend_from_slice(&[packet_type::SQL_BATCH, 0, 0, 7, 0, 0, 1, 0]);
    let mut stream = IncomingMessageStream::new();

    assert!(stream.feed(&wire).is_err());
    assert_eq!(
        stream.take_pending_events(),
        vec![
            MessageStreamEvent::MessageStarted(Message::new(packet_type::TABULAR_RESULT)),
            MessageStreamEvent::Data {
                message_type: packet_type::TABULAR_RESULT,
                data: b"partial".to_vec(),
            },
        ]
    );
    assert!(stream.take_pending_events().is_empty());
}
