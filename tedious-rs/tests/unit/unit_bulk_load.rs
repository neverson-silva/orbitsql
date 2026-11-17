use tedious_rs::payloads::{BulkLoadColumn, BulkLoadPayload, BulkLoadRow};
use tedious_rs::value_codec::EncodedParameter;

#[test]
fn serializes_bulk_metadata_rows_and_node_done_token() {
    let payload = BulkLoadPayload {
        columns: vec![BulkLoadColumn {
            name: "id".to_owned(),
            table_name: "items".to_owned(),
            type_info: vec![0x38, 4],
            nullable: false,
            legacy_lob: false,
        }],
        tds_version: "7_4".to_owned(),
        rows: vec![BulkLoadRow {
            values: vec![EncodedParameter {
                type_info: vec![],
                length: vec![4],
                data: vec![1, 0, 0, 0],
            }],
        }],
    };
    let data = payload.data().unwrap();
    assert_eq!(&data[0..3], &[0x81, 1, 0]);
    assert_eq!(&data[7..9], &[0x04, 0x00]);
    assert_eq!(&data[9..11], &[0x38, 4]);
    assert_eq!(data[11], 2);
    assert_eq!(&data[data.len() - 13..data.len() - 8], &[0xFD, 0, 0, 0, 0]);
    assert_eq!(&data[data.len() - 8..], &[0; 8]);
}

#[test]
fn bulk_payload_chunks_reassemble_to_aggregate_data() {
    let payload = BulkLoadPayload {
        columns: vec![BulkLoadColumn {
            name: "id".to_owned(),
            table_name: "items".to_owned(),
            type_info: vec![0x38, 4],
            nullable: false,
            legacy_lob: false,
        }],
        tds_version: "7_4".to_owned(),
        rows: vec![BulkLoadRow {
            values: vec![EncodedParameter {
                type_info: vec![],
                length: vec![4],
                data: vec![1, 0, 0, 0],
            }],
        }],
    };
    let chunks = payload.buffers().unwrap();
    let mut rebuilt: Vec<u8> = Vec::new();
    for chunk in &chunks {
        rebuilt.extend(chunk);
    }
    assert_eq!(rebuilt, payload.data().unwrap());
    assert_eq!(payload.to_string("  "), "  BulkLoad");
    assert_eq!(chunks[1], vec![0xD1]);
}

#[test]
fn rejects_bulk_rows_with_wrong_column_count() {
    let payload = BulkLoadPayload {
        columns: vec![BulkLoadColumn {
            name: "id".to_owned(),
            table_name: "items".to_owned(),
            type_info: vec![0x38, 4],
            nullable: false,
            legacy_lob: false,
        }],
        rows: vec![BulkLoadRow { values: Vec::new() }],
        tds_version: "7_4".to_owned(),
    };
    assert!(payload.data().is_err());
}

#[test]
fn follows_pre_72_bulk_wire_widths() {
    let payload = BulkLoadPayload {
        columns: vec![BulkLoadColumn {
            name: "id".to_owned(),
            table_name: "items".to_owned(),
            type_info: vec![0x38, 4],
            nullable: true,
            legacy_lob: false,
        }],
        rows: vec![BulkLoadRow {
            values: vec![EncodedParameter {
                type_info: Vec::new(),
                length: vec![4],
                data: vec![1, 0, 0, 0],
            }],
        }],
        tds_version: "7_1".to_owned(),
    };
    let data = payload.data().unwrap();
    assert_eq!(&data[0..3], &[0x81, 1, 0]);
    assert_eq!(&data[5..7], &[0x05, 0x00]);
    assert_eq!(&data[7..9], &[0x38, 4]);
    assert_eq!(&data[data.len() - 9..data.len() - 4], &[0xFD, 0, 0, 0, 0]);
    assert_eq!(&data[data.len() - 4..], &[0; 4]);
}

#[test]
fn writes_table_name_before_column_name_for_legacy_lob_metadata() {
    let payload = BulkLoadPayload {
        columns: vec![BulkLoadColumn {
            name: "description".to_owned(),
            table_name: "items".to_owned(),
            type_info: vec![0x23, 0xFF, 0xFF, 0xFF, 0xFF, 0x09, 0x04, 0, 0, 52],
            nullable: true,
            legacy_lob: true,
        }],
        rows: vec![BulkLoadRow {
            values: vec![EncodedParameter {
                type_info: Vec::new(),
                length: 2u32.to_le_bytes().to_vec(),
                data: vec![b'o', b'k'],
            }],
        }],
        tds_version: "7_4".to_owned(),
    };
    let data = payload.data().unwrap();
    let table = [5, 0, b'i', 0, b't', 0, b'e', 0, b'm', 0, b's', 0];
    assert!(data.windows(table.len()).any(|window| window == table));
    assert!(data.windows(2).any(|window| window == [0x10, 0x00]));
}
