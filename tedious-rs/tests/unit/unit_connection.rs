use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use std::sync::{Arc, Mutex};
use tedious_rs::connection::{
    Authentication, Connection, ConnectionConfig, ConnectionEvent, ConnectionState, TokenProvider,
};
use tedious_rs::transaction::IsolationLevel;

#[test]
fn builds_sql_authentication_configuration_with_node_defaults() {
    let config = ConnectionConfig::sql("localhost", "sa", "secret");
    assert_eq!(config.port, 1433);
    assert_eq!(config.packet_size, 4096);
    assert_eq!(config.tds_version, "7_4");
    assert_eq!(config.connect_timeout, Duration::from_secs(15));
    assert!(config.encrypt);
    assert!(!config.read_only);
    assert!(!config.read_only_intent);
    assert!(!config.column_encryption_setting);
    assert_eq!(
        config.column_encryption_key_cache_ttl,
        Duration::from_secs(7200)
    );
    assert!(!config.debug.packet);
    assert!(!config.debug.data);
    assert!(!config.debug.payload);
    assert!(!config.debug.token);
    assert_eq!(
        config.authentication,
        Authentication::Sql {
            user_name: "sa".to_owned(),
            password: "secret".to_owned()
        }
    );
}

#[test]
fn exposes_token_credential_authentication_as_a_functional_mode() {
    let config = ConnectionConfig::azure_active_directory_token_credential("localhost");
    assert!(matches!(
        config.authentication,
        Authentication::AzureActiveDirectoryTokenCredential
    ));
}

#[tokio::test]
async fn token_credential_mode_requires_an_injected_credential_before_transport() {
    let mut connection = Connection::new(
        ConnectionConfig::azure_active_directory_token_credential("localhost"),
    );
    let error = connection.connect().await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EAUTH"));
    assert_eq!(connection.state, ConnectionState::Final);
}

#[test]
fn separates_read_only_authorization_from_read_only_intent() {
    let mut config = ConnectionConfig::sql("localhost", "sa", "secret");
    config.read_only_intent = true;
    assert!(!config.read_only);
    assert!(config.read_only_intent);
}

#[tokio::test]
async fn enabled_column_encryption_requires_a_key_store_provider_before_execution() {
    let mut config = ConnectionConfig::sql("localhost", "sa", "secret");
    config.column_encryption_setting = true;
    let mut connection = Connection::new(config);
    connection.state = ConnectionState::LoggedIn;
    let mut request = tedious_rs::Request::new("SELECT @value", |_| {});
    request
        .add_typed_parameter(
            "value",
            tedious_rs::parameter::SqlParameterType::Int,
            tedious_rs::value_codec::Value::Integer(1),
            Default::default(),
        )
        .unwrap();
    let error = connection.exec_sql(&mut request).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EAE_CONFIGURATION"));
    assert_eq!(request.state, tedious_rs::RequestState::Failed);
}

#[test]
fn applies_connection_isolation_level_independently_from_transaction_default() {
    let mut config = ConnectionConfig::sql("localhost", "sa", "secret");
    config.connection_isolation_level = Some(IsolationLevel::Serializable);
    config.session.isolation_level = Some(IsolationLevel::ReadUncommitted);
    let connection = Connection::new(config);
    let sql = connection.get_initial_sql();
    assert!(sql.ends_with("set transaction isolation level serializable\nset xact_abort off"));
    assert!(!sql.contains("set transaction isolation level read uncommitted"));
}

#[test]
fn exposes_initial_sql_and_state_transition_contracts() {
    let mut config = ConnectionConfig::sql("localhost", "sa", "secret");
    config.session.enable_ansi_null = Some(false);
    let mut connection = Connection::new(config);
    assert_eq!(
        connection.get_initial_sql(),
        "set ansi_nulls off\nset ansi_null_dflt_on on\nset ansi_padding on\nset ansi_warnings on\nset arithabort on\nset concat_null_yields_null on\nset datefirst 7\nset dateformat mdy\nset implicit_transactions off\nset language us_english\nset numeric_roundabort off\nset quoted_identifier on\nset textsize 2147483647\nset transaction isolation level read committed\nset xact_abort off"
    );
    connection.transition_to(ConnectionState::Connecting);
    assert_eq!(connection.state, ConnectionState::Connecting);
}

