use std::sync::{Arc, Mutex};
use std::time::Duration;

use tedious_rs::always_encrypted::types::StatementColumnEncryptionSetting;
use tedious_rs::errors::RequestError;
use tedious_rs::parameter::{ParameterOptions, SqlParameterType};
use tedious_rs::request::{Request, RequestEvent, RequestResult, RequestState};
use tedious_rs::token::rows::Row;
use tedious_rs::value_codec::Value;

#[test]
fn request_preserves_statement_column_encryption_setting_with_node_default() {
    let request = Request::new("SELECT 1", |_| {});
    assert_eq!(
        request.statement_column_encryption_setting,
        StatementColumnEncryptionSetting::UseConnectionSetting
    );
    let request = Request::with_options(
        "SELECT 1",
        |_| {},
        tedious_rs::RequestOptions {
            statement_column_encryption_setting: StatementColumnEncryptionSetting::Disabled,
        },
    );
    assert_eq!(
        request.statement_column_encryption_setting,
        StatementColumnEncryptionSetting::Disabled
    );
    assert!(!request.should_honor_column_encryption(true));
    let request = Request::with_options(
        "SELECT 1",
        |_| {},
        tedious_rs::RequestOptions {
            statement_column_encryption_setting: StatementColumnEncryptionSetting::Enabled,
        },
    );
    assert!(request.should_honor_column_encryption(false));
}

#[test]
fn optional_parameter_helpers_preserve_node_null_and_default_options() {
    let mut request = Request::new("SELECT @value", |_| {});
    let data_type = tedious_rs::type_by_name("Int").unwrap();
    request
        .add_parameter_optional("value", data_type, None, None)
        .unwrap();
    request
        .add_output_parameter_optional("result", data_type, Some(Value::Integer(2)), None)
        .unwrap();
    assert_eq!(request.parameters.len(), 2);
    assert_eq!(request.parameters[0].encoded.length, vec![0]);
    assert!(request.parameters[1].output);
}

#[test]
fn emits_cancel_only_once_for_a_created_request() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    let mut request = Request::new(
        "SELECT 1",
        |_result: Result<RequestResult, tedious_rs::errors::RequestError>| {},
    );
    request.on(move |event| seen.lock().unwrap().push(event.clone()));
    request.cancel();
    request.cancel();
    assert_eq!(request.state, RequestState::Canceled);
    assert_eq!(*events.lock().unwrap(), vec![RequestEvent::Cancel]);
}

#[test]
fn canceled_request_finishes_once_without_duplicate_callback() {
    let callback_count = Arc::new(Mutex::new(0));
    let callback_seen = Arc::clone(&callback_count);
    let mut request = Request::new("SELECT 1", move |result| {
        assert!(result.is_err());
        *callback_seen.lock().unwrap() += 1;
    });
    request.row_count = 9;
    request.cancel();
    request.finish();
    request.finish();
    assert_eq!(*callback_count.lock().unwrap(), 1);
    assert_eq!(request.state, RequestState::Completed);
}

#[test]
fn preparing_failure_emits_error_without_completion_event_or_user_callback() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    let callback_called = Arc::new(Mutex::new(false));
    let callback_flag = Arc::clone(&callback_called);
    let mut request = Request::new("SELECT 1", move |_| {
        *callback_flag.lock().unwrap() = true;
    });
    request.preparing = true;
    request.on(move |event| seen.lock().unwrap().push(event.clone()));
    let error =
        tedious_rs::errors::RequestError::new("prepare failed", Some("EPREPARE".to_owned()));
    request.fail_preparing(error.clone());
    assert_eq!(request.state, RequestState::Created);
    assert!(!*callback_called.lock().unwrap());
    assert_eq!(*events.lock().unwrap(), vec![RequestEvent::Error(error)]);
}

#[test]
fn preparing_failure_preserves_server_error_and_completion_callback() {
    let callback_called = Arc::new(Mutex::new(0));
    let callback_count = Arc::clone(&callback_called);
    let mut request = Request::new("SELECT 1", move |result| {
        assert!(result.is_ok());
        *callback_count.lock().unwrap() += 1;
    });
    request.preparing = true;
    let server_error =
        tedious_rs::errors::RequestError::new("server prepare failed", Some("EREQUEST".to_owned()));
    request.error = Some(server_error.clone());
    request.fail_preparing(tedious_rs::errors::RequestError::new(
        "prepare failed",
        Some("EPREPARE".to_owned()),
    ));
    assert_eq!(*callback_called.lock().unwrap(), 0);
    assert_eq!(request.error, Some(server_error));
    assert_eq!(*callback_called.lock().unwrap(), 0);
}

