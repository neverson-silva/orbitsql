use tedious_rs::connection::dispatch_request_tokens;
use tedious_rs::errors::RequestError;
use tedious_rs::request::Request;
use tedious_rs::token::handler::{RequestTokenHandler, TokenHandler};
use tedious_rs::token::parser::{MessageToken, Token};
use tedious_rs::token::stream::StreamToken;

fn error_token() -> MessageToken {
    MessageToken {
        token: 0xAA,
        number: 208,
        state: 2,
        class: 16,
        message: "invalid object".to_owned(),
        server_name: "server-a".to_owned(),
        proc_name: "procedure-a".to_owned(),
        line_number: 19,
    }
}

#[test]
fn request_error_populates_node_and_rust_field_names() {
    let mut handler = RequestTokenHandler::new(false, false);
    handler.on_error(&error_token()).unwrap();

    let error = handler.error.as_ref().unwrap();
    assert_eq!(error.serverName.as_deref(), Some("server-a"));
    assert_eq!(error.procName.as_deref(), Some("procedure-a"));
    assert_eq!(error.lineNumber, Some(19));
    assert_eq!(error.server_name.as_deref(), Some("server-a"));
    assert_eq!(error.proc_name.as_deref(), Some("procedure-a"));
    assert_eq!(error.line_number, Some(19));
}

#[test]
fn aggregate_error_keeps_node_shape_and_member_details() {
    let mut handler = RequestTokenHandler::new(false, false);
    let first = error_token();
    let mut second = error_token();
    second.number = 2627;
    second.message = "duplicate key".to_owned();

    handler.on_error(&first).unwrap();
    handler.on_error(&second).unwrap();

    let aggregate = handler.error.as_ref().unwrap();
    assert_eq!(aggregate.message, "");
    assert_eq!(aggregate.code, None);
    assert_eq!(aggregate.errors.len(), 2);
    assert_eq!(aggregate.errors[0].message, "invalid object");
    assert_eq!(aggregate.errors[0].code.as_deref(), Some("EREQUEST"));
    assert_eq!(aggregate.errors[1].message, "duplicate key");
    assert_eq!(aggregate.errors[1].code.as_deref(), Some("EREQUEST"));
    assert_eq!(aggregate.errors[0].serverName.as_deref(), Some("server-a"));
    assert_eq!(aggregate.errors[1].lineNumber, Some(19));
}

#[test]
fn request_error_aggregate_constructor_preserves_member_message_and_code() {
    let first = RequestError::new("first", Some("EREQUEST".to_owned()));
    let second = RequestError::new("second", Some("EREQUEST".to_owned()));
    let aggregate = RequestError::aggregate(vec![first.clone(), second.clone()]);

    assert_eq!(aggregate.errors, vec![first, second]);
    assert_eq!(aggregate.errors[0].message, "first");
    assert_eq!(aggregate.errors[0].code.as_deref(), Some("EREQUEST"));
}

#[test]
fn dispatched_request_aggregate_matches_node_error_shape() {
    let mut request = Request::new("SELECT 1", |_| {});
    let first = error_token();
    let mut second = error_token();
    second.number = 2627;
    second.message = "duplicate key".to_owned();

    dispatch_request_tokens(
        &mut request,
        vec![
            StreamToken::Control(Token::Error(first)),
            StreamToken::Control(Token::Error(second)),
        ],
        false,
        false,
        false,
    );

    let aggregate = request.error.as_ref().unwrap();
    assert_eq!(aggregate.message, "");
    assert_eq!(aggregate.code, None);
    assert_eq!(aggregate.errors.len(), 2);
    assert_eq!(aggregate.errors[0].serverName.as_deref(), Some("server-a"));
    assert_eq!(aggregate.errors[1].procName.as_deref(), Some("procedure-a"));
}
