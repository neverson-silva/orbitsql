use orbitsql_ui_lib::backend::adapters::runtime::map_database_session_row;
use orbitsql_ui_lib::backend::domain::session_manager::{
    DatabaseSession, annotate_blocking_graph, extract_current_statement,
    transaction_isolation_name, validate_kill_session_id,
};
use tedious_rs::value_codec::Value;

fn session(session_id: i32, blocking_session_id: Option<i32>) -> DatabaseSession {
    DatabaseSession {
        session_id,
        status: "sleeping".to_owned(),
        request_status: None,
        database: Some("tedious_rs".to_owned()),
        login: Some("integration".to_owned()),
        original_login: Some("integration".to_owned()),
        host: Some("localhost".to_owned()),
        application: Some("test".to_owned()),
        client_interface: Some("tedious-rs".to_owned()),
        client_address: Some("127.0.0.1".to_owned()),
        local_address: Some("127.0.0.1".to_owned()),
        local_port: Some(1433),
        login_time: Some("2026-10-06T00:00:00".to_owned()),
        command: None,
        duration_ms: "0".to_owned(),
        cpu_time_ms: 0,
        reads: "0".to_owned(),
        writes: "0".to_owned(),
        logical_reads: "0".to_owned(),
        row_count: "0".to_owned(),
        wait_type: None,
        wait_time_ms: 0,
        wait_resource: None,
        last_wait_type: None,
        blocking_session_id,
        blocker_session_id: None,
        root_blocker_session_id: None,
        is_blocking: false,
        is_root_blocker: false,
        blocked_session_count: 0,
        blocking_depth: 0,
        open_transaction_count: 0,
        transaction_isolation_level: 2,
        transaction_isolation_name: "Read committed".to_owned(),
        batch_text: None,
        current_statement: None,
        percent_complete: 0.0,
        estimated_completion_time_ms: "0".to_owned(),
    }
}

#[test]
fn current_statement_offsets_use_utf16_code_units_for_unicode_batches() {
    let prefix = "SELECT N'😀東京'; ";
    let statement = "SELECT N'🛰️';";
    let batch = format!("{prefix}{statement} SELECT 3");
    let start = (prefix.encode_utf16().count() * 2) as i64;
    let end = start + (statement.encode_utf16().count() * 2) as i64;

    assert_eq!(
        extract_current_statement(&batch, start, end).as_deref(),
        Some(statement)
    );
    assert_eq!(
        extract_current_statement(&batch, start, -1).as_deref(),
        Some("SELECT N'🛰️'; SELECT 3")
    );
}

#[test]
fn current_statement_offsets_reject_invalid_and_reversed_ranges() {
    assert_eq!(extract_current_statement("SELECT 1", -2, -1), None);
    assert_eq!(extract_current_statement("SELECT 1", 12, 8), None);
    assert_eq!(extract_current_statement("SELECT 1", 0, -2), None);
}

#[test]
fn blocking_graph_marks_blocked_chains_and_root_blockers() {
    let mut sessions = vec![
        session(101, None),
        session(102, Some(101)),
        session(103, Some(102)),
        session(104, Some(-2)),
        session(105, Some(105)),
    ];

    annotate_blocking_graph(&mut sessions);

    assert!(sessions[0].is_blocking);
    assert!(sessions[0].is_root_blocker);
    assert_eq!(sessions[0].blocked_session_count, 1);
    assert!(sessions[1].is_blocking);
    assert!(!sessions[1].is_root_blocker);
    assert_eq!(sessions[1].blocked_session_count, 1);
    assert_eq!(sessions[1].root_blocker_session_id, Some(101));
    assert_eq!(sessions[2].blocker_session_id, Some(102));
    assert_eq!(sessions[2].root_blocker_session_id, Some(101));
    assert_eq!(sessions[1].blocking_depth, 1);
    assert_eq!(sessions[2].blocking_depth, 2);
    assert_eq!(sessions[3].blocker_session_id, None);
    assert_eq!(sessions[4].blocker_session_id, None);
    assert_eq!(sessions[4].blocking_session_id, None);
}

#[test]
fn blocking_graph_handles_cycles_without_inventing_root_blockers() {
    let mut sessions = vec![session(201, Some(202)), session(202, Some(201))];

    annotate_blocking_graph(&mut sessions);

    assert!(!sessions[0].is_root_blocker);
    assert!(!sessions[1].is_root_blocker);
    assert!(
        sessions
            .iter()
            .all(|entry| entry.root_blocker_session_id.is_none())
    );
    assert!(sessions.iter().all(|entry| entry.blocking_depth <= 1));
}

#[test]
fn kill_session_rejects_nonpositive_and_monitor_session_ids() {
    assert_eq!(
        validate_kill_session_id(0, 71).unwrap_err().code,
        "SESSION_KILL_INVALID_ID"
    );
    assert_eq!(
        validate_kill_session_id(-1, 71).unwrap_err().code,
        "SESSION_KILL_INVALID_ID"
    );
    assert_eq!(
        validate_kill_session_id(71, 71).unwrap_err().code,
        "SESSION_KILL_SELF"
    );
    assert_eq!(validate_kill_session_id(72, 71), Ok(()));
}

#[test]
fn transaction_isolation_levels_keep_sql_server_names() {
    assert_eq!(transaction_isolation_name(0), "Unspecified");
    assert_eq!(transaction_isolation_name(1), "Read uncommitted");
    assert_eq!(transaction_isolation_name(2), "Read committed");
    assert_eq!(transaction_isolation_name(3), "Repeatable read");
    assert_eq!(transaction_isolation_name(4), "Serializable");
    assert_eq!(transaction_isolation_name(5), "Snapshot");
    assert_eq!(transaction_isolation_name(99), "Unknown");
}

#[test]
fn session_row_mapping_preserves_bigint_duration_and_unicode_statement() {
    let mut values = vec![Value::Null; 33];
    let prefix = "SELECT N'雪🛰️'; ";
    let statement = "SELECT N'東京😀'";
    let batch = format!("{prefix}{statement}");
    let start = (prefix.encode_utf16().count() * 2) as i64;
    let end = start + (statement.encode_utf16().count() * 2) as i64;
    values[1] = Value::Integer(45);
    values[2] = Value::Text("running".to_owned());
    values[4] = Value::Text("tedious_rs".to_owned());
    values[5] = Value::Text("integration".to_owned());
    values[15] = Value::BigInteger("86400000".to_owned());
    values[20] = Value::BigInteger("9007199254740993".to_owned());
    values[27] = Value::Integer(5);
    values[28] = Value::Text(batch);
    values[29] = Value::Integer(start);
    values[30] = Value::Integer(end);
    values[31] = Value::Float(12.5);
    values[32] = Value::BigInteger("4000".to_owned());
    let row = values.iter().collect::<Vec<_>>();

    let mapped = map_database_session_row(&row).expect("session id is required");

    assert_eq!(mapped.session_id, 45);
    assert_eq!(mapped.duration_ms, "86400000");
    assert_eq!(mapped.row_count, "9007199254740993");
    assert_eq!(mapped.current_statement.as_deref(), Some(statement));
    assert_eq!(mapped.transaction_isolation_name, "Snapshot");
    assert_eq!(mapped.percent_complete, 12.5);
    assert_eq!(mapped.estimated_completion_time_ms, "4000");
}