#[test]
fn prepared_request_reuses_completion_callback_for_each_execution() {
    let callback_count = Arc::new(Mutex::new(0));
    let observed = Arc::clone(&callback_count);
    let mut request = Request::new("SELECT @value", move |result| {
        assert!(result.is_ok());
        *observed.lock().unwrap() += 1;
    });

    request.begin().unwrap();
    request.finish();
    request.state = RequestState::Created;
    request.begin().unwrap();
    request.finish();

    assert_eq!(*callback_count.lock().unwrap(), 2);
}

#[test]
fn completed_request_resets_execution_state_and_keeps_fnmut_callback() {
    let callback_results = Arc::new(Mutex::new(Vec::new()));
    let seen_results = Arc::clone(&callback_results);
    let mut request = Request::new("SELECT 1", move |result| {
        seen_results.lock().unwrap().push(result.is_ok());
    });

    request.rows.push(Row {
        columns: Vec::new(),
    });
    request.row_count = 7;
    request.error = Some(RequestError::new("old error", Some("EOLD".to_owned())));
    request.begin().unwrap();
    request.rows.push(Row {
        columns: Vec::new(),
    });
    request.row_count = 1;
    request.finish();

    request.begin().unwrap();

    assert_eq!(request.state, RequestState::Running);
    assert!(request.rows.is_empty());
    assert_eq!(request.row_count, 0);
    assert!(request.errors.is_empty());
    assert!(request.error.is_none());
    request.finish();
    assert_eq!(*callback_results.lock().unwrap(), vec![true, true]);
}

#[test]
fn failed_request_can_be_reused_but_canceled_request_cannot() {
    let mut request = Request::new("SELECT 1", |_| {});
    request.begin().unwrap();
    request.fail(RequestError::new("failed", Some("EFAIL".to_owned())));
    request.begin().unwrap();
    assert_eq!(request.state, RequestState::Running);

    request.cancel();
    request.fail(RequestError::new("Canceled.", Some("ECANCEL".to_owned())));
    let error = request.begin().unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ECANCEL"));
}

#[test]
fn reused_request_gets_a_fresh_cancellation_cycle() {
    let mut request = Request::new("SELECT 1", |_| {});
    request.begin().unwrap();
    let old_handle = request.cancellation_handle();
    request.finish();

    request.begin().unwrap();
    let current_handle = request.cancellation_handle();
    old_handle.cancel();
    assert!(!current_handle.is_cancelled());
    current_handle.cancel();
    assert!(current_handle.is_cancelled());
    assert!(request.cancel());
    assert_eq!(request.state, RequestState::Canceled);
}

#[test]
fn preparing_failure_can_be_followed_by_a_new_prepare_attempt() {
    let mut request = Request::new("SELECT 1", |_| {});
    request.preparing = true;
    request.fail_preparing(tedious_rs::errors::RequestError::new(
        "prepare failed",
        Some("EPREPARE".to_owned()),
    ));

    assert_eq!(request.state, RequestState::Created);
    request.preparing = true;
    assert!(request.begin().is_ok());
}

#[test]
fn completion_callback_runs_before_request_completed_event() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    let callback_seen = Arc::clone(&events);
    let mut request = Request::new("SELECT 1", move |result| {
        assert!(result.is_ok());
        callback_seen.lock().unwrap().push(RequestEvent::Done {
            row_count: Some(0),
            more: false,
            rows: None,
        });
    });
    request.on(move |event| seen.lock().unwrap().push(event.clone()));
    request.begin().unwrap();
    request.finish();
    assert_eq!(request.state, RequestState::Completed);
    assert_eq!(events.lock().unwrap().len(), 2);
    assert!(matches!(
        events.lock().unwrap()[0],
        RequestEvent::Done { .. }
    ));
    assert!(matches!(
        events.lock().unwrap()[1],
        RequestEvent::RequestCompleted
    ));
}

#[test]
fn invalid_second_begin_is_reported_without_changing_state() {
    let mut request = Request::new("SELECT 1", |_| {});
    request.begin().unwrap();
    let error = request.begin().unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EINVALIDSTATE"));
    assert_eq!(request.state, RequestState::Running);
}

#[test]
fn pause_and_resume_are_idempotent_and_independent_of_request_state() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    let mut request = Request::new("SELECT 1", |_| {});
    request.on(move |event| seen.lock().unwrap().push(event.clone()));
    request.pause();
    request.pause();
    request.resume();
    request.resume();
    assert!(!request.paused);
    assert_eq!(
        *events.lock().unwrap(),
        vec![RequestEvent::Pause, RequestEvent::Resume]
    );
}