#[test]
fn strict_tls_configuration_selects_tds8_without_collapsing_to_starttls() {
    let config = ConnectionConfig::sql_strict("localhost", "sa", "secret");
    assert!(config.strict_tls);
    assert!(config.encrypt);
    assert_eq!(config.tds_version, "8_0");
}

#[tokio::test]
async fn strict_tls_rejects_a_non_tds8_configuration_before_network_io() {
    let mut config = ConnectionConfig::sql("localhost", "sa", "secret");
    config.strict_tls = true;
    let mut connection = Connection::new(config);
    let error = connection.connect().await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ETDSVERSION"));
}

#[tokio::test]
async fn custom_connector_is_used_before_the_default_connector_path() {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let accepted = Arc::new(Mutex::new(false));
    let accepted_seen = Arc::clone(&accepted);
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        *accepted_seen.lock().unwrap() = true;
        drop(socket);
    });
    let called = Arc::new(Mutex::new(false));
    let called_seen = Arc::clone(&called);
    let mut config = ConnectionConfig::sql("invalid-host-for-custom-connector", "sa", "secret");
    config.max_retries_on_transient_errors = 0;
    config.connect_timeout = Duration::from_millis(100);
    let mut connection = Connection::new(config);
    connection.set_custom_connector(move || {
        let called_seen = Arc::clone(&called_seen);
        async move {
            *called_seen.lock().unwrap() = true;
            tokio::net::TcpStream::connect(address).await
        }
    });
    assert!(connection.connect().await.is_err());
    server.await.unwrap();
    assert!(*called.lock().unwrap());
    assert!(*accepted.lock().unwrap());
}

#[tokio::test]
async fn starts_in_initialized_state_and_rejects_queries_before_connecting() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    assert_eq!(connection.state, ConnectionState::Initialized);
    let error = connection.exec_sql_batch("SELECT 1").await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    let error = connection.reset().await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
}

#[test]
fn preserves_read_only_and_ntlm_configuration_without_falling_back_to_sql_auth() {
    let mut config = ConnectionConfig::sql("localhost", "user", "password");
    config.read_only = true;
    config.authentication = Authentication::Ntlm {
        user_name: "user".to_owned(),
        password: "password".to_owned(),
        domain: None,
    };
    let connection = Connection::new(config.clone());
    assert!(config.read_only);
    assert!(matches!(
        connection.config.authentication,
        Authentication::Ntlm { domain: None, .. }
    ));
}

#[test]
fn exposes_transaction_helpers_with_node_values() {
    let connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    assert_eq!(connection.current_transaction_descriptor(), &[0; 8]);
    assert_eq!(
        Connection::isolation_level_text(IsolationLevel::ReadCommitted),
        "read committed"
    );
    assert_eq!(
        Connection::get_isolation_level_text(IsolationLevel::Snapshot),
        "snapshot"
    );
    assert_eq!(
        Connection::isolation_level_text(IsolationLevel::NoChange),
        "read committed"
    );
}

#[test]
fn preserves_node_retry_defaults_and_allows_explicit_retry_policy() {
    let mut config = ConnectionConfig::sql("localhost", "user", "password");
    assert_eq!(config.connection_retry_interval, Duration::from_millis(500));
    assert_eq!(config.max_retries_on_transient_errors, 3);
    config.connection_retry_interval = Duration::from_millis(1);
    config.max_retries_on_transient_errors = 0;
    assert_eq!(config.connection_retry_interval, Duration::from_millis(1));
    assert_eq!(config.max_retries_on_transient_errors, 0);
    assert_eq!(config.request_timeout, Duration::from_secs(15));
    assert_eq!(config.cancel_timeout, Duration::from_secs(5));
    assert_eq!(config.keep_alive_initial_delay, Duration::from_secs(30));
    assert_eq!(config.local_address, None);
    assert_eq!(config.trusted_server_name_ae, None);
}

