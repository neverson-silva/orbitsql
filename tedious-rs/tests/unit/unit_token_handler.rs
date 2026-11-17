use tedious_rs::token::handler::{
    AttentionTokenHandler, HandlerEvent, InitialSqlTokenHandler, Login7TokenHandler,
    RequestTokenHandler, TokenHandler, UnexpectedTokenError, dispatch_token,
};
use tedious_rs::token::parser::{
    DoneToken, EnvChangeKind, EnvChangeToken, FeatureExtAckToken, LoginAckToken, MessageToken,
    ReturnStatusToken, Token,
};
use tedious_rs::token::rows::Row;

struct RecordingHandler {
    seen: Vec<&'static str>,
}

impl TokenHandler for RecordingHandler {
    fn on_done(&mut self, _token: &DoneToken) -> Result<(), UnexpectedTokenError> {
        self.seen.push("DONE");
        Ok(())
    }
}

#[test]
fn dispatches_done_token_to_the_matching_handler_method() {
    let mut handler = RecordingHandler { seen: Vec::new() };
    dispatch_token(
        &mut handler,
        &Token::Done(DoneToken {
            token: 0xFD,
            more: false,
            sql_error: false,
            attention: false,
            server_error: false,
            row_count: Some(1),
            cur_cmd: 0,
        }),
    )
    .unwrap();
    assert_eq!(handler.seen, vec!["DONE"]);
}

#[test]
fn default_handler_rejects_unexpected_token_with_node_compatible_context() {
    let mut handler = RecordingHandler { seen: Vec::new() };
    let error = dispatch_token(
        &mut handler,
        &Token::Info(tedious_rs::token::parser::MessageToken {
            token: 0xAB,
            number: 1,
            state: 1,
            class: 0,
            message: "info".to_owned(),
            server_name: "server".to_owned(),
            proc_name: "proc".to_owned(),
            line_number: 1,
        }),
    )
    .unwrap_err();
    assert_eq!(error.token, "INFO");
    assert_eq!(error.handler, "RecordingHandler");
}

#[test]
fn initial_sql_handler_applies_environment_changes_and_rejects_rows() {
    let mut handler = InitialSqlTokenHandler::new();
    handler
        .on_env_change(&EnvChangeToken {
            change_type: 8,
            kind: EnvChangeKind::Binary {
                new_value: vec![1, 2],
                old_value: Vec::new(),
            },
        })
        .unwrap();
    handler
        .on_env_change(&EnvChangeToken {
            change_type: 10,
            kind: EnvChangeKind::Binary {
                new_value: Vec::new(),
                old_value: Vec::new(),
            },
        })
        .unwrap();
    assert!(!handler.in_transaction);
    assert_eq!(handler.events, vec![HandlerEvent::RollbackTransaction]);
    let error = handler
        .on_row(&Row {
            columns: Vec::new(),
        })
        .unwrap_err();
    assert_eq!(error.token, "ROW");
}

#[test]
fn initial_sql_handler_applies_typed_environment_changes() {
    let mut handler = InitialSqlTokenHandler::new();
    handler
        .on_env_change(&EnvChangeToken {
            change_type: 8,
            kind: EnvChangeKind::BeginTransaction {
                new_value: vec![1, 2],
                old_value: Vec::new(),
            },
        })
        .unwrap();
    handler
        .on_env_change(&EnvChangeToken {
            change_type: 9,
            kind: EnvChangeKind::CommitTransaction {
                new_value: Vec::new(),
                old_value: vec![1, 2],
            },
        })
        .unwrap();
    handler
        .on_env_change(&EnvChangeToken {
            change_type: 10,
            kind: EnvChangeKind::RollbackTransaction {
                new_value: Vec::new(),
                old_value: Vec::new(),
            },
        })
        .unwrap();
    handler
        .on_env_change(&EnvChangeToken {
            change_type: 18,
            kind: EnvChangeKind::ResetConnection {
                new_value: Vec::new(),
                old_value: Vec::new(),
            },
        })
        .unwrap();
    assert_eq!(
        handler.events,
        vec![
            HandlerEvent::RollbackTransaction,
            HandlerEvent::ResetConnection
        ]
    );
    assert!(!handler.in_transaction);
}

