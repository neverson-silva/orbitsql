use tedious_rs::token::parser::{EnvChangeKind, Token, parse_token};

fn message_payload() -> Vec<u8> {
    let mut value = vec![0xAB, 0, 0, 0x34, 0x12, 0, 0, 1, 16];
    value.extend([2, 0, 0x41, 0, 0x42, 0]);
    value.extend([3, 0x73, 0x00, 0x73, 0x00, 0x73, 0x00]);
    value.push(0);
    value.extend(7u32.to_le_bytes());
    let length = (value.len() - 3) as u16;
    value[1..3].copy_from_slice(&length.to_le_bytes());
    value
}

#[test]
fn parses_done_variants_and_tds_row_count_width() {
    let mut done = vec![0xFD, 0x10, 0, 0x22, 0, 0x88, 0x77, 0x66, 0x55, 0, 0, 0, 0];
    let parsed = parse_token(&done, 0, "7_4").unwrap();
    match parsed.value {
        Token::Done(value) => {
            assert_eq!(value.cur_cmd, 0x22);
            assert_eq!(value.row_count, Some(0x0000000055667788));
            assert!(!value.more);
        }
        _ => panic!(),
    }
    done.truncate(6);
    assert!(parse_token(&done, 0, "7_4").is_err());

    let legacy = [0xFD, 0x10, 0, 0x22, 0, 0x88, 0x77, 0x66, 0x55];
    match parse_token(&legacy, 0, "7_0").unwrap().value {
        Token::Done(value) => assert_eq!(value.row_count, Some(0x55667788)),
        _ => panic!(),
    }
}

#[test]
fn parses_info_error_and_return_status_tokens() {
    let info = message_payload();
    match parse_token(&info, 0, "7_4").unwrap().value {
        Token::Info(value) => {
            assert_eq!(value.number, 0x1234);
            assert_eq!(value.message, "AB");
            assert_eq!(value.server_name, "sss");
            assert_eq!(value.line_number, 7);
        }
        _ => panic!(),
    }
    let mut error = info.clone();
    error[0] = 0xAA;
    assert!(matches!(
        parse_token(&error, 0, "7_4").unwrap().value,
        Token::Error(_)
    ));
    let status = [0x79, 0x78, 0x56, 0x34, 0x12];
    assert!(
        matches!(parse_token(&status, 0, "7_4").unwrap().value, Token::ReturnStatus(value) if value.value == 0x12345678)
    );

    let mut legacy = message_payload();
    legacy.truncate(legacy.len() - 2);
    let line_offset = legacy.len() - 2;
    legacy[line_offset..].copy_from_slice(&7u16.to_le_bytes());
    let message_length = (legacy.len() - 3) as u16;
    legacy[1..3].copy_from_slice(&message_length.to_le_bytes());
    match parse_token(&legacy, 0, "7_0").unwrap().value {
        Token::Info(value) => assert_eq!(value.line_number, 7),
        _ => panic!(),
    }
}

#[test]
fn rejects_unknown_token() {
    assert!(parse_token(&[0x01], 0, "7_4").is_err());
}

#[test]
fn parses_envchange_text_numeric_binary_and_routing_variants() {
    let mut database = vec![0xE3, 0, 0, 1, 2, b'd', 0, b'b', 0, 1, b'o', 0];
    database[1..3].copy_from_slice(&9u16.to_le_bytes());
    match parse_token(&database, 0, "7_4").unwrap().value {
        Token::EnvChange(value) => assert!(
            matches!(value.kind, EnvChangeKind::Text { new_value, old_value } if new_value == "db" && old_value == "o")
        ),
        _ => panic!(),
    }

    let mut packet = vec![0xE3, 0, 0, 4, 1, b'4', 0, 1, b'2', 0];
    packet[1..3].copy_from_slice(&7u16.to_le_bytes());
    assert!(
        matches!(parse_token(&packet, 0, "7_4").unwrap().value, Token::EnvChange(value) if matches!(value.kind, EnvChangeKind::PacketSize { new_value: 4, old_value: 2 }))
    );

    let mut binary = vec![0xE3, 0, 0, 7, 5, 9, 4, 0, 0, 0, 5, 8, 4, 0, 0, 0];
    binary[1..3].copy_from_slice(&13u16.to_le_bytes());
    assert!(
        matches!(parse_token(&binary, 0, "7_4").unwrap().value, Token::EnvChange(value) if matches!(&value.kind, EnvChangeKind::Collation { new_value: Some(_), old_value: Some(_) }))
    );

    let route_data = [0, 0xD2, 0x04, 2, 0, b'S', 0, b'Q', 0];
    let mut route = vec![0xE3, 14, 0, 20];
    route.extend((route_data.len() as u16).to_le_bytes());
    route.extend(route_data);
    route.extend([0, 0]);
    assert!(
        matches!(parse_token(&route, 0, "7_4").unwrap().value, Token::EnvChange(value) if matches!(&value.kind, EnvChangeKind::Routing(route) if route.port == 1234 && route.server == "SQ"))
    );
}

