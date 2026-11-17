use std::sync::{Arc, Mutex};

use tedious_rs::debug::{Debug, DebugOptions};
use tedious_rs::incoming_message_stream::IncomingMessageStream;
use tedious_rs::message::{Message, MessageStreamEvent, packetize};
use tedious_rs::outgoing_message_stream::OutgoingMessageStream;
use tedious_rs::packet::packet_type;
use tedious_rs::transport::MessageIo;

#[test]
fn outgoing_stream_packetizes_messages_and_applies_backpressure() {
    let mut stream = OutgoingMessageStream::new(12).unwrap();
    let mut message = Message::new(packet_type::SQL_BATCH);
    message.data = (0..25).collect();
    stream.write_message(&message).unwrap();
    assert_eq!(stream.pending_packets(), 7);
    stream.pause();
    assert!(stream.read_packet().is_none());
    stream.resume();
    let mut first = stream.read_packet().unwrap();
    assert_eq!(first.packet_id(None), 1);
    assert!(!first.is_last());
    let mut last = first;
    while let Some(packet) = stream.read_packet() {
        last = packet;
    }
    assert!(last.is_last());
}

#[test]
fn incoming_stream_reassembles_packets_across_arbitrary_chunks() {
    let mut message = Message::new(packet_type::RPC_REQUEST);
    message.data = (0..19).collect();
    let packets = packetize(&message, 14).unwrap();
    let wire = packets
        .iter()
        .flat_map(|packet| packet.buffer.clone())
        .collect::<Vec<_>>();
    let mut stream = IncomingMessageStream::new();
    let mut result = Vec::new();
    for chunk in wire.chunks(3) {
        result.extend(stream.feed(chunk).unwrap());
    }
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message_type, packet_type::RPC_REQUEST);
    assert_eq!(result[0].data, message.data);
}

#[test]
fn incoming_stream_pause_defers_complete_messages_until_resume() {
    let mut message = Message::new(packet_type::SQL_BATCH);
    message.data = b"select 1".to_vec();
    let wire = packetize(&message, 64).unwrap()[0].buffer.clone();
    let mut stream = IncomingMessageStream::new();
    stream.pause();
    assert!(stream.feed(&wire).unwrap().is_empty());
    assert!(stream.is_paused());
    let messages = stream.resume().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].data, b"select 1");
}

#[test]
fn incoming_stream_keeps_node_message_flags_independent_of_packet_flags() {
    let mut message = Message::new(packet_type::TABULAR_RESULT);
    message.data = b"result".to_vec();
    let mut packet = packetize(&message, 64).unwrap().pop().unwrap();
    packet.reset_connection(true);
    packet.ignore(true);

    let mut stream = IncomingMessageStream::new();
    let received = stream.feed(&packet.buffer).unwrap();

    assert_eq!(received.len(), 1);
    assert_eq!(received[0].data, message.data);
    assert!(!received[0].reset_connection);
    assert!(!received[0].ignore);
}