#[test]
fn initial_sql_handler_ignores_return_status() {
    let mut handler = InitialSqlTokenHandler::new();
    assert!(
        handler
            .on_return_status(&ReturnStatusToken { value: 7 })
            .is_ok()
    );
    assert!(handler.events.is_empty());
}

#[test]
fn initial_and_login_handlers_record_info_messages() {
    let token = MessageToken {
        token: 0xAB,
        number: 1,
        state: 1,
        class: 0,
        message: "info".to_owned(),
        server_name: "server".to_owned(),
        proc_name: "proc".to_owned(),
        line_number: 1,
    };
    let mut initial = InitialSqlTokenHandler::new();
    initial.on_info(&token).unwrap();
    assert_eq!(initial.events, vec![HandlerEvent::Info(token.clone())]);

    let mut login = Login7TokenHandler::new(false);
    login.on_info(&token).unwrap();
    assert_eq!(login.events, vec![HandlerEvent::Info(token)]);
}

#[test]
fn request_handler_retains_info_messages_without_emitting_request_events() {
    let token = MessageToken {
        token: 0xAB,
        number: 1,
        state: 1,
        class: 0,
        message: "info".to_owned(),
        server_name: "server".to_owned(),
        proc_name: "proc".to_owned(),
        line_number: 1,
    };
    let mut handler = RequestTokenHandler::new(false, false);
    handler.on_info(&token).unwrap();
    assert_eq!(handler.info_messages, vec![token]);
    assert!(handler.events.is_empty());
}

#[test]
fn login_handler_preserves_ack_sspi_feature_and_routing_state() {
    let mut handler = Login7TokenHandler::new(true);
    handler
        .on_feature_ext_ack(&FeatureExtAckToken {
            fed_auth: None,
            utf8_support: None,
        })
        .unwrap();
    assert_eq!(handler.login_error.as_ref().unwrap().code.as_deref(), None);
    handler.login_error = None;
    handler
        .on_login_ack(&LoginAckToken {
            interface: Some("SQL_TSQL".to_owned()),
            tds_version: Some("7_4".to_owned()),
            prog_name: "server".to_owned(),
            prog_version: [1, 0, 0, 0],
        })
        .unwrap();
    assert!(handler.login_ack_received);
    handler
        .on_env_change(&EnvChangeToken {
            change_type: 20,
            kind: EnvChangeKind::Routing(tedious_rs::token::parser::RoutingChange {
                protocol: 0,
                port: 1444,
                server: "db\\instance".to_owned(),
                old_value: Vec::new(),
            }),
        })
        .unwrap();
    assert_eq!(
        handler.routing_data.unwrap().instance.as_deref(),
        Some("instance")
    );
}

#[test]
fn login_handler_rejects_extra_federated_ack_data_and_unknown_features() {
    let mut federated = Login7TokenHandler::new(true);
    federated
        .on_feature_ext_ack(&FeatureExtAckToken {
            fed_auth: Some(vec![1]),
            utf8_support: None,
        })
        .unwrap();
    assert!(federated.login_error.is_some());
    assert_eq!(federated.login_error.as_ref().unwrap().code, None);

    let mut standard = Login7TokenHandler::new(false);
    standard
        .on_feature_ext_ack(&FeatureExtAckToken {
            fed_auth: None,
            utf8_support: None,
        })
        .unwrap();
    assert!(standard.login_error.is_some());
    assert_eq!(standard.login_error.as_ref().unwrap().code, None);
}