#[test]
fn builds_node_compatible_initial_session_sql_in_stable_order() {
    let sql = tedious_rs::connection::SessionOptions::default().initial_sql();
    assert_eq!(
        sql,
        "set ansi_nulls on\nset ansi_null_dflt_on on\nset ansi_padding on\nset ansi_warnings on\nset arithabort on\nset concat_null_yields_null on\nset datefirst 7\nset dateformat mdy\nset implicit_transactions off\nset language us_english\nset numeric_roundabort off\nset quoted_identifier on\nset textsize 2147483647\nset transaction isolation level read committed\nset xact_abort off"
    );
}

#[test]
fn connection_accepts_a_column_name_replacer_adapter() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    connection.set_column_name_replacer(|name, index, _| format!("{index}_{name}"));
}

#[tokio::test]
async fn rejects_invalid_connection_options_before_opening_transport() {
    let mut config = ConnectionConfig::sql("localhost", "sa", "secret");
    config.tds_version = "7_9".to_owned();
    let mut connection = Connection::new(config);
    assert_eq!(
        connection.connect().await.unwrap_err().code.as_deref(),
        Some("ETDSVERSION")
    );

    let mut config = ConnectionConfig::sql("localhost", "sa", "secret");
    config.connection_retry_interval = Duration::ZERO;
    let mut connection = Connection::new(config);
    assert_eq!(
        connection.connect().await.unwrap_err().code.as_deref(),
        Some("ERETRYINTERVAL")
    );

    let mut config = ConnectionConfig::sql("localhost", "sa", "secret");
    config.session.datefirst = Some(8);
    let mut connection = Connection::new(config);
    assert_eq!(
        connection.connect().await.unwrap_err().code.as_deref(),
        Some("EDATEFIRST")
    );
}

#[test]
fn preserves_federated_access_token_authentication() {
    let mut config = ConnectionConfig::sql("localhost", "ignored", "ignored");
    config.authentication = Authentication::FedAuth {
        token: "access-token".to_owned(),
    };
    let connection = Connection::new(config);
    assert!(matches!(
        connection.config.authentication,
        Authentication::FedAuth { token } if token == "access-token"
    ));
}

#[test]
fn builds_node_named_azure_access_token_authentication() {
    let config = ConnectionConfig::azure_active_directory_access_token("localhost", "token");
    assert_eq!(
        config.authentication,
        Authentication::AzureActiveDirectoryAccessToken {
            token: "token".to_owned()
        }
    );
}

#[test]
fn encodes_fed_auth_token_message_with_node_framing() {
    let data = tedious_rs::encode_fed_auth_token_payload("A😀").unwrap();
    assert_eq!(&data[0..4], &10_u32.to_le_bytes());
    assert_eq!(&data[4..8], &6_u32.to_le_bytes());
    assert_eq!(&data[8..], &[65, 0, 0x3D, 0xD8, 0x00, 0xDE]);
}

#[test]
fn builds_fed_auth_scope_from_the_spn_origin() {
    assert_eq!(
        tedious_rs::fed_auth_scope("https://database.windows.net/server?x=1").unwrap(),
        "https://database.windows.net/.default"
    );
    assert!(tedious_rs::fed_auth_scope("not a url").is_err());
}

#[test]
fn applies_the_tds_version_negotiated_by_login_ack() {
    let version = tedious_rs::connection::negotiated_tds_version(&[
        tedious_rs::token::parser::Token::LoginAck(tedious_rs::token::parser::LoginAckToken {
            interface: Some("SQL_TSQL".to_owned()),
            tds_version: Some("7_1".to_owned()),
            prog_name: "server".to_owned(),
            prog_version: [1, 0, 0, 0],
        }),
    ])
    .unwrap();
    assert_eq!(version, "7_1");
}

#[test]
fn rejects_unknown_or_unsupported_login_ack_values() {
    let unknown = tedious_rs::connection::negotiated_tds_version(&[
        tedious_rs::token::parser::Token::LoginAck(tedious_rs::token::parser::LoginAckToken {
            interface: Some("SQL_TSQL".to_owned()),
            tds_version: None,
            prog_name: "server".to_owned(),
            prog_version: [1, 0, 0, 0],
        }),
    ])
    .unwrap_err();
    assert_eq!(unknown.code.as_deref(), Some("ETDS"));

    let unsupported = tedious_rs::connection::negotiated_tds_version(&[
        tedious_rs::token::parser::Token::LoginAck(tedious_rs::token::parser::LoginAckToken {
            interface: None,
            tds_version: Some("7_4".to_owned()),
            prog_name: "server".to_owned(),
            prog_version: [1, 0, 0, 0],
        }),
    ])
    .unwrap_err();
    assert_eq!(unsupported.code.as_deref(), Some("EINTERFACENOTSUPP"));
}

