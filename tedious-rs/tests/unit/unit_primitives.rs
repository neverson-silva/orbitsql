use tedious_rs::errors::NotEnoughDataError;
use tedious_rs::packet::{
    HEADER_LENGTH, Packet, is_packet_complete, packet_length, packet_type, status,
};
use tedious_rs::tds_versions::{versions, versions_by_value};
use tedious_rs::token::helpers::*;
use tedious_rs::tracking_buffer::{Encoding, WritableTrackingBuffer};

#[test]
fn packet_header_uses_tds_layout_and_defaults() {
    let mut packet = Packet::new(packet_type::PRELOGIN);
    assert_eq!(packet.buffer, vec![0x12, 0, 0, 8, 0, 0, 1, 0]);
    assert_eq!(packet.length(), 8);
    assert_eq!(packet.packet_type(), packet_type::PRELOGIN);
    assert_eq!(packet.packet_id(None), 1);
}

#[test]
fn names_every_packet_type_declared_by_node() {
    let values = [
        (packet_type::SQL_BATCH, "SQL_BATCH"),
        (packet_type::RPC_REQUEST, "RPC_REQUEST"),
        (packet_type::TABULAR_RESULT, "TABULAR_RESULT"),
        (packet_type::ATTENTION, "ATTENTION"),
        (packet_type::BULK_LOAD, "BULK_LOAD"),
        (packet_type::FEDAUTH_TOKEN, "FEDAUTH_TOKEN"),
        (packet_type::TRANSACTION_MANAGER, "TRANSACTION_MANAGER"),
        (packet_type::LOGIN7, "LOGIN7"),
        (packet_type::NTLMAUTH_PKT, "NTLMAUTH_PKT"),
        (packet_type::PRELOGIN, "PRELOGIN"),
    ];
    for (value, name) in values {
        assert_eq!(tedious_rs::message::message_type_name(value), name);
    }
}

#[test]
fn packet_mutations_preserve_existing_status_bits() {
    let mut packet = Packet::new(packet_type::SQL_BATCH);
    packet.reset_connection(true);
    packet.ignore(true);
    assert!(!packet.last(None));
    assert_eq!(packet.status(), status::RESET_CONNECTION | status::IGNORE);
    packet.last(Some(true));
    packet.reset_connection(false);
    packet.ignore(false);
    assert!(packet.is_last());
    assert_eq!(packet.status(), status::EOM);
}

#[test]
fn packet_data_updates_length_and_completion() {
    let mut packet = Packet::new(packet_type::TABULAR_RESULT);
    packet.add_data(&[1, 2, 3]).unwrap();
    assert_eq!(packet.data(), &[1, 2, 3]);
    assert_eq!(packet.length(), 11);
    assert!(is_packet_complete(&packet.buffer));
    assert_eq!(packet_length(&packet.buffer), 11);
    assert_eq!(HEADER_LENGTH, 8);
}

#[test]
fn incomplete_packet_is_not_complete() {
    assert!(!is_packet_complete(&[0x04; 7]));
    assert!(!is_packet_complete(&[0x04, 0, 0, 12, 0, 0, 1, 0, 1]));
}

#[test]
fn packet_completion_compares_buffer_length_with_declared_length_like_node() {
    let mut packet = vec![0; HEADER_LENGTH];
    packet[2..4].copy_from_slice(&0_u16.to_be_bytes());
    assert!(is_packet_complete(&packet));

    packet[2..4].copy_from_slice(&9_u16.to_be_bytes());
    assert!(!is_packet_complete(&packet));
    packet.push(0);
    assert!(is_packet_complete(&packet));
}

#[test]
fn packet_status_names_follow_node_status_order() {
    let mut packet = Packet::new(packet_type::SQL_BATCH);
    assert_eq!(packet.status_as_string(), "");
    packet.buffer[1] = status::EOM
        | status::IGNORE
        | status::RESET_CONNECTION
        | status::RESET_CONNECTION_SKIP_TRAN;
    assert_eq!(
        packet.status_as_string(),
        "EOM IGNORE RESETCONNECTION RESETCONNECTIONSKIPTRAN"
    );
}

#[test]
fn packet_status_names_preserve_node_spacing_for_sparse_bits() {
    let mut packet = Packet::new(packet_type::SQL_BATCH);
    packet.buffer[1] = status::EOM | status::RESET_CONNECTION;
    assert_eq!(packet.status_as_string(), "EOM  RESETCONNECTION");
}

#[test]
fn unknown_packet_type_matches_node_header_text() {
    let packet = Packet::new(0xff);
    assert!(packet.header_to_string("").contains("type:0xFF(undefined)"));
}

#[test]
fn versions_round_trip_both_directions() {
    let forward = versions();
    let reverse = versions_by_value();
    assert_eq!(forward["7_4"], 0x74000004);
    assert_eq!(reverse[&0x74000004], "7_4");
    assert_eq!(forward.len(), 6);
}

