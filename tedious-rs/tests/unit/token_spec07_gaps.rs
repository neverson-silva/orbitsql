use tedious_rs::token::handler::{
    HandlerEvent, InitialSqlTokenHandler, Login7TokenHandler, RequestTokenHandler, TokenHandler,
    dispatch_token,
};
use tedious_rs::token::parser::{
    EnvChangeKind, EnvChangeToken, Token, TokenParseError, parse_token,
};
use tedious_rs::token::stream::{StreamToken, TokenStreamParser};

fn env(change_type: u8, body: &[u8]) -> Vec<u8> {
    let length = 1 + body.len();
    let mut result = vec![0xe3, length as u16 as u8, (length >> 8) as u8, change_type];
    result.extend_from_slice(body);
    result
}

fn text(new_value: &str, old_value: &str) -> Vec<u8> {
    let mut result = vec![new_value.len() as u8];
    for unit in new_value.encode_utf16() {
        result.extend_from_slice(&unit.to_le_bytes());
    }
    result.push(old_value.len() as u8);
    for unit in old_value.encode_utf16() {
        result.extend_from_slice(&unit.to_le_bytes());
    }
    result
}

fn binary(new_value: &[u8], old_value: &[u8]) -> Vec<u8> {
    let mut result = vec![new_value.len() as u8];
    result.extend_from_slice(new_value);
    result.push(old_value.len() as u8);
    result.extend_from_slice(old_value);
    result
}

fn wide_binary(new_value: &[u8], old_value: &[u8]) -> Vec<u8> {
    let mut result = vec![(new_value.len() & 0xff) as u8, (new_value.len() >> 8) as u8];
    result.extend_from_slice(new_value);
    result.extend_from_slice(&[(old_value.len() & 0xff) as u8, (old_value.len() >> 8) as u8]);
    result.extend_from_slice(old_value);
    result
}

