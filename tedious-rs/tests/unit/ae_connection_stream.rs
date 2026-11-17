use tedious_rs::token::stream::{StreamParseError, StreamToken, TokenStreamParser};

fn encrypted_metadata() -> Vec<u8> {
    vec![
        0x81, 1, 0, 0, 0, 0, 0, 0, 0x08, 0x38, 1, b'X', 0, 3, 0, 2, 1, 0,
    ]
}

#[test]
fn encrypted_row_fails_closed_without_provider() {
    let mut parser = TokenStreamParser::new("7_4", false);
    let result = parser.feed(&encrypted_metadata());
    assert_eq!(
        result,
        Err(StreamParseError::Invalid(
            "column encryption metadata is unavailable".to_owned()
        ))
    );
}

#[test]
fn plaintext_row_remains_available_without_encryption_metadata() {
    let mut parser = TokenStreamParser::new("7_4", false);
    let metadata = vec![0x81, 1, 0, 0, 0, 0, 0, 0, 0, 0x38, 1, b'X', 0];
    let tokens = parser.feed(&metadata).unwrap();
    assert!(matches!(tokens[0], StreamToken::ColMetadata(_)));
    let tokens = parser.feed(&[0xD1, 7, 0, 0, 0]).unwrap();
    assert!(
        matches!(&tokens[0], StreamToken::Row(row) if row.columns[0].value == tedious_rs::value_codec::Value::Integer(7))
    );
}
