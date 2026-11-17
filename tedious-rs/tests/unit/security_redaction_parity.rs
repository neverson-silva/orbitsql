use std::sync::{Arc, Mutex};

use tedious_rs::debug::{Debug, DebugOptions};
use tedious_rs::packet::{Packet, packet_type};

fn debug_output(packet_type: u8, data: &[u8]) -> String {
    let output = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&output);
    let mut debug = Debug::new(DebugOptions {
        data: true,
        ..DebugOptions::default()
    });
    debug.on(move |value| captured.lock().unwrap().push(value.to_owned()));
    let mut packet = Packet::new(packet_type);
    packet.add_data(data).unwrap();
    debug.data(&packet);
    output.lock().unwrap().join("\n")
}

#[test]
fn debug_data_redacts_node_sensitive_packet_payloads() {
    let sensitive_packet_types = [
        packet_type::SQL_BATCH,
        packet_type::RPC_REQUEST,
        packet_type::TABULAR_RESULT,
        packet_type::BULK_LOAD,
        packet_type::LOGIN7,
        packet_type::NTLMAUTH_PKT,
        packet_type::FEDAUTH_TOKEN,
    ];
    let secret = b"secret-value-7f4d";

    for packet_type in sensitive_packet_types {
        let rendered = debug_output(packet_type, secret);
        assert_eq!(rendered, "  [REDACTED PACKET DATA]");
        assert!(!rendered.contains("secret-value-7f4d"));
        assert!(!rendered.contains("736563726574"));
    }
}

#[test]
fn debug_data_keeps_non_sensitive_node_packet_format() {
    let rendered = debug_output(packet_type::PRELOGIN, &[1, 2, 3]);
    assert_eq!(rendered, "  0000  010203  ...");
}