#[test]
fn feature_ack_error_uses_node_message_and_code() {
    let mut handler = Login7TokenHandler::new(true);
    handler
        .on_feature_ext_ack(&FeatureExtAckToken {
            fed_auth: Some(vec![1]),
            utf8_support: None,
        })
        .unwrap();
    let error = handler.login_error.unwrap();
    assert_eq!(
        error.message,
        "Active Directory authentication acknowledgment includes extra data"
    );
    assert_eq!(error.code, None);
}

#[test]
fn login_handler_marks_transient_only_before_retry_limit() {
    let mut retryable = Login7TokenHandler::with_transient_retry_state(false, 0, 3);
    retryable
        .on_error(&MessageToken {
            token: 0xAA,
            number: 4060,
            state: 1,
            class: 16,
            message: "login failed".to_owned(),
            server_name: String::new(),
            proc_name: String::new(),
            line_number: 0,
        })
        .unwrap();
    assert_eq!(retryable.login_error.unwrap().is_transient, Some(true));

    let mut exhausted = Login7TokenHandler::with_transient_retry_state(false, 3, 3);
    exhausted
        .on_error(&MessageToken {
            token: 0xAA,
            number: 4060,
            state: 1,
            class: 16,
            message: "login failed".to_owned(),
            server_name: String::new(),
            proc_name: String::new(),
            line_number: 0,
        })
        .unwrap();
    assert_eq!(exhausted.login_error.unwrap().is_transient, None);
}

#[test]
fn request_handler_emits_rows_errors_return_status_and_done_variants() {
    let mut handler = RequestTokenHandler::new(true, true);
    handler
        .on_row(&Row {
            columns: Vec::new(),
        })
        .unwrap();
    handler
        .on_error(&MessageToken {
            token: 0xAA,
            number: 208,
            state: 1,
            class: 16,
            message: "missing".to_owned(),
            server_name: "server".to_owned(),
            proc_name: "proc".to_owned(),
            line_number: 4,
        })
        .unwrap();
    handler
        .on_return_status(&ReturnStatusToken { value: 3 })
        .unwrap();
    handler
        .on_done_proc(&DoneToken {
            token: 0xFE,
            more: true,
            sql_error: false,
            attention: false,
            server_error: false,
            row_count: Some(2),
            cur_cmd: 0,
        })
        .unwrap();
    assert_eq!(handler.errors[0].code.as_deref(), Some("EREQUEST"));
    assert_eq!(handler.error, Some(handler.errors[0].clone()));
    assert_eq!(handler.row_count, 2);
    assert!(matches!(
        handler.events[0],
        tedious_rs::request::RequestEvent::NamedRow(_)
    ));
    assert!(matches!(
        handler.events[1],
        tedious_rs::request::RequestEvent::DoneProc { .. }
    ));
    assert_eq!(handler.return_status, None);
}

#[test]
fn request_handler_does_not_reuse_return_status_after_done_proc() {
    let mut handler = RequestTokenHandler::new(false, false);
    let done = DoneToken {
        token: 0xFE,
        more: false,
        sql_error: false,
        attention: false,
        server_error: false,
        row_count: None,
        cur_cmd: 0,
    };
    handler
        .on_return_status(&ReturnStatusToken { value: 17 })
        .unwrap();
    handler.on_done_proc(&done).unwrap();
    handler.on_done_proc(&done).unwrap();
    assert!(matches!(
        handler.events[0],
        tedious_rs::request::RequestEvent::DoneProc {
            return_status: Some(17),
            ..
        }
    ));
    assert!(matches!(
        handler.events[1],
        tedious_rs::request::RequestEvent::DoneProc {
            return_status: None,
            ..
        }
    ));
    assert_eq!(handler.return_status, None);
}

#[test]
fn request_handler_keeps_rows_for_request_completion_without_done_collection() {
    let mut handler = RequestTokenHandler::new(false, false);
    handler.set_row_collection_on_request_completion(true);
    let row = Row {
        columns: Vec::new(),
    };
    handler.on_row(&row).unwrap();
    handler
        .on_done(&DoneToken {
            token: 0xFD,
            more: false,
            sql_error: false,
            attention: false,
            server_error: false,
            row_count: None,
            cur_cmd: 0,
        })
        .unwrap();
    assert_eq!(handler.rows, vec![row]);
    assert!(matches!(
        handler.events[1],
        tedious_rs::request::RequestEvent::Done { rows: None, .. }
    ));
}