#[test]
fn drops_response_events_queued_before_cancel() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    let mut request = Request::new("SELECT 1", |_| {});
    request.on(move |event| seen.lock().unwrap().push(event.clone()));
    request.pause();
    request.emit(RequestEvent::Done {
        row_count: Some(1),
        more: false,
        rows: None,
    });
    assert_eq!(*events.lock().unwrap(), vec![RequestEvent::Pause]);
    request.resume();
    assert!(matches!(events.lock().unwrap()[1], RequestEvent::Resume));
    assert!(matches!(
        events.lock().unwrap()[2],
        RequestEvent::Done { .. }
    ));
    request.pause();
    request.emit(RequestEvent::Done {
        row_count: Some(2),
        more: false,
        rows: None,
    });
    request.cancel();
    assert!(
        events
            .lock()
            .unwrap()
            .ends_with(&[RequestEvent::Pause, RequestEvent::Cancel])
    );
    request.resume();
    assert!(!events.lock().unwrap().iter().any(|event| matches!(
        event,
        RequestEvent::Done {
            row_count: Some(2),
            ..
        }
    )));
}

#[test]
fn suppresses_request_response_events_after_cancel() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    let mut request = Request::new("SELECT 1", |_| {});
    request.on(move |event| seen.lock().unwrap().push(event.clone()));
    request.begin().unwrap();
    request.cancel();
    request.emit(RequestEvent::ColumnMetadata(Vec::new()));
    request.emit(RequestEvent::Row(tedious_rs::token::rows::Row {
        columns: Vec::new(),
    }));
    request.emit(RequestEvent::Done {
        row_count: Some(1),
        more: false,
        rows: None,
    });
    request.emit(RequestEvent::Error(tedious_rs::errors::RequestError::new(
        "server failure",
        Some("EREQUEST".to_owned()),
    )));
    request.emit(RequestEvent::Order(Vec::new()));
    assert_eq!(*events.lock().unwrap(), vec![RequestEvent::Cancel]);
}

#[test]
fn failed_request_callback_precedes_request_completed_event() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    let callback_seen = Arc::clone(&events);
    let mut request = Request::new("SELECT 1", move |result| {
        assert!(result.is_err());
        callback_seen.lock().unwrap().push(RequestEvent::Done {
            row_count: None,
            more: false,
            rows: None,
        });
    });
    request.on(move |event| seen.lock().unwrap().push(event.clone()));
    request.begin().unwrap();
    request.fail(tedious_rs::errors::RequestError::new("failure", None));
    assert!(matches!(
        events.lock().unwrap()[0],
        RequestEvent::Done { .. }
    ));
    assert!(matches!(
        events.lock().unwrap()[1],
        RequestEvent::RequestCompleted
    ));
}

#[test]
fn request_events_preserve_done_variants_and_protocol_row_count() {
    let result = Arc::new(Mutex::new(None));
    let seen_result = Arc::clone(&result);
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen_events = Arc::clone(&events);
    let mut request = Request::new("SELECT 1", move |value| {
        *seen_result.lock().unwrap() = Some(value.unwrap());
    });
    request.on(move |event| seen_events.lock().unwrap().push(event.clone()));
    request.begin().unwrap();
    request.row_count = 7;
    request.emit(RequestEvent::DoneProc {
        row_count: Some(3),
        more: true,
        return_status: Some(0),
        rows: None,
    });
    request.emit(RequestEvent::DoneInProc {
        row_count: Some(4),
        more: false,
        rows: None,
    });
    request.finish();
    assert!(matches!(
        events.lock().unwrap()[0],
        RequestEvent::DoneProc { .. }
    ));
    assert!(matches!(
        events.lock().unwrap()[1],
        RequestEvent::DoneInProc { .. }
    ));
    assert_eq!(result.lock().unwrap().as_ref().unwrap().row_count, 7);
}

#[test]
fn canceled_request_cannot_begin_and_returns_ecancel() {
    let mut request = Request::new("SELECT 1", |_| {});
    request.cancel();
    let error = request.begin().unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ECANCEL"));
}

#[test]
fn request_timeout_can_override_connection_default_and_be_disabled() {
    let mut request = Request::new("SELECT 1", |_| {});
    assert_eq!(request.timeout, None);
    request.set_timeout(Some(Duration::from_millis(25)));
    assert_eq!(request.timeout, Some(Duration::from_millis(25)));
    request.set_timeout(Some(Duration::ZERO));
    assert_eq!(request.timeout, Some(Duration::ZERO));
}

