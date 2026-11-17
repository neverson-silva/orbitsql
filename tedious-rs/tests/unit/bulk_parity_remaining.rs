use tedious_rs::bulk_load::{BulkLoad, BulkLoadColumnDefinition, BulkLoadOptions};
use tedious_rs::payloads::{
    BulkLoadColumn, BulkLoadPayload, BulkLoadRow, RpcParameter, RpcRequestPayload,
    TvpColumnDefinition, TvpPayload, TvpRow,
};
use tedious_rs::value_codec::{EncodedParameter, Value, encode_int};

fn int_column(name: &str) -> BulkLoadColumnDefinition {
    BulkLoadColumnDefinition {
        name: name.to_owned(),
        obj_name: name.to_owned(),
        declaration: "int".to_owned(),
        type_info: vec![0x26, 4],
        nullable: true,
        legacy_lob: false,
    }
}

#[test]
fn array_rows_follow_node_column_iteration_for_missing_and_extra_values() {
    let mut bulk = BulkLoad::new("dbo.items", BulkLoadOptions::default()).unwrap();
    bulk.add_column(int_column("first")).unwrap();
    bulk.add_column(int_column("second")).unwrap();

    bulk.add_row(vec![encode_int(&Value::Integer(7)).unwrap()])
        .unwrap();
    bulk.add_row(vec![
        encode_int(&Value::Integer(8)).unwrap(),
        encode_int(&Value::Integer(9)).unwrap(),
        encode_int(&Value::Integer(10)).unwrap(),
    ])
    .unwrap();

    assert_eq!(bulk.rows[0].values.len(), 2);
    assert_eq!(bulk.rows[0].values[0].data, 7_i32.to_le_bytes());
    assert_eq!(bulk.rows[0].values[1].length, vec![0]);
    assert_eq!(bulk.rows[1].values.len(), 2);
    assert_eq!(bulk.rows[1].values[0].data, 8_i32.to_le_bytes());
    assert_eq!(bulk.rows[1].values[1].data, 9_i32.to_le_bytes());
}

#[test]
fn typed_rows_preserve_declared_order_while_ignoring_extra_values() {
    let mut bulk = BulkLoad::new("dbo.items", BulkLoadOptions::default()).unwrap();
    bulk.add_typed_column(
        "first",
        tedious_rs::data_type::type_by_name("Int").unwrap(),
        Default::default(),
        true,
    )
    .unwrap();
    bulk.add_typed_column(
        "second",
        tedious_rs::data_type::type_by_name("Int").unwrap(),
        Default::default(),
        true,
    )
    .unwrap();
    bulk.add_value_row(vec![
        Value::Integer(11),
        Value::Integer(12),
        Value::Integer(13),
    ])
    .unwrap();

    assert_eq!(bulk.rows[0].values.len(), 2);
    assert_eq!(bulk.rows[0].values[0].data, 11_i32.to_le_bytes());
    assert_eq!(bulk.rows[0].values[1].data, 12_i32.to_le_bytes());
}

#[test]
fn missing_typed_values_are_encoded_as_null_in_column_order() {
    let mut bulk = BulkLoad::new("dbo.items", BulkLoadOptions::default()).unwrap();
    bulk.add_typed_column(
        "first",
        tedious_rs::data_type::type_by_name("Int").unwrap(),
        Default::default(),
        true,
    )
    .unwrap();
    bulk.add_typed_column(
        "second",
        tedious_rs::data_type::type_by_name("Int").unwrap(),
        Default::default(),
        true,
    )
    .unwrap();

    bulk.add_value_row(vec![Value::Integer(14)]).unwrap();

    assert_eq!(bulk.rows[0].values.len(), 2);
    assert_eq!(bulk.rows[0].values[0].data, 14_i32.to_le_bytes());
    assert_eq!(bulk.rows[0].values[1].length, vec![0]);
    assert!(bulk.rows[0].values[1].data.is_empty());
}

#[test]
fn nullable_unknown_metadata_is_version_gated_and_keeps_other_flags() {
    let mut bulk = BulkLoad::new("dbo.items", BulkLoadOptions::default()).unwrap();
    bulk.add_column(int_column("id")).unwrap();
    bulk.set_nullable_unknown_columns([0]).unwrap();
    assert_eq!(&bulk.get_col_metadata().unwrap()[7..9], &[0x04, 0x80]);

    bulk.set_tds_version("7_1");
    assert_eq!(&bulk.get_col_metadata().unwrap()[5..7], &[0x04, 0x00]);
}