#[test]
fn request_handler_does_not_retain_rows_when_request_completion_collection_is_disabled() {
    let mut handler = RequestTokenHandler::new(false, false);
    handler
        .on_row(&Row {
            columns: Vec::new(),
        })
        .unwrap();
    assert!(handler.rows.is_empty());
}

#[test]
fn request_handler_emits_unknown_error_before_done_when_done_error_is_set() {
    let mut handler = RequestTokenHandler::new(false, false);
    handler
        .on_done(&DoneToken {
            token: 0xFD,
            more: false,
            sql_error: true,
            attention: false,
            server_error: false,
            row_count: Some(0),
            cur_cmd: 0,
        })
        .unwrap();
    assert!(handler.errors.is_empty());
    assert_eq!(
        handler
            .error
            .as_ref()
            .and_then(|error| error.code.as_deref()),
        Some("UNKNOWN")
    );
    assert!(matches!(
        handler.events[0],
        tedious_rs::request::RequestEvent::Done { .. }
    ));
}

#[test]
fn request_handler_does_not_duplicate_unknown_error_after_error_token() {
    let mut handler = RequestTokenHandler::new(false, false);
    handler
        .on_error(&MessageToken {
            token: 0xAA,
            number: 50000,
            state: 1,
            class: 16,
            message: "server error".to_owned(),
            server_name: "server".to_owned(),
            proc_name: "proc".to_owned(),
            line_number: 1,
        })
        .unwrap();
    handler
        .on_done(&DoneToken {
            token: 0xFD,
            more: false,
            sql_error: true,
            attention: false,
            server_error: false,
            row_count: Some(0),
            cur_cmd: 0,
        })
        .unwrap();
    assert_eq!(handler.errors.len(), 1);
    assert_eq!(handler.errors[0].code.as_deref(), Some("EREQUEST"));
}

#[test]
fn attention_handler_only_accepts_attention_done() {
    let mut handler = AttentionTokenHandler::new();
    handler
        .on_done(&DoneToken {
            token: 0xFD,
            more: false,
            sql_error: false,
            attention: true,
            server_error: false,
            row_count: None,
            cur_cmd: 0,
        })
        .unwrap();
    assert!(handler.attention_received);
}

#[test]
fn request_handler_aggregates_all_request_errors_without_losing_individual_errors() {
    let mut handler = RequestTokenHandler::new(false, false);
    for number in [208, 2627] {
        handler
            .on_error(&MessageToken {
                token: 0xAA,
                number,
                state: 1,
                class: 16,
                message: format!("error {number}"),
                server_name: "server".to_owned(),
                proc_name: "proc".to_owned(),
                line_number: 4,
            })
            .unwrap();
    }
    assert_eq!(handler.errors.len(), 2);
    let aggregate = handler.error.as_ref().unwrap();
    assert!(aggregate.message.is_empty());
    assert!(aggregate.code.is_none());
    assert_eq!(aggregate.errors, handler.errors);
}

#[test]
fn request_handler_filters_duplicate_column_metadata_when_using_names() {
    let parsed = tedious_rs::token::rows::parse_colmetadata(
        &[1, 0, 0, 0, 0, 0, 0, 0, 0x38, 2, b'I', 0, b'd', 0],
        "7_4",
        false,
    )
    .unwrap()
    .value;
    let metadata = vec![parsed[0].clone(), parsed[0].clone()];
    let mut handler = RequestTokenHandler::new(false, true);
    handler.on_col_metadata(&metadata).unwrap();
    assert_eq!(handler.columns.len(), 1);
    assert_eq!(handler.columns[0].col_name, "Id");
}
