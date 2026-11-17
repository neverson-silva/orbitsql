use tedious_rs::incoming_message_stream::{IncomingMessageStream, MessageStreamEvent};
use tedious_rs::message::{Message, packetize};
use tedious_rs::packet::packet_type;

fn packet(message_type: u8, last: bool, data: &[u8]) -> Vec<u8> {
    let mut message = Message::new(message_type);
    message.data = data.to_vec();
    let mut packet = packetize(&message, 64).unwrap().remove(0);
    packet.last(Some(last));
    packet.buffer
}

#[test]
fn feed_incremental_continues_the_buffer_started_by_feed() {
    let wire = packet(packet_type::TABULAR_RESULT, true, b"payload");
    let split = 5;
    let mut stream = IncomingMessageStream::new();

    assert!(stream.feed(&wire[..split]).unwrap().is_empty());
    let events = stream.feed_incremental(&wire[split..]).unwrap();

    assert_eq!(
        events,
        vec![
            MessageStreamEvent::MessageStarted(Message::new(packet_type::TABULAR_RESULT)),
            MessageStreamEvent::Data {
                message_type: packet_type::TABULAR_RESULT,
                data: b"payload".to_vec(),
            },
            MessageStreamEvent::MessageEnded {
                message_type: packet_type::TABULAR_RESULT,
            },
        ]
    );
    assert_eq!(stream.buffered_len(), 0);
}

#[test]
fn feed_continues_the_message_started_by_feed_incremental() {
    let first = packet(packet_type::TABULAR_RESULT, false, b"first");
    let second = packet(packet_type::LOGIN7, true, b"second");
    let mut stream = IncomingMessageStream::new();

    assert_eq!(
        stream.feed_incremental(&first).unwrap(),
        vec![
            MessageStreamEvent::MessageStarted(Message::new(packet_type::TABULAR_RESULT)),
            MessageStreamEvent::Data {
                message_type: packet_type::TABULAR_RESULT,
                data: b"first".to_vec(),
            },
        ]
    );
    let message = stream.feed(&second).unwrap();

    assert_eq!(
        message,
        vec![Message {
            message_type: packet_type::TABULAR_RESULT,
            reset_connection: false,
            ignore: false,
            data: b"firstsecond".to_vec(),
        }]
    );
}

#[test]
fn pause_and_terminal_state_are_shared_between_feed_apis() {
    let wire = packet(packet_type::PRELOGIN, true, b"payload");
    let mut stream = IncomingMessageStream::new();

    stream.pause();
    assert!(stream.feed(&wire).unwrap().is_empty());
    assert!(stream.feed_incremental(&[]).unwrap().is_empty());
    assert_eq!(stream.buffered_len(), wire.len());

    let events = stream.resume_incremental().unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(stream.finish(), Ok(()));
    assert!(stream.feed(&[]).is_err());
    assert!(stream.feed_incremental(&[]).is_err());
}
