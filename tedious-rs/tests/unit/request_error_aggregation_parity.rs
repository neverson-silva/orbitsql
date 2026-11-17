use tedious_rs::request::RequestEvent;
use tedious_rs::token::handler::{RequestTokenHandler, TokenHandler};
use tedious_rs::token::parser::{DoneToken, MessageToken};

fn error(number: u32, message: &str) -> MessageToken {
    MessageToken {
        token: 0xAA,
        number,
        state: 1,
        class: 16,
        message: message.to_owned(),
        server_name: "server".to_owned(),
        proc_name: "procedure".to_owned(),
        line_number: 7,
    }
}

fn done(sql_error: bool) -> DoneToken {
    DoneToken {
        token: 0xFD,
        more: false,
        sql_error,
        attention: false,
        server_error: false,
        row_count: Some(0),
        cur_cmd: 0,
    }
}

#[test]
fn multiple_error_messages_preserve_order_without_request_error_events() {
    let mut handler = RequestTokenHandler::new(false, false);

    handler.on_error(&error(208, "first")).unwrap();
    handler.on_error(&error(2627, "second")).unwrap();
    handler.on_done(&done(true)).unwrap();

    assert_eq!(handler.errors.len(), 2);
    assert_eq!(handler.errors[0].message, "first");
    assert_eq!(handler.errors[1].message, "second");
    let aggregate = handler.error.as_ref().unwrap();
    assert!(aggregate.message.is_empty());
    assert_eq!(aggregate.errors, handler.errors);
    assert_eq!(handler.events.len(), 1);
    assert!(matches!(handler.events[0], RequestEvent::Done { .. }));
}

#[test]
fn done_error_without_error_message_is_one_unknown_error() {
    let mut handler = RequestTokenHandler::new(false, false);

    handler.on_done(&done(true)).unwrap();

    assert!(handler.errors.is_empty());
    assert_eq!(
        handler
            .error
            .as_ref()
            .and_then(|error| error.code.as_deref()),
        Some("UNKNOWN")
    );
    assert!(matches!(handler.events[0], RequestEvent::Done { .. }));
}

#[test]
fn request_error_events_remain_available_for_explicit_emission() {
    let mut request = tedious_rs::request::Request::new("SELECT 1", |_| {});
    let error = tedious_rs::errors::RequestError::new("error", Some("EREQUEST".to_owned()));
    assert!(!request.emit_checked(RequestEvent::Error(error)));
}