fn done() -> Vec<u8> {
    vec![0xfd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
}

#[test]
fn parses_all_envchange_wire_variants_and_exact_following_offset() {
    let cases = [
        (1, text("db", "old")),
        (2, text("lang", "old")),
        (3, text("charset", "old")),
        (4, text("4096", "2048")),
        (7, binary(&[], &[])),
        (8, binary(&[1, 2], &[])),
        (9, binary(&[], &[1, 2])),
        (10, binary(&[], &[1, 2])),
        (13, text("partner", "old")),
        (17, binary(&[], &[])),
        (18, binary(&[3], &[])),
        (99, binary(&[7, 8], &[9])),
    ];
    for (change_type, body) in cases {
        let mut input = env(change_type, &body);
        input.extend(done());
        let parsed = parse_token(&input, 0, "7_4").expect("complete ENVCHANGE");
        assert_eq!(
            parsed.offset,
            input.len() - done().len(),
            "ENVCHANGE offset for type {change_type}"
        );
        assert!(
            matches!(parsed.value, Token::EnvChange(value) if value.change_type == change_type)
        );
        let following = parse_token(&input, parsed.offset, "7_4").expect("following token");
        assert_eq!(following.offset, input.len());
        assert!(matches!(following.value, Token::Done(_)));
    }
}

#[test]
fn parses_routing_change_without_indexing_truncated_route() {
    let route = [0, 0x39, 0x30, 2, 0, b'S', 0, b'Q', 0];
    let input = env(20, &wide_binary(&route, &[]));
    let parsed = parse_token(&input, 0, "7_4").expect("routing token");
    assert!(
        matches!(parsed.value, Token::EnvChange(value) if matches!(&value.kind, EnvChangeKind::Routing(route) if route.port == 12345 && route.server == "SQ"))
    );
    for end in 0..input.len() {
        let result = parse_token(&input[..end], 0, "7_4");
        assert!(matches!(result, Err(TokenParseError::NotEnoughData(_))));
    }
    let mut malformed = env(20, &wide_binary(&[0, 1, 0, 4, 0, b'S', 0], &[]));
    malformed.extend(done());
    assert!(matches!(
        parse_token(&malformed, 0, "7_4"),
        Err(TokenParseError::Invalid(_))
    ));
}

#[test]
fn stream_preserves_fragments_and_skips_txn_ended_and_unknown_envchange() {
    let mut input = env(17, &binary(&[], &[]));
    input.extend(env(99, &binary(&[1], &[])));
    input.extend(done());
    let mut parser = TokenStreamParser::new("7_4", false);
    for byte in &input[..input.len() - 1] {
        if let Ok(tokens) = parser.feed(std::slice::from_ref(byte)) {
            assert!(tokens.is_empty());
        }
    }
    let result = parser
        .feed(&input[input.len() - 1..])
        .expect("stream completion");
    assert_eq!(result.len(), 1);
    assert!(matches!(result[0], StreamToken::Control(Token::Done(_))));
    assert_eq!(parser.finish(), Ok(()));
}

#[test]
fn unknown_token_is_terminal_and_truncated_token_is_retryable() {
    assert_eq!(
        parse_token(&[0x01], 0, "7_4"),
        Err(TokenParseError::Unsupported(1))
    );
    let mut parser = TokenStreamParser::new("7_4", false);
    assert!(parser.feed(&[0xfd]).is_err());
    assert!(parser.feed(&[0, 0]).is_err());
    let result = parser.feed(&done()[3..]);
    assert!(
        result.is_ok(),
        "completed token should be accepted: {result:?}"
    );
}

#[test]
fn dispatches_each_envchange_variant_to_individual_handlers() {
    let mut initial = InitialSqlTokenHandler::new();
    for (change_type, kind) in [
        (
            1,
            EnvChangeKind::Text {
                new_value: "db".into(),
                old_value: "".into(),
            },
        ),
        (
            2,
            EnvChangeKind::Text {
                new_value: "lang".into(),
                old_value: "".into(),
            },
        ),
        (
            3,
            EnvChangeKind::Text {
                new_value: "charset".into(),
                old_value: "".into(),
            },
        ),
        (
            4,
            EnvChangeKind::PacketSize {
                new_value: 4096,
                old_value: 2048,
            },
        ),
        (
            7,
            EnvChangeKind::Collation {
                new_value: None,
                old_value: None,
            },
        ),
        (
            8,
            EnvChangeKind::BeginTransaction {
                new_value: vec![1],
                old_value: vec![],
            },
        ),
        (
            9,
            EnvChangeKind::CommitTransaction {
                new_value: vec![],
                old_value: vec![1],
            },
        ),
        (
            10,
            EnvChangeKind::RollbackTransaction {
                new_value: vec![],
                old_value: vec![],
            },
        ),
        (
            18,
            EnvChangeKind::ResetConnection {
                new_value: vec![],
                old_value: vec![],
            },
        ),
    ] {
        dispatch_token(
            &mut initial,
            &Token::EnvChange(EnvChangeToken { change_type, kind }),
        )
        .expect("individual ENVCHANGE handler");
    }
    assert!(initial.events.contains(&HandlerEvent::ResetConnection));
    let mut login = Login7TokenHandler::new(false);
    dispatch_token(
        &mut login,
        &Token::EnvChange(EnvChangeToken {
            change_type: 13,
            kind: EnvChangeKind::Text {
                new_value: "p".into(),
                old_value: "".into(),
            },
        }),
    )
    .expect("login mirroring handler");
    assert!(
        login
            .events
            .contains(&HandlerEvent::DatabaseMirroringPartner("p".into()))
    );
    dispatch_token(
        &mut login,
        &Token::EnvChange(EnvChangeToken {
            change_type: 20,
            kind: EnvChangeKind::Routing(tedious_rs::token::parser::RoutingChange {
                protocol: 0,
                port: 1,
                server: "s".into(),
                old_value: vec![],
            }),
        }),
    )
    .expect("login routing handler");
    assert_eq!(login.routing_data.as_ref().map(|value| value.port), Some(1));
    let mut request = RequestTokenHandler::new(false, false);
    dispatch_token(
        &mut request,
        &Token::EnvChange(EnvChangeToken {
            change_type: 8,
            kind: EnvChangeKind::BeginTransaction {
                new_value: vec![4],
                old_value: vec![],
            },
        }),
    )
    .expect("request transaction handler");
    assert!(request.in_transaction);
    assert_eq!(request.transaction_descriptors, vec![vec![0; 8], vec![4]]);
}

struct BaseHandler;

impl TokenHandler for BaseHandler {}

#[test]
fn base_handler_rejects_each_individual_envchange_callback() {
    let variants = [1, 2, 3, 4, 7, 8, 9, 10, 13, 18, 20];
    for change_type in variants {
        let mut handler = BaseHandler;
        let token = EnvChangeToken {
            change_type,
            kind: EnvChangeKind::Unknown {
                new_value: vec![],
                old_value: vec![],
            },
        };
        assert!(dispatch_token(&mut handler, &Token::EnvChange(token)).is_err());
    }
}
