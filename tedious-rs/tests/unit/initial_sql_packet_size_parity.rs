use tedious_rs::token::handler::{InitialSqlTokenHandler, dispatch_token};
use tedious_rs::token::parser::{EnvChangeKind, EnvChangeToken, Token};

#[test]
fn initial_sql_preserves_server_negotiated_packet_size() {
    let mut handler = InitialSqlTokenHandler::new();
    dispatch_token(
        &mut handler,
        &Token::EnvChange(EnvChangeToken {
            change_type: 4,
            kind: EnvChangeKind::PacketSize {
                new_value: 8192,
                old_value: 4096,
            },
        }),
    )
    .unwrap();

    assert_eq!(
        handler.events,
        vec![tedious_rs::token::handler::HandlerEvent::PacketSize(8192)]
    );
}