#[test]
fn parses_unknown_envchange_as_consumable_data_without_typing_it() {
    let mut unknown = vec![0xE3, 0, 0, 99, 3, 1, 2, 3];
    unknown[1..3].copy_from_slice(&5u16.to_le_bytes());
    let parsed = parse_token(&unknown, 0, "7_4").unwrap();
    assert_eq!(parsed.offset, unknown.len());
    assert!(matches!(
        parsed.value,
        Token::EnvChange(value)
            if value.change_type == 99
                && matches!(&value.kind, EnvChangeKind::Unknown { new_value, old_value } if new_value == &vec![3, 1, 2, 3] && old_value.is_empty())
    ));
}

#[test]
fn optional_envchange_parser_skips_unknown_types_like_node() {
    let mut unknown = vec![0xE3, 0, 0, 99, 3, 1, 2, 3];
    unknown[1..3].copy_from_slice(&5u16.to_le_bytes());
    let parsed = tedious_rs::token::env_change_token_parser::parse_optional(&unknown).unwrap();
    assert_eq!(parsed.offset, unknown.len());
    assert!(parsed.value.is_none());
}

#[test]
fn parses_login_ack_feature_ack_order_and_fed_auth_info() {
    let mut login = vec![
        0xAD, 0, 0, 1, 0x74, 0x00, 0x00, 0x04, 2, b'S', 0, b'V', 0, 1, 2, 3, 4,
    ];
    login[1..3].copy_from_slice(&14u16.to_le_bytes());
    match parse_token(&login, 0, "7_4").unwrap().value {
        Token::LoginAck(value) => {
            assert_eq!(value.interface.as_deref(), Some("SQL_TSQL"));
            assert_eq!(value.tds_version.as_deref(), Some("7_4"));
            assert_eq!(value.prog_name, "SV");
        }
        _ => panic!(),
    }

    let feature = [0xAE, 0x02, 1, 0, 0, 0, 0xAB, 0x0A, 1, 0, 0, 0, 1, 0xFF];
    match parse_token(&feature, 0, "7_4").unwrap().value {
        Token::FeatureExtAck(value) => {
            assert_eq!(value.fed_auth, Some(vec![0xAB]));
            assert_eq!(value.utf8_support, Some(true));
        }
        _ => panic!(),
    }

    let order = [0xA9, 4, 0, 2, 0, 7, 0];
    assert!(
        matches!(parse_token(&order, 0, "7_4").unwrap().value, Token::Order(value) if value == vec![2, 7])
    );

    let mut fed = vec![0xEE, 0, 0, 0, 0, 1, 0, 0, 0, 2, 6, 0, 0, 0, 13, 0, 0, 0];
    fed[1..5].copy_from_slice(&19u32.to_le_bytes());
    fed.extend("spn".encode_utf16().flat_map(u16::to_le_bytes));
    match parse_token(&fed, 0, "7_4").unwrap().value {
        Token::FedAuthInfo(value) => assert_eq!(value.spn.as_deref(), Some("spn")),
        _ => panic!(),
    }
}

#[test]
fn treats_empty_utf8_feature_ack_data_as_false_like_node() {
    let feature = [0xAE, 0x0A, 0, 0, 0, 0, 0xFF];
    match parse_token(&feature, 0, "7_4").unwrap().value {
        Token::FeatureExtAck(value) => assert_eq!(value.utf8_support, Some(false)),
        _ => panic!(),
    }
}

fn bvar(value: &str) -> Vec<u8> {
    let units: Vec<u16> = value.encode_utf16().collect();
    let mut bytes = vec![units.len() as u8];
    for unit in units {
        bytes.extend(unit.to_le_bytes());
    }
    bytes
}