#[test]
fn completion_row_collection_is_disabled_by_default_and_configurable() {
    let received = std::sync::Arc::new(std::sync::Mutex::new(None));
    let seen = std::sync::Arc::clone(&received);
    let mut request = Request::new("SELECT 1", move |result| {
        *seen.lock().unwrap() = Some(result.unwrap().rows);
    });
    request.begin().unwrap();
    request.finish();
    assert_eq!(received.lock().unwrap().as_ref().unwrap().len(), 0);

    let received = std::sync::Arc::new(std::sync::Mutex::new(None));
    let seen = std::sync::Arc::clone(&received);
    let mut request = Request::new("SELECT 1", move |result| {
        *seen.lock().unwrap() = Some(result.unwrap().rows);
    });
    request.set_row_collection_on_completion(true);
    request.begin().unwrap();
    request.rows.push(tedious_rs::token::rows::Row {
        columns: Vec::new(),
    });
    request.finish();
    assert_eq!(received.lock().unwrap().as_ref().unwrap().len(), 1);
}

#[test]
fn typed_parameters_validate_and_reencode_values_for_prepared_execution() {
    let mut request = Request::new("SELECT @id, @name", |_| {});
    request
        .add_typed_parameter(
            "id",
            SqlParameterType::Int,
            Value::Integer(1),
            ParameterOptions::default(),
        )
        .unwrap();
    request
        .add_typed_parameter(
            "name",
            SqlParameterType::NVarChar,
            Value::Text("one".to_owned()),
            ParameterOptions {
                length: Some(20),
                ..ParameterOptions::default()
            },
        )
        .unwrap();
    request
        .set_typed_parameter_value("id", Value::Integer(2))
        .unwrap();
    assert_eq!(request.parameters[0].encoded.data, 2i32.to_le_bytes());
    assert!(
        request
            .set_typed_parameter_value("id", Value::Text("invalid".to_owned()))
            .is_err()
    );
}

#[test]
fn typed_parameter_declaration_matches_node_parameter_contract() {
    let mut request = Request::new("SELECT @id, @name", |_| {});
    request
        .add_typed_parameter(
            "id",
            SqlParameterType::Int,
            Value::Integer(1),
            ParameterOptions::default(),
        )
        .unwrap();
    request
        .add_typed_output_parameter(
            "name",
            SqlParameterType::NVarChar,
            Value::Text("value".to_owned()),
            ParameterOptions {
                length: Some(20),
                ..ParameterOptions::default()
            },
        )
        .unwrap();
    assert_eq!(
        request.make_params_declaration().unwrap(),
        "@id int, @name nvarchar(20) OUTPUT"
    );
    request
        .set_typed_parameter_value("name", Value::Text("longer".to_owned()))
        .unwrap();
    assert_eq!(
        request.make_params_declaration().unwrap(),
        "@id int, @name nvarchar(20) OUTPUT"
    );
    assert_eq!(
        request.make_params_parameter().unwrap(),
        "@id int, @name nvarchar(20) OUTPUT"
    );
}

