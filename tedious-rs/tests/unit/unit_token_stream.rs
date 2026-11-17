use tedious_rs::token::parser::Token;
use tedious_rs::token::rows::parse_colmetadata;
use tedious_rs::token::stream::{StreamToken, TokenStreamEvent, TokenStreamParser};

fn metadata() -> Vec<u8> {
    let value = vec![0x81, 1, 0, 0, 0, 0, 0, 0, 0, 0x38, 2, b'I', 0, b'd', 0];
    let _ = parse_colmetadata(&value[1..], "7_4", false).unwrap();
    value
}

#[test]
fn retains_incomplete_tokens_and_decodes_following_rows() {
    let mut parser = TokenStreamParser::new("7_4", false);
    let column = metadata();
    assert!(parser.feed(&column[..4]).is_err());
    assert_eq!(parser.buffered_len(), 4);
    let mut rest = column[4..].to_vec();
    rest.extend([0xD1, 7, 0, 0, 0]);
    let tokens = parser.feed(&rest).unwrap();
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0], StreamToken::ColMetadata(columns) if columns[0].col_name == "Id"));
    assert!(
        matches!(&tokens[1], StreamToken::Row(row) if row.columns[0].value == tedious_rs::value_codec::Value::Integer(7))
    );
}

#[test]
fn nbcrow_requires_metadata_and_preserves_null_bitmap_semantics() {
    let mut parser = TokenStreamParser::new("7_4", false);
    assert!(parser.feed(&[0xD2, 1]).is_err());
    let mut parser = TokenStreamParser::new("7_4", false);
    let _ = parser.feed(&metadata()).unwrap();
    let tokens = parser.feed(&[0xD2, 1]).unwrap();
    assert!(
        matches!(&tokens[0], StreamToken::NbcRow(row) if row.columns[0].value == tedious_rs::value_codec::Value::Null)
    );
}

#[test]
fn applies_column_name_replacer_before_rows_are_decoded() {
    let mut parser = TokenStreamParser::new("7_4", true);
    parser.set_column_name_replacer(Some(std::sync::Arc::new(|name, index, _| {
        format!("{index}_{name}")
    })));
    let tokens = parser.feed(&metadata()).unwrap();
    assert!(
        matches!(&tokens[0], StreamToken::ColMetadata(columns) if columns[0].col_name == "0_Id")
    );
    let tokens = parser.feed(&[0xD1, 7, 0, 0, 0]).unwrap();
    assert!(
        matches!(&tokens[0], StreamToken::Row(row) if row.columns[0].metadata.col_name == "0_Id")
    );
}

#[test]
fn propagates_use_utc_to_temporal_rows() {
    let mut parser = TokenStreamParser::new_with_options("7_4", false, false);
    let metadata = [0x81, 1, 0, 0, 0, 0, 0, 0, 0, 0x28, 1, b'D', 0];
    parser.feed(&metadata).unwrap();
    let tokens = parser.feed(&[0xD1, 3, 0x07, 0x24, 0x0B]).unwrap();
    assert!(matches!(
        &tokens[0],
        StreamToken::Row(row) if row.columns[0].value == tedious_rs::value_codec::Value::LocalDate {
            year: 2000,
            month: 1,
            day: 1
        }
    ));
}

#[test]
fn pause_defers_tokens_until_resume() {
    let mut parser = TokenStreamParser::new("7_4", false);
    parser.pause();
    assert!(parser.is_paused());
    assert!(parser.feed(&metadata()).unwrap().is_empty());
    assert_eq!(parser.buffered_len(), metadata().len());
    let tokens = parser.resume().unwrap();
    assert!(!parser.is_paused());
    assert!(matches!(tokens[0], StreamToken::ColMetadata(_)));
}

#[test]
fn finish_rejects_a_truncated_token_and_accepts_complete_input() {
    let mut parser = TokenStreamParser::new("7_4", false);
    parser.feed(&[0x81]).unwrap_err();
    assert_eq!(
        parser.finish(),
        Err(tedious_rs::token::stream::StreamParseError::NotEnoughData)
    );
    let mut parser = TokenStreamParser::new("7_4", false);
    parser.feed(&metadata()).unwrap();
    assert_eq!(parser.finish(), Ok(()));
}

#[test]
fn finish_emits_end_once_after_clean_eof() {
    let mut parser = TokenStreamParser::new("7_4", false);
    parser.feed(&[]).unwrap();
    parser.finish().unwrap();
    assert!(parser.finish().is_err());
    assert_eq!(
        parser
            .take_events()
            .iter()
            .filter(|event| matches!(event, TokenStreamEvent::End))
            .count(),
        1
    );
}

