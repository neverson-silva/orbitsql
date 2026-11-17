use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use tedious_rs::connection::{Connection, ConnectionConfig, ConnectionEvent, ConnectionState};

#[test]
fn transition_to_emits_node_state_change_names_with_debug_enabled() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&events);
    let mut config = ConnectionConfig::sql("localhost", "user", "");
    config.debug.token = true;
    let mut connection = Connection::new(config);
    connection.on_named("debug", move |event| {
        if let ConnectionEvent::Debug(message) = event {
            observed.lock().unwrap().push(message.clone());
        }
    });

    connection.transition_to(ConnectionState::Connecting);
    connection.transition_to(ConnectionState::Connecting);
    connection.transition_to(ConnectionState::Rerouting);
    connection.transition_to(ConnectionState::TransientFailureRetry);
    connection.transition_to(ConnectionState::SentLogin7WithNtlm);
    connection.transition_to(ConnectionState::SentLogin7WithFedAuth);
    connection.transition_to(ConnectionState::SentLogin7WithStandardLogin);
    connection.transition_to(ConnectionState::SentTlsSslNegotiation);
    connection.transition_to(ConnectionState::SentPrelogin);
    connection.transition_to(ConnectionState::LoggedInSendingInitialSql);
    connection.transition_to(ConnectionState::LoggedIn);
    connection.transition_to(ConnectionState::SentClientRequest);
    connection.transition_to(ConnectionState::SentAttention);
    connection.transition_to(ConnectionState::Final);

    assert_eq!(
        *events.lock().unwrap(),
        vec![
            "State change: Initialized -> Connecting",
            "State is already Connecting",
            "State change: Connecting -> ReRouting",
            "State change: ReRouting -> TRANSIENT_FAILURE_RETRY",
            "State change: TRANSIENT_FAILURE_RETRY -> SentLogin7WithNTLMLogin",
            "State change: SentLogin7WithNTLMLogin -> SentLogin7WithFedauth",
            "State change: SentLogin7WithFedauth -> SentLogin7WithStandardLogin",
            "State change: SentLogin7WithStandardLogin -> SentTLSSSLNegotiation",
            "State change: SentTLSSSLNegotiation -> SentPrelogin",
            "State change: SentPrelogin -> LoggedInSendingInitialSql",
            "State change: LoggedInSendingInitialSql -> LoggedIn",
            "State change: LoggedIn -> SentClientRequest",
            "State change: SentClientRequest -> SentAttention",
            "State change: SentAttention -> Final",
        ]
    );
}

#[test]
fn every_node_connection_state_has_the_same_rust_name_and_variant() {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../tedious-node/src/connection.ts"),
    )
    .expect("Node connection source must be readable");
    let state_start = source
        .find("Connection.prototype.STATE = {")
        .expect("Node state table must exist");
    let state_source = &source[state_start..];
    let mut current_state = None;
    let mut parsed_states = Vec::new();
    for line in state_source.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_suffix(": {")
            && !name.contains(' ')
            && !name.contains('}')
        {
            current_state = Some(name.to_owned());
        }
        if let Some(name) = trimmed
            .strip_prefix("name: '")
            .and_then(|value| value.strip_suffix("',"))
            && let Some(key) = current_state.take()
        {
            parsed_states.push((key, name.to_owned()));
        }
    }
    assert!(!parsed_states.is_empty());
    let rust_states = vec![
        ("INITIALIZED", ConnectionState::Initialized),
        ("CONNECTING", ConnectionState::Connecting),
        ("SENT_PRELOGIN", ConnectionState::SentPrelogin),
        ("REROUTING", ConnectionState::Rerouting),
        (
            "TRANSIENT_FAILURE_RETRY",
            ConnectionState::TransientFailureRetry,
        ),
        (
            "SENT_TLSSSLNEGOTIATION",
            ConnectionState::SentTlsSslNegotiation,
        ),
        (
            "SENT_LOGIN7_WITH_STANDARD_LOGIN",
            ConnectionState::SentLogin7WithStandardLogin,
        ),
        ("SENT_LOGIN7_WITH_NTLM", ConnectionState::SentLogin7WithNtlm),
        (
            "SENT_LOGIN7_WITH_FEDAUTH",
            ConnectionState::SentLogin7WithFedAuth,
        ),
        (
            "LOGGED_IN_SENDING_INITIAL_SQL",
            ConnectionState::LoggedInSendingInitialSql,
        ),
        ("LOGGED_IN", ConnectionState::LoggedIn),
        ("SENT_CLIENT_REQUEST", ConnectionState::SentClientRequest),
        ("SENT_ATTENTION", ConnectionState::SentAttention),
        ("FINAL", ConnectionState::Final),
    ];
    assert_eq!(parsed_states.len(), rust_states.len());
    for ((node_key, node_name), (rust_key, rust_state)) in parsed_states.iter().zip(rust_states) {
        assert_eq!(node_key, rust_key);
        let mut config = ConnectionConfig::sql("localhost", "user", "");
        config.debug.token = true;
        let mut connection = Connection::new(config);
        connection.transition_to(rust_state);
        assert_eq!(connection.state, rust_state);
        assert!(!node_name.is_empty());
    }
}
