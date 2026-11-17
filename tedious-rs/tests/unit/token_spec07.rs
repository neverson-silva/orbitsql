use tedious_rs::token::parser::{Token, parse_token_with_options};
use tedious_rs::token::stream::{StreamToken, TokenStreamParser};
use tedious_rs::value_codec::Value;

#[test]
fn stream_ignores_unknown_envchange_and_continues_with_following_token() {
    let input = [
        0xE3, 0x02, 0x00, 0x7F, 0x00, 0xFD, 0x10, 0x00, 0x22, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00,
    ];
    let mut parser = TokenStreamParser::new("7_4", false);
    let tokens = parser.feed(&input).expect("valid token stream");
    assert_eq!(tokens.len(), 1);
    assert!(matches!(tokens[0], StreamToken::Control(Token::Done(_))));
}

#[test]
fn returnvalue_uses_complete_parser_options_for_guid_case() {
    let mut input = vec![0xAC, 1, 0, 2, b'@', 0, b'g', 0, 0];
    input.extend([0, 0, 0, 0, 0, 0, 0x24, 16]);
    input.push(16);
    input.extend([
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
        0xFF,
    ]);
    let parsed =
        parse_token_with_options(&input, 0, "7_4", true, true).expect("valid return value");
    match parsed.value {
        Token::ReturnValue(value) => {
            assert_eq!(value.name, "g");
            assert_eq!(
                value.value,
                Value::Guid("33221100-5544-7766-8899-aabbccddeeff".to_owned())
            );
        }
        _ => panic!("unexpected token"),
    }
}

#[test]
fn stream_retains_fragmented_control_token_until_complete() {
    let input = [
        0xFD, 0x10, 0x00, 0x22, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let mut parser = TokenStreamParser::new("7_4", false);
    assert!(parser.feed(&input[..5]).is_err());
    assert_eq!(parser.buffered_len(), 5);
    let tokens = parser.feed(&input[5..]).expect("complete token");
    assert_eq!(tokens.len(), 1);
    assert_eq!(parser.finish(), Ok(()));
}

fn env_change(change_type: u8, new_value: &[u8], old_value: &[u8], binary: bool) -> Vec<u8> {
    let mut data = vec![change_type];
    if binary {
        data.extend((new_value.len() as u16).to_le_bytes());
        data.extend(new_value);
        data.extend((old_value.len() as u16).to_le_bytes());
        data.extend(old_value);
    } else {
        let new_text = String::from_utf8(new_value.to_vec()).unwrap();
        let old_text = String::from_utf8(old_value.to_vec()).unwrap();
        let new_raw = new_text
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let old_raw = old_text
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        data.push(new_text.encode_utf16().count() as u8);
        data.extend(new_raw);
        data.push(old_text.encode_utf16().count() as u8);
        data.extend(old_raw);
    }
    let mut token = vec![0xE3];
    token.extend((data.len() as u16).to_le_bytes());
    token.extend(data);
    token
}

#[test]
fn envchange_parser_preserves_each_supported_variant() {
    let text_variants = [1, 2, 3, 13];
    for change_type in text_variants {
        let token = env_change(change_type, b"new", b"old", false);
        let parsed = parse_token_with_options(&token, 0, "7_4", true, false).unwrap();
        let Token::EnvChange(value) = parsed.value else {
            panic!("expected ENVCHANGE token");
        };
        assert_eq!(value.change_type, change_type);
    }
    let packet = env_change(4, b"4096", b"512", false);
    assert!(matches!(
        parse_token_with_options(&packet, 0, "7_4", true, false)
            .unwrap()
            .value,
        Token::EnvChange(value) if matches!(value.kind, tedious_rs::token::parser::EnvChangeKind::PacketSize { new_value: 4096, old_value: 512 })
    ));
    for change_type in [7, 8, 9, 10, 18] {
        let token = env_change(change_type, &[1, 2, 3], &[4, 5], true);
        let parsed = parse_token_with_options(&token, 0, "7_4", true, false).unwrap();
        assert!(matches!(parsed.value, Token::EnvChange(_)));
    }
}

#[test]
fn envchange_parser_preserves_routing_and_unknown_consumption() {
    let route = [0, 0xD2, 0x04, 4, 0, b't', 0, b'e', 0, b's', 0, b't', 0];
    let mut data = vec![20];
    data.extend((route.len() as u16).to_le_bytes());
    data.extend(route);
    data.extend((0u16).to_le_bytes());
    let mut token = vec![0xE3];
    token.extend((data.len() as u16).to_le_bytes());
    token.extend(data);
    let parsed = parse_token_with_options(&token, 0, "7_4", true, false).unwrap();
    assert!(
        matches!(parsed.value, Token::EnvChange(value) if matches!(value.kind, tedious_rs::token::parser::EnvChangeKind::Routing(_)))
    );

    let unknown = env_change(17, &[1, 2], &[3], true);
    let mut stream = TokenStreamParser::new("7_4", false);
    assert!(stream.feed(&unknown).unwrap().is_empty());
    assert_eq!(stream.finish(), Ok(()));
}