#[test]
fn builds_read_only_sql_and_ntlm_configurations_without_a_domain() {
    let sql = ConnectionConfig::sql_read_only("localhost", "sa", "secret");
    assert!(sql.read_only);
    let ntlm = ConnectionConfig::ntlm("localhost", "user", "secret", None);
    assert!(ntlm.read_only);
    assert!(matches!(
        ntlm.authentication,
        Authentication::Ntlm { domain: None, .. }
    ));
}

#[test]
fn accepts_token_credential_when_server_requires_federated_authentication() {
    let config = ConnectionConfig::azure_active_directory_token_credential("localhost");
    assert!(tedious_rs::supports_required_fed_auth(
        &config.authentication
    ));
    assert!(tedious_rs::supports_required_fed_auth(
        &Authentication::AzureActiveDirectoryAccessToken {
            token: "token".to_owned()
        }
    ));
    assert!(!tedious_rs::supports_required_fed_auth(
        &Authentication::Sql {
            user_name: "sa".to_owned(),
            password: "secret".to_owned(),
        }
    ));
}

#[test]
fn validates_feature_acknowledgement_for_federated_and_standard_login() {
    use tedious_rs::token::parser::{FeatureExtAckToken, Token};

    let federated = [Token::FeatureExtAck(FeatureExtAckToken {
        fed_auth: Some(Vec::new()),
        utf8_support: None,
    })];
    assert!(tedious_rs::validate_feature_ext_ack(&federated, true).is_ok());

    let missing = [Token::FeatureExtAck(FeatureExtAckToken {
        fed_auth: None,
        utf8_support: None,
    })];
    assert_eq!(
        tedious_rs::validate_feature_ext_ack(&missing, true)
            .unwrap_err()
            .code
            .as_deref(),
        Some("ELOGIN")
    );

    let unexpected = [Token::FeatureExtAck(FeatureExtAckToken {
        fed_auth: Some(Vec::new()),
        utf8_support: Some(true),
    })];
    assert!(tedious_rs::validate_feature_ext_ack(&unexpected, false).is_err());
}

#[test]
fn builds_all_node_azure_authentication_modes() {
    let password = ConnectionConfig::azure_active_directory_password(
        "localhost",
        "client",
        Some("user".to_owned()),
        Some("password".to_owned()),
        Some("tenant".to_owned()),
    );
    assert!(matches!(
        password.authentication,
        Authentication::AzureActiveDirectoryPassword {
            client_id,
            user_name: Some(_),
            password: Some(_),
            tenant_id: Some(_)
        } if client_id == "client"
    ));

    let vm = ConnectionConfig::azure_active_directory_msi_vm("localhost", None);
    assert!(matches!(
        vm.authentication,
        Authentication::AzureActiveDirectoryMsiVm { client_id: None }
    ));
    let app_service = ConnectionConfig::azure_active_directory_msi_app_service(
        "localhost",
        Some("client".to_owned()),
    );
    assert!(matches!(
        app_service.authentication,
        Authentication::AzureActiveDirectoryMsiAppService { client_id: Some(_) }
    ));
    let secret = ConnectionConfig::azure_active_directory_service_principal_secret(
        "localhost",
        "client",
        "secret",
        "tenant",
    );
    assert!(matches!(
        secret.authentication,
        Authentication::AzureActiveDirectoryServicePrincipalSecret { .. }
    ));
    let default =
        ConnectionConfig::azure_active_directory_default("localhost", Some("client".to_owned()));
    assert!(matches!(
        default.authentication,
        Authentication::AzureActiveDirectoryDefault { client_id: Some(_) }
    ));
}

