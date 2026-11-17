use tedious_rs::all_headers::write_to_tracking_buffer;
use tedious_rs::collation::{Collation, Encoding, UTF8};
use tedious_rs::guid_parser::{
    buffer_to_lower_case_guid, buffer_to_upper_case_guid, guid_to_array,
};
use tedious_rs::tracking_buffer::WritableTrackingBuffer;
use tedious_rs::transaction::{
    ISOLATION_LEVEL, IsolationLevel, TM_BEGIN_XACT, TM_COMMIT_XACT, Transaction,
    assert_valid_isolation_level, isolation_level_by_value,
};

#[test]
fn collation_decodes_and_round_trips_wire_layout() {
    let source = [0x09, 0x04, 0x00, 0x00, 0x00];
    let mut collation = Collation::from_buffer(&source, 0).unwrap();
    assert_eq!(collation.lcid, 0x0409);
    assert_eq!(collation.flags, 0);
    assert_eq!(collation.version, 0);
    assert_eq!(collation.sort_id, 0);
    assert_eq!(collation.codepage, Some(Encoding::Cp1252));
    assert_eq!(collation.to_buffer(), source);
}

#[test]
fn utf8_collation_takes_precedence_over_sort_and_language() {
    let mut collation = Collation::new(0x0409, UTF8, 2, 52);
    assert_eq!(collation.codepage, Some(Encoding::Utf8));
    assert_eq!(collation.to_buffer(), [0x09, 0x04, 0x00, 0x24, 52]);
}

#[test]
fn collation_unknown_encoding_remains_unknown() {
    let collation = Collation::new(0x9999, 0, 0, 250);
    assert_eq!(collation.codepage, None);
    assert!(Collation::from_buffer(&[1, 2, 3, 4], 0).is_none());
}

#[test]
fn guid_conversion_uses_sql_server_byte_order() {
    let bytes: Vec<u8> = (0..16).collect();
    assert_eq!(
        buffer_to_upper_case_guid(&bytes).unwrap(),
        "03020100-0504-0706-0809-0A0B0C0D0E0F"
    );
    assert_eq!(
        buffer_to_lower_case_guid(&bytes).unwrap(),
        "03020100-0504-0706-0809-0a0b0c0d0e0f"
    );
    assert_eq!(
        guid_to_array("03020100-0504-0706-0809-0A0B0C0D0E0F").unwrap(),
        bytes
    );
}

#[test]
fn guid_parser_rejects_wrong_shape_and_digit() {
    assert!(guid_to_array("03020100-0504-0706-0809-0E0F").is_err());
    assert!(guid_to_array("03020100-0504-0706-0809-0A0B0C0D0E0G").is_err());
    assert!(buffer_to_upper_case_guid(&[0; 15]).is_none());
}

#[test]
fn transaction_header_matches_all_headers_source_order() {
    let descriptor = [1, 2, 3, 4, 5, 6, 7, 8];
    let mut buffer = WritableTrackingBuffer::new(64, None, false);
    write_to_tracking_buffer(&mut buffer, &descriptor, 9);
    assert_eq!(
        buffer.data(),
        vec![
            22, 0, 0, 0, 18, 0, 0, 0, 2, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 0, 0
        ]
    );
}

#[test]
fn transaction_header_does_not_silently_truncate_the_descriptor() {
    let descriptor = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let mut buffer = WritableTrackingBuffer::new(64, None, false);
    write_to_tracking_buffer(&mut buffer, &descriptor, 1);
    assert_eq!(&buffer.data()[10..20], &descriptor);
    assert_eq!(&buffer.data()[20..24], &1_u32.to_le_bytes());
}

#[test]
fn transaction_payloads_preserve_operation_and_name_encoding() {
    let transaction = Transaction::new("abc", IsolationLevel::Serializable);
    let begin = transaction
        .try_begin_payload(&[1, 2, 3, 4, 5, 6, 7, 8])
        .unwrap();
    assert_eq!(&begin[22..24], &(TM_BEGIN_XACT).to_le_bytes());
    assert_eq!(begin[24], 4);
    assert_eq!(&begin[26..32], &[0x61, 0, 0x62, 0, 0x63, 0]);
    assert_eq!(transaction.isolation_level_to_tsql(), "SERIALIZABLE");
    let commit = transaction.try_commit_payload(&[0; 8]).unwrap();
    assert_eq!(&commit[22..24], &(TM_COMMIT_XACT).to_le_bytes());
    assert_eq!(commit.last().copied(), Some(0));
}

#[test]
fn exposes_transaction_maps_validation_and_descriptions_like_node() {
    assert_eq!(ISOLATION_LEVEL[0], ("NO_CHANGE", 0));
    assert_eq!(ISOLATION_LEVEL[5], ("SNAPSHOT", 5));
    assert_eq!(isolation_level_by_value(4), Some("SERIALIZABLE"));
    assert!(assert_valid_isolation_level(0, "level").is_ok());
    assert!(assert_valid_isolation_level(6, "level").is_err());
    let transaction = Transaction::new("abc", IsolationLevel::Serializable);
    assert_eq!(
        transaction.description(TM_BEGIN_XACT),
        "Begin Transaction: name=abc, isolationLevel=SERIALIZABLE"
    );
    assert_eq!(
        transaction.description(TM_COMMIT_XACT),
        "Commit Transaction: name=abc"
    );
}

#[test]
fn transaction_defaults_to_no_change_when_requested_without_an_isolation_level() {
    let transaction = Transaction::with_default_isolation_level("abc");
    assert_eq!(transaction.isolation_level, IsolationLevel::NoChange);
    assert_eq!(transaction.isolation_level_to_tsql(), "");
    let payload = transaction.try_begin_payload(&[0; 8]).unwrap();
    assert_eq!(payload[24], 0);
    assert_eq!(
        Transaction::default().isolation_level,
        IsolationLevel::NoChange
    );
}

#[test]
fn transaction_rejects_names_that_do_not_fit_the_wire_length_field() {
    let transaction = Transaction::with_default_isolation_level("A".repeat(128));
    let expected = "transaction name exceeds the TDS one-byte length field";
    assert_eq!(
        transaction.try_begin_payload(&[0; 8]).unwrap_err().0,
        expected
    );
    assert_eq!(
        transaction.try_commit_payload(&[0; 8]).unwrap_err().0,
        expected
    );
    assert_eq!(
        transaction.try_rollback_payload(&[0; 8]).unwrap_err().0,
        expected
    );
    assert_eq!(
        transaction.try_save_payload(&[0; 8]).unwrap_err().0,
        expected
    );
}
