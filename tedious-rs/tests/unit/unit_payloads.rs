use tedious_rs::payloads::{
    EncryptedParameterMetadata, RpcParameter, RpcRequestPayload, SqlBatchPayload,
};
use tedious_rs::value_codec::Value;
use tedious_rs::value_codec::encode_int;

use tedious_rs::collation::Collation;
use tedious_rs::parameter::{ParameterOptions, SqlParameterType};

#[test]
fn serializes_sql_batch_with_transaction_header_before_ucs2_sql() {
    let payload = SqlBatchPayload {
        sql_text: "select 1".to_owned(),
        transaction_descriptor: vec![1, 2, 3, 4],
        tds_version: "7_4".to_owned(),
    };
    let buffers = payload.buffers().unwrap();
    assert_eq!(buffers.len(), 2);
    assert_eq!(
        buffers[1],
        "select 1"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(u32::from_le_bytes(buffers[0][0..4].try_into().unwrap()), 18);
}

#[test]
fn formats_sql_batch_diagnostic_like_node() {
    let payload = SqlBatchPayload {
        sql_text: "SELECT 1".to_owned(),
        transaction_descriptor: Vec::new(),
        tds_version: "7_4".to_owned(),
    };
    assert_eq!(payload.to_string("  "), "  SQL Batch - SELECT 1");
}

#[test]
fn omits_transaction_header_for_every_tds_version_before_7_2() {
    let payload = SqlBatchPayload {
        sql_text: "select 1".to_owned(),
        transaction_descriptor: vec![1, 2, 3, 4],
        tds_version: "7_0".to_owned(),
    };
    let buffers = payload.buffers().unwrap();
    assert_eq!(buffers.len(), 1);
    assert_eq!(
        buffers[0],
        "select 1"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    );
}

#[test]
fn serializes_rpc_procedure_parameters_in_ordinal_order() {
    let encoded = encode_int(&Value::Integer(7)).unwrap();
    let payload = RpcRequestPayload {
        procedure: Some("sp_executesql".to_owned()),
        procedure_id: None,
        parameters: vec![RpcParameter {
            name: "id".to_owned(),
            output: true,
            encoded,
            encrypted_metadata: None,
        }],
        transaction_descriptor: Vec::new(),
        tds_version: "7_1".to_owned(),
    };
    let buffers = payload.buffers().unwrap();
    assert_eq!(buffers.len(), 5);
    assert_eq!(&buffers[1], &[3, b'@', 0, b'i', 0, b'd', 0, 1]);
    assert_eq!(&buffers[2], &[0x26, 4]);
    assert_eq!(&buffers[3], &[4]);
    assert_eq!(&buffers[4], &7i32.to_le_bytes());
}

#[test]
fn serializes_rpc_empty_parameter_without_default_value_flag() {
    let payload = RpcRequestPayload {
        procedure: Some("proc".to_owned()),
        procedure_id: None,
        parameters: vec![RpcParameter {
            name: "optional".to_owned(),
            output: false,
            encoded: tedious_rs::value_codec::EncodedParameter {
                type_info: Vec::new(),
                length: Vec::new(),
                data: Vec::new(),
            },
            encrypted_metadata: None,
        }],
        transaction_descriptor: Vec::new(),
        tds_version: "7_1".to_owned(),
    };
    let buffers = payload.buffers().unwrap();
    assert_eq!(buffers.len(), 5);
    assert_eq!(
        buffers[1],
        vec![
            9, b'@', 0, b'o', 0, b'p', 0, b't', 0, b'i', 0, b'o', 0, b'n', 0, b'a', 0, b'l', 0, 0
        ]
    );
    assert!(buffers[2].is_empty());
    assert!(buffers[3].is_empty());
    assert!(buffers[4].is_empty());
}

#[test]
fn serializes_rpc_empty_value_type_info_length_and_data_in_order() {
    let payload = RpcRequestPayload {
        procedure: Some("proc".to_owned()),
        procedure_id: None,
        parameters: vec![RpcParameter {
            name: "value".to_owned(),
            output: false,
            encoded: tedious_rs::value_codec::EncodedParameter {
                type_info: vec![0xA7, 5, 0],
                length: vec![0],
                data: vec![],
            },
            encrypted_metadata: None,
        }],
        transaction_descriptor: Vec::new(),
        tds_version: "7_1".to_owned(),
    };
    let buffers = payload.buffers().unwrap();
    assert_eq!(buffers.len(), 5);
    assert_eq!(
        buffers[1],
        vec![6, b'@', 0, b'v', 0, b'a', 0, b'l', 0, b'u', 0, b'e', 0, 0]
    );
    assert_eq!(buffers[2], vec![0xA7, 5, 0]);
    assert_eq!(buffers[3], vec![0]);
    assert!(buffers[4].is_empty());
}

#[test]
fn tvp_type_info_uses_empty_database_name_like_node() {
    let payload = tedious_rs::payloads::TvpPayload {
        database: "ignored_database".to_owned(),
        schema: "dbo".to_owned(),
        type_name: "items_type".to_owned(),
        columns: Vec::new(),
        rows: Vec::new(),
    };
    assert_eq!(
        payload.type_info().unwrap(),
        vec![
            0xF3, 0, 3, b'd', 0, b'b', 0, b'o', 0, 10, b'i', 0, b't', 0, b'e', 0, b'm', 0, b's', 0,
            b'_', 0, b't', 0, b'y', 0, b'p', 0, b'e', 0
        ]
    );
}

#[test]
fn tvp_null_uses_node_null_length_and_terminators() {
    let encoded = tedious_rs::value_codec::encode_tvp(&Value::Null).unwrap();
    assert_eq!(encoded.type_info, vec![0xF3, 0, 0, 0]);
    assert_eq!(encoded.length, vec![0xFF, 0xFF]);
    assert_eq!(encoded.data, vec![0, 0]);
}

#[test]
fn serializes_null_text_ntext_and_image_rpc_parameters_like_node() {
    let collation = Collation::new(0x0409, 0, 0, 0);
    let parameters = [
        ("text", SqlParameterType::Text),
        ("ntext", SqlParameterType::NText),
        ("image", SqlParameterType::Image),
    ]
    .into_iter()
    .map(|(name, parameter_type)| RpcParameter {
        name: name.to_owned(),
        output: false,
        encoded: parameter_type
            .encode(
                &Value::Null,
                &ParameterOptions {
                    collation: Some(collation.clone()),
                    ..Default::default()
                },
            )
            .unwrap(),
        encrypted_metadata: None,
    })
    .collect();
    let payload = RpcRequestPayload {
        procedure: Some("proc".to_owned()),
        procedure_id: None,
        parameters,
        transaction_descriptor: Vec::new(),
        tds_version: "7_1".to_owned(),
    };
    let buffers = payload.buffers().unwrap();
    assert_eq!(buffers.len(), 13);
    assert_eq!(
        buffers[1],
        vec![5, b'@', 0, b't', 0, b'e', 0, b'x', 0, b't', 0, 0]
    );
    assert_eq!(
        buffers[2],
        vec![0x23, 0xFF, 0xFF, 0xFF, 0xFF, 9, 4, 0, 0, 0]
    );
    assert_eq!(buffers[3], vec![0xFF, 0xFF, 0xFF, 0xFF]);
    assert!(buffers[4].is_empty());
    assert_eq!(
        buffers[5],
        vec![6, b'@', 0, b'n', 0, b't', 0, b'e', 0, b'x', 0, b't', 0, 0]
    );
    assert_eq!(
        buffers[6],
        vec![0x63, 0xFF, 0xFF, 0xFF, 0xFF, 9, 4, 0, 0, 0]
    );
    assert_eq!(buffers[7], vec![0xFF, 0xFF, 0xFF, 0xFF]);
    assert!(buffers[8].is_empty());
    assert_eq!(
        buffers[9],
        vec![6, b'@', 0, b'i', 0, b'm', 0, b'a', 0, b'g', 0, b'e', 0, 0]
    );
    assert_eq!(buffers[10], vec![0x22, 0xFF, 0xFF, 0xFF, 0xFF]);
    assert_eq!(buffers[11], vec![0xFF, 0xFF, 0xFF, 0xFF]);
    assert!(buffers[12].is_empty());
}

#[test]
fn ignores_encrypted_rpc_metadata_like_node_payload() {
    let payload = RpcRequestPayload {
        procedure: Some("sp_executesql".to_owned()),
        procedure_id: None,
        parameters: vec![RpcParameter {
            name: "secret".to_owned(),
            output: false,
            encoded: tedious_rs::value_codec::EncodedParameter {
                type_info: vec![0xA5, 0xFF, 0xFF],
                length: vec![1, 0, 0, 0, 0, 0, 0, 0],
                data: vec![0xAA],
            },
            encrypted_metadata: Some(EncryptedParameterMetadata {
                base_type_info: vec![0x26, 4],
                algorithm_id: 2,
                encryption_type: 1,
                normalization_rule_version: 1,
                database_id: 6,
                cek_id: 7,
                cek_version: 1,
                cek_md_version: 8,
            }),
        }],
        transaction_descriptor: Vec::new(),
        tds_version: "7_4".to_owned(),
    };
    let bytes = payload.buffers().unwrap().concat();
    let parameter_start = bytes
        .windows(15)
        .position(|window| {
            window
                == [
                    7, b'@', 0, b's', 0, b'e', 0, b'c', 0, b'r', 0, b'e', 0, b't', 0,
                ]
        })
        .unwrap();
    assert_eq!(bytes[parameter_start + 15], 0x00);
    assert_eq!(
        &bytes[parameter_start + 16..],
        &[0xA5, 0xFF, 0xFF, 1, 0, 0, 0, 0, 0, 0, 0, 0xAA]
    );
}

#[test]
fn serializes_tvp_type_info_metadata_rows_and_end_tokens() {
    let tvp = tedious_rs::payloads::TvpPayload {
        database: String::new(),
        schema: "dbo".to_owned(),
        type_name: "items_type".to_owned(),
        columns: vec![tedious_rs::payloads::TvpColumnDefinition {
            name: "id".to_owned(),
            type_info: vec![0x38],
        }],
        rows: vec![tedious_rs::payloads::TvpRow {
            values: vec![tedious_rs::value_codec::EncodedParameter {
                type_info: vec![],
                length: vec![4],
                data: vec![1, 0, 0, 0],
            }],
        }],
    };
    let buffers = tvp.buffers().unwrap();
    assert_eq!(buffers[0][0], 0xF3);
    assert_eq!(buffers[1], vec![1, 0]);
    assert_eq!(&buffers[2][0..6], &[0, 0, 0, 0, 0, 0]);
    assert_eq!(&buffers[2][6..8], &[0x38, 0]);
    assert_eq!(buffers[2].last(), Some(&0));
    assert_eq!(buffers[2][buffers[2].len() - 8..], [0, 1, 4, 1, 0, 0, 0, 0]);
}

#[test]
fn rejects_rpc_without_procedure() {
    let payload = RpcRequestPayload {
        procedure: None,
        procedure_id: None,
        parameters: Vec::new(),
        transaction_descriptor: Vec::new(),
        tds_version: "7_1".to_owned(),
    };
    assert!(payload.buffers().is_err());
}

#[test]
fn preserves_tvp_metadata_without_rust_type_registry_validation() {
    let empty_type = tedious_rs::payloads::TvpPayload {
        database: String::new(),
        schema: "dbo".to_owned(),
        type_name: "items_type".to_owned(),
        columns: vec![tedious_rs::payloads::TvpColumnDefinition {
            name: "id".to_owned(),
            type_info: Vec::new(),
        }],
        rows: Vec::new(),
    };
    assert!(empty_type.validate().is_ok());
    assert!(empty_type.buffers().is_ok());

    let unknown_type = tedious_rs::payloads::TvpPayload {
        columns: vec![tedious_rs::payloads::TvpColumnDefinition {
            name: "id".to_owned(),
            type_info: vec![0xFE],
        }],
        ..empty_type
    };
    assert!(unknown_type.validate().is_ok());
}

#[test]
fn serializes_tvp_rows_shorter_than_declared_columns_like_node() {
    let payload = tedious_rs::payloads::TvpPayload {
        database: String::new(),
        schema: "dbo".to_owned(),
        type_name: "items_type".to_owned(),
        columns: vec![
            tedious_rs::payloads::TvpColumnDefinition {
                name: String::new(),
                type_info: vec![0x38],
            },
            tedious_rs::payloads::TvpColumnDefinition {
                name: String::new(),
                type_info: vec![0x38],
            },
        ],
        rows: vec![tedious_rs::payloads::TvpRow {
            values: vec![tedious_rs::value_codec::EncodedParameter {
                type_info: Vec::new(),
                length: vec![4],
                data: vec![1, 0, 0, 0],
            }],
        }],
    };
    assert!(payload.validate().is_ok());
    assert!(payload.data().is_ok());
}

#[test]
fn rejects_tvp_rows_with_more_values_than_declared_columns() {
    let payload = tedious_rs::payloads::TvpPayload {
        database: String::new(),
        schema: "dbo".to_owned(),
        type_name: "items_type".to_owned(),
        columns: vec![tedious_rs::payloads::TvpColumnDefinition {
            name: String::new(),
            type_info: vec![0x38],
        }],
        rows: vec![tedious_rs::payloads::TvpRow {
            values: vec![
                tedious_rs::value_codec::EncodedParameter {
                    type_info: Vec::new(),
                    length: vec![4],
                    data: vec![1, 0, 0, 0],
                },
                tedious_rs::value_codec::EncodedParameter {
                    type_info: Vec::new(),
                    length: vec![4],
                    data: vec![2, 0, 0, 0],
                },
            ],
        }],
    };
    assert_eq!(
        payload.data(),
        Err("TVP row value count exceeds column count".to_owned())
    );
}

#[test]
fn builds_tvp_from_typed_values_and_reports_cell_context() {
    let columns = [tedious_rs::payloads::TvpTypedColumnDefinition {
        name: "id".to_owned(),
        parameter_type: tedious_rs::parameter::SqlParameterType::Int,
        options: Default::default(),
    }];
    let payload = tedious_rs::payloads::TvpPayload::from_typed_values(
        "",
        "dbo",
        "items_type",
        &columns,
        &[vec![tedious_rs::value_codec::Value::Integer(7)]],
        None,
    )
    .unwrap();
    assert_eq!(payload.columns[0].type_info, vec![0x26, 4]);
    assert_eq!(payload.rows[0].values[0].data, 7i32.to_le_bytes());

    let error = tedious_rs::payloads::TvpPayload::from_typed_values(
        "",
        "dbo",
        "items_type",
        &columns,
        &[vec![tedious_rs::value_codec::Value::Text(
            "invalid".to_owned(),
        )]],
        None,
    )
    .unwrap_err();
    assert!(error.contains("TVP column 'id'"));
    assert!(error.contains("row index 0"));
}