#[tokio::test]
async fn azure_password_auth_rejects_missing_required_options_before_network_io() {
    let config = ConnectionConfig::azure_active_directory_password(
        "localhost",
        "client",
        None,
        Some("password".to_owned()),
        Some("tenant".to_owned()),
    );
    let mut connection = Connection::new(config);
    let error = connection.connect().await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EAUTH"));
    assert!(error.message.contains("user_name"));
}

#[tokio::test]
async fn read_only_connection_rejects_non_select_before_network_io() {
    let mut config = ConnectionConfig::sql("localhost", "user", "password");
    config.read_only = true;
    let mut connection = Connection::new(config);
    let error = connection
        .exec_sql_batch("UPDATE dbo.items SET value = 1")
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EREADONLY"));
}

#[tokio::test]
async fn read_only_request_apis_reject_mutating_operations_before_network_io() {
    let mut config = ConnectionConfig::sql("localhost", "user", "password");
    config.read_only = true;
    let mut connection = Connection::new(config);
    let mut request = tedious_rs::request::Request::new("UPDATE dbo.items SET value = 1", |_| {});
    let error = connection.exec_sql(&mut request).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EREADONLY"));
    assert_eq!(request.state, tedious_rs::request::RequestState::Failed);

    let mut procedure = tedious_rs::request::Request::new("dbo.write_item", |_| {});
    let error = connection.call_procedure(&mut procedure).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EREADONLY"));
    assert_eq!(procedure.state, tedious_rs::request::RequestState::Failed);
}

#[tokio::test]
async fn read_only_connection_rejects_bulk_and_transaction_operations() {
    let mut config = ConnectionConfig::sql("localhost", "user", "password");
    config.read_only = true;
    let mut connection = Connection::new(config);
    let bulk = tedious_rs::payloads::BulkLoadPayload {
        columns: Vec::new(),
        rows: Vec::new(),
        tds_version: "7_4".to_owned(),
    };
    assert_eq!(
        connection
            .exec_bulk_load(&bulk)
            .await
            .unwrap_err()
            .code
            .as_deref(),
        Some("EREADONLY")
    );
    assert_eq!(
        connection
            .begin_transaction("tx", IsolationLevel::ReadCommitted)
            .await
            .unwrap_err()
            .code
            .as_deref(),
        Some("EREADONLY")
    );
}

#[tokio::test]
async fn read_only_prepared_select_is_allowed_before_connection_validation() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "user", "password"));
    connection.config.read_only = true;
    let mut request = tedious_rs::request::Request::new("SELECT @id", |_| {});
    request
        .add_typed_parameter(
            "id",
            tedious_rs::parameter::SqlParameterType::Int,
            tedious_rs::value_codec::Value::Integer(1),
            tedious_rs::parameter::ParameterOptions::default(),
        )
        .unwrap();
    let error = connection.prepare(&mut request).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));

    request.handle = Some(12);
    let error = connection.execute_prepared(&mut request).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
}

#[tokio::test]
async fn read_only_prepared_mutation_is_rejected_before_connection_validation() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "user", "password"));
    connection.config.read_only = true;
    let mut request = tedious_rs::request::Request::new("UPDATE dbo.items SET id = 2", |_| {});
    let error = connection.prepare(&mut request).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EREADONLY"));
}

#[tokio::test]
async fn prepared_statement_operations_validate_connection_and_handle_state() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    let mut request = tedious_rs::request::Request::new("SELECT @id", |_| {});
    let error = connection.prepare(&mut request).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    let error = connection.execute_prepared(&mut request).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    let error = connection.unprepare(&mut request).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    let error = connection.execute(&mut request, None).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
}

#[tokio::test]
async fn unprepare_reuses_completed_request_callback_on_connection_failure() {
    let callback_results = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&callback_results);
    let mut request = tedious_rs::Request::new("SELECT 1", move |result| {
        observed
            .lock()
            .unwrap()
            .push(result.err().and_then(|error| error.code));
    });
    request.handle = Some(17);
    request.begin().unwrap();
    request.finish();
    callback_results.lock().unwrap().clear();
    let completed_events = Arc::new(Mutex::new(0));
    let completed_events_seen = Arc::clone(&completed_events);
    request.on_named("requestCompleted", move |_| {
        *completed_events_seen.lock().unwrap() += 1;
    });

    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    let error = connection.unprepare(&mut request).await.unwrap_err();

    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    assert_eq!(
        callback_results.lock().unwrap().as_slice(),
        &[Some("ENOTCONNECTED".to_owned())]
    );
    assert_eq!(request.state, tedious_rs::RequestState::Failed);
    assert_eq!(*completed_events.lock().unwrap(), 1);
    assert_eq!(request.handle, Some(17));
}

