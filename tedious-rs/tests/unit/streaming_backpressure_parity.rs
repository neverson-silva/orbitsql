use std::process::Command;

use tedious_rs::incoming_message_stream::{
    IncomingMessageStream, MessageStreamControl, MessageStreamEvent,
};
use tedious_rs::message::{Message, MessageStreamError, packetize};
use tedious_rs::outgoing_message_stream::OutgoingMessageStream;
use tedious_rs::packet::packet_type;

fn wire(messages: &[Message], packet_size: usize) -> Vec<u8> {
    messages
        .iter()
        .flat_map(|message| packetize(message, packet_size).unwrap())
        .flat_map(|packet| packet.buffer)
        .collect()
}

#[test]
fn incoming_pause_preserves_wire_bytes_and_message_order() {
    let first = Message {
        message_type: packet_type::PRELOGIN,
        reset_connection: false,
        ignore: false,
        data: (0..17).collect(),
    };
    let second = Message {
        message_type: packet_type::LOGIN7,
        reset_connection: false,
        ignore: false,
        data: vec![41, 42, 43],
    };
    let bytes = wire(&[first.clone(), second.clone()], 16);
    let mut stream = IncomingMessageStream::new();
    stream.pause();
    for chunk in bytes.chunks(2) {
        assert!(stream.feed(chunk).unwrap().is_empty());
    }
    assert!(stream.buffered_len() > 0);
    let received = stream.resume().unwrap();
    assert_eq!(received, vec![first, second]);
}

#[test]
fn incoming_invalid_length_is_terminal_without_later_emission() {
    let mut stream = IncomingMessageStream::new();
    let error = stream
        .feed(&[packet_type::SQL_BATCH, 0, 0, 7, 0, 0, 1, 0])
        .unwrap_err();
    assert_eq!(
        error,
        MessageStreamError("Unable to process incoming packet".to_owned())
    );
    assert_eq!(stream.feed(&[]), Err(error.clone()));
    assert_eq!(stream.resume(), Err(error));
}

#[test]
fn incoming_finish_is_terminal_after_clean_eof() {
    let mut stream = IncomingMessageStream::new();
    assert_eq!(stream.finish(), Ok(()));
    assert_eq!(
        stream.feed(&[]),
        Err(MessageStreamError(
            "incoming stream has finished".to_owned()
        ))
    );
}

#[test]
fn outgoing_pause_stops_reads_without_reordering_packets() {
    let mut stream = OutgoingMessageStream::new(16).unwrap();
    let message = Message {
        message_type: packet_type::SQL_BATCH,
        reset_connection: true,
        ignore: false,
        data: (0..29).collect(),
    };
    stream.write_message(&message).unwrap();
    stream.pause();
    assert!(stream.read_packet().is_none());
    stream.resume();
    let packets = std::iter::from_fn(|| stream.read_packet()).collect::<Vec<_>>();
    let mut reconstructed = Vec::new();
    for (index, packet) in packets.iter().enumerate() {
        assert_eq!(packet.buffer[6], (index as u8).wrapping_add(1));
        reconstructed.extend_from_slice(packet.data());
    }
    assert_eq!(reconstructed, message.data);
    assert!(packets.last().is_some_and(|packet| packet.is_last()));
}

#[test]
fn outgoing_parts_error_discards_buffered_packets_and_becomes_terminal() {
    let mut stream = OutgoingMessageStream::new(16).unwrap();
    let error = MessageStreamError("part failed".to_owned());
    assert_eq!(
        stream.write_message_parts(
            packet_type::SQL_BATCH,
            false,
            [Ok(vec![1, 2, 3, 4, 5, 6, 7, 8]), Err(error.clone())],
        ),
        Err(error.clone())
    );
    assert_eq!(stream.pending_packets(), 0);
    assert!(stream.read_packet().is_none());
    assert_eq!(
        stream.write_message(&Message::new(packet_type::SQL_BATCH)),
        Err(error)
    );
}