#[test]
fn validates_typed_parameters_with_node_fallback_encoding() {
    let mut request = Request::new("SELECT @value", |_| {});
    request
        .add_typed_parameter(
            "value",
            SqlParameterType::VarChar,
            Value::Text("Ж".to_owned()),
            ParameterOptions {
                length: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
    request
        .validate_parameters(Some(&tedious_rs::collation::Collation::new(
            0x0409, 0, 0, 0,
        )))
        .unwrap();
    assert_eq!(request.parameters[0].encoded.data, vec![b'?']);
}

#[test]
fn fallback_encoding_does_not_fail_request_validation() {
    let callback_result = std::sync::Arc::new(std::sync::Mutex::new(None));
    let callback_result_seen = std::sync::Arc::clone(&callback_result);
    let mut request = Request::new("SELECT @value", move |result| {
        *callback_result_seen.lock().unwrap() = Some(result.is_err());
    });
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let events_seen = std::sync::Arc::clone(&events);
    request.on(move |event| {
        events_seen.lock().unwrap().push(event.clone());
    });
    request
        .add_typed_parameter(
            "value",
            SqlParameterType::VarChar,
            Value::Text("Ж".to_owned()),
            ParameterOptions {
                length: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
    request
        .validate_parameters(Some(&tedious_rs::collation::Collation::new(
            0x0409, 0, 0, 0,
        )))
        .unwrap();
    assert_eq!(*callback_result.lock().unwrap(), None);
    assert_eq!(request.error, None);
    assert_eq!(request.state, RequestState::Created);
    assert!(events.lock().unwrap().is_empty());
}

#[test]
fn resolves_non_unicode_default_length_from_encoded_collation_bytes() {
    let mut request = Request::new("SELECT @value", |_| {});
    request
        .add_typed_parameter(
            "value",
            SqlParameterType::VarChar,
            Value::Text("한글".to_owned()),
            ParameterOptions {
                collation: Some(tedious_rs::collation::Collation::new(0x0412, 0, 0, 0)),
                ..Default::default()
            },
        )
        .unwrap();
    request
        .validate_parameters(Some(&tedious_rs::collation::Collation::new(
            0x0412, 0, 0, 0,
        )))
        .unwrap();
}

#[test]
fn copies_effective_collation_into_unicode_parameter_type_info() {
    let mut collation = tedious_rs::collation::Collation::new(0x0412, 0, 0, 0);
    let mut request = Request::new("SELECT @value", |_| {});
    request
        .add_typed_parameter(
            "value",
            SqlParameterType::NVarChar,
            Value::Text("한글".to_owned()),
            ParameterOptions {
                length: Some(2),
                collation: Some(collation.clone()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        &request.parameters[0].encoded.type_info[3..8],
        &collation.to_buffer()
    );
}

#[test]
fn typed_parameters_can_be_reencoded_with_server_collation() {
    let mut request = Request::new("SELECT @value", |_| {});
    request
        .add_typed_parameter(
            "value",
            SqlParameterType::VarChar,
            Value::Text("A".to_owned()),
            ParameterOptions {
                length: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
    let original_type_info = request.parameters[0].encoded.type_info.clone();
    request
        .reencode_typed_parameters(Some(&tedious_rs::collation::Collation::new(
            0x0419, 0, 0, 0,
        )))
        .unwrap();
    assert_eq!(request.parameters[0].encoded.data, vec![b'A']);
    assert_ne!(request.parameters[0].encoded.type_info, original_type_info);
}

#[test]
fn non_unicode_text_can_wait_for_server_collation_before_encoding() {
    let mut request = Request::new("SELECT @value", |_| {});
    request
        .add_typed_parameter(
            "value",
            SqlParameterType::VarChar,
            Value::Text("Ж".to_owned()),
            ParameterOptions {
                length: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
    request
        .reencode_typed_parameters(Some(&tedious_rs::collation::Collation::new(
            0x0419, 0, 0, 0,
        )))
        .unwrap();
    assert_eq!(request.parameters[0].encoded.data, vec![0xC6]);
}

#[test]
fn add_parameter_honors_output_option_like_node() {
    let mut request = Request::new("SELECT @value", |_| {});
    request
        .add_parameter(
            "value",
            tedious_rs::data_type::type_by_name("Int").unwrap(),
            Value::Integer(7),
            ParameterOptions {
                output: true,
                ..ParameterOptions::default()
            },
        )
        .unwrap();
    assert!(request.parameters[0].output);
}

#[test]
fn add_parameter_defers_collation_dependent_encoding_until_validation() {
    let mut request = Request::new("SELECT @value", |_| {});
    request
        .add_parameter(
            "value",
            tedious_rs::data_type::type_by_name("VarChar").unwrap(),
            Value::Text("Ж".to_owned()),
            ParameterOptions::default(),
        )
        .unwrap();
    assert!(request.parameters[0].encoded.type_info.is_empty());
    assert!(request.parameters[0].encoded.length.is_empty());
    assert!(request.parameters[0].encoded.data.is_empty());
    request
        .validate_parameters(Some(&tedious_rs::collation::Collation::new(
            0x0419, 0, 0, 0,
        )))
        .unwrap();
    assert_eq!(request.parameters[0].encoded.data, vec![0xC6]);
    assert_eq!(
        request.make_params_parameter().unwrap(),
        "@value varchar(1)"
    );
}

#[test]
fn collation_dependent_validation_rejects_missing_server_collation() {
    let mut request = Request::new("SELECT @value", |_| {});
    request
        .add_parameter(
            "value",
            tedious_rs::data_type::type_by_name("VarChar").unwrap(),
            Value::Text("value".to_owned()),
            ParameterOptions::default(),
        )
        .unwrap();
    let error = request.validate_parameters(None).unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EPARAM"));
    assert!(error.message.contains("No collation was set by the server"));
}

#[test]
fn add_parameter_defers_value_validation_until_execution_validation() {
    let mut request = Request::new("SELECT @value", |_| {});
    assert!(
        request
            .add_parameter(
                "value",
                tedious_rs::data_type::type_by_name("Int").unwrap(),
                Value::Text("not an integer".to_owned()),
                ParameterOptions::default(),
            )
            .is_ok()
    );
    assert!(request.validate_parameters(None).is_err());
}