#[tokio::test]
async fn prepare_reports_not_connected_through_request_error_event() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    let mut request = tedious_rs::Request::new("SELECT 1", |_| {});
    let seen = Arc::clone(&observed);
    request.on_named("error", move |event| {
        if let tedious_rs::RequestEvent::Error(error) = event {
            seen.lock().unwrap().push(error.code.clone());
        }
    });
    let error = connection.prepare(&mut request).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    assert_eq!(
        *observed.lock().unwrap(),
        vec![Some("ENOTCONNECTED".to_owned())]
    );
    assert!(!request.preparing);
    assert_eq!(request.state, tedious_rs::RequestState::Created);
}

#[tokio::test]
async fn retrying_prepare_reemits_prepare_error_and_clears_preparing_state() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    let mut request = tedious_rs::Request::new("SELECT 1", |_| {});
    let seen = Arc::clone(&observed);
    request.on_named("error", move |event| {
        if let tedious_rs::RequestEvent::Error(error) = event {
            seen.lock().unwrap().push(error.code.clone());
        }
    });

    let first_error = connection.prepare(&mut request).await.unwrap_err();
    let second_error = connection.prepare(&mut request).await.unwrap_err();

    assert_eq!(first_error.code.as_deref(), Some("ENOTCONNECTED"));
    assert_eq!(second_error.code.as_deref(), Some("ENOTCONNECTED"));
    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            Some("ENOTCONNECTED".to_owned()),
            Some("ENOTCONNECTED".to_owned())
        ]
    );
    assert!(request.error.is_none());
    assert!(!request.preparing);
    assert_eq!(request.state, tedious_rs::RequestState::Created);
}

#[tokio::test]
async fn execute_replaces_missing_prepared_values_with_null_like_node() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    let mut request = tedious_rs::request::Request::new("SELECT @id", |_| {});
    request
        .add_typed_parameter(
            "id",
            tedious_rs::parameter::SqlParameterType::Int,
            tedious_rs::value_codec::Value::Integer(7),
            tedious_rs::parameter::ParameterOptions::default(),
        )
        .unwrap();
    request.handle = Some(42);
    let error = connection.execute(&mut request, None).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    assert_eq!(request.parameters[0].encoded.length, vec![0]);
    assert!(request.parameters[0].encoded.data.is_empty());
}

#[tokio::test]
async fn execute_prepared_with_values_updates_parameters_added_through_public_api() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    let mut request = tedious_rs::request::Request::new("SELECT @id", |_| {});
    request
        .add_parameter_value(
            "id",
            tedious_rs::data_type::type_by_name("Int").unwrap(),
            tedious_rs::value_codec::Value::Integer(7),
            Default::default(),
        )
        .unwrap();
    request.handle = Some(42);

    let error = connection
        .execute_prepared_with_values(
            &mut request,
            &[("id".to_owned(), tedious_rs::value_codec::Value::Integer(9))],
        )
        .await
        .unwrap_err();

    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    assert_eq!(request.parameters[0].encoded.data, 9i32.to_le_bytes());
}

#[tokio::test]
async fn sql_batch_request_api_reuses_request_lifecycle_contract() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    let mut request = tedious_rs::request::Request::new("SELECT 1", |_| {});
    let error = connection
        .exec_sql_batch_request(&mut request)
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    assert_eq!(request.state, tedious_rs::request::RequestState::Failed);
}

#[tokio::test]
async fn connection_request_scheduler_lease_is_async() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    let scheduler = connection.request_scheduler();
    let _lease = scheduler.acquire().await.unwrap();
    drop(_lease);
    let mut request = tedious_rs::request::Request::new("SELECT 1", |_| {});
    let error = connection.exec_sql(&mut request).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    assert_eq!(request.state, tedious_rs::request::RequestState::Failed);
}

