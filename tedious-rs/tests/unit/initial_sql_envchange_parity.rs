use tedious_rs::token::handler::{InitialSqlTokenHandler, TokenHandler};
use tedious_rs::token::parser::{EnvChangeKind, EnvChangeToken};
use tedious_rs::token::stream::{TokenStreamParser, TokenStreamState};

#[test]
fn initial_sql_envchange_preserves_node_transaction_descriptor_stack_state() {
    let mut handler = InitialSqlTokenHandler::new();
    handler.transaction_descriptors[0] = vec![9; 8];

    handler
        .on_env_change(&EnvChangeToken {
            change_type: 8,
            kind: EnvChangeKind::Binary {
                new_value: vec![1; 8],
                old_value: vec![9; 8],
            },
        })
        .unwrap();
    handler
        .on_env_change(&EnvChangeToken {
            change_type: 9,
            kind: EnvChangeKind::Binary {
                new_value: vec![9; 8],
                old_value: vec![1; 8],
            },
        })
        .unwrap();

    assert_eq!(handler.transaction_descriptors, vec![vec![9; 8]]);
    assert!(!handler.in_transaction);
}

#[test]
fn initial_sql_eof_finalization_rejects_a_trailing_partial_token() {
    let mut parser = TokenStreamParser::new("7_4", false);

    assert!(matches!(
        parser.feed(&[0xFD]),
        Err(tedious_rs::token::stream::StreamParseError::Token(_))
    ));
    assert_eq!(
        parser.finish(),
        Err(tedious_rs::token::stream::StreamParseError::NotEnoughData)
    );
    assert_eq!(*parser.state(), TokenStreamState::Failed);
}