#[test]
fn incoming_stream_publishes_incremental_message_events() {
    let first = packetize(
        &Message {
            message_type: packet_type::TABULAR_RESULT,
            reset_connection: false,
            ignore: false,
            data: b"first".to_vec(),
        },
        64,
    )
    .unwrap()
    .into_iter()
    .next()
    .map(|mut packet| {
        packet.last(Some(false));
        packet.buffer
    })
    .unwrap();
    let second = packetize(
        &Message {
            message_type: packet_type::TABULAR_RESULT,
            reset_connection: false,
            ignore: false,
            data: b"second".to_vec(),
        },
        64,
    )
    .unwrap()
    .into_iter()
    .next()
    .map(|mut packet| {
        packet.last(Some(true));
        packet.buffer
    })
    .unwrap();
    let mut stream = IncomingMessageStream::new();
    let mut wire = first;
    wire.extend_from_slice(&second);
    let events = stream.feed_incremental(&wire).unwrap();
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
fn legacy_feed_remains_materializing_and_exposes_the_node_backpressure_gap() {
    let mut message = Message::new(packet_type::TABULAR_RESULT);
    message.data = b"firstsecond".to_vec();
    let mut packets = packetize(&message, 64).unwrap();
    packets[0].last(Some(false));
    let mut stream = IncomingMessageStream::new();
    assert!(stream.feed(&packets[0].buffer).unwrap().is_empty());
    assert_eq!(stream.buffered_len(), 0);
}

#[test]
fn incoming_stream_keeps_incremental_events_paused_until_resume() {
    let message = Message {
        message_type: packet_type::PRELOGIN,
        reset_connection: false,
        ignore: false,
        data: b"payload".to_vec(),
    };
    let wire = packetize(&message, 64).unwrap()[0].buffer.clone();
    let mut stream = IncomingMessageStream::new();
    stream.pause();
    assert!(stream.feed_incremental(&wire).unwrap().is_empty());
    assert_eq!(stream.resume_incremental().unwrap().len(), 3);
}

#[test]
fn incoming_stream_processes_buffered_data_through_the_node_named_operation() {
    let message = Message {
        message_type: packet_type::PRELOGIN,
        reset_connection: false,
        ignore: false,
        data: b"payload".to_vec(),
    };
    let wire = packetize(&message, 64).unwrap()[0].buffer.clone();
    let mut stream = IncomingMessageStream::new();
    stream.pause();
    assert!(stream.feed_incremental(&wire).unwrap().is_empty());
    let mut events = Vec::new();
    let result = stream.process_buffered_data(&mut |event| {
        events.push(event);
        tedious_rs::message::MessageStreamControl::Continue
    });
    assert_eq!(
        result.unwrap(),
        tedious_rs::message::MessageStreamControl::Continue
    );
    assert_eq!(events.len(), 3);
}

#[test]
fn incoming_stream_keeps_first_message_type_when_packet_type_changes() {
    let first = packet(packet_type::PRELOGIN, false, b"left");
    let second = packet(packet_type::LOGIN7, true, b"right");
    let mut stream = IncomingMessageStream::new();
    let mut wire = first;
    wire.extend_from_slice(&second);
    let messages = stream.feed(&wire).unwrap();
    assert_eq!(messages[0].message_type, packet_type::PRELOGIN);
    assert_eq!(messages[0].data, b"leftright");
}

#[test]
fn incoming_stream_preserves_completed_messages_before_invalid_packet_error() {
    let first = Message {
        message_type: packet_type::TABULAR_RESULT,
        reset_connection: false,
        ignore: false,
        data: b"first".to_vec(),
    };
    let mut wire = packetize(&first, 64).unwrap().remove(0).buffer;
    wire.extend_from_slice(&[packet_type::SQL_BATCH, 0, 0, 7, 0, 0, 1, 0]);
    let mut stream = tedious_rs::message::IncomingMessageStream::new();

    assert!(stream.feed(&wire).is_err());
    assert_eq!(stream.take_pending_messages(), vec![first]);
    assert!(stream.take_pending_messages().is_empty());
}

#[test]
fn incoming_stream_preserves_partial_message_events_before_invalid_packet_error() {
    let mut wire = packet(packet_type::TABULAR_RESULT, false, b"partial");
    wire.extend_from_slice(&[packet_type::SQL_BATCH, 0, 0, 7, 0, 0, 1, 0]);
    let mut stream = tedious_rs::message::IncomingMessageStream::new();

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
    assert!(stream.take_pending_messages().is_empty());
}

fn packet(message_type: u8, last: bool, data: &[u8]) -> Vec<u8> {
    let mut message = Message::new(message_type);
    message.data = data.to_vec();
    let mut packet = packetize(&message, 64).unwrap().remove(0);
    packet.last(Some(last));
    packet.buffer
}

#[test]
fn outgoing_stream_allows_packet_size_changes_before_next_message() {
    let mut stream = OutgoingMessageStream::new(32).unwrap();
    assert_eq!(stream.packet_size(), 32);
    stream.set_packet_size(16).unwrap();
    assert_eq!(stream.packet_size(), 16);
    assert!(stream.set_packet_size(8).is_err());
}

#[test]
fn packet_ids_wrap_at_256_like_node_packet_id() {
    let mut message = Message::new(packet_type::SQL_BATCH);
    message.data = vec![0; 256 * 8];
    let packets = packetize(&message, 16).unwrap();
    assert_eq!(packets.len(), 256);
    assert_eq!(packets[0].buffer[6], 1);
    assert_eq!(packets[254].buffer[6], 255);
    assert_eq!(packets[255].buffer[6], 0);
}

#[test]
fn ignored_message_emits_one_empty_ignored_eom_packet() {
    let message = Message {
        message_type: packet_type::ATTENTION,
        reset_connection: false,
        ignore: true,
        data: b"discarded".to_vec(),
    };
    let packets = packetize(&message, 64).unwrap();
    assert_eq!(packets.len(), 1);
    assert!(packets[0].data().is_empty());
    assert!(packets[0].is_last());
    assert_ne!(packets[0].status() & tedious_rs::packet::status::IGNORE, 0);
}

#[test]
fn debug_emits_only_enabled_categories_to_external_listeners() {
    let output = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&output);
    let mut debug = Debug::new(DebugOptions {
        data: true,
        ..DebugOptions::default()
    });
    debug.on(move |value| seen.lock().unwrap().push(value.to_owned()));
    let packet = packetize(&Message::new(packet_type::PRELOGIN), 64)
        .unwrap()
        .pop()
        .unwrap();
    debug.packet("Sent", &packet);
    assert!(output.lock().unwrap().is_empty());
    debug.data(&packet);
    assert_eq!(output.lock().unwrap().as_slice(), &[String::new()]);
}