#[tokio::test]
async fn transaction_operations_require_logged_in_connection_and_keep_empty_descriptor() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    let error = connection
        .begin_transaction("tx", IsolationLevel::ReadCommitted)
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    assert_eq!(connection.transaction_descriptor, vec![0; 8]);
    assert_eq!(connection.transaction_depth, 0);
}

#[tokio::test]
async fn transaction_helper_does_not_invoke_operation_when_begin_fails() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    let called = Arc::new(Mutex::new(false));
    let seen = Arc::clone(&called);
    let error = connection
        .transaction("tx", IsolationLevel::ReadCommitted, move |_| async move {
            *seen.lock().unwrap() = true;
            Ok::<_, tedious_rs::errors::ConnectionError>(())
        })
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    assert!(!*called.lock().unwrap());
}

#[tokio::test]
async fn pre_72_transaction_operations_use_sql_batch_fallback() {
    let mut config = ConnectionConfig::sql("localhost", "sa", "secret");
    config.tds_version = "7_1".to_owned();
    let mut connection = Connection::new(config);
    assert_eq!(
        connection
            .begin_transaction("tx", IsolationLevel::ReadCommitted)
            .await
            .unwrap_err()
            .code
            .as_deref(),
        Some("ENOTCONNECTED")
    );
    assert_eq!(
        connection
            .commit_transaction("tx")
            .await
            .unwrap_err()
            .code
            .as_deref(),
        Some("ENOTCONNECTED")
    );
    assert_eq!(
        connection
            .rollback_transaction("tx")
            .await
            .unwrap_err()
            .code
            .as_deref(),
        Some("ENOTCONNECTED")
    );
    assert_eq!(
        connection
            .save_transaction("savepoint")
            .await
            .unwrap_err()
            .code
            .as_deref(),
        Some("ENOTCONNECTED")
    );
}

#[test]
fn cancel_without_an_active_request_returns_false() {
    let connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    assert!(!connection.cancel());
}

#[test]
fn cancel_request_without_active_request_returns_false() {
    let connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    assert!(!connection.cancel_request());
}

#[test]
fn cancel_in_sent_client_request_without_registered_request_returns_false() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    connection.transition_to(ConnectionState::SentClientRequest);
    assert!(!connection.cancel());
}

#[test]
fn cancel_on_an_idle_authenticated_connection_returns_false() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    connection.transition_to(ConnectionState::LoggedIn);
    assert!(!connection.cancel());
}

#[test]
fn request_execution_state_is_available_for_attention_cancellation() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    connection.transition_to(ConnectionState::LoggedIn);
    connection.transition_to(ConnectionState::SentClientRequest);
    assert_eq!(connection.state, ConnectionState::SentClientRequest);
}

#[tokio::test]
async fn close_emits_end_after_transitioning_to_final() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    connection.on(move |event| seen.lock().unwrap().push(event.clone()));
    connection.close().await;
    assert_eq!(connection.state, ConnectionState::Final);
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| matches!(event, ConnectionEvent::End))
            .count(),
        1
    );
}

#[tokio::test]
async fn close_is_idempotent_and_emits_end_once() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    connection.on(move |event| seen.lock().unwrap().push(event.clone()));
    connection.close().await;
    connection.close().await;
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| matches!(event, ConnectionEvent::End))
            .count(),
        1
    );
}

#[tokio::test]
async fn request_started_after_close_completes_with_eclose_once() {
    let callbacks = Arc::new(Mutex::new(0));
    let observed = Arc::clone(&callbacks);
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    connection.close().await;
    let mut request = tedious_rs::Request::new("SELECT 1", move |result| {
        assert_eq!(result.unwrap_err().code.as_deref(), Some("ECLOSE"));
        *observed.lock().unwrap() += 1;
    });

    let error = connection
        .exec_sql_batch_request(&mut request)
        .await
        .unwrap_err();

    assert_eq!(error.code.as_deref(), Some("ECLOSE"));
    assert_eq!(*callbacks.lock().unwrap(), 1);
    assert_eq!(request.state, tedious_rs::RequestState::Failed);
}

