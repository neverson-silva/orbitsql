use tedious_rs::incoming_message_stream::{
    IncomingMessageStream, MessageStreamControl, MessageStreamEvent,
};
use tedious_rs::message::{Message, packetize};
use tedious_rs::outgoing_message_stream::OutgoingMessageStream;
use tedious_rs::packet::packet_type;

fn packet(message: &Message, packet_size: usize) -> Vec<u8> {
    packetize(message, packet_size)
        .unwrap()
        .into_iter()
        .flat_map(|packet| packet.buffer)
        .collect()
}

#[test]
fn incoming_drain_reprocesses_buffered_data_after_sink_backpressure() {
    let message = Message {
        message_type: packet_type::TABULAR_RESULT,
        reset_connection: false,
        ignore: false,
        data: b"payload".to_vec(),
    };
    let wire = packet(&message, 64);
    let mut stream = IncomingMessageStream::new();
    let mut events = Vec::new();
    let mut pending = true;

    assert_eq!(
        stream
            .feed_incremental_with_sink(&wire, &mut |event| {
                events.push(event);
                if pending {
                    pending = false;
                    MessageStreamControl::Pending
                } else {
                    MessageStreamControl::Continue
                }
            })
            .unwrap(),
        MessageStreamControl::Pending
    );
    assert_eq!(stream.buffered_len(), wire.len());

    assert_eq!(
        stream
            .drain_incremental_with_sink(&mut |event| {
                events.push(event);
                MessageStreamControl::Continue
            })
            .unwrap(),
        MessageStreamControl::Continue
    );
    assert_eq!(stream.buffered_len(), 0);
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], MessageStreamEvent::MessageStarted(_)));
    assert!(matches!(events[1], MessageStreamEvent::Data { .. }));
    assert!(matches!(events[2], MessageStreamEvent::MessageEnded { .. }));
}

#[test]
fn outgoing_drain_resumes_packet_delivery_after_sink_backpressure() {
    let mut stream = OutgoingMessageStream::new(12).unwrap();
    let message = Message {
        message_type: packet_type::SQL_BATCH,
        reset_connection: false,
        ignore: false,
        data: (0..12).collect(),
    };
    stream.write_message(&message).unwrap();
    let mut packets = Vec::new();

    assert_eq!(
        stream
            .read_packet_with_sink(&mut |packet| {
                packets.push(packet);
                MessageStreamControl::Pending
            })
            .unwrap(),
        MessageStreamControl::Pending
    );
    assert!(stream.is_paused());
    assert_eq!(stream.pending_packets(), 2);

    stream.resume();
    while stream
        .read_packet_with_sink(&mut |packet| {
            packets.push(packet);
            MessageStreamControl::Continue
        })
        .unwrap()
        == MessageStreamControl::Continue
        && stream.pending_packets() > 0
    {}

    assert_eq!(packets.len(), 3);
    assert!(packets.last().is_some_and(|packet| packet.is_last()));
}