#[test]
fn outgoing_finish_keeps_queued_packets_readable_and_rejects_new_messages() {
    let mut stream = OutgoingMessageStream::new(16).unwrap();
    let message = Message {
        message_type: packet_type::SQL_BATCH,
        reset_connection: false,
        ignore: false,
        data: b"payload".to_vec(),
    };
    stream.write_message(&message).unwrap();
    stream.finish();

    let packet = stream
        .read_packet()
        .expect("queued packet must remain readable");
    assert!(packet.is_last());
    assert_eq!(packet.data(), message.data);
    assert!(stream.read_packet().is_none());
    assert!(stream.write_message(&message).is_err());
}

#[test]
fn outgoing_parts_join_fragmented_input_before_packetizing() {
    let mut stream = OutgoingMessageStream::new(12).unwrap();
    let parts = [
        Ok(b"ab".to_vec()),
        Ok(Vec::new()),
        Ok(b"cdefghijkl".to_vec()),
        Ok(b"mnop".to_vec()),
    ];
    stream
        .write_message_parts(packet_type::SQL_BATCH, true, parts)
        .unwrap();

    let packets = std::iter::from_fn(|| stream.read_packet()).collect::<Vec<_>>();
    assert_eq!(packets.len(), 4);
    assert_eq!(packets[0].data(), b"abcd");
    assert_eq!(packets[1].data(), b"efgh");
    assert_eq!(packets[2].data(), b"ijkl");
    assert_eq!(packets[3].data(), b"mnop");
    assert!(!packets[0].is_last());
    assert!(packets[3].is_last());
    assert!(
        packets
            .iter()
            .all(|packet| packet.status() & tedious_rs::packet::status::RESET_CONNECTION != 0)
    );
}

#[test]
fn node_and_rust_incoming_stream_runtime_sequences_match() {
    let output = Command::new("node")
        .arg("tests/compatibility/node_incoming_stream_runtime_parity.js")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Node stream runtime failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let node_events = serde_json::from_slice::<Vec<String>>(&output.stdout).unwrap();

    let first = Message {
        message_type: packet_type::TABULAR_RESULT,
        reset_connection: false,
        ignore: false,
        data: b"first".to_vec(),
    };
    let second = Message {
        message_type: packet_type::TABULAR_RESULT,
        reset_connection: false,
        ignore: false,
        data: b"second".to_vec(),
    };
    let first_wire = packet(&first, 64, false);
    let second_wire = packet(&second, 64, true);
    let mut stream = IncomingMessageStream::new();
    let mut rust_events = Vec::new();
    assert_eq!(
        stream
            .feed_incremental_with_sink(&first_wire, &mut |event| {
                record_event(&mut rust_events, event);
                MessageStreamControl::Continue
            })
            .unwrap(),
        MessageStreamControl::Continue
    );
    rust_events.push("firstWrite:false".to_owned());
    stream.pause();
    assert_eq!(
        stream
            .feed_incremental_with_sink(&second_wire, &mut |event| {
                record_event(&mut rust_events, event);
                MessageStreamControl::Continue
            })
            .unwrap(),
        MessageStreamControl::Pending
    );
    stream
        .resume_incremental_with_sink(&mut |event| {
            record_event(&mut rust_events, event);
            MessageStreamControl::Continue
        })
        .unwrap();
    rust_events.push("secondWrite:true".to_owned());
    assert_eq!(node_events, rust_events);
}

fn packet(message: &Message, packet_size: usize, last: bool) -> Vec<u8> {
    packetize(message, packet_size)
        .unwrap()
        .into_iter()
        .flat_map(|mut packet| {
            packet.last(Some(last));
            packet.buffer
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn record_event(events: &mut Vec<String>, event: MessageStreamEvent) {
    match event {
        MessageStreamEvent::MessageStarted(message) => {
            events.push(format!("message:{}", message.message_type))
        }
        MessageStreamEvent::Data { data, .. } => events.push(format!("data:{}", hex(&data))),
        MessageStreamEvent::MessageEnded { .. } => events.push("messageEnd".to_owned()),
    }
}