fn usvar(value: &str) -> Vec<u8> {
    let units: Vec<u16> = value.encode_utf16().collect();
    let mut bytes = (units.len() as u16).to_le_bytes().to_vec();
    for unit in units {
        bytes.extend(unit.to_le_bytes());
    }
    bytes
}

#[test]
fn parses_all_remaining_server_stream_tokens_with_exact_offsets() {
    let offset = [0x78, 7, 0, 9, 0, 0xFD, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let parsed = parse_token(&offset, 0, "7_4").unwrap();
    assert!(
        matches!(parsed.value, Token::Offset(value) if value.identifier == 7 && value.offset == 9)
    );
    assert_eq!(parsed.offset, 5);

    let mut table_payload = Vec::new();
    table_payload.push(2);
    table_payload.extend(usvar("dbo"));
    table_payload.extend(usvar("orders"));
    let mut table = vec![0xA4];
    table.extend((table_payload.len() as u16).to_le_bytes());
    table.extend(table_payload);
    let parsed = parse_token(&table, 0, "7_4").unwrap();
    assert!(matches!(parsed.value, Token::TabName(value) if value.tables.len() == 1));
    assert_eq!(parsed.offset, table.len());

    let mut colinfo_payload = vec![1, 2, 4, 2, 3, 0x20];
    colinfo_payload.extend(bvar("id"));
    let mut colinfo = vec![0xA5];
    colinfo.extend((colinfo_payload.len() as u16).to_le_bytes());
    colinfo.extend(colinfo_payload);
    let parsed = parse_token(&colinfo, 0, "7_4").unwrap();
    assert!(
        matches!(parsed.value, Token::ColInfo(value) if value.columns[1].column_name.as_deref() == Some("id"))
    );
    assert_eq!(parsed.offset, colinfo.len());

    let session = vec![0xE4, 10, 0, 0, 0, 0, 0, 0, 0, 0, 7, 3, 1, 2, 3];
    let parsed = parse_token(&session, 0, "7_4").unwrap();
    assert!(
        matches!(parsed.value, Token::SessionState(value) if value.sequence_number == 0 && value.entries[0].value == vec![1, 2, 3])
    );
    assert_eq!(parsed.offset, session.len());

    let classification = [0xA3, 0, 0, 0, 0, 0, 0];
    let parsed = parse_token(&classification, 0, "7_4").unwrap();
    assert!(
        matches!(parsed.value, Token::DataClassification(value) if value.labels.is_empty() && value.columns.is_empty())
    );
    assert_eq!(parsed.offset, classification.len());
}

#[test]
fn parses_alt_metadata_and_altrow_using_the_matching_metadata_id() {
    let mut metadata = vec![0x88, 1, 0, 5, 0, 0, 0x4D, 1, 0];
    metadata.extend([0, 0, 0, 0, 0, 0, 0x38]);
    metadata.extend(bvar("total"));
    let parsed = parse_token(&metadata, 0, "7_4").unwrap();
    assert!(
        matches!(&parsed.value, Token::AltMetadata(value) if value.id == 5 && value.columns.len() == 1)
    );
    assert_eq!(parsed.offset, metadata.len());

    let columns = match parsed.value {
        Token::AltMetadata(value) => value.columns,
        _ => unreachable!(),
    };
    let row = vec![0xD3, 5, 0, 4, 42, 0, 0, 0];
    let parsed =
        tedious_rs::token::parser::parse_alt_row_with_options(&row, 1, &columns, true).unwrap();
    assert!(
        matches!(parsed.value, Token::AltRow(value) if value.id == 5 && value.row.columns[0].value == tedious_rs::value_codec::Value::Integer(42))
    );
    assert_eq!(parsed.offset, row.len());
}

#[test]
fn parses_return_value_metadata_value_and_normalizes_parameter_name() {
    let mut data = vec![0xAC, 1, 0, 2, b'@', 0, b'x', 0, 0];
    data.extend([0, 0, 0, 0, 0, 0, 0x38, 7, 0, 0, 0]);
    let parsed = tedious_rs::token::parser::parse_token(&data, 0, "7_4").unwrap();
    match parsed.value {
        tedious_rs::token::parser::Token::ReturnValue(value) => {
            assert_eq!(value.ordinal, 1);
            assert_eq!(value.name, "x");
            assert_eq!(value.metadata.data_type.id, 0x38);
            assert_eq!(value.value, tedious_rs::value_codec::Value::Integer(7));
        }
        _ => panic!("unexpected token"),
    }
}