#[test]
fn exposes_pause_resume_token_and_failure_transitions() {
    let mut parser = TokenStreamParser::new("7_4", false);
    assert_eq!(
        parser.state(),
        &tedious_rs::token::stream::TokenStreamState::Reading
    );
    parser.pause();
    assert_eq!(
        parser.state(),
        &tedious_rs::token::stream::TokenStreamState::Paused
    );
    parser.resume().unwrap();
    assert_eq!(
        parser.state(),
        &tedious_rs::token::stream::TokenStreamState::Reading
    );
    let error = parser.feed(&[0xD1]).unwrap_err();
    assert_eq!(
        error,
        tedious_rs::token::stream::StreamParseError::Invalid(
            "ROW received before COLMETADATA".to_owned()
        )
    );
    assert_eq!(
        parser.state(),
        &tedious_rs::token::stream::TokenStreamState::Failed
    );
    assert!(
        parser
            .events()
            .iter()
            .any(|event| matches!(event, tedious_rs::token::stream::TokenStreamEvent::Paused))
    );
    assert!(
        parser
            .events()
            .iter()
            .any(|event| matches!(event, tedious_rs::token::stream::TokenStreamEvent::Resumed))
    );
    assert!(parser.events().iter().any(|event| matches!(
        event,
        tedious_rs::token::stream::TokenStreamEvent::Failed(_)
    )));
}

fn bvar(value: &str) -> Vec<u8> {
    let units: Vec<u16> = value.encode_utf16().collect();
    let mut bytes = vec![units.len() as u8];
    for unit in units {
        bytes.extend(unit.to_le_bytes());
    }
    bytes
}

#[test]
fn consumes_sql_variant_columns_without_leaving_type_bytes_in_the_stream() {
    let mut data = vec![0x81, 3, 0];
    data.extend([0, 0, 0, 0, 0, 0, 0x38]);
    data.extend(bvar("left"));
    data.extend([0, 0, 0, 0, 0, 0, 0x62, 0, 0, 0, 0]);
    data.extend(bvar("middle"));
    data.extend([0, 0, 0, 0, 0, 0, 0x38]);
    data.extend(bvar("right"));
    data.push(0xD1);
    data.extend(7i32.to_le_bytes());
    let variant = [6, 0, 0, 0, 0x38, 0, 42, 0, 0, 0];
    data.extend(variant);
    data.extend(9i32.to_le_bytes());
    data.extend([0xFD, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    let mut parser = TokenStreamParser::new("7_4", false);
    let mut tokens = Vec::new();
    for byte in data {
        match parser.feed(&[byte]) {
            Ok(values) => tokens.extend(values),
            Err(
                tedious_rs::token::stream::StreamParseError::NotEnoughData
                | tedious_rs::token::stream::StreamParseError::Token(_)
                | tedious_rs::token::stream::StreamParseError::Row(_),
            ) => {}
            Err(error) => panic!("unexpected stream error: {error:?}"),
        }
    }
    assert!(matches!(&tokens[0], StreamToken::ColMetadata(columns) if columns.len() == 3));
    assert!(
        matches!(&tokens[1], StreamToken::Row(row) if row.columns[1].value == tedious_rs::value_codec::Value::Integer(42))
    );
    assert!(matches!(&tokens[2], StreamToken::Control(Token::Done(_))));
}

#[test]
fn consumes_alt_metadata_and_altrow_before_done() {
    let mut data = vec![0x88, 1, 0, 5, 0, 0, 0x4D, 1, 0];
    data.extend([0, 0, 0, 0, 0, 0, 0x38]);
    data.extend(bvar("total"));
    data.extend([0xD3, 5, 0, 4, 42, 0, 0, 0]);
    data.extend([0xFD, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let mut parser = TokenStreamParser::new("7_4", false);
    let tokens = parser.feed(&data).unwrap();
    assert!(matches!(
        &tokens[0],
        StreamToken::Control(Token::AltMetadata(_))
    ));
    assert!(
        matches!(&tokens[1], StreamToken::Control(Token::AltRow(value)) if value.row.columns[0].value == tedious_rs::value_codec::Value::Integer(42))
    );
    assert!(matches!(&tokens[2], StreamToken::Control(Token::Done(_))));
}

#[test]
fn retains_partial_altrow_after_emitting_altmetadata() {
    let mut data = vec![0x88, 1, 0, 5, 0, 0, 0x4D, 1, 0];
    data.extend([0, 0, 0, 0, 0, 0, 0x38]);
    data.extend(bvar("total"));
    let altrow_offset = data.len();
    data.extend([0xD3, 5, 0, 4, 42, 0, 0, 0]);

    let mut parser = TokenStreamParser::new("7_4", false);
    let tokens = parser.feed(&data[..altrow_offset + 3]).unwrap();
    assert_eq!(tokens.len(), 1);
    assert!(matches!(
        &tokens[0],
        StreamToken::Control(Token::AltMetadata(_))
    ));
    assert_eq!(parser.buffered_len(), 3);

    let tokens = parser.feed(&data[altrow_offset + 3..]).unwrap();
    assert!(matches!(
        &tokens[0],
        StreamToken::Control(Token::AltRow(value))
            if value.row.columns[0].value == tedious_rs::value_codec::Value::Integer(42)
    ));
}