#[test]
fn nullable_unknown_index_error_is_returned_before_metadata_generation() {
    let mut bulk = BulkLoad::new("dbo.items", BulkLoadOptions::default()).unwrap();
    bulk.add_column(int_column("id")).unwrap();
    let error = bulk.set_nullable_unknown_columns([1]).unwrap_err();
    assert_eq!(error, "bulk nullable-unknown column index is out of range");
    assert!(bulk.get_col_metadata().is_ok());
}

#[test]
fn row_and_column_mutation_errors_preserve_node_order() {
    let mut bulk = BulkLoad::new("dbo.items", BulkLoadOptions::default()).unwrap();
    bulk.add_column(int_column("id")).unwrap();
    bulk.add_row(vec![]).unwrap();

    let column_error = bulk.add_column(int_column("later")).unwrap_err();
    assert_eq!(
        column_error,
        "cannot add bulk-load columns after the first row"
    );

    bulk.begin_execution().unwrap();
    let row_error = bulk.add_row(vec![]).unwrap_err();
    assert_eq!(
        row_error,
        "cannot add bulk-load rows after execution starts"
    );
    assert_eq!(bulk.rows.len(), 1);
}

#[test]
fn bulk_rows_and_done_token_keep_source_order_in_wire_data() {
    let payload = BulkLoadPayload {
        columns: vec![BulkLoadColumn {
            name: "id".to_owned(),
            table_name: "items".to_owned(),
            type_info: vec![0x38, 4],
            nullable: false,
            legacy_lob: false,
        }],
        rows: vec![
            BulkLoadRow {
                values: vec![EncodedParameter {
                    type_info: Vec::new(),
                    length: vec![4],
                    data: 21_i32.to_le_bytes().to_vec(),
                }],
            },
            BulkLoadRow {
                values: vec![EncodedParameter {
                    type_info: Vec::new(),
                    length: vec![4],
                    data: 22_i32.to_le_bytes().to_vec(),
                }],
            },
        ],
        tds_version: "7_4".to_owned(),
    };
    let buffers = payload.buffers().unwrap();
    assert_eq!(buffers[1], vec![0xD1]);
    assert_eq!(buffers[2], vec![4]);
    assert_eq!(buffers[3], 21_i32.to_le_bytes());
    assert_eq!(buffers[4], vec![0xD1]);
    assert_eq!(buffers[5], vec![4]);
    assert_eq!(buffers[6], 22_i32.to_le_bytes());
    assert_eq!(buffers[7][0], 0xFD);
}

#[test]
fn rpc_parameters_keep_ordinal_order_and_output_status() {
    let payload = RpcRequestPayload {
        procedure: Some("proc".to_owned()),
        procedure_id: None,
        parameters: vec![
            RpcParameter {
                name: "first".to_owned(),
                output: false,
                encoded: encode_int(&Value::Integer(31)).unwrap(),
                encrypted_metadata: None,
            },
            RpcParameter {
                name: "second".to_owned(),
                output: true,
                encoded: encode_int(&Value::Integer(32)).unwrap(),
                encrypted_metadata: None,
            },
        ],
        transaction_descriptor: Vec::new(),
        tds_version: "7_1".to_owned(),
    };
    let buffers = payload.buffers().unwrap();
    assert_eq!(buffers[1][1], b'@');
    assert_eq!(buffers[1].last(), Some(&0));
    assert_eq!(buffers[5][1], b'@');
    assert_eq!(buffers[5].last(), Some(&1));
    assert_eq!(buffers[4], 31_i32.to_le_bytes());
    assert_eq!(buffers[8], 32_i32.to_le_bytes());
}

#[test]
fn tvp_rows_keep_declared_column_order_and_allow_short_rows() {
    let payload = TvpPayload {
        database: String::new(),
        schema: "dbo".to_owned(),
        type_name: "items_type".to_owned(),
        columns: vec![
            TvpColumnDefinition {
                name: String::new(),
                type_info: vec![0x38, 4],
            },
            TvpColumnDefinition {
                name: String::new(),
                type_info: vec![0x38, 4],
            },
        ],
        rows: vec![TvpRow {
            values: vec![EncodedParameter {
                type_info: Vec::new(),
                length: vec![4],
                data: 41_i32.to_le_bytes().to_vec(),
            }],
        }],
    };
    let data = payload.data().unwrap();
    assert!(data.windows(4).any(|window| window == 41_i32.to_le_bytes()));
    assert!(!data.windows(4).any(|window| window == 42_i32.to_le_bytes()));
}