#[test]
fn tracking_buffer_writes_endian_values_and_strings() {
    let mut buffer = WritableTrackingBuffer::new(2, Some(Encoding::Utf8), false);
    buffer.write_u8(0x01);
    buffer.write_u16_le(0x0302);
    buffer.write_u16_be(0x0504);
    buffer.write_u24_le(0x080706);
    buffer.write_u32_le(0x0c0b0a09);
    buffer.write_string("Rust", None).unwrap();
    assert_eq!(
        buffer.data(),
        vec![
            1, 2, 3, 5, 4, 6, 7, 8, 9, 10, 11, 12, b'R', b'u', b's', b't'
        ]
    );
}

#[test]
fn tracking_buffer_writes_ucs2_length_in_characters() {
    let mut buffer = WritableTrackingBuffer::new(1, Some(Encoding::Ucs2), false);
    buffer.write_us_varchar("A😀", None).unwrap();
    assert_eq!(buffer.data(), vec![3, 0, 0x41, 0, 0x3d, 0xd8, 0, 0xde]);
}

#[test]
fn tracking_buffer_rejects_length_prefix_overflow() {
    let mut buffer = WritableTrackingBuffer::new(1, Some(Encoding::Ucs2), false);
    assert!(buffer.write_b_varchar(&"A".repeat(256), None).is_err());
    assert!(buffer.write_us_varchar(&"A".repeat(65536), None).is_err());
    assert!(buffer.write_us_varbyte(&vec![0; 65536]).is_err());
}

#[test]
fn tracking_buffer_writes_unknown_plp_body() {
    let mut buffer = WritableTrackingBuffer::new(1, Some(Encoding::Utf8), true);
    buffer.write_plp_body(&[1, 2, 3]);
    assert_eq!(
        buffer.data(),
        vec![
            0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 3, 0, 0, 0, 1, 2, 3, 0, 0, 0, 0
        ]
    );
}

#[test]
fn tracking_buffer_patches_global_offsets_after_buffer_rollover() {
    let mut buffer = WritableTrackingBuffer::new(2, Some(Encoding::Utf8), false);
    buffer.write_u8(1);
    buffer.write_u8(2);
    buffer.write_u8(3);
    buffer.make_room_for(4);
    buffer.write_buffer(&[0; 4]);
    buffer.patch_u32_le(3, 0x44332211);
    assert_eq!(buffer.data(), vec![1, 2, 3, 0x11, 0x22, 0x33, 0x44]);
}

#[test]
fn tracking_buffer_writes_signed_and_unsigned_64_bit_values() {
    let mut buffer = WritableTrackingBuffer::new(1, None, false);
    buffer.write_big_i64_le(-2);
    buffer.write_big_u64_le(3);
    assert_eq!(
        buffer.data(),
        vec![
            0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 3, 0, 0, 0, 0, 0, 0, 0
        ]
    );
}

#[test]
fn helpers_decode_values_and_advance_offsets() {
    let bytes = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
    let result = read_u24_le(&bytes, 1).unwrap();
    assert_eq!(result.value, 0x443322);
    assert_eq!(result.offset, 4);
    let result = read_u32_be(&bytes, 4).unwrap();
    assert_eq!(result.value, 0x55667788);
    assert_eq!(result.offset, 8);
}

#[test]
fn helpers_decode_variable_unicode_and_binary_values() {
    let unicode = [2, 0x41, 0, 0x42, 0];
    let text = read_b_var_char(&unicode, 0).unwrap();
    assert_eq!(text.value, "AB");
    assert_eq!(text.offset, 5);
    let binary = [3, 9, 8, 7];
    let bytes = read_b_var_byte(&binary, 0).unwrap();
    assert_eq!(bytes.value, vec![9, 8, 7]);
    assert_eq!(bytes.offset, 4);
}

#[test]
fn helpers_report_required_end_offset_without_panicking() {
    let error = read_u32_le(&[1, 2], 1).unwrap_err();
    assert_eq!(error, NotEnoughDataError::new(5));
    let error = read_us_var_byte(&[2, 0, 1], 0).unwrap_err();
    assert_eq!(error.byte_count, 4);
}

#[test]
fn helpers_reject_truncated_fixed_width_values_without_panicking() {
    assert!(read_u32_be(&[1, 2, 3], 0).is_err());
    assert!(read_u64_le(&[1, 2, 3], 0).is_err());
    assert!(read_u_numeric96_le(&[1, 2, 3], 0).is_err());
    assert!(read_u_numeric128_le(&[1, 2, 3], 0).is_err());
}

#[test]
fn helpers_decode_numeric_widths() {
    let bytes = [1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0];
    assert_eq!(
        read_u_numeric64_le(&bytes, 0).unwrap().value,
        0x0000000200000001
    );
    assert_eq!(
        read_u_numeric96_le(&bytes, 0).unwrap().value,
        0x000000030000000200000001
    );
    assert_eq!(
        read_u_numeric128_le(&bytes, 0).unwrap().value,
        0x00000004000000030000000200000001
    );
}