#[test]
fn debug_formats_packet_events_using_packet_debug_contract() {
    let output = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&output);
    let mut debug = Debug::new(DebugOptions {
        packet: true,
        data: true,
        ..DebugOptions::default()
    });
    debug.on(move |value| seen.lock().unwrap().push(value.to_owned()));
    let mut packet = packetize(&Message::new(packet_type::PRELOGIN), 64)
        .unwrap()
        .pop()
        .unwrap();
    packet.add_data(&[1, 2, 3]).unwrap();
    debug.packet("Sent", &packet);
    debug.data(&packet);
    let output = output.lock().unwrap();
    assert_eq!(
        output[0],
        "\nSent\n  type:0x12(PRELOGIN), status:0x01(EOM), length:0x000B, spid:0x0000, packetId:0x01, window:0x00"
    );
    assert_eq!(output[1], "  0000  010203  ...");
}

#[tokio::test]
async fn message_io_emits_packet_and_data_debug_events_on_wire_operations() {
    let (left, right) = tokio::io::duplex(128);
    let output = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&output);
    let mut writer = MessageIo::new(left, 64).unwrap();
    writer.set_debug(
        DebugOptions {
            packet: true,
            data: true,
            ..DebugOptions::default()
        },
        Arc::new(move |value| seen.lock().unwrap().push(value)),
    );
    let mut reader = MessageIo::new(right, 64).unwrap();
    let received_output = Arc::clone(&output);
    reader.set_debug(
        DebugOptions {
            packet: true,
            data: true,
            ..DebugOptions::default()
        },
        Arc::new(move |value| received_output.lock().unwrap().push(value)),
    );
    let message = Message {
        message_type: packet_type::SQL_BATCH,
        reset_connection: false,
        ignore: false,
        data: b"select 1".to_vec(),
    };
    let expected = message.clone();
    let send = tokio::spawn(async move {
        writer.send_message(&message).await.unwrap();
    });
    let received = reader.read_message().await.unwrap();
    send.await.unwrap();
    assert_eq!(received, expected);
    let output = output.lock().unwrap();
    assert!(output.iter().any(|value| value.contains("Sent")));
    assert!(output.iter().any(|value| value.contains("Received")));
}
