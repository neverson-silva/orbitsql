use tedious_rs::message::{IncomingMessageStream, Message, packetize};
use tedious_rs::packet::{packet_type, status};

#[test]
fn packetizes_message_with_node_packet_size_and_sequence() {
    let mut message = Message::new(packet_type::SQL_BATCH);
    message.data = (0..20).collect();
    message.reset_connection = true;
    let mut packets = packetize(&message, 16).unwrap();
    assert_eq!(packets.len(), 3);
    assert_eq!(packets[0].packet_id(None), 1);
    assert_eq!(packets[1].packet_id(None), 2);
    assert_eq!(packets[2].packet_id(None), 3);
    assert_eq!(packets[0].data(), &[0, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(packets[2].data(), &[16, 17, 18, 19]);
    assert_eq!(
        packets[0].status() & status::RESET_CONNECTION,
        status::RESET_CONNECTION
    );
    assert_eq!(packets[2].status() & status::EOM, status::EOM);
}

#[test]
fn reassembles_fragmented_packets_and_multiple_messages() {
    let mut first = Message::new(packet_type::PRELOGIN);
    first.data = vec![1, 2, 3, 4, 5];
    let mut second = Message::new(packet_type::LOGIN7);
    second.data = vec![6, 7];
    let first_wire = packetize(&first, 10).unwrap();
    let second_wire = packetize(&second, 10).unwrap();
    let mut wire = Vec::new();
    for packet in &first_wire {
        wire.extend_from_slice(&packet.buffer);
    }
    wire.extend_from_slice(&second_wire[0].buffer);
    let mut stream = IncomingMessageStream::new();
    assert!(stream.feed(&wire[..3]).unwrap().is_empty());
    assert!(stream.feed(&wire[3..11]).unwrap().is_empty());
    let messages = stream.feed(&wire[11..]).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].message_type, packet_type::PRELOGIN);
    assert_eq!(messages[0].data, vec![1, 2, 3, 4, 5]);
    assert_eq!(messages[1].message_type, packet_type::LOGIN7);
    assert_eq!(messages[1].data, vec![6, 7]);
}

#[test]
fn rejects_invalid_packet_size_and_short_wire_length() {
    let message = Message::new(packet_type::SQL_BATCH);
    assert!(packetize(&message, 8).is_err());
    let mut stream = IncomingMessageStream::new();
    assert!(stream.feed(&[1, 0, 0, 7, 0, 0, 0, 1]).is_err());
}

#[test]
fn packetizes_attention_as_an_empty_end_of_message_packet() {
    let message = Message::new(packet_type::ATTENTION);
    let packets = packetize(&message, 4096).unwrap();
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].packet_type(), packet_type::ATTENTION);
    assert_eq!(packets[0].data(), &[] as &[u8]);
    assert_eq!(packets[0].status() & status::EOM, status::EOM);
}

#[test]
fn formats_packet_header_and_data_like_node_debug_output() {
    let mut packet = tedious_rs::packet::Packet::new(packet_type::PRELOGIN);
    packet.buffer[1] = status::EOM | status::IGNORE;
    packet.buffer[4..6].copy_from_slice(&1_u16.to_be_bytes());
    packet.buffer[7] = 3;
    packet.add_data(&[1, 2, 3]).unwrap();
    assert_eq!(packet.status_as_string(), "EOM IGNORE");
    assert_eq!(
        packet.header_to_string("--"),
        "--type:0x12(PRELOGIN), status:0x03(EOM IGNORE), length:0x000B, spid:0x0001, packetId:0x01, window:0x03"
    );
    assert_eq!(packet.data_to_string("--"), "--0000  010203  ...");
    assert_eq!(
        packet.to_string("--"),
        "--type:0x12(PRELOGIN), status:0x03(EOM IGNORE), length:0x000B, spid:0x0001, packetId:0x01, window:0x03\n----0000  010203  ..."
    );
    assert_eq!(packet.payload_string(), "");
}