#[tokio::test]
async fn failed_connection_emits_connect_error_before_end() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    let mut connection = Connection::new(ConnectionConfig::sql("127.0.0.1", "sa", "secret"));
    connection.config.port = 1;
    connection.config.connect_timeout = Duration::from_millis(20);
    connection.on(move |event| seen.lock().unwrap().push(event.clone()));
    assert!(connection.connect().await.is_err());
    let events = events.lock().unwrap();
    let terminal_events = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                ConnectionEvent::ConnectError(_) | ConnectionEvent::End
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(terminal_events.len(), 2);
    assert!(matches!(
        terminal_events[0],
        ConnectionEvent::ConnectError(_)
    ));
    assert_eq!(*terminal_events[1], ConnectionEvent::End);
}

#[tokio::test]
async fn connect_rejects_reentry_after_a_terminal_failure() {
    let mut connection = Connection::new(ConnectionConfig::sql("127.0.0.1", "sa", "secret"));
    connection.config.port = 1;
    connection.config.connect_timeout = Duration::from_millis(20);
    assert!(connection.connect().await.is_err());
    assert_eq!(connection.state, ConnectionState::Final);
    let error = connection.connect().await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EINVALIDSTATE"));
}

#[tokio::test]
async fn connect_callback_receives_the_same_failure_once() {
    let callback_error = Arc::new(Mutex::new(None));
    let seen = Arc::clone(&callback_error);
    let mut connection = Connection::new(ConnectionConfig::sql("127.0.0.1", "sa", "secret"));
    connection.config.port = 1;
    connection.config.connect_timeout = Duration::from_millis(20);
    let result = connection
        .connect_with_callback(move |error| *seen.lock().unwrap() = error)
        .await;
    assert!(result.is_err());
    assert_eq!(
        callback_error.lock().unwrap().as_ref(),
        result.as_ref().err()
    );
}

struct FixedTokenProvider {
    token: Result<String, tedious_rs::errors::ConnectionError>,
}

impl TokenProvider for FixedTokenProvider {
    fn get_token(
        &self,
    ) -> Pin<
        Box<dyn Future<Output = Result<String, tedious_rs::errors::ConnectionError>> + Send + '_>,
    > {
        let token = self.token.clone();
        Box::pin(async move { token })
    }
}

#[tokio::test]
async fn token_provider_refreshes_fed_auth_without_network_access() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    connection.set_token_provider(FixedTokenProvider {
        token: Ok("access-token".to_owned()),
    });
    connection.refresh_access_token().await.unwrap();
    assert_eq!(
        connection.config.authentication,
        Authentication::FedAuth {
            token: "access-token".to_owned()
        }
    );
}

#[tokio::test]
async fn empty_token_provider_result_is_rejected_with_eauth() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    connection.set_token_provider(FixedTokenProvider {
        token: Ok("   ".to_owned()),
    });
    let error = connection.refresh_access_token().await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EAUTH"));
}

#[test]
fn connection_defaults_expose_node_compatible_public_options() {
    let config = ConnectionConfig::sql("localhost", "sa", "secret");
    assert_eq!(config.app_name, "Tedious");
    assert!(config.workstation_id.is_none());
    assert!(!config.camel_case_columns);
    assert!(!config.use_column_names);
    assert!(!config.lower_case_guids);
    assert!(config.use_utc);
    assert!(!config.row_collection_on_done);
    assert!(!config.row_collection_on_request_completion);
}

#[tokio::test]
async fn read_only_rejects_a_mutating_statement_after_a_select() {
    let mut config = ConnectionConfig::sql("localhost", "sa", "secret");
    config.read_only = true;
    let mut connection = Connection::new(config);
    let error = connection
        .exec_sql_batch("SELECT 1; UPDATE dbo.items SET value = 1")
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EREADONLY"));
}

#[tokio::test]
async fn read_only_rejects_select_into_and_mutating_keywords_inside_comments_or_literals_are_ignored()
 {
    let mut config = ConnectionConfig::sql("localhost", "sa", "secret");
    config.read_only = true;
    let mut connection = Connection::new(config);
    let error = connection
        .exec_sql_batch("SELECT 'UPDATE; DELETE' AS value -- INSERT")
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));

    let error = connection
        .exec_sql_batch("SELECT value INTO dbo.items_copy FROM dbo.items")
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EREADONLY"));
}
