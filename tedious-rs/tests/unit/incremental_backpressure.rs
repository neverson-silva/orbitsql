use tedious_rs::incoming_message_stream::{
    IncomingMessageStream, MessageStreamControl, MessageStreamEvent,
};
use tedious_rs::message::{Message, packetize};
use tedious_rs::packet::packet_type;

fn packet(message_type: u8, end: bool, data: &[u8]) -> Vec<u8> {
    let mut message = Message::new(message_type);
    message.data = data.to_vec();
    let mut packet = packetize(&message, 64).unwrap().remove(0);
    packet.last(Some(end));
    packet.buffer
}

#[test]
fn incremental_sink_resumes_after_pending_data_at_the_next_packet() {
    let first = packet(packet_type::TABULAR_RESULT, false, b"first");
    let second = packet(packet_type::TABULAR_RESULT, true, b"second");
    let mut wire = first.clone();
    wire.extend_from_slice(&second);
    let mut stream = IncomingMessageStream::new();
    let mut events = Vec::new();
    let mut first_data = true;
    let result = stream
        .feed_incremental_with_sink(&wire, &mut |event| {
            let pending = first_data && matches!(event, MessageStreamEvent::Data { .. });
            if matches!(event, MessageStreamEvent::Data { .. }) {
                first_data = false;
            }
            events.push(event);
            if pending {
                MessageStreamControl::Pending
            } else {
                MessageStreamControl::Continue
            }
        })
        .unwrap();

    assert_eq!(result, MessageStreamControl::Pending);
    assert_eq!(stream.buffered_len(), second.len());
    assert_eq!(events.len(), 2);

    assert_eq!(
        stream
            .resume_incremental_with_sink(&mut |event| {
                events.push(event);
                MessageStreamControl::Continue
            })
            .unwrap(),
        MessageStreamControl::Continue
    );
    assert_eq!(stream.buffered_len(), 0);
    assert_eq!(
        events,
        vec![
            MessageStreamEvent::MessageStarted(Message::new(packet_type::TABULAR_RESULT)),
            MessageStreamEvent::Data {
                message_type: packet_type::TABULAR_RESULT,
                data: b"first".to_vec(),
            },
            MessageStreamEvent::Data {
                message_type: packet_type::TABULAR_RESULT,
                data: b"second".to_vec(),
            },
            MessageStreamEvent::MessageEnded {
                message_type: packet_type::TABULAR_RESULT,
            },
        ]
    );
}

#[test]
fn incremental_sink_pending_between_messages_preserves_the_second_message() {
    let first = packet(packet_type::PRELOGIN, true, b"one");
    let second = packet(packet_type::LOGIN7, true, b"two");
    let mut wire = first.clone();
    wire.extend_from_slice(&second);
    let mut stream = IncomingMessageStream::new();
    let mut events = Vec::new();
    assert_eq!(
        stream
            .feed_incremental_with_sink(&wire, &mut |event| {
                let pending = matches!(event, MessageStreamEvent::MessageEnded { .. });
                events.push(event);
                if pending {
                    MessageStreamControl::Pending
                } else {
                    MessageStreamControl::Continue
                }
            })
            .unwrap(),
        MessageStreamControl::Pending
    );
    assert_eq!(stream.buffered_len(), second.len());
    assert_eq!(events.len(), 3);

    stream
        .resume_incremental_with_sink(&mut |event| {
            events.push(event);
            MessageStreamControl::Continue
        })
        .unwrap();

    assert_eq!(stream.buffered_len(), 0);
    assert_eq!(
        events[3],
        MessageStreamEvent::MessageStarted(Message::new(packet_type::LOGIN7))
    );
    assert_eq!(
        events[4],
        MessageStreamEvent::Data {
            message_type: packet_type::LOGIN7,
            data: b"two".to_vec(),
        }
    );
    assert_eq!(
        events[5],
        MessageStreamEvent::MessageEnded {
            message_type: packet_type::LOGIN7,
        }
    );
}

#[test]
fn incremental_sink_pending_at_message_start_retries_the_same_packet_data() {
    let wire = packet(packet_type::RPC_REQUEST, true, b"payload");
    let mut stream = IncomingMessageStream::new();
    let mut events = Vec::new();
    assert_eq!(
        stream
            .feed_incremental_with_sink(&wire, &mut |event| {
                events.push(event);
                MessageStreamControl::Pending
            })
            .unwrap(),
        MessageStreamControl::Pending
    );
    assert_eq!(stream.buffered_len(), wire.len());

    stream
        .resume_incremental_with_sink(&mut |event| {
            events.push(event);
            MessageStreamControl::Continue
        })
        .unwrap();

    assert_eq!(stream.buffered_len(), 0);
    assert_eq!(events.len(), 3);
    assert_eq!(
        events[0],
        MessageStreamEvent::MessageStarted(Message::new(packet_type::RPC_REQUEST))
    );
    assert_eq!(
        events[1],
        MessageStreamEvent::Data {
            message_type: packet_type::RPC_REQUEST,
            data: b"payload".to_vec(),
        }
    );
    assert_eq!(
        events[2],
        MessageStreamEvent::MessageEnded {
            message_type: packet_type::RPC_REQUEST,
        }
    );
}
