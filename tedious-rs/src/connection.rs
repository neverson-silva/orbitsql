use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::always_encrypted::get_parameter_encryption_metadata::{
    CekMetadataRow, ParameterEncryptionMetadataRow, build_parameter_encryption_metadata,
};
use crate::always_encrypted::key_crypto::encrypt_encoded_parameter;
use crate::always_encrypted::symmetric_key_cache::KeyStoreProvider;
use crate::always_encrypted::types::SqlServerEncryptionType;
use crate::azure_auth::{
    AzureClientSecretTokenProvider, AzureDefaultTokenProvider, AzureManagedIdentityTokenProvider,
    AzureUsernamePasswordTokenProvider, TokenCredential, TokenCredentialAdapter,
};
use crate::bulk_load::{BulkLoad, BulkLoadCancellationHandle};
use crate::connector::{
    ConnectorError, connect_in_parallel_with_options, connect_in_sequence_with_options,
};
use crate::debug::DebugOptions;
use crate::errors::{ConnectionError, RequestError};
use crate::instance_lookup::lookup_instance;
use crate::login7_payload::FedAuth;
use crate::login7_payload::{Login7Options, Login7Payload};
use crate::message::{Message, message_type_name};
use crate::ntlm::{create_ntlm_request, create_ntlm_response};
use crate::packet::{HEADER_LENGTH, Packet, packet_type};
use crate::payloads::{BulkLoadPayload, BulkLoadRow, RpcRequestPayload, SqlBatchPayload};
use crate::prelogin_payload::{PreloginOptions, PreloginPayload, PreloginVersion};
use crate::public_api::EventEmitterError;
use crate::request::{Request, RequestCancellationHandle, RequestEvent};
use crate::request_scheduler::RequestScheduler;
use crate::tds_versions::versions;
use crate::token::handler::{InitialSqlTokenHandler, dispatch_stream_token};
use crate::token::parser::{Token, TokenParseError, parse_token};
use crate::token::rows::ColumnMetadata;
use crate::token::stream::{StreamParseError, StreamToken, TokenStreamParser};
use crate::transaction::{IsolationLevel, Transaction};
use crate::transient_error_lookup::is_transient_error;
use crate::transport::{
    IncomingMessageChunk, IncomingMessageChunkResult, MessageIo, TdsTlsStream, TransportError,
    connect_strict_tls, connect_tds_tls,
};
use crate::value_codec::{Value, encode_nvarchar};
use futures_util::{Stream, StreamExt, stream};
use tokio::io::AsyncWriteExt;
use tokio::sync::Notify;
use tokio::time::{sleep_until, timeout};
use tokio_native_tls::TlsStream;
use tokio_rustls::client::TlsStream as RustlsTlsStream;

pub type ConnectionListenerId = u64;
type ConnectionListener = Arc<dyn Fn(&ConnectionEvent) + Send + Sync>;
struct ConnectionListenerEntry {
    id: ConnectionListenerId,
    event_name: Option<String>,
    once: bool,
    fired: Arc<AtomicBool>,
    callback: ConnectionListener,
}
pub type TokenFuture<'a> =
    Pin<Box<dyn Future<Output = Result<String, ConnectionError>> + Send + 'a>>;
pub type CustomConnectorFuture =
    Pin<Box<dyn Future<Output = Result<tokio::net::TcpStream, std::io::Error>> + Send>>;
pub type CustomConnector = Arc<dyn Fn() -> CustomConnectorFuture + Send + Sync>;
pub type DestinationConnector = Arc<dyn Fn(String, u16) -> CustomConnectorFuture + Send + Sync>;

#[derive(Clone, PartialEq, Eq)]
pub struct RoutingData {
    pub protocol: u8,
    pub server: String,
    pub port: u16,
    pub instance: Option<String>,
    pub old_value: Vec<u8>,
}

impl fmt::Debug for RoutingData {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoutingData")
            .field("protocol", &self.protocol)
            .field("server", &self.server)
            .field("port", &self.port)
            .field("instance", &self.instance)
            .field(
                "old_value",
                &format_args!("[REDACTED; {} bytes]", self.old_value.len()),
            )
            .finish()
    }
}

#[derive(Clone)]
pub struct ConnectionCloseHandle {
    cancelled: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

struct ActiveRequestControl {
    id: u64,
    cancellation: RequestCancellationHandle,
    end_pending: Arc<AtomicBool>,
}

#[derive(Clone)]
struct RequestPauseControl {
    paused: Arc<AtomicBool>,
    resume: Arc<Notify>,
}

#[derive(Clone)]
pub struct StreamPauseHandle {
    control: RequestPauseControl,
}

impl StreamPauseHandle {
    pub fn pause(&self) {
        self.control.pause();
    }

    pub fn resume(&self) {
        self.control.resume();
    }

    pub fn is_paused(&self) -> bool {
        self.control.is_paused()
    }
}

impl RequestPauseControl {
    fn new(paused: bool) -> Self {
        Self {
            paused: Arc::new(AtomicBool::new(paused)),
            resume: Arc::new(Notify::new()),
        }
    }

    fn pause(&self) {
        self.paused.store(true, Ordering::Release);
    }

    fn resume(&self) {
        self.paused.store(false, Ordering::Release);
        self.resume.notify_waiters();
    }

    fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }
}

struct ActiveRequestGuard {
    slot: Arc<std::sync::Mutex<Option<ActiveRequestControl>>>,
    id: u64,
    end_pending: Arc<AtomicBool>,
    listeners: Arc<std::sync::Mutex<Vec<ConnectionListenerEntry>>>,
}

impl Drop for ActiveRequestGuard {
    fn drop(&mut self) {
        if let Ok(mut active_request) = self.slot.lock()
            && active_request
                .as_ref()
                .is_some_and(|request| request.id == self.id)
        {
            *active_request = None;
        }
        if self.end_pending.swap(false, Ordering::AcqRel) {
            Connection::emit_listeners(&self.listeners, ConnectionEvent::End);
        }
    }
}

impl ConnectionCloseHandle {
    pub fn close(&self) {
        if !self.cancelled.swap(true, Ordering::AcqRel) {
            self.notify.notify_waiters();
        }
    }

    pub fn is_closed(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

pub trait TokenProvider: Send + Sync {
    fn get_token(&self) -> TokenFuture<'_>;

    fn get_token_for_scope(&self, _scope: &str) -> TokenFuture<'_> {
        self.get_token()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum Authentication {
    Sql {
        user_name: String,
        password: String,
    },
    Ntlm {
        user_name: String,
        password: String,
        domain: Option<String>,
    },
    FedAuth {
        token: String,
    },
    AzureActiveDirectoryAccessToken {
        token: String,
    },
    AzureActiveDirectoryPassword {
        client_id: String,
        user_name: Option<String>,
        password: Option<String>,
        tenant_id: Option<String>,
    },
    AzureActiveDirectoryMsiVm {
        client_id: Option<String>,
    },
    AzureActiveDirectoryMsiAppService {
        client_id: Option<String>,
    },
    AzureActiveDirectoryServicePrincipalSecret {
        client_id: String,
        client_secret: String,
        tenant_id: String,
    },
    AzureActiveDirectoryDefault {
        client_id: Option<String>,
    },
    AzureActiveDirectoryTokenCredential,
}

impl fmt::Debug for Authentication {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sql {
                user_name,
                password: _,
            } => formatter
                .debug_struct("Sql")
                .field("user_name", user_name)
                .field("password", &"[REDACTED]")
                .finish(),
            Self::Ntlm {
                user_name,
                password: _,
                domain,
            } => formatter
                .debug_struct("Ntlm")
                .field("user_name", user_name)
                .field("password", &"[REDACTED]")
                .field("domain", domain)
                .finish(),
            Self::FedAuth { token: _ } => formatter
                .debug_struct("FedAuth")
                .field("token", &"[REDACTED]")
                .finish(),
            Self::AzureActiveDirectoryAccessToken { token: _ } => formatter
                .debug_struct("AzureActiveDirectoryAccessToken")
                .field("token", &"[REDACTED]")
                .finish(),
            Self::AzureActiveDirectoryPassword {
                client_id,
                user_name,
                password,
                tenant_id,
            } => formatter
                .debug_struct("AzureActiveDirectoryPassword")
                .field("client_id", client_id)
                .field("user_name", user_name)
                .field("password", &password.as_ref().map(|_| "[REDACTED]"))
                .field("tenant_id", tenant_id)
                .finish(),
            Self::AzureActiveDirectoryMsiVm { client_id } => formatter
                .debug_struct("AzureActiveDirectoryMsiVm")
                .field("client_id", client_id)
                .finish(),
            Self::AzureActiveDirectoryMsiAppService { client_id } => formatter
                .debug_struct("AzureActiveDirectoryMsiAppService")
                .field("client_id", client_id)
                .finish(),
            Self::AzureActiveDirectoryServicePrincipalSecret {
                client_id,
                client_secret: _,
                tenant_id,
            } => formatter
                .debug_struct("AzureActiveDirectoryServicePrincipalSecret")
                .field("client_id", client_id)
                .field("client_secret", &"[REDACTED]")
                .field("tenant_id", tenant_id)
                .finish(),
            Self::AzureActiveDirectoryDefault { client_id } => formatter
                .debug_struct("AzureActiveDirectoryDefault")
                .field("client_id", client_id)
                .finish(),
            Self::AzureActiveDirectoryTokenCredential => {
                formatter.write_str("AzureActiveDirectoryTokenCredential")
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ConnectionConfig {
    pub server: String,
    pub port: u16,
    pub instance_name: Option<String>,
    pub multi_subnet_failover: bool,
    pub database: Option<String>,
    pub authentication: Authentication,
    pub packet_size: usize,
    pub tds_version: String,
    pub encrypt: bool,
    pub strict_tls: bool,
    pub trust_server_certificate: bool,
    pub read_only: bool,
    pub read_only_intent: bool,
    pub connection_isolation_level: Option<IsolationLevel>,
    pub connect_timeout: Duration,
    pub connection_retry_interval: Duration,
    pub max_retries_on_transient_errors: u32,
    pub request_timeout: Duration,
    pub cancel_timeout: Duration,
    pub keep_alive_initial_delay: Duration,
    pub local_address: Option<String>,
    pub debug: DebugOptions,
    pub session: SessionOptions,
    pub fallback_to_default_db: bool,
    pub app_name: String,
    pub workstation_id: Option<String>,
    pub server_name: Option<String>,
    pub trusted_server_name_ae: Option<String>,
    pub column_encryption_setting: bool,
    pub column_encryption_key_cache_ttl: Duration,
    pub camel_case_columns: bool,
    pub use_column_names: bool,
    pub lower_case_guids: bool,
    pub use_utc: bool,
    pub row_collection_on_done: bool,
    pub row_collection_on_request_completion: bool,
}

impl fmt::Debug for ConnectionConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConnectionConfig")
            .field("server", &self.server)
            .field("port", &self.port)
            .field("instance_name", &self.instance_name)
            .field("multi_subnet_failover", &self.multi_subnet_failover)
            .field("database", &self.database)
            .field("authentication", &self.authentication)
            .field("packet_size", &self.packet_size)
            .field("tds_version", &self.tds_version)
            .field("encrypt", &self.encrypt)
            .field("strict_tls", &self.strict_tls)
            .field("trust_server_certificate", &self.trust_server_certificate)
            .field("read_only", &self.read_only)
            .field("read_only_intent", &self.read_only_intent)
            .field(
                "connection_isolation_level",
                &self.connection_isolation_level,
            )
            .field("connect_timeout", &self.connect_timeout)
            .field("connection_retry_interval", &self.connection_retry_interval)
            .field(
                "max_retries_on_transient_errors",
                &self.max_retries_on_transient_errors,
            )
            .field("request_timeout", &self.request_timeout)
            .field("cancel_timeout", &self.cancel_timeout)
            .field("keep_alive_initial_delay", &self.keep_alive_initial_delay)
            .field("local_address", &self.local_address)
            .field("debug", &self.debug)
            .field("session", &self.session)
            .field("fallback_to_default_db", &self.fallback_to_default_db)
            .field("app_name", &self.app_name)
            .field("workstation_id", &self.workstation_id)
            .field("server_name", &self.server_name)
            .field("trusted_server_name_ae", &self.trusted_server_name_ae)
            .field("column_encryption_setting", &self.column_encryption_setting)
            .field(
                "column_encryption_key_cache_ttl",
                &self.column_encryption_key_cache_ttl,
            )
            .field("camel_case_columns", &self.camel_case_columns)
            .field("use_column_names", &self.use_column_names)
            .field("lower_case_guids", &self.lower_case_guids)
            .field("use_utc", &self.use_utc)
            .field("row_collection_on_done", &self.row_collection_on_done)
            .field(
                "row_collection_on_request_completion",
                &self.row_collection_on_request_completion,
            )
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionOptions {
    pub enable_ansi_null: Option<bool>,
    pub enable_ansi_null_default: Option<bool>,
    pub enable_ansi_padding: Option<bool>,
    pub enable_ansi_warnings: Option<bool>,
    pub enable_arith_abort: Option<bool>,
    pub enable_concat_null_yields_null: Option<bool>,
    pub enable_cursor_close_on_commit: Option<bool>,
    pub datefirst: Option<u8>,
    pub date_format: Option<String>,
    pub enable_implicit_transactions: Option<bool>,
    pub language: Option<String>,
    pub enable_numeric_roundabort: Option<bool>,
    pub enable_quoted_identifier: Option<bool>,
    pub textsize: Option<i32>,
    pub isolation_level: Option<IsolationLevel>,
    pub abort_transaction_on_error: Option<bool>,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            enable_ansi_null: Some(true),
            enable_ansi_null_default: Some(true),
            enable_ansi_padding: Some(true),
            enable_ansi_warnings: Some(true),
            enable_arith_abort: Some(true),
            enable_concat_null_yields_null: Some(true),
            enable_cursor_close_on_commit: None,
            datefirst: Some(7),
            date_format: Some("mdy".to_owned()),
            enable_implicit_transactions: Some(false),
            language: Some("us_english".to_owned()),
            enable_numeric_roundabort: Some(false),
            enable_quoted_identifier: Some(true),
            textsize: Some(2_147_483_647),
            isolation_level: Some(IsolationLevel::ReadCommitted),
            abort_transaction_on_error: Some(false),
        }
    }
}

impl SessionOptions {
    pub fn initial_sql(&self) -> String {
        self.initial_sql_with_isolation(self.isolation_level)
    }

    pub fn initial_sql_with_isolation(&self, isolation_level: Option<IsolationLevel>) -> String {
        let mut statements = Vec::new();
        let switch = |statements: &mut Vec<String>, name: &str, value: Option<bool>| {
            if let Some(value) = value {
                statements.push(format!("set {name} {}", if value { "on" } else { "off" }));
            }
        };
        switch(&mut statements, "ansi_nulls", self.enable_ansi_null);
        switch(
            &mut statements,
            "ansi_null_dflt_on",
            self.enable_ansi_null_default,
        );
        switch(&mut statements, "ansi_padding", self.enable_ansi_padding);
        switch(&mut statements, "ansi_warnings", self.enable_ansi_warnings);
        switch(&mut statements, "arithabort", self.enable_arith_abort);
        switch(
            &mut statements,
            "concat_null_yields_null",
            self.enable_concat_null_yields_null,
        );
        switch(
            &mut statements,
            "cursor_close_on_commit",
            self.enable_cursor_close_on_commit,
        );
        if let Some(value) = self.datefirst {
            statements.push(format!("set datefirst {value}"));
        }
        if let Some(value) = &self.date_format {
            statements.push(format!("set dateformat {value}"));
        }
        switch(
            &mut statements,
            "implicit_transactions",
            self.enable_implicit_transactions,
        );
        if let Some(value) = &self.language {
            statements.push(format!("set language {value}"));
        }
        switch(
            &mut statements,
            "numeric_roundabort",
            self.enable_numeric_roundabort,
        );
        switch(
            &mut statements,
            "quoted_identifier",
            self.enable_quoted_identifier,
        );
        if let Some(value) = self.textsize {
            statements.push(format!("set textsize {value}"));
        }
        if let Some(value) = isolation_level {
            let name = match value {
                IsolationLevel::NoChange => "",
                IsolationLevel::ReadUncommitted => "read uncommitted",
                IsolationLevel::ReadCommitted => "read committed",
                IsolationLevel::RepeatableRead => "repeatable read",
                IsolationLevel::Serializable => "serializable",
                IsolationLevel::Snapshot => "snapshot",
            };
            if !name.is_empty() {
                statements.push(format!("set transaction isolation level {name}"));
            }
        }
        switch(
            &mut statements,
            "xact_abort",
            self.abort_transaction_on_error,
        );
        statements.join("\n")
    }
}

impl ConnectionConfig {
    pub fn sql(
        server: impl Into<String>,
        user_name: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            server: server.into(),
            port: 1433,
            instance_name: None,
            multi_subnet_failover: false,
            database: None,
            authentication: Authentication::Sql {
                user_name: user_name.into(),
                password: password.into(),
            },
            packet_size: 4096,
            tds_version: "7_4".to_owned(),
            encrypt: true,
            strict_tls: false,
            trust_server_certificate: false,
            read_only: false,
            read_only_intent: false,
            connection_isolation_level: Some(IsolationLevel::ReadCommitted),
            connect_timeout: Duration::from_secs(15),
            connection_retry_interval: Duration::from_millis(500),
            max_retries_on_transient_errors: 3,
            request_timeout: Duration::from_secs(15),
            cancel_timeout: Duration::from_secs(5),
            keep_alive_initial_delay: Duration::from_secs(30),
            local_address: None,
            debug: DebugOptions::default(),
            session: SessionOptions::default(),
            fallback_to_default_db: false,
            app_name: "Tedious".to_owned(),
            workstation_id: None,
            server_name: None,
            trusted_server_name_ae: None,
            column_encryption_setting: false,
            column_encryption_key_cache_ttl: Duration::from_secs(2 * 60 * 60),
            camel_case_columns: false,
            use_column_names: false,
            lower_case_guids: false,
            use_utc: true,
            row_collection_on_done: false,
            row_collection_on_request_completion: false,
        }
    }

    pub fn sql_from_environment() -> Result<Self, ConnectionError> {
        Err(ConnectionError::new(
            "environment credentials are reserved for NTLM; use ntlm_from_environment",
            Some("EENVPROFILE".to_owned()),
        ))
    }

    pub fn sql_read_only(
        server: impl Into<String>,
        user_name: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        let mut config = Self::sql(server, user_name, password);
        config.read_only = true;
        config
    }

    pub fn ntlm(
        server: impl Into<String>,
        user_name: impl Into<String>,
        password: impl Into<String>,
        domain: Option<String>,
    ) -> Self {
        let mut config = Self::sql(server, String::new(), String::new());
        config.authentication = Authentication::Ntlm {
            user_name: user_name.into(),
            password: password.into(),
            domain,
        };
        config.read_only = true;
        config
    }

    pub fn sql_strict(
        server: impl Into<String>,
        user_name: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        let mut config = Self::sql(server, user_name, password);
        config.strict_tls = true;
        config.tds_version = "8_0".to_owned();
        config
    }

    pub fn azure_active_directory_access_token(
        server: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        let mut config = Self::sql(server, String::new(), String::new());
        config.authentication = Authentication::AzureActiveDirectoryAccessToken {
            token: token.into(),
        };
        config
    }

    pub fn azure_active_directory_password(
        server: impl Into<String>,
        client_id: impl Into<String>,
        user_name: Option<String>,
        password: Option<String>,
        tenant_id: Option<String>,
    ) -> Self {
        let mut config = Self::sql(server, String::new(), String::new());
        config.authentication = Authentication::AzureActiveDirectoryPassword {
            client_id: client_id.into(),
            user_name,
            password,
            tenant_id,
        };
        config
    }

    pub fn azure_active_directory_msi_vm(
        server: impl Into<String>,
        client_id: Option<String>,
    ) -> Self {
        let mut config = Self::sql(server, String::new(), String::new());
        config.authentication = Authentication::AzureActiveDirectoryMsiVm { client_id };
        config
    }

    pub fn azure_active_directory_msi_app_service(
        server: impl Into<String>,
        client_id: Option<String>,
    ) -> Self {
        let mut config = Self::sql(server, String::new(), String::new());
        config.authentication = Authentication::AzureActiveDirectoryMsiAppService { client_id };
        config
    }

    pub fn azure_active_directory_service_principal_secret(
        server: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        tenant_id: impl Into<String>,
    ) -> Self {
        let mut config = Self::sql(server, String::new(), String::new());
        config.authentication = Authentication::AzureActiveDirectoryServicePrincipalSecret {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            tenant_id: tenant_id.into(),
        };
        config
    }

    pub fn azure_active_directory_default(
        server: impl Into<String>,
        client_id: Option<String>,
    ) -> Self {
        let mut config = Self::sql(server, String::new(), String::new());
        config.authentication = Authentication::AzureActiveDirectoryDefault { client_id };
        config
    }

    pub fn azure_active_directory_token_credential(server: impl Into<String>) -> Self {
        let mut config = Self::sql(server, String::new(), String::new());
        config.authentication = Authentication::AzureActiveDirectoryTokenCredential;
        config
    }

    pub fn ntlm_from_environment(domain: Option<String>) -> Result<Self, ConnectionError> {
        let host = environment_value("TEST_DB_HOST")?;
        let user_name = environment_value("VPN_USER")?;
        let password = environment_value("VPN_PWD")?;
        let mut config = Self::ntlm(host, user_name, password, domain);
        config.database = Some(environment_value("TEST_DB_NAME")?);
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConnectionError> {
        if !versions().contains_key(self.tds_version.as_str()) {
            return Err(ConnectionError::new(
                "unsupported TDS version",
                Some("ETDSVERSION".to_owned()),
            ));
        }
        if self.strict_tls && self.tds_version != "8_0" {
            return Err(ConnectionError::new(
                "strict TLS requires TDS version 8_0",
                Some("ETDSVERSION".to_owned()),
            ));
        }
        if self.port == 0 && self.instance_name.is_none() {
            return Err(ConnectionError::new(
                "port must be greater than 0",
                Some("EPORT".to_owned()),
            ));
        }
        if self.packet_size <= crate::packet::HEADER_LENGTH || self.packet_size > u16::MAX as usize
        {
            return Err(ConnectionError::new(
                "packet size is outside the valid TDS range",
                Some("EPACKETSIZE".to_owned()),
            ));
        }
        if self.connection_retry_interval.is_zero() {
            return Err(ConnectionError::new(
                "connection retry interval must be greater than 0",
                Some("ERETRYINTERVAL".to_owned()),
            ));
        }
        if let Some(datefirst) = self.session.datefirst
            && !(1..=7).contains(&datefirst)
        {
            return Err(ConnectionError::new(
                "datefirst must be between 1 and 7",
                Some("EDATEFIRST".to_owned()),
            ));
        }
        if let Some(textsize) = self.session.textsize
            && !(-1..=2_147_483_647).contains(&textsize)
        {
            return Err(ConnectionError::new(
                "textsize must be between -1 and 2147483647",
                Some("ETEXTSIZE".to_owned()),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Initialized,
    Connecting,
    SentPrelogin,
    SentTlsSslNegotiation,
    SentLogin7WithStandardLogin,
    SentLogin7WithNtlm,
    SentLogin7WithFedAuth,
    LoggedInSendingInitialSql,
    SentClientRequest,
    LoggedIn,
    SentAttention,
    Rerouting,
    TransientFailureRetry,
    Final,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionEvent {
    Connect,
    ConnectError(ConnectionError),
    Error(ConnectionError),
    End,
    NewListener {
        event_name: String,
        listener_id: ConnectionListenerId,
    },
    RemoveListener {
        event_name: String,
        listener_id: ConnectionListenerId,
    },
    Rerouting,
    Retry,
    RollbackTransaction,
    CharsetChange(String),
    DatabaseChange(String),
    LanguageChange(String),
    InfoMessage(crate::token::parser::MessageToken),
    ErrorMessage(crate::token::parser::MessageToken),
    ResetConnection,
    Secure,
    DatabaseMirroringPartner(String),
    PacketSizeChange(u32),
    CollationChange(crate::collation::Collation),
    Debug(String),
}

impl From<crate::public_api::ConnectionEventName> for String {
    fn from(event_name: crate::public_api::ConnectionEventName) -> Self {
        event_name.as_str().to_owned()
    }
}

enum ConnectionIo {
    Plain(MessageIo<tokio::net::TcpStream>),
    Tls(MessageIo<TlsStream<TdsTlsStream<tokio::net::TcpStream>>>),
    DirectTls(Box<MessageIo<RustlsTlsStream<tokio::net::TcpStream>>>),
}

enum ControlledSendOutcome {
    Completed(Result<(), TransportError>),
    Canceled,
    Closed,
    TimedOut,
}

fn token_debug_name(token: &Token) -> &'static str {
    match token {
        Token::Done(_) => "Done",
        Token::Info(_) => "Info",
        Token::Error(_) => "Error",
        Token::ReturnStatus(_) => "ReturnStatus",
        Token::ReturnValue(_) => "ReturnValue",
        Token::LoginAck(_) => "LoginAck",
        Token::Sspi(_) => "Sspi",
        Token::FeatureExtAck(_) => "FeatureExtAck",
        Token::FedAuthInfo(_) => "FedAuthInfo",
        Token::Order(_) => "Order",
        Token::EnvChange(_) => "EnvChange",
        Token::Offset(_) => "Offset",
        Token::TabName(_) => "TabName",
        Token::ColInfo(_) => "ColInfo",
        Token::DataClassification(_) => "DataClassification",
        Token::AltMetadata(_) => "AltMetadata",
        Token::AltRow(_) => "AltRow",
        Token::SessionState(_) => "SessionState",
    }
}

fn stream_token_debug_name(token: &StreamToken) -> String {
    match token {
        StreamToken::Control(token) => format!("StreamToken::Control({})", token_debug_name(token)),
        StreamToken::ColMetadata(columns) => {
            format!(
                "StreamToken::ColMetadata([REDACTED; {} columns])",
                columns.len()
            )
        }
        StreamToken::Row(row) => format!(
            "StreamToken::Row([REDACTED; {} columns])",
            row.columns.len()
        ),
        StreamToken::NbcRow(row) => {
            format!(
                "StreamToken::NbcRow([REDACTED; {} columns])",
                row.columns.len()
            )
        }
    }
}

impl ConnectionIo {
    fn set_debug(&mut self, options: DebugOptions, listener: Arc<dyn Fn(String) + Send + Sync>) {
        match self {
            Self::Plain(io) => io.set_debug(options, listener),
            Self::Tls(io) => io.set_debug(options, listener),
            Self::DirectTls(io) => io.set_debug(options, listener),
        }
    }

    async fn send_message(&mut self, message: &Message) -> Result<(), TransportError> {
        match self {
            Self::Plain(io) => io.send_message(message).await,
            Self::Tls(io) => io.send_message(message).await,
            Self::DirectTls(io) => io.send_message(message).await,
        }
    }

    async fn send_message_buffers(
        &mut self,
        message_type: u8,
        buffers: &[Vec<u8>],
    ) -> Result<(), TransportError> {
        match self {
            Self::Plain(io) => io.send_message_buffers(message_type, buffers).await,
            Self::Tls(io) => io.send_message_buffers(message_type, buffers).await,
            Self::DirectTls(io) => io.send_message_buffers(message_type, buffers).await,
        }
    }

    async fn send_message_stream_with_cancellation<S>(
        &mut self,
        message_type: u8,
        parts: S,
        cancellation: &BulkLoadCancellationHandle,
    ) -> Result<bool, TransportError>
    where
        S: Stream<Item = Result<Vec<u8>, TransportError>>,
    {
        match self {
            Self::Plain(io) => {
                io.send_message_stream_with_cancellation(message_type, parts, cancellation)
                    .await
            }
            Self::Tls(io) => {
                io.send_message_stream_with_cancellation(message_type, parts, cancellation)
                    .await
            }
            Self::DirectTls(io) => {
                io.send_message_stream_with_cancellation(message_type, parts, cancellation)
                    .await
            }
        }
    }

    async fn send_message_buffers_with_cancellation(
        &mut self,
        message_type: u8,
        buffers: &[Vec<u8>],
        cancellation: &BulkLoadCancellationHandle,
    ) -> Result<bool, TransportError> {
        match self {
            Self::Plain(io) => {
                io.send_message_buffers_with_cancellation(message_type, buffers, cancellation)
                    .await
            }
            Self::Tls(io) => {
                io.send_message_buffers_with_cancellation(message_type, buffers, cancellation)
                    .await
            }
            Self::DirectTls(io) => {
                io.send_message_buffers_with_cancellation(message_type, buffers, cancellation)
                    .await
            }
        }
    }

    async fn send_message_parts_with_cancellation<I>(
        &mut self,
        message_type: u8,
        parts: I,
        cancellation: &BulkLoadCancellationHandle,
    ) -> Result<bool, TransportError>
    where
        I: IntoIterator<Item = Result<Vec<u8>, TransportError>>,
    {
        match self {
            Self::Plain(io) => {
                io.send_message_parts_with_cancellation(message_type, parts, cancellation)
                    .await
            }
            Self::Tls(io) => {
                io.send_message_parts_with_cancellation(message_type, parts, cancellation)
                    .await
            }
            Self::DirectTls(io) => {
                io.send_message_parts_with_cancellation(message_type, parts, cancellation)
                    .await
            }
        }
    }

    async fn read_message(&mut self) -> Result<Message, TransportError> {
        match self {
            Self::Plain(io) => io.read_message().await,
            Self::Tls(io) => io.read_message().await,
            Self::DirectTls(io) => io.read_message().await,
        }
    }

    async fn next_message_chunk(&mut self) -> Result<IncomingMessageChunkResult, TransportError> {
        match self {
            Self::Plain(io) => io.next_message_chunk().await,
            Self::Tls(io) => io.next_message_chunk().await,
            Self::DirectTls(io) => io.next_message_chunk().await,
        }
    }

    fn pause_incoming(&self) {
        match self {
            Self::Plain(io) => io.pause_incoming(),
            Self::Tls(io) => io.pause_incoming(),
            Self::DirectTls(io) => io.pause_incoming(),
        }
    }

    fn resume_incoming(&self) {
        match self {
            Self::Plain(io) => io.resume_incoming(),
            Self::Tls(io) => io.resume_incoming(),
            Self::DirectTls(io) => io.resume_incoming(),
        }
    }

    async fn send_attention(&mut self) -> Result<(), TransportError> {
        match self {
            Self::Plain(io) => io.send_attention().await,
            Self::Tls(io) => io.send_attention().await,
            Self::DirectTls(io) => io.send_attention().await,
        }
    }

    fn packet_size(&self) -> usize {
        match self {
            Self::Plain(io) => io.packet_size(),
            Self::Tls(io) => io.packet_size(),
            Self::DirectTls(io) => io.packet_size(),
        }
    }

    async fn write_packet(&mut self, packet: &Packet) -> Result<(), TransportError> {
        match self {
            Self::Plain(io) => io.stream_mut().write_all(&packet.buffer).await?,
            Self::Tls(io) => io.stream_mut().write_all(&packet.buffer).await?,
            Self::DirectTls(io) => io.stream_mut().write_all(&packet.buffer).await?,
        }
        Ok(())
    }

    async fn flush(&mut self) -> Result<(), TransportError> {
        match self {
            Self::Plain(io) => io.stream_mut().flush().await?,
            Self::Tls(io) => io.stream_mut().flush().await?,
            Self::DirectTls(io) => io.stream_mut().flush().await?,
        }
        Ok(())
    }

    fn set_packet_size(&mut self, packet_size: usize) -> Result<(), TransportError> {
        match self {
            Self::Plain(io) => io.set_packet_size(packet_size),
            Self::Tls(io) => {
                io.set_packet_size(packet_size)?;
                io.stream_mut()
                    .get_mut()
                    .get_mut()
                    .get_mut()
                    .set_packet_size(packet_size)
            }
            Self::DirectTls(io) => io.set_packet_size(packet_size),
        }
    }
}

pub struct Connection {
    pub config: ConnectionConfig,
    pub state: ConnectionState,
    message_io: Option<ConnectionIo>,
    pub fed_auth_required: bool,
    pub transaction_descriptor: Vec<u8>,
    pub transaction_depth: usize,
    pub database_collation: Option<crate::collation::Collation>,
    pub routing: Option<RoutingData>,
    pub cur_transient_retry_count: u32,
    token_provider: Option<Arc<dyn TokenProvider>>,
    custom_connector: Option<CustomConnector>,
    destination_connector: Option<DestinationConnector>,
    key_store_provider: Option<Arc<dyn KeyStoreProvider>>,
    request_scheduler: Arc<RequestScheduler>,
    listeners: Arc<std::sync::Mutex<Vec<ConnectionListenerEntry>>>,
    next_listener_id: ConnectionListenerId,
    column_name_replacer: Option<crate::token::stream::ColumnNameReplacer>,
    pending_result_crypto_metadata: Option<Vec<crate::always_encrypted::types::CryptoMetadata>>,
    force_encrypt: Option<Vec<bool>>,
    close_handle: ConnectionCloseHandle,
    active_request: Arc<std::sync::Mutex<Option<ActiveRequestControl>>>,
    next_active_request_id: AtomicU64,
}

impl Connection {
    fn controlled_send_decision(
        &self,
        cancellation: Option<&RequestCancellationHandle>,
        deadline: Option<Instant>,
    ) -> Option<ControlledSendOutcome> {
        if cancellation.is_some_and(RequestCancellationHandle::is_cancelled) {
            Some(ControlledSendOutcome::Canceled)
        } else if self.close_handle.is_closed() {
            Some(ControlledSendOutcome::Closed)
        } else if deadline.is_some_and(|value| Instant::now() >= value) {
            Some(ControlledSendOutcome::TimedOut)
        } else {
            None
        }
    }

    fn message_packet(
        message_type: u8,
        packet_id: u8,
        reset_connection: bool,
        last: bool,
        ignore: bool,
        data: &[u8],
    ) -> Result<Packet, TransportError> {
        let mut packet = Packet::new(message_type);
        packet.buffer[6] = packet_id;
        packet.reset_connection(reset_connection);
        packet.last(Some(last));
        packet.ignore(ignore);
        packet
            .add_data(data)
            .map_err(TransportError::InvalidPacket)?;
        Ok(packet)
    }

    async fn finish_controlled_message(
        &mut self,
        message_type: u8,
        reset_connection: bool,
        packet_id: u8,
        data: &[u8],
        outcome: ControlledSendOutcome,
    ) -> ControlledSendOutcome {
        let Some(io) = self.message_io.as_mut() else {
            return ControlledSendOutcome::Completed(Err(TransportError::InvalidPacket(
                "connection is not logged in".to_owned(),
            )));
        };
        let packet =
            match Self::message_packet(message_type, packet_id, reset_connection, true, true, data)
            {
                Ok(packet) => packet,
                Err(error) => return ControlledSendOutcome::Completed(Err(error)),
            };
        if let Err(error) = io.write_packet(&packet).await {
            return ControlledSendOutcome::Completed(Err(error));
        }
        if let Err(error) = io.flush().await {
            return ControlledSendOutcome::Completed(Err(error));
        }
        outcome
    }

    async fn send_message_buffers_controlled(
        &mut self,
        message_type: u8,
        reset_connection: bool,
        buffers: &[Vec<u8>],
        cancellation: Option<&RequestCancellationHandle>,
        deadline: Option<Instant>,
    ) -> ControlledSendOutcome {
        let Some(packet_size) = self.message_io.as_ref().map(ConnectionIo::packet_size) else {
            return ControlledSendOutcome::Completed(Err(TransportError::InvalidPacket(
                "connection is not logged in".to_owned(),
            )));
        };
        if packet_size <= HEADER_LENGTH {
            return ControlledSendOutcome::Completed(Err(TransportError::InvalidPacket(
                "invalid packet size".to_owned(),
            )));
        }
        let payload_size = packet_size - HEADER_LENGTH;
        let mut payload = Vec::with_capacity(payload_size);
        let mut packet_id = 1u8;
        let mut emitted = false;
        for buffer in buffers {
            let mut offset = 0;
            while offset < buffer.len() {
                if payload.len() == payload_size {
                    if let Some(outcome) = self.controlled_send_decision(cancellation, deadline) {
                        return self
                            .finish_controlled_message(
                                message_type,
                                reset_connection,
                                packet_id,
                                &payload,
                                outcome,
                            )
                            .await;
                    }
                    let packet = match Self::message_packet(
                        message_type,
                        packet_id,
                        reset_connection,
                        false,
                        false,
                        &payload,
                    ) {
                        Ok(packet) => packet,
                        Err(error) => return ControlledSendOutcome::Completed(Err(error)),
                    };
                    let Some(io) = self.message_io.as_mut() else {
                        return ControlledSendOutcome::Completed(Err(
                            TransportError::InvalidPacket("connection is not logged in".to_owned()),
                        ));
                    };
                    if let Err(error) = io.write_packet(&packet).await {
                        return ControlledSendOutcome::Completed(Err(error));
                    }
                    emitted = true;
                    packet_id = packet_id.wrapping_add(1);
                    payload.clear();
                }
                let length = (payload_size - payload.len()).min(buffer.len() - offset);
                payload.extend_from_slice(&buffer[offset..offset + length]);
                offset += length;
            }
        }
        if let Some(outcome) = self.controlled_send_decision(cancellation, deadline) {
            return self
                .finish_controlled_message(
                    message_type,
                    reset_connection,
                    packet_id,
                    &payload,
                    outcome,
                )
                .await;
        }
        let packet = match Self::message_packet(
            message_type,
            packet_id,
            reset_connection,
            true,
            false,
            &payload,
        ) {
            Ok(packet) => packet,
            Err(error) => return ControlledSendOutcome::Completed(Err(error)),
        };
        let Some(io) = self.message_io.as_mut() else {
            return ControlledSendOutcome::Completed(Err(TransportError::InvalidPacket(
                "connection is not logged in".to_owned(),
            )));
        };
        if let Err(error) = io.write_packet(&packet).await {
            return ControlledSendOutcome::Completed(Err(error));
        }
        if let Err(error) = io.flush().await {
            return ControlledSendOutcome::Completed(Err(error));
        }
        if let Some(outcome) = self.controlled_send_decision(cancellation, deadline) {
            if matches!(outcome, ControlledSendOutcome::Canceled) && !emitted {
                return ControlledSendOutcome::Canceled;
            }
            return outcome;
        }
        ControlledSendOutcome::Completed(Ok(()))
    }

    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            config,
            state: ConnectionState::Initialized,
            message_io: None,
            fed_auth_required: false,
            transaction_descriptor: vec![0; 8],
            transaction_depth: 0,
            database_collation: None,
            routing: None,
            cur_transient_retry_count: 0,
            token_provider: None,
            custom_connector: None,
            destination_connector: None,
            key_store_provider: None,
            request_scheduler: RequestScheduler::new(),
            listeners: Arc::new(std::sync::Mutex::new(Vec::new())),
            next_listener_id: 0,
            column_name_replacer: None,
            pending_result_crypto_metadata: None,
            force_encrypt: None,
            close_handle: ConnectionCloseHandle {
                cancelled: Arc::new(AtomicBool::new(false)),
                notify: Arc::new(Notify::new()),
            },
            active_request: Arc::new(std::sync::Mutex::new(None)),
            next_active_request_id: AtomicU64::new(0),
        }
    }

    fn configure_debug(&self, io: &mut ConnectionIo) {
        if self.config.debug.packet || self.config.debug.data {
            let listeners = Arc::clone(&self.listeners);
            io.set_debug(
                self.config.debug,
                Arc::new(move |message| {
                    let event = ConnectionEvent::Debug(message);
                    Connection::emit_listeners(&listeners, event);
                }),
            );
        }
    }

    fn emit_debug_payload(&self, label: &str) {
        if self.config.debug.payload {
            self.emit(ConnectionEvent::Debug(format!("{label} [REDACTED]")));
        }
    }

    pub fn set_column_name_replacer<F>(&mut self, replacer: F)
    where
        F: Fn(&str, usize, &ColumnMetadata) -> String + Send + Sync + 'static,
    {
        self.column_name_replacer = Some(Arc::new(replacer));
    }

    pub fn set_custom_connector<F, Fut>(&mut self, connector: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<tokio::net::TcpStream, std::io::Error>> + Send + 'static,
    {
        self.custom_connector = Some(Arc::new(move || Box::pin(connector())));
    }

    pub fn set_destination_connector<F, Fut>(&mut self, connector: F)
    where
        F: Fn(String, u16) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<tokio::net::TcpStream, std::io::Error>> + Send + 'static,
    {
        self.destination_connector = Some(Arc::new(move |server, port| {
            Box::pin(connector(server, port))
        }));
    }

    pub fn close_handle(&self) -> ConnectionCloseHandle {
        self.close_handle.clone()
    }

    pub fn get_initial_sql(&self) -> String {
        self.config
            .session
            .initial_sql_with_isolation(self.config.connection_isolation_level)
    }

    pub fn transition_to(&mut self, new_state: ConnectionState) {
        if self.state == new_state {
            if self.config.debug.payload || self.config.debug.token {
                self.emit(ConnectionEvent::Debug(format!(
                    "State is already {}",
                    connection_state_name(new_state)
                )));
            }
            return;
        }
        if self.config.debug.payload || self.config.debug.token {
            self.emit(ConnectionEvent::Debug(format!(
                "State change: {} -> {}",
                connection_state_name(self.state),
                connection_state_name(new_state)
            )));
        }
        self.state = new_state;
    }

    fn create_token_stream_parser(&mut self) -> TokenStreamParser {
        let mut parser = TokenStreamParser::new_with_options(
            &self.config.tds_version,
            self.config.camel_case_columns,
            self.config.use_utc,
        );
        parser.set_column_name_replacer(self.column_name_replacer.clone());
        if let Some(metadata) = self.pending_result_crypto_metadata.take() {
            parser.set_column_encryption(
                metadata,
                self.key_store_provider.clone(),
                self.config.trusted_server_name_ae.clone(),
                self.config.column_encryption_key_cache_ttl,
            );
        }
        parser
    }

    async fn read_response_tokens_incremental(
        &mut self,
        terminal_error_message: &str,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        Box::pin(
            self.read_response_tokens_incremental_with_cancellation_and_callback(
                terminal_error_message,
                None,
                None,
                |_, _| Ok(()),
            ),
        )
        .await
    }

    async fn read_response_tokens_incremental_with_cancellation(
        &mut self,
        terminal_error_message: &str,
        cancellation: Option<&RequestCancellationHandle>,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        self.read_response_tokens_incremental_with_cancellation_and_callback(
            terminal_error_message,
            cancellation,
            None,
            |_, _| Ok(()),
        )
        .await
    }

    async fn read_response_tokens_incremental_with_cancellation_and_callback<F>(
        &mut self,
        terminal_error_message: &str,
        cancellation: Option<&RequestCancellationHandle>,
        first_packet_deadline: Option<Instant>,
        callback: F,
    ) -> Result<Vec<StreamToken>, ConnectionError>
    where
        F: FnMut(&mut Self, &[StreamToken]) -> Result<(), ConnectionError>,
    {
        self.read_response_tokens_incremental_with_cancellation_and_callback_and_pause(
            terminal_error_message,
            cancellation,
            first_packet_deadline,
            None,
            callback,
            true,
        )
        .await
    }

    async fn read_response_tokens_incremental_with_cancellation_and_callback_and_pause<F>(
        &mut self,
        terminal_error_message: &str,
        cancellation: Option<&RequestCancellationHandle>,
        first_packet_deadline: Option<Instant>,
        pause_control: Option<RequestPauseControl>,
        mut callback: F,
        collect_tokens: bool,
    ) -> Result<Vec<StreamToken>, ConnectionError>
    where
        F: FnMut(&mut Self, &[StreamToken]) -> Result<(), ConnectionError>,
    {
        let mut parser = self.create_token_stream_parser();
        let mut tokens = Vec::new();
        let mut has_more_results = None;
        let mut awaiting_first_packet = first_packet_deadline;
        let mut pending_chunk: Option<(Vec<u8>, bool)> = None;
        if pause_control
            .as_ref()
            .is_some_and(RequestPauseControl::is_paused)
        {
            parser.pause();
            if let Some(io) = self.message_io.as_ref() {
                io.pause_incoming();
            }
        }
        loop {
            if self.close_handle.is_closed() {
                return Err(self.terminate_connection_after_close(ConnectionError::new(
                    "Connection closed before request completed.",
                    Some("ECLOSE".to_owned()),
                )));
            }
            if let Some(pause_control) = pause_control.as_ref()
                && pause_control.is_paused()
            {
                if !parser.is_paused() {
                    parser.pause();
                }
                if let Some(io) = self.message_io.as_ref() {
                    io.pause_incoming();
                }
                loop {
                    let resumed = pause_control.resume.notified();
                    tokio::pin!(resumed);
                    if !pause_control.is_paused() {
                        break;
                    }
                    if let Some(cancellation) = cancellation {
                        tokio::select! {
                            _ = &mut resumed => {}
                            _ = cancellation.cancelled() => {
                                if let Some(io) = self.message_io.as_ref() {
                                    io.resume_incoming();
                                }
                                if parser.is_paused() {
                                    let _ = parser.resume();
                                }
                                if let Some((_, end_of_message)) = pending_chunk.take()
                                    && !end_of_message
                                {
                                    loop {
                                        let chunk = self
                                            .message_io
                                            .as_mut()
                                            .ok_or_else(|| {
                                                ConnectionError::new(
                                                    "connection is not logged in",
                                                    Some("ENOTCONNECTED".to_owned()),
                                                )
                                            })?
                                            .next_message_chunk()
                                            .await
                                            .map_err(connection_transport_error)?;
                                        if matches!(
                                            chunk,
                                            IncomingMessageChunkResult::Chunk(chunk)
                                                if chunk.end_of_message
                                        ) {
                                            break;
                                        }
                                    }
                                }
                                return Box::pin(self.cancel_active_request(terminal_error_message)).await;
                            }
                            _ = self.close_handle.notify.notified() => {
                                return Err(self.terminate_connection_after_close(ConnectionError::new(
                                    "Connection closed before request completed.",
                                    Some("ECLOSE".to_owned()),
                                )));
                            }
                        }
                    } else {
                        let close_handle = self.close_handle.clone();
                        let notified = close_handle.notify.notified();
                        tokio::pin!(notified);
                        tokio::select! {
                            _ = &mut resumed => {}
                            _ = &mut notified => {
                                return Err(self.terminate_connection_after_close(ConnectionError::new(
                                    "Connection closed before request completed.",
                                    Some("ECLOSE".to_owned()),
                                )));
                            }
                        }
                    }
                }
                if let Some(io) = self.message_io.as_ref() {
                    io.resume_incoming();
                }
                let resumed_tokens = match parser.resume() {
                    Ok(tokens) => tokens,
                    Err(error) if is_recoverable_stream_parse_error(&error) => Vec::new(),
                    Err(error) => {
                        return Err(ConnectionError::new(
                            format!("{terminal_error_message}: {error:?}"),
                            Some("ETOKEN".to_owned()),
                        ));
                    }
                };
                callback(self, &resumed_tokens)?;
                if collect_tokens {
                    tokens.extend(resumed_tokens.iter().cloned());
                }
                if let Some(more) = resumed_tokens.iter().rev().find_map(|token| match token {
                    StreamToken::Control(Token::Done(done)) => Some(done.more),
                    _ => None,
                }) {
                    has_more_results = Some(more);
                }
                if pause_control.is_paused() {
                    continue;
                }
            }
            let chunk = if let Some((data, end_of_message)) = pending_chunk.take() {
                IncomingMessageChunkResult::Chunk(IncomingMessageChunk {
                    message_type: packet_type::TABULAR_RESULT,
                    reset_connection: false,
                    ignore: false,
                    data,
                    end_of_message,
                })
            } else if let Some(deadline) = awaiting_first_packet {
                let timer = sleep_until(tokio::time::Instant::from_std(deadline));
                tokio::pin!(timer);
                if let Some(cancellation) = cancellation {
                    tokio::select! {
                        chunk = async {
                            self.message_io
                                .as_mut()
                                .ok_or_else(|| ConnectionError::new("connection is not logged in", Some("ENOTCONNECTED".to_owned())))?
                                .next_message_chunk()
                                .await
                                .map_err(connection_transport_error)
                        } => chunk?,
                        _ = cancellation.cancelled() => {
                            return Box::pin(self.cancel_active_request(terminal_error_message)).await;
                        }
                        _ = self.close_handle.notify.notified() => {
                            return Err(self.terminate_connection_after_close(ConnectionError::new(
                                "Connection closed before request completed.",
                                Some("ECLOSE".to_owned()),
                            )));
                        }
                        _ = &mut timer => {
                            return Err(ConnectionError::new(
                                "request timed out before the first response packet",
                                Some("ETIMEOUT".to_owned()),
                            ));
                        }
                    }
                } else {
                    let close_handle = self.close_handle.clone();
                    let notified = close_handle.notify.notified();
                    tokio::pin!(notified);
                    tokio::select! {
                        chunk = async {
                            self.message_io
                                .as_mut()
                                .ok_or_else(|| ConnectionError::new("connection is not logged in", Some("ENOTCONNECTED".to_owned())))?
                                .next_message_chunk()
                                .await
                                .map_err(connection_transport_error)
                        } => chunk?,
                        _ = &mut notified => {
                            return Err(self.terminate_connection_after_close(ConnectionError::new(
                                "Connection closed before request completed.",
                                Some("ECLOSE".to_owned()),
                            )));
                        }
                        _ = &mut timer => {
                            return Err(ConnectionError::new(
                                "request timed out before the first response packet",
                                Some("ETIMEOUT".to_owned()),
                            ));
                        }
                    }
                }
            } else if let Some(cancellation) = cancellation {
                tokio::select! {
                    chunk = async {
                        self.message_io
                            .as_mut()
                            .ok_or_else(|| ConnectionError::new("connection is not logged in", Some("ENOTCONNECTED".to_owned())))?
                            .next_message_chunk()
                            .await
                            .map_err(connection_transport_error)
                    } => chunk?,
                    _ = cancellation.cancelled() => {
                        return Box::pin(self.cancel_active_request(terminal_error_message)).await;
                    }
                    _ = self.close_handle.notify.notified() => {
                        return Err(self.terminate_connection_after_close(ConnectionError::new(
                            "Connection closed before request completed.",
                            Some("ECLOSE".to_owned()),
                        )));
                    }
                }
            } else {
                let close_handle = self.close_handle.clone();
                let notified = close_handle.notify.notified();
                tokio::pin!(notified);
                tokio::select! {
                    chunk = async {
                        self.message_io
                            .as_mut()
                            .ok_or_else(|| ConnectionError::new(
                                "connection is not logged in",
                                Some("ENOTCONNECTED".to_owned()),
                            ))?
                            .next_message_chunk()
                            .await
                            .map_err(connection_transport_error)
                    } => chunk?,
                    _ = &mut notified => {
                        return Err(self.terminate_connection_after_close(ConnectionError::new(
                            "Connection closed before request completed.",
                            Some("ECLOSE".to_owned()),
                        )));
                    }
                }
            };
            awaiting_first_packet = None;
            match chunk {
                IncomingMessageChunkResult::Chunk(chunk) => {
                    let end_of_message = chunk.end_of_message;
                    let emitted = match parser.feed(&chunk.data) {
                        Ok(tokens) => tokens,
                        Err(error) if is_recoverable_stream_parse_error(&error) => Vec::new(),
                        Err(error) => {
                            return Err(ConnectionError::new(
                                format!("{terminal_error_message}: {error:?}"),
                                Some("ETOKEN".to_owned()),
                            ));
                        }
                    };
                    callback(self, &emitted)?;
                    if collect_tokens {
                        tokens.extend(emitted.iter().cloned());
                    }
                    if let Some(more) = emitted.iter().rev().find_map(|token| match token {
                        StreamToken::Control(Token::Done(done)) => Some(done.more),
                        _ => None,
                    }) {
                        has_more_results = Some(more);
                    }
                    if end_of_message {
                        parser.finish().map_err(|error| {
                            ConnectionError::new(
                                format!("{terminal_error_message}: {error:?}"),
                                Some("ETOKEN".to_owned()),
                            )
                        })?;
                        if has_more_results == Some(true) {
                            has_more_results = None;
                            parser = self.create_token_stream_parser();
                            continue;
                        }
                        return Ok(tokens);
                    }
                }
                IncomingMessageChunkResult::EndOfStream => {
                    return Err(ConnectionError::new(
                        "unexpected end of message stream",
                        Some("ETRANSPORT".to_owned()),
                    ));
                }
                IncomingMessageChunkResult::Paused => {
                    if pause_control.is_some() {
                        if !parser.is_paused() {
                            parser.pause();
                        }
                        continue;
                    }
                    return Err(ConnectionError::new(
                        "incoming message stream is paused",
                        Some("EPAUSED".to_owned()),
                    ));
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn read_response_tokens_incremental_for_request(
        &mut self,
        terminal_error_message: &str,
        cancellation: Option<&RequestCancellationHandle>,
        first_packet_deadline: Option<Instant>,
        request: &mut Request,
        row_collection_on_done: bool,
        lower_case_guids: bool,
        use_column_names: bool,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        let pause_control = RequestPauseControl::new(request.paused);
        let pause_listener_control = pause_control.clone();
        let resume_listener_control = pause_control.clone();
        let pause_listener = request.on_named("pause", move |_| {
            pause_listener_control.pause();
        });
        let resume_listener = request.on_named("resume", move |_| {
            resume_listener_control.resume();
        });
        let result = self
            .read_response_tokens_incremental_with_cancellation_and_callback_and_pause(
                terminal_error_message,
                cancellation,
                first_packet_deadline,
                Some(pause_control),
                |connection, emitted| {
                    connection.emit_stream_server_events(emitted)?;
                    dispatch_request_tokens(
                        request,
                        emitted.to_vec(),
                        row_collection_on_done,
                        lower_case_guids,
                        use_column_names,
                    );
                    Ok(())
                },
                true,
            )
            .await;
        request.remove_listener(pause_listener);
        request.remove_listener(resume_listener);
        result
    }

    async fn cancel_active_request(
        &mut self,
        _terminal_error_message: &str,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        let result = if self.config.cancel_timeout.is_zero() {
            self.cancel_active_request_inner().await
        } else {
            match timeout(
                self.config.cancel_timeout,
                self.cancel_active_request_inner(),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => {
                    let error = ConnectionError::new(
                        format!(
                            "Failed to cancel request in {}ms",
                            self.config.cancel_timeout.as_millis()
                        ),
                        Some("ETIMEOUT".to_owned()),
                    );
                    Err(self.terminate_connection_with_error(error))
                }
            }
        };
        match result {
            Ok(()) => Err(ConnectionError::new(
                "Canceled.",
                Some("ECANCEL".to_owned()),
            )),
            Err(error) => Err(error),
        }
    }

    async fn cancel_active_request_inner(&mut self) -> Result<(), ConnectionError> {
        let attention_result = {
            let io = self.message_io.as_mut().ok_or_else(|| {
                ConnectionError::new(
                    "connection is not logged in",
                    Some("ENOTCONNECTED".to_owned()),
                )
            })?;
            io.send_attention().await
        };
        if let Err(error) = attention_result {
            return Err(self.terminate_connection_with_error(connection_transport_error(error)));
        }
        loop {
            let response = self
                .message_io
                .as_mut()
                .ok_or_else(|| {
                    ConnectionError::new(
                        "connection is not logged in",
                        Some("ENOTCONNECTED".to_owned()),
                    )
                })?
                .next_message_chunk()
                .await
                .map_err(connection_transport_error)?;
            match response {
                IncomingMessageChunkResult::Chunk(chunk) if chunk.end_of_message => break,
                IncomingMessageChunkResult::Chunk(_) => {}
                IncomingMessageChunkResult::EndOfStream => {
                    return Err(self.terminate_connection_with_error(ConnectionError::new(
                        "unexpected end of message stream",
                        Some("ETRANSPORT".to_owned()),
                    )));
                }
                IncomingMessageChunkResult::Paused => {
                    return Err(self.terminate_connection_with_error(ConnectionError::new(
                        "incoming message stream is paused",
                        Some("EPAUSED".to_owned()),
                    )));
                }
            }
        }
        self.transition_to(ConnectionState::SentAttention);
        let tokens = self
            .read_response_tokens_incremental("attention acknowledgement parsing failed")
            .await?;
        if !attention_acknowledged(&tokens) {
            return Err(self.terminate_connection_with_error(ConnectionError::new(
                "attention acknowledgement was not received",
                Some("EATTENTION".to_owned()),
            )));
        }
        self.emit_stream_server_events(&tokens)?;
        self.transition_to(ConnectionState::LoggedIn);
        Ok(())
    }

    async fn acquire_request(
        &self,
        request: &mut Request,
    ) -> Result<crate::request_scheduler::RequestLease, ConnectionError> {
        match self.request_scheduler.acquire().await {
            Ok(lease) => Ok(lease),
            Err(error) => {
                request.fail(error.clone());
                Err(ConnectionError::new(error.message, error.code))
            }
        }
    }

    fn register_active_request(&self, request: &Request) -> ActiveRequestGuard {
        self.register_active_request_with_handle(request.cancellation_handle())
    }

    fn register_active_request_with_handle(
        &self,
        cancellation: RequestCancellationHandle,
    ) -> ActiveRequestGuard {
        let id = self
            .next_active_request_id
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);
        let end_pending = Arc::new(AtomicBool::new(false));
        let control = ActiveRequestControl {
            id,
            cancellation,
            end_pending: Arc::clone(&end_pending),
        };
        if let Ok(mut active_request) = self.active_request.lock() {
            *active_request = Some(control);
        }
        ActiveRequestGuard {
            slot: Arc::clone(&self.active_request),
            id,
            end_pending,
            listeners: Arc::clone(&self.listeners),
        }
    }

    pub async fn connect(&mut self) -> Result<(), ConnectionError> {
        if self.state != ConnectionState::Initialized {
            return Err(ConnectionError::new(
                format!(
                    "`.connect` can not be called on a Connection in `{:?}` state.",
                    self.state
                ),
                Some("EINVALIDSTATE".to_owned()),
            ));
        }
        self.transition_to(ConnectionState::Connecting);
        self.cur_transient_retry_count = 0;
        loop {
            let result = self.connect_attempt().await;
            match result {
                Ok(()) => return Ok(()),
                Err(error) if error.code.as_deref() == Some("ECLOSE") => {
                    self.message_io.take();
                    self.emit(ConnectionEvent::ConnectError(error.clone()));
                    self.transition_to(ConnectionState::Final);
                    self.emit(ConnectionEvent::End);
                    return Err(error);
                }
                Err(error) if error.code.as_deref() == Some("EREROUTE") => {
                    self.perform_re_routing();
                }
                Err(error)
                    if error.is_transient == Some(true)
                        && self.cur_transient_retry_count
                            < self.config.max_retries_on_transient_errors =>
                {
                    self.perform_transient_failure_retry().await?;
                }
                Err(error) => {
                    self.emit(ConnectionEvent::ConnectError(error.clone()));
                    self.message_io.take();
                    self.transition_to(ConnectionState::Final);
                    self.emit(ConnectionEvent::End);
                    return Err(error);
                }
            }
        }
    }

    fn perform_re_routing(&mut self) {
        self.reset_connection_attempt_state();
        self.transition_to(ConnectionState::Rerouting);
        self.emit(ConnectionEvent::Rerouting);
        self.transition_to(ConnectionState::Connecting);
    }

    async fn perform_transient_failure_retry(&mut self) -> Result<(), ConnectionError> {
        self.cur_transient_retry_count += 1;
        self.reset_connection_attempt_state();
        self.transition_to(ConnectionState::TransientFailureRetry);
        let sleep = tokio::time::sleep(self.config.connection_retry_interval);
        tokio::pin!(sleep);
        tokio::select! {
            _ = self.close_handle.notify.notified() => {
                let error = ConnectionError::new(
                    "Connection closed before the connection was established.",
                    Some("ECLOSE".to_owned()),
                );
                self.cleanup_connection();
                self.emit(ConnectionEvent::ConnectError(error.clone()));
                self.transition_to(ConnectionState::Final);
                self.emit(ConnectionEvent::End);
                return Err(error);
            }
            _ = &mut sleep => {}
        }
        self.emit(ConnectionEvent::Retry);
        self.transition_to(ConnectionState::Connecting);
        Ok(())
    }

    fn connection_target_error(
        &self,
        mut error: ConnectionError,
        server: &str,
        port: u16,
    ) -> ConnectionError {
        if self.routing.is_some() {
            error.message = format!(
                "Failed to connect to {server}:{port} (redirected from {}:{}) - {}",
                self.config.server, self.config.port, error.message
            );
        }
        error
    }

    fn connection_timeout_error(&self) -> ConnectionError {
        let origin = format!("{}:{}", self.config.server, self.config.port);
        let (server, port, routing_message) = match self.routing.as_ref() {
            Some(route) => (
                route.server.as_str(),
                route.port,
                format!(" (redirected from {origin})"),
            ),
            None => (self.config.server.as_str(), self.config.port, String::new()),
        };
        ConnectionError::new(
            format!(
                "Failed to connect to {server}:{port}{routing_message} in {}ms",
                self.config.connect_timeout.as_millis()
            ),
            Some("ETIMEOUT".to_owned()),
        )
    }

    async fn connect_attempt(&mut self) -> Result<(), ConnectionError> {
        let close_handle = self.close_handle.clone();
        let already_closed = close_handle.is_closed();
        let notified = close_handle.notify.notified();
        let attempt = async {
            if self.config.connect_timeout.is_zero() {
                self.connect_inner().await
            } else {
                match timeout(self.config.connect_timeout, self.connect_inner()).await {
                    Ok(result) => result,
                    Err(_) => Err(self.connection_timeout_error()),
                }
            }
        };
        tokio::pin!(attempt);
        tokio::pin!(notified);
        if already_closed {
            return Err(ConnectionError::new(
                "Connection closed before the connection was established.",
                Some("ECLOSE".to_owned()),
            ));
        }
        tokio::select! {
            biased;
            result = &mut attempt => result,
            _ = &mut notified => Err(ConnectionError::new(
                "Connection closed before the connection was established.",
                Some("ECLOSE".to_owned()),
            )),
        }
    }

    fn reset_connection_attempt_state(&mut self) {
        self.cleanup_connection();
        self.fed_auth_required = false;
        self.transaction_descriptor = vec![0; 8];
        self.transaction_depth = 0;
        self.database_collation = None;
    }

    fn cleanup_connection(&mut self) {
        self.message_io.take();
    }

    fn wrap_socket_error(&self, error: TransportError) -> ConnectionError {
        connection_transport_error(error)
    }

    async fn read_prelogin_response(
        &mut self,
        io: &mut ConnectionIo,
    ) -> Result<PreloginPayload, ConnectionError> {
        self.transition_to(ConnectionState::SentPrelogin);
        let response = io
            .read_message()
            .await
            .map_err(|error| self.wrap_socket_error(error))?;
        PreloginPayload::from_bytes(response.data)
            .map_err(|error| ConnectionError::new(error.0, Some("EPRELOGIN".to_owned())))
    }

    async fn send_prelogin(&mut self, io: &mut ConnectionIo) -> Result<(), ConnectionError> {
        let prelogin = PreloginPayload::new(PreloginOptions {
            encrypt: self.config.encrypt && !self.config.strict_tls,
            version: PreloginVersion {
                major: 0,
                minor: 0,
                build: 0,
                subbuild: 0,
            },
        })
        .map_err(|error| ConnectionError::new(error.0, Some("EPRELOGIN".to_owned())))?;
        self.emit_debug_payload("PreLogin");
        io.send_message(&Message {
            message_type: packet_type::PRELOGIN,
            reset_connection: false,
            ignore: false,
            data: prelogin.data,
        })
        .await
        .map_err(|error| self.wrap_socket_error(error))
    }

    #[allow(clippy::too_many_arguments)]
    async fn send_login7_packet(
        &mut self,
        io: &mut ConnectionIo,
        server: &str,
        routed: &Option<RoutingData>,
        user_name: Option<String>,
        password: Option<String>,
        sspi: Option<Vec<u8>>,
        fed_auth: Option<FedAuth>,
    ) -> Result<(bool, bool), ConnectionError> {
        let has_ntlm = sspi.is_some();
        let has_fed_auth = fed_auth.is_some();
        let mut login = Login7Payload::new(Login7Options {
            tds_version: versions()[self.config.tds_version.as_str()],
            packet_size: self.config.packet_size as u32,
            client_prog_ver: 0,
            client_pid: std::process::id(),
            connection_id: 0,
            client_time_zone: 0,
            client_lcid: 0x409,
        });
        login.user_name = user_name;
        login.password = password;
        login.server_name = Some(
            match routed.as_ref().and_then(|route| route.instance.as_deref()) {
                Some(instance) => format!("{server}\\{instance}"),
                None => server.to_owned(),
            },
        );
        login.database = self.config.database.clone();
        login.app_name = Some(self.config.app_name.clone());
        login.library_name = Some("Tedious".to_owned());
        login.hostname = self
            .config
            .workstation_id
            .clone()
            .or_else(|| std::env::var("HOSTNAME").ok())
            .or_else(|| gethostname::gethostname().into_string().ok());
        login.language = self.config.session.language.clone();
        login.client_id = Some(vec![1, 2, 3, 4, 5, 6]);
        login.init_db_fatal = !self.config.fallback_to_default_db;
        login.sspi = sspi;
        login.fed_auth = fed_auth;
        login.read_only_intent = self.config.read_only_intent;
        self.emit_debug_payload("Login7");
        let login_data = login
            .to_buffer()
            .map_err(|error| ConnectionError::new(error.0, Some("ELOGIN7".to_owned())))?;
        io.send_message(&Message {
            message_type: packet_type::LOGIN7,
            reset_connection: false,
            ignore: false,
            data: login_data,
        })
        .await
        .map_err(|error| self.wrap_socket_error(error))?;
        let access_token_login = matches!(
            self.config.authentication,
            Authentication::AzureActiveDirectoryAccessToken { .. }
        );
        self.transition_to(if has_ntlm {
            ConnectionState::SentLogin7WithNtlm
        } else if has_fed_auth && !access_token_login {
            ConnectionState::SentLogin7WithFedAuth
        } else {
            ConnectionState::SentLogin7WithStandardLogin
        });
        Ok((has_ntlm, has_fed_auth))
    }

    async fn send_initial_sql(
        &mut self,
        io: &mut ConnectionIo,
        negotiated: &PreloginPayload,
    ) -> Result<(), ConnectionError> {
        let initial_sql = self.get_initial_sql();
        self.transition_to(ConnectionState::LoggedInSendingInitialSql);
        let initial_payload = SqlBatchPayload {
            sql_text: initial_sql,
            transaction_descriptor: self.transaction_descriptor.clone(),
            tds_version: self.config.tds_version.clone(),
        };
        let buffers = initial_payload
            .buffers()
            .map_err(|error| ConnectionError::new(error, Some("EPAYLOAD".to_owned())))?;
        io.send_message_buffers(packet_type::SQL_BATCH, &buffers)
            .await
            .map_err(|error| self.wrap_socket_error(error))?;
        let initial_response = io
            .read_message()
            .await
            .map_err(|error| self.wrap_socket_error(error))?;
        let mut initial_parser = self.create_token_stream_parser();
        let initial_stream_tokens =
            initial_parser
                .feed(&initial_response.data)
                .map_err(|error| {
                    ConnectionError::new(
                        format!("initial SQL token stream failed: {error:?}"),
                        Some("EINITIALSQL".to_owned()),
                    )
                })?;
        initial_parser.finish().map_err(|error| {
            ConnectionError::new(
                format!("initial SQL token stream failed: {error:?}"),
                Some("EINITIALSQL".to_owned()),
            )
        })?;
        let close_handle = self.close_handle.clone();
        let mut initial_handler = InitialSqlTokenHandler::with_close_callback(move || {
            close_handle.close();
        });
        initial_handler.transaction_descriptors[0] = self.transaction_descriptor.clone();
        for token in &initial_stream_tokens {
            if let Err(error) = dispatch_stream_token(&mut initial_handler, token) {
                return Err(ConnectionError::new(
                    error.to_string(),
                    Some("ECLOSE".to_owned()),
                ));
            }
        }
        if let Some(transaction_descriptor) = initial_handler.transaction_descriptors.last() {
            self.transaction_descriptor = transaction_descriptor.clone();
        }
        self.transaction_depth = if initial_handler.in_transaction {
            initial_handler
                .transaction_descriptors
                .len()
                .saturating_sub(1)
        } else {
            0
        };
        let initial_tokens = initial_stream_tokens
            .iter()
            .filter_map(|token| match token {
                crate::token::stream::StreamToken::Control(token) => Some(token.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        if let Some(packet_size) = initial_handler
            .events
            .iter()
            .rev()
            .find_map(|event| match event {
                crate::token::handler::HandlerEvent::PacketSize(value) => Some(*value as usize),
                _ => None,
            })
        {
            io.set_packet_size(packet_size)
                .map_err(|error| self.wrap_socket_error(error))?;
        }
        self.emit_stream_server_events(&initial_stream_tokens)?;
        if let Some(error) = initial_tokens.iter().find_map(|token| match token {
            Token::Error(value) => Some(value),
            _ => None,
        }) {
            return Err(ConnectionError::new(
                error.message.clone(),
                Some("EINITIALSQL".to_owned()),
            ));
        }
        self.fed_auth_required = negotiated.fed_auth_required != 0;
        self.transition_to(ConnectionState::LoggedIn);
        Ok(())
    }

    async fn send_fed_auth_token_message(
        &mut self,
        io: &mut ConnectionIo,
        token: &str,
    ) -> Result<(), ConnectionError> {
        let data = encode_fed_auth_token_payload(token)?;
        io.send_message(&Message {
            message_type: packet_type::FEDAUTH_TOKEN,
            reset_connection: false,
            ignore: false,
            data,
        })
        .await
        .map_err(|error| self.wrap_socket_error(error))
    }

    async fn perform_sent_login7_with_standard_login(
        &mut self,
        io: &mut ConnectionIo,
        has_fed_auth: bool,
    ) -> Result<Vec<Token>, ConnectionError> {
        let login_response = io
            .read_message()
            .await
            .map_err(|error| self.wrap_socket_error(error))?;
        let tokens = parse_all_tokens(&login_response.data, &self.config.tds_version)?;
        validate_feature_ext_ack(&tokens, has_fed_auth)?;
        Ok(tokens)
    }

    async fn perform_sent_login7_with_ntlm_login(
        &mut self,
        io: &mut ConnectionIo,
        credentials: (String, String, Option<String>),
        has_fed_auth: bool,
    ) -> Result<Vec<Token>, ConnectionError> {
        let (user_name, password, configured_domain) = credentials;
        loop {
            let response = io
                .read_message()
                .await
                .map_err(|error| self.wrap_socket_error(error))?;
            let tokens = parse_all_tokens(&response.data, &self.config.tds_version)?;
            if tokens
                .iter()
                .any(|token| matches!(token, Token::LoginAck(_) | Token::FeatureExtAck(_)))
            {
                validate_feature_ext_ack(&tokens, has_fed_auth)?;
                return Ok(tokens);
            }
            let challenge = tokens
                .iter()
                .find_map(|token| match token {
                    Token::Sspi(value) => Some(value.ntlm_packet.clone()),
                    _ => None,
                })
                .ok_or_else(|| {
                    ConnectionError::new(
                        "NTLM challenge was not returned by SQL Server",
                        Some("ENTLMCHALLENGE".to_owned()),
                    )
                })?;
            let domain = configured_domain
                .clone()
                .unwrap_or_else(|| decode_ntlm_field(&challenge.domain));
            let client_nonce: [u8; 8] = rand::random();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| {
                    ConnectionError::new(
                        "system clock is before Unix epoch",
                        Some("ENTIME".to_owned()),
                    )
                })?;
            let response = create_ntlm_response(
                &domain,
                &user_name,
                &password,
                &challenge.nonce,
                &challenge.target,
                &client_nonce,
                now.as_millis() as i64,
            )
            .map_err(|error| ConnectionError::new(error.0, Some("ENTLM".to_owned())))?;
            io.send_message(&Message {
                message_type: packet_type::NTLMAUTH_PKT,
                reset_connection: false,
                ignore: false,
                data: response,
            })
            .await
            .map_err(|error| self.wrap_socket_error(error))?;
        }
    }

    async fn perform_sent_login7_with_fed_auth(
        &mut self,
        io: &mut ConnectionIo,
        has_fed_auth: bool,
    ) -> Result<Vec<Token>, ConnectionError> {
        let response = io
            .read_message()
            .await
            .map_err(|error| self.wrap_socket_error(error))?;
        let mut tokens = parse_all_tokens(&response.data, &self.config.tds_version)?;
        if tokens
            .iter()
            .any(|token| matches!(token, Token::LoginAck(_) | Token::FeatureExtAck(_)))
        {
            validate_feature_ext_ack(&tokens, has_fed_auth)?;
            return Ok(tokens);
        }
        let provider = self.token_provider.clone().ok_or_else(|| {
            ConnectionError::new(
                "federated token provider is not configured",
                Some("EFEDAUTH".to_owned()),
            )
        })?;
        if !tokens
            .iter()
            .any(|token| matches!(token, Token::FedAuthInfo(_)))
        {
            if let Some(error) = tokens.iter().find_map(|token| match token {
                Token::Error(value) => Some(value),
                _ => None,
            }) {
                return Err(ConnectionError::new(
                    error.message.clone(),
                    Some("EFEDAUTH".to_owned()),
                ));
            }
            return Err(ConnectionError::new(
                "SQL Server did not return federated authentication information",
                Some("EFEDAUTH".to_owned()),
            ));
        }
        let fed_auth_info = tokens.iter().find_map(|token| match token {
            Token::FedAuthInfo(value) => Some(value),
            _ => None,
        });
        let info = fed_auth_info.filter(|value| {
            value.sts_url.as_deref().is_some_and(|url| !url.is_empty())
                && value.spn.as_deref().is_some_and(|spn| !spn.is_empty())
        });
        let scope = info
            .and_then(|value| value.spn.as_deref())
            .and_then(|spn| fed_auth_scope(spn).ok())
            .ok_or_else(|| {
                ConnectionError::new(
                    "SQL Server returned incomplete federated authentication information",
                    Some("EFEDAUTH".to_owned()),
                )
            })?;
        let token = provider.get_token_for_scope(&scope).await?;
        if token.trim().is_empty() {
            return Err(ConnectionError::new(
                "federated token provider returned an empty token",
                Some("EFEDAUTH".to_owned()),
            ));
        }
        self.send_fed_auth_token_message(io, &token).await?;
        let response = io
            .read_message()
            .await
            .map_err(|error| self.wrap_socket_error(error))?;
        tokens = parse_all_tokens(&response.data, &self.config.tds_version)?;
        validate_feature_ext_ack(&tokens, has_fed_auth)?;
        Ok(tokens)
    }

    async fn perform_tls_negotiation(
        &mut self,
        io: ConnectionIo,
        negotiated: &PreloginPayload,
        server: &str,
    ) -> Result<ConnectionIo, ConnectionError> {
        if !self.config.strict_tls && !self.config.encrypt && matches!(negotiated.encryption, 1 | 3)
        {
            return Err(ConnectionError::new(
                "server requires TLS negotiation",
                Some("ETLSREQUIRED".to_owned()),
            ));
        }
        if !self.config.strict_tls && matches!(negotiated.encryption, 1 | 3) {
            let stream = match io {
                ConnectionIo::Plain(value) => value.into_inner(),
                ConnectionIo::Tls(_) | ConnectionIo::DirectTls(_) => {
                    return Err(ConnectionError::new(
                        "invalid TLS negotiation state",
                        Some("ETLSSTATE".to_owned()),
                    ));
                }
            };
            let tls = connect_tds_tls(
                stream,
                self.config.server_name.as_deref().unwrap_or(server),
                self.config.trust_server_certificate,
                self.config.packet_size,
            )
            .await
            .map_err(|error| self.wrap_socket_error(error))?;
            let io = ConnectionIo::Tls(
                MessageIo::new(tls, self.config.packet_size)
                    .map_err(|error| self.wrap_socket_error(error))?,
            );
            self.transition_to(ConnectionState::SentTlsSslNegotiation);
            return Ok(io);
        }
        Ok(io)
    }

    pub async fn connect_with_callback<F>(&mut self, callback: F) -> Result<(), ConnectionError>
    where
        F: FnOnce(Option<ConnectionError>) + Send + 'static,
    {
        let callback = Arc::new(Mutex::new(Some(callback)));
        let callback_for_event = Arc::clone(&callback);
        let listener_id = self.once_named("connect", move |event| {
            let error = match event {
                ConnectionEvent::Connect => None,
                ConnectionEvent::ConnectError(error) => Some(error.clone()),
                _ => return,
            };
            if let Some(callback) = callback_for_event
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take()
            {
                callback(error);
            }
        });
        let result = self.connect().await;
        self.remove_listener(listener_id);
        if let Some(callback) = callback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            callback(result.clone().err());
        }
        result
    }

    async fn connect_inner(&mut self) -> Result<(), ConnectionError> {
        self.config.validate()?;
        let integrated_auth = matches!(
            self.config.authentication,
            Authentication::AzureActiveDirectoryPassword { .. }
                | Authentication::AzureActiveDirectoryMsiVm { .. }
                | Authentication::AzureActiveDirectoryMsiAppService { .. }
                | Authentication::AzureActiveDirectoryServicePrincipalSecret { .. }
                | Authentication::AzureActiveDirectoryDefault { .. }
                | Authentication::AzureActiveDirectoryTokenCredential
        );
        if self.token_provider.is_none() {
            let provider: Option<Arc<dyn TokenProvider>> = match self.config.authentication.clone()
            {
                Authentication::AzureActiveDirectoryPassword {
                    client_id,
                    user_name,
                    password,
                    tenant_id,
                } => {
                    let tenant_id = tenant_id.unwrap_or_else(|| "common".to_owned());
                    let user_name = user_name.ok_or_else(|| {
                        ConnectionError::new(
                            "Azure Active Directory password authentication requires user_name",
                            Some("EAUTH".to_owned()),
                        )
                    })?;
                    let password = password.ok_or_else(|| {
                        ConnectionError::new(
                            "Azure Active Directory password authentication requires password",
                            Some("EAUTH".to_owned()),
                        )
                    })?;
                    Some(Arc::new(AzureUsernamePasswordTokenProvider::new(
                        tenant_id, client_id, user_name, password,
                    )))
                }
                Authentication::AzureActiveDirectoryMsiVm { client_id } => Some(Arc::new(
                    AzureManagedIdentityTokenProvider::virtual_machine(client_id),
                )),
                Authentication::AzureActiveDirectoryMsiAppService { client_id } => {
                    let endpoint = std::env::var("IDENTITY_ENDPOINT")
                        .or_else(|_| std::env::var("MSI_ENDPOINT"))
                        .map_err(|_| {
                            ConnectionError::new(
                                "Azure App Service identity endpoint is not configured",
                                Some("EAUTH".to_owned()),
                            )
                        })?;
                    let header = std::env::var("IDENTITY_HEADER")
                        .or_else(|_| std::env::var("MSI_SECRET"))
                        .unwrap_or_default();
                    Some(Arc::new(AzureManagedIdentityTokenProvider::app_service(
                        endpoint, header, client_id,
                    )))
                }
                Authentication::AzureActiveDirectoryServicePrincipalSecret {
                    client_id,
                    client_secret,
                    tenant_id,
                } => Some(Arc::new(AzureClientSecretTokenProvider::new(
                    tenant_id,
                    client_id,
                    client_secret,
                ))),
                Authentication::AzureActiveDirectoryDefault { client_id } => {
                    let provider = match client_id {
                        Some(value) => AzureDefaultTokenProvider::new().with_client_id(value),
                        None => AzureDefaultTokenProvider::new(),
                    };
                    Some(Arc::new(provider))
                }
                Authentication::AzureActiveDirectoryTokenCredential => {
                    if self.token_provider.is_none() {
                        return Err(ConnectionError::new(
                            "token credential is not configured",
                            Some("EAUTH".to_owned()),
                        ));
                    }
                    None
                }
                _ => None,
            };
            self.token_provider = provider;
        }
        if self.token_provider.is_some() && !integrated_auth {
            self.refresh_access_token().await?;
        }
        let routed = self.routing.clone();
        let server = routed
            .as_ref()
            .map_or_else(|| self.config.server.clone(), |route| route.server.clone());
        let port = match (&routed, &self.config.instance_name) {
            (Some(route), _) => route.port,
            (None, Some(instance)) => lookup_instance(
                &self.config.server,
                instance,
                self.config.connect_timeout.min(Duration::from_secs(2)),
                3,
            )
            .await
            .map_err(|error| ConnectionError::new(error.0, Some("EINSTLOOKUP".to_owned())))?,
            (None, None) => self.config.port,
        };
        let stream = if let Some(connector) = &self.destination_connector {
            match connector(server.clone(), port).await {
                Ok(stream) => Ok(stream),
                Err(error) => Err(ConnectorError::AllFailed(vec![error.to_string()])),
            }
        } else if let Some(connector) = &self.custom_connector {
            match connector().await {
                Ok(stream) => Ok(stream),
                Err(error) => Err(ConnectorError::AllFailed(vec![error.to_string()])),
            }
        } else if self.config.multi_subnet_failover {
            connect_in_parallel_with_options(
                &server,
                port,
                self.config.connect_timeout,
                Some(self.config.keep_alive_initial_delay),
                self.config.local_address.as_deref(),
            )
            .await
        } else {
            connect_in_sequence_with_options(
                &server,
                port,
                self.config.connect_timeout,
                Some(self.config.keep_alive_initial_delay),
                self.config.local_address.as_deref(),
            )
            .await
        }
        .map_err(|error| self.connection_target_error(connector_error(error), &server, port))?;
        let mut io = if self.config.strict_tls {
            let tls = connect_strict_tls(
                stream,
                self.config.server_name.as_deref().unwrap_or(&server),
                self.config.trust_server_certificate,
            )
            .await
            .map_err(connection_transport_error)?;
            ConnectionIo::DirectTls(Box::new(
                MessageIo::new(tls, self.config.packet_size).map_err(connection_transport_error)?,
            ))
        } else {
            ConnectionIo::Plain(
                MessageIo::new(stream, self.config.packet_size)
                    .map_err(connection_transport_error)?,
            )
        };
        self.configure_debug(&mut io);
        self.send_prelogin(&mut io).await?;
        let negotiated = self.read_prelogin_response(&mut io).await?;
        if negotiated.fed_auth_required != 0
            && !supports_required_fed_auth(&self.config.authentication)
        {
            return Err(ConnectionError::new(
                "server requires federated authentication",
                Some("EFEDAUTHREQUIRED".to_owned()),
            ));
        }
        io = self
            .perform_tls_negotiation(io, &negotiated, &server)
            .await?;
        self.configure_debug(&mut io);
        if self.config.strict_tls || matches!(negotiated.encryption, 1 | 3) {
            self.emit(ConnectionEvent::Secure);
        }
        let (user_name, password, sspi, ntlm_credentials, fed_auth) =
            match &self.config.authentication {
                Authentication::Sql {
                    user_name,
                    password,
                } => (
                    Some(user_name.clone()),
                    Some(password.clone()),
                    None,
                    None,
                    None,
                ),
                Authentication::Ntlm {
                    user_name,
                    password,
                    domain,
                } => (
                    None,
                    None,
                    Some(create_ntlm_request(domain.as_deref().unwrap_or(""), None)),
                    Some((user_name.clone(), password.clone(), domain.clone())),
                    None,
                ),
                Authentication::FedAuth { token }
                | Authentication::AzureActiveDirectoryAccessToken { token } => (
                    None,
                    None,
                    None,
                    None,
                    Some(FedAuth::SecurityToken {
                        echo: false,
                        token: token.clone(),
                    }),
                ),
                Authentication::AzureActiveDirectoryPassword { .. }
                | Authentication::AzureActiveDirectoryMsiVm { .. }
                | Authentication::AzureActiveDirectoryMsiAppService { .. }
                | Authentication::AzureActiveDirectoryServicePrincipalSecret { .. }
                | Authentication::AzureActiveDirectoryDefault { .. }
                | Authentication::AzureActiveDirectoryTokenCredential => (
                    None,
                    None,
                    None,
                    None,
                    Some(FedAuth::Adal {
                        echo: negotiated.fed_auth_required != 0,
                        integrated: true,
                    }),
                ),
            };
        let (_has_ntlm, has_fed_auth) = self
            .send_login7_packet(
                &mut io, &server, &routed, user_name, password, sspi, fed_auth,
            )
            .await?;
        let tokens = if let Some((user_name, password, configured_domain)) = ntlm_credentials {
            self.perform_sent_login7_with_ntlm_login(
                &mut io,
                (user_name, password, configured_domain),
                has_fed_auth,
            )
            .await?
        } else if integrated_auth {
            self.perform_sent_login7_with_fed_auth(&mut io, has_fed_auth)
                .await?
        } else {
            self.perform_sent_login7_with_standard_login(&mut io, has_fed_auth)
                .await?
        };
        if !tokens.iter().any(|token| matches!(token, Token::Error(_))) {
            self.config.tds_version = negotiated_tds_version(&tokens)?;
        }
        self.emit_server_events(&tokens)?;
        io.set_packet_size(self.config.packet_size)
            .map_err(connection_transport_error)?;
        if let Some(error) = tokens.iter().find_map(|token| match token {
            Token::Error(value) => Some(value),
            _ => None,
        }) {
            let mut result = ConnectionError::new(error.message.clone(), Some("ELOGIN".to_owned()));
            result.is_transient = Some(is_transient_error(error.number));
            return Err(result);
        }
        if let Some(route) = tokens.iter().find_map(|token| match token {
            Token::EnvChange(change) => match &change.kind {
                crate::token::parser::EnvChangeKind::Routing(route) => {
                    let (server, instance) = route.server_and_instance();
                    Some(RoutingData {
                        protocol: route.protocol,
                        server,
                        port: route.port,
                        instance,
                        old_value: route.old_value.clone(),
                    })
                }
                _ => None,
            },
            _ => None,
        }) {
            self.routing = Some(route);
            self.message_io.take();
            return Err(ConnectionError::new(
                "connection redirected by server",
                Some("EREROUTE".to_owned()),
            ));
        }
        self.send_initial_sql(&mut io, &negotiated).await?;
        self.message_io = Some(io);
        self.emit(ConnectionEvent::Connect);
        Ok(())
    }

    pub fn on(
        &mut self,
        listener: impl Fn(&ConnectionEvent) + Send + Sync + 'static,
    ) -> ConnectionListenerId {
        self.add_listener(None, listener, false)
    }

    pub fn on_named(
        &mut self,
        event_name: impl Into<String>,
        listener: impl Fn(&ConnectionEvent) + Send + Sync + 'static,
    ) -> ConnectionListenerId {
        self.add_listener(Some(event_name.into()), listener, false)
    }

    pub fn once(
        &mut self,
        listener: impl Fn(&ConnectionEvent) + Send + Sync + 'static,
    ) -> ConnectionListenerId {
        self.add_listener(None, listener, true)
    }

    pub fn once_named(
        &mut self,
        event_name: impl Into<String>,
        listener: impl Fn(&ConnectionEvent) + Send + Sync + 'static,
    ) -> ConnectionListenerId {
        self.add_listener(Some(event_name.into()), listener, true)
    }

    fn add_listener(
        &mut self,
        event_name: Option<String>,
        listener: impl Fn(&ConnectionEvent) + Send + Sync + 'static,
        once: bool,
    ) -> ConnectionListenerId {
        let id = self.next_listener_id;
        self.next_listener_id = self.next_listener_id.wrapping_add(1);
        if let Some(event_name) = event_name.as_deref() {
            self.emit(ConnectionEvent::NewListener {
                event_name: event_name.to_owned(),
                listener_id: id,
            });
        }
        self.listeners
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(ConnectionListenerEntry {
                id,
                event_name,
                once,
                fired: Arc::new(AtomicBool::new(false)),
                callback: Arc::new(listener),
            });
        id
    }

    pub fn off(&mut self, id: ConnectionListenerId) -> bool {
        self.remove_listener(id)
    }

    pub fn remove_listener(&mut self, id: ConnectionListenerId) -> bool {
        let mut listeners = self
            .listeners
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let length = listeners.len();
        let removed = listeners
            .iter()
            .find(|listener| listener.id == id)
            .map(|listener| (listener.id, listener.event_name.clone()));
        listeners.retain(|listener| listener.id != id);
        let changed = listeners.len() != length;
        drop(listeners);
        if let Some((listener_id, Some(event_name))) = removed {
            self.emit(ConnectionEvent::RemoveListener {
                event_name,
                listener_id,
            });
        }
        changed
    }

    pub fn listener_count(&self) -> usize {
        self.listeners
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }

    pub fn listener_count_named(&self, event_name: impl Into<String>) -> usize {
        let event_name = event_name.into();
        self.listeners
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .filter(|listener| listener.event_name.as_deref() == Some(event_name.as_str()))
            .count()
    }

    pub fn listeners(&self) -> Vec<ConnectionListenerId> {
        self.listeners
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .map(|listener| listener.id)
            .collect()
    }

    pub fn listeners_named(&self, event_name: impl Into<String>) -> Vec<ConnectionListenerId> {
        let event_name = event_name.into();
        self.listeners
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .filter(|listener| listener.event_name.as_deref() == Some(event_name.as_str()))
            .map(|listener| listener.id)
            .collect()
    }

    pub fn remove_all_named_listeners(&mut self, event_name: impl Into<String>) -> usize {
        let mut ids = self.listeners_named(event_name);
        ids.reverse();
        let count = ids.len();
        for id in ids {
            self.remove_listener(id);
        }
        count
    }

    pub fn request_scheduler(&self) -> Arc<RequestScheduler> {
        Arc::clone(&self.request_scheduler)
    }

    pub fn set_token_provider<P>(&mut self, provider: P)
    where
        P: TokenProvider + 'static,
    {
        self.token_provider = Some(Arc::new(provider));
    }

    pub fn set_key_store_provider<P>(&mut self, provider: P)
    where
        P: KeyStoreProvider + 'static,
    {
        self.key_store_provider = Some(Arc::new(provider));
    }

    async fn maybe_encrypt_request_parameters(
        &mut self,
        request: &mut Request,
    ) -> Result<(), ConnectionError> {
        let encrypt_parameters = crate::always_encrypted::utils::should_encrypt_parameters(
            request.statement_column_encryption_setting,
            self.config.column_encryption_setting,
        );
        let decrypt_results = crate::always_encrypted::utils::should_decrypt_results(
            request.statement_column_encryption_setting,
            self.config.column_encryption_setting,
        );
        if !decrypt_results {
            return Ok(());
        }
        if request.crypto_metadata_loaded {
            self.pending_result_crypto_metadata = request.crypto_metadata.clone();
            return Ok(());
        }
        let provider = self.key_store_provider.clone().ok_or_else(|| {
            ConnectionError::new(
                "column encryption is enabled but no key store provider is configured",
                Some("EAE_CONFIGURATION".to_owned()),
            )
        })?;
        let force_encrypt = self
            .force_encrypt
            .clone()
            .unwrap_or_else(|| request.force_encrypt_parameters().to_vec());
        if force_encrypt.len() != request.parameters.len() {
            return Err(ConnectionError::new(
                "force-encrypt settings do not match parameter count",
                Some("EAE_CONFIGURATION".to_owned()),
            ));
        }
        let metadata = self
            .describe_parameter_encryption(request, &force_encrypt)
            .await?;
        if !encrypt_parameters {
            request.crypto_metadata = Some(metadata.clone());
            request.crypto_metadata_loaded = true;
            self.pending_result_crypto_metadata = Some(metadata);
            return Ok(());
        }
        self.encrypt_request_parameters_with_metadata(
            request,
            metadata,
            provider.as_ref(),
            self.config.column_encryption_key_cache_ttl,
        )
        .await
    }

    pub fn set_token_credential<C>(&mut self, credential: C)
    where
        C: TokenCredential + 'static,
    {
        self.token_provider = Some(Arc::new(TokenCredentialAdapter::new(credential)));
        self.config.authentication = Authentication::AzureActiveDirectoryTokenCredential;
    }

    pub async fn refresh_access_token(&mut self) -> Result<(), ConnectionError> {
        let provider = self.token_provider.clone().ok_or_else(|| {
            ConnectionError::new(
                "no federated token provider configured",
                Some("EAUTH".to_owned()),
            )
        })?;
        let token = provider.get_token().await?;
        if token.trim().is_empty() {
            return Err(ConnectionError::new(
                "federated token provider returned an empty token",
                Some("EAUTH".to_owned()),
            ));
        }
        self.config.authentication = Authentication::FedAuth { token };
        Ok(())
    }

    pub fn emit(&self, event: ConnectionEvent) {
        let _ = Self::emit_listeners(&self.listeners, event);
    }

    pub fn emit_checked(&self, event: ConnectionEvent) -> Result<usize, EventEmitterError> {
        let is_error = matches!(event, ConnectionEvent::Error(_));
        let count = Self::emit_listeners(&self.listeners, event);
        if is_error && count == 0 {
            return Err(EventEmitterError::UnhandledError);
        }
        Ok(count)
    }

    fn emit_listeners(
        listeners: &Arc<std::sync::Mutex<Vec<ConnectionListenerEntry>>>,
        event: ConnectionEvent,
    ) -> usize {
        let entries = {
            let listeners = listeners
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            listeners
                .iter()
                .map(|entry| ConnectionListenerEntry {
                    id: entry.id,
                    event_name: entry.event_name.clone(),
                    once: entry.once,
                    fired: Arc::clone(&entry.fired),
                    callback: Arc::clone(&entry.callback),
                })
                .collect::<Vec<_>>()
        };
        let mut count = 0;
        for entry in entries {
            if entry
                .event_name
                .as_deref()
                .is_some_and(|name| name != connection_event_name(&event))
            {
                continue;
            }
            count += 1;
            if entry.once && entry.fired.swap(true, Ordering::AcqRel) {
                continue;
            }
            if entry.once {
                let removed = {
                    let mut listeners = listeners
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let length = listeners.len();
                    listeners.retain(|listener| listener.id != entry.id);
                    listeners.len() != length
                };
                if removed && let Some(event_name) = entry.event_name {
                    Self::emit_listeners(
                        listeners,
                        ConnectionEvent::RemoveListener {
                            event_name,
                            listener_id: entry.id,
                        },
                    );
                }
            }
            (entry.callback)(&event);
        }
        count
    }

    fn emit_stream_server_events(&mut self, tokens: &[StreamToken]) -> Result<(), ConnectionError> {
        if self.config.debug.token {
            for token in tokens {
                self.emit(ConnectionEvent::Debug(stream_token_debug_name(token)));
            }
        }
        let control_tokens = tokens
            .iter()
            .filter_map(|token| match token {
                StreamToken::Control(token) => Some(token.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        self.emit_server_events(&control_tokens)
    }

    fn emit_server_events(&mut self, tokens: &[Token]) -> Result<(), ConnectionError> {
        for token in tokens {
            if self.config.debug.token {
                self.emit(ConnectionEvent::Debug(format!(
                    "Token::{} [REDACTED]",
                    token_debug_name(token)
                )));
            }
            match token {
                Token::Info(value) => self.emit(ConnectionEvent::InfoMessage(value.clone())),
                Token::Error(value) => self.emit(ConnectionEvent::ErrorMessage(value.clone())),
                Token::EnvChange(value) => match &value.kind {
                    crate::token::parser::EnvChangeKind::Text {
                        new_value,
                        old_value: _,
                    } => match value.change_type {
                        1 => self.emit(ConnectionEvent::DatabaseChange(new_value.clone())),
                        2 => self.emit(ConnectionEvent::LanguageChange(new_value.clone())),
                        3 => self.emit(ConnectionEvent::CharsetChange(new_value.clone())),
                        13 => {
                            self.emit(ConnectionEvent::DatabaseMirroringPartner(new_value.clone()))
                        }
                        _ => {}
                    },
                    crate::token::parser::EnvChangeKind::PacketSize { new_value, .. } => {
                        let packet_size = usize::try_from(*new_value).map_err(|_| {
                            ConnectionError::new(
                                "server returned an invalid packet size",
                                Some("EPACKETSIZE".to_owned()),
                            )
                        })?;
                        if packet_size <= crate::packet::HEADER_LENGTH
                            || packet_size > u16::MAX as usize
                        {
                            return Err(ConnectionError::new(
                                "server returned an invalid packet size",
                                Some("EPACKETSIZE".to_owned()),
                            ));
                        }
                        if let Some(io) = &mut self.message_io {
                            io.set_packet_size(packet_size)
                                .map_err(connection_transport_error)?;
                        }
                        self.config.packet_size = packet_size;
                        self.emit(ConnectionEvent::PacketSizeChange(*new_value))
                    }
                    crate::token::parser::EnvChangeKind::Collation {
                        new_value: Some(collation),
                        ..
                    } if value.change_type == 7 => {
                        self.database_collation = Some(collation.clone());
                        self.emit(ConnectionEvent::CollationChange(collation.clone()));
                    }
                    crate::token::parser::EnvChangeKind::Binary { new_value, .. }
                        if value.change_type == 7 =>
                    {
                        if let Some(collation) =
                            crate::collation::Collation::from_buffer(new_value, 0)
                        {
                            self.database_collation = Some(collation.clone());
                            self.emit(ConnectionEvent::CollationChange(collation));
                        }
                    }
                    crate::token::parser::EnvChangeKind::BeginTransaction { new_value, .. }
                        if value.change_type == 8 =>
                    {
                        self.transaction_descriptor = new_value.clone();
                        self.transaction_depth = self.transaction_depth.saturating_add(1);
                    }
                    crate::token::parser::EnvChangeKind::Binary { new_value, .. }
                        if value.change_type == 8 =>
                    {
                        self.transaction_descriptor = new_value.clone();
                        self.transaction_depth = self.transaction_depth.saturating_add(1);
                    }
                    crate::token::parser::EnvChangeKind::CommitTransaction {
                        new_value, ..
                    } if value.change_type == 9 => {
                        self.transaction_descriptor = new_value.clone();
                        self.transaction_depth = self.transaction_depth.saturating_sub(1);
                    }
                    crate::token::parser::EnvChangeKind::Binary { new_value, .. }
                        if value.change_type == 9 =>
                    {
                        self.transaction_descriptor = new_value.clone();
                        self.transaction_depth = self.transaction_depth.saturating_sub(1);
                    }
                    crate::token::parser::EnvChangeKind::RollbackTransaction {
                        new_value, ..
                    } if value.change_type == 10 => {
                        self.transaction_descriptor = new_value.clone();
                        self.transaction_depth = 0;
                        self.emit(ConnectionEvent::RollbackTransaction);
                    }
                    crate::token::parser::EnvChangeKind::Binary { new_value, .. }
                        if value.change_type == 10 =>
                    {
                        self.transaction_descriptor = new_value.clone();
                        self.transaction_depth = 0;
                        self.emit(ConnectionEvent::RollbackTransaction);
                    }
                    crate::token::parser::EnvChangeKind::ResetConnection { .. }
                        if value.change_type == 18 =>
                    {
                        self.emit(ConnectionEvent::ResetConnection)
                    }
                    crate::token::parser::EnvChangeKind::Binary { .. }
                        if value.change_type == 18 =>
                    {
                        self.emit(ConnectionEvent::ResetConnection)
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        Ok(())
    }

    pub async fn exec_sql_batch(
        &mut self,
        sql_text: impl Into<String>,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        self.exec_sql_batch_internal(sql_text.into(), false).await
    }

    pub async fn exec_sql_batch_stream<F>(
        &mut self,
        sql_text: impl Into<String>,
        cancellation: Option<&RequestCancellationHandle>,
        mut callback: F,
    ) -> Result<(), ConnectionError>
    where
        F: FnMut(&[StreamToken], &StreamPauseHandle) -> Result<(), ConnectionError> + Send,
    {
        let sql_text = sql_text.into();
        let sent_client_request = self.state == ConnectionState::SentClientRequest;
        if self.config.read_only && !is_select_statement(&sql_text) {
            return Err(ConnectionError::new(
                "read-only connection accepts SELECT statements only",
                Some("EREADONLY".to_owned()),
            ));
        }
        if self.close_handle.is_closed() {
            return Err(ConnectionError::new(
                "Connection closed before request completed.",
                Some("ECLOSE".to_owned()),
            ));
        }
        if self.message_io.is_none() {
            return Err(ConnectionError::new(
                "connection is not logged in",
                Some("ENOTCONNECTED".to_owned()),
            ));
        }
        if self.state != ConnectionState::LoggedIn && !sent_client_request {
            return Err(ConnectionError::new(
                "connection is not logged in",
                Some("ENOTCONNECTED".to_owned()),
            ));
        }
        if !sent_client_request {
            self.transition_to(ConnectionState::SentClientRequest);
        }
        let payload = SqlBatchPayload {
            sql_text,
            transaction_descriptor: self.transaction_descriptor.clone(),
            tds_version: self.config.tds_version.clone(),
        };
        let buffers = payload
            .buffers()
            .map_err(|error| ConnectionError::new(error, Some("EPAYLOAD".to_owned())))?;
        let request_timeout = self.config.request_timeout;
        let deadline = (!request_timeout.is_zero()).then(|| Instant::now() + request_timeout);
        match self
            .send_message_buffers_controlled(
                packet_type::SQL_BATCH,
                false,
                &buffers,
                cancellation,
                deadline,
            )
            .await
        {
            ControlledSendOutcome::Completed(Ok(())) => {}
            ControlledSendOutcome::Completed(Err(error)) => {
                return Err(self.terminate_connection_with_error(connection_transport_error(error)));
            }
            ControlledSendOutcome::Closed => {
                return Err(self.terminate_connection_after_close(ConnectionError::new(
                    "Connection closed before request completed.",
                    Some("ECLOSE".to_owned()),
                )));
            }
            ControlledSendOutcome::TimedOut => {
                return Err(
                    self.terminate_connection_with_error(Self::request_timeout_error(
                        request_timeout,
                    )),
                );
            }
            ControlledSendOutcome::Canceled => {
                return Box::pin(self.cancel_active_request("token stream failed"))
                    .await
                    .map(|_| ());
            }
        }
        let pause_control = RequestPauseControl::new(false);
        let pause_handle = StreamPauseHandle {
            control: pause_control.clone(),
        };
        let result = self
            .read_response_tokens_incremental_with_cancellation_and_callback_and_pause(
                "token stream failed",
                cancellation,
                deadline,
                Some(pause_control),
                |connection, emitted| {
                    connection.emit_stream_server_events(emitted)?;
                    callback(emitted, &pause_handle)
                },
                false,
            )
            .await;
        match result {
            Ok(_) => {
                if sent_client_request || self.state == ConnectionState::SentClientRequest {
                    self.transition_to(ConnectionState::LoggedIn);
                }
                Ok(())
            }
            Err(error) if error.code.as_deref() == Some("ECANCEL") => {
                if sent_client_request || self.state == ConnectionState::SentClientRequest {
                    self.transition_to(ConnectionState::LoggedIn);
                }
                Err(error)
            }
            Err(error) => Err(self.terminate_connection_with_error(error)),
        }
    }

    async fn exec_sql_batch_internal_with_cancellation_for_request(
        &mut self,
        sql_text: String,
        reset_connection: bool,
        cancellation: Option<&RequestCancellationHandle>,
        request: &mut Request,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        let request_timeout = request.timeout.unwrap_or(self.config.request_timeout);
        let first_packet_deadline =
            (!request_timeout.is_zero()).then(|| Instant::now() + request_timeout);
        self.exec_sql_batch_internal_with_cancellation_impl(
            sql_text,
            reset_connection,
            cancellation,
            Some(request),
            first_packet_deadline,
        )
        .await
    }

    async fn exec_sql_batch_internal(
        &mut self,
        sql_text: String,
        reset_connection: bool,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        let request_timeout = self.config.request_timeout;
        let first_packet_deadline =
            (!request_timeout.is_zero()).then(|| Instant::now() + request_timeout);
        let result = self
            .exec_sql_batch_internal_with_cancellation_impl(
                sql_text,
                reset_connection,
                None,
                None,
                first_packet_deadline,
            )
            .await;
        match result {
            Err(error) if error.code.as_deref() == Some("ETIMEOUT") => {
                self.cancel_after_timeout().await;
                Err(Self::request_timeout_error(request_timeout))
            }
            result => result,
        }
    }

    async fn exec_sql_batch_internal_with_cancellation_impl(
        &mut self,
        sql_text: String,
        reset_connection: bool,
        cancellation: Option<&RequestCancellationHandle>,
        mut request: Option<&mut Request>,
        first_packet_deadline: Option<Instant>,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        let sent_client_request = self.state == ConnectionState::SentClientRequest;
        if self.config.read_only && !is_select_statement(&sql_text) {
            return Err(ConnectionError::new(
                "read-only connection accepts SELECT statements only",
                Some("EREADONLY".to_owned()),
            ));
        }
        if self.close_handle.is_closed() {
            return Err(ConnectionError::new(
                "Connection closed before request completed.",
                Some("ECLOSE".to_owned()),
            ));
        }
        if self.message_io.is_none() {
            return Err(ConnectionError::new(
                "connection is not logged in",
                Some("ENOTCONNECTED".to_owned()),
            ));
        }
        if self.state != ConnectionState::LoggedIn && !sent_client_request {
            return Err(ConnectionError::new(
                "connection is not logged in",
                Some("ENOTCONNECTED".to_owned()),
            ));
        }
        if !sent_client_request {
            self.transition_to(ConnectionState::SentClientRequest);
        }
        let payload = SqlBatchPayload {
            sql_text,
            transaction_descriptor: self.transaction_descriptor.clone(),
            tds_version: self.config.tds_version.clone(),
        };
        let buffers = payload
            .buffers()
            .map_err(|error| ConnectionError::new(error, Some("EPAYLOAD".to_owned())))?;
        self.emit_debug_payload("SQL Batch");
        let canceled_before_send =
            cancellation.is_some_and(RequestCancellationHandle::is_cancelled);
        let request_dispatch = request.is_some();
        let mut canceled_during_send = false;
        let send_result = if canceled_before_send {
            let io = self.message_io.as_mut().ok_or_else(|| {
                ConnectionError::new(
                    "connection is not logged in",
                    Some("ENOTCONNECTED".to_owned()),
                )
            })?;
            let data = buffers
                .iter()
                .flat_map(|buffer| buffer.iter().copied())
                .collect();
            ControlledSendOutcome::Completed(
                io.send_message(&Message {
                    message_type: packet_type::SQL_BATCH,
                    reset_connection,
                    ignore: true,
                    data,
                })
                .await,
            )
        } else {
            self.send_message_buffers_controlled(
                packet_type::SQL_BATCH,
                reset_connection,
                &buffers,
                cancellation,
                first_packet_deadline,
            )
            .await
        };
        match send_result {
            ControlledSendOutcome::Completed(Ok(())) => {}
            ControlledSendOutcome::Completed(Err(error)) => {
                return Err(self.terminate_connection_with_error(connection_transport_error(error)));
            }
            ControlledSendOutcome::Closed => {
                return Err(self.terminate_connection_after_close(ConnectionError::new(
                    "Connection closed before request completed.",
                    Some("ECLOSE".to_owned()),
                )));
            }
            ControlledSendOutcome::TimedOut => {
                return Err(self.terminate_connection_with_error(ConnectionError::new(
                    "request timed out before the first response packet",
                    Some("ETIMEOUT".to_owned()),
                )));
            }
            ControlledSendOutcome::Canceled => {
                canceled_during_send = true;
            }
        }
        let tokens = match if (canceled_before_send || canceled_during_send)
            && let Some(request) = request.as_deref_mut()
        {
            self.read_response_tokens_incremental_for_request(
                "token stream failed",
                None,
                first_packet_deadline,
                request,
                self.config.row_collection_on_done,
                self.config.lower_case_guids,
                self.config.use_column_names,
            )
            .await
        } else if let Some(request) = request.as_mut() {
            self.read_response_tokens_incremental_for_request(
                "token stream failed",
                cancellation,
                first_packet_deadline,
                request,
                self.config.row_collection_on_done,
                self.config.lower_case_guids,
                self.config.use_column_names,
            )
            .await
        } else {
            self.read_response_tokens_incremental_with_cancellation(
                "token stream failed",
                cancellation,
            )
            .await
        } {
            Ok(tokens) => tokens,
            Err(error) if matches!(error.code.as_deref(), Some("ECANCEL") | Some("ETIMEOUT")) => {
                return Err(error);
            }
            Err(error) => return Err(self.terminate_connection_with_error(error)),
        };
        if !request_dispatch {
            self.emit_stream_server_events(&tokens)?;
        }
        if canceled_before_send || canceled_during_send {
            if self.state == ConnectionState::SentClientRequest {
                self.transition_to(ConnectionState::LoggedIn);
            }
            return Err(ConnectionError::new(
                "Canceled.",
                Some("ECANCEL".to_owned()),
            ));
        }
        if sent_client_request || self.state == ConnectionState::SentClientRequest {
            self.transition_to(ConnectionState::LoggedIn);
        }
        Ok(tokens)
    }

    pub async fn reset(&mut self) -> Result<Vec<StreamToken>, ConnectionError> {
        if self.config.read_only {
            return Err(ConnectionError::new(
                "read-only connection accepts SELECT statements only",
                Some("EREADONLY".to_owned()),
            ));
        }
        let tokens = self
            .exec_sql_batch_internal(self.get_initial_sql(), true)
            .await?;
        if self.config.tds_version.as_str() < "7_2" {
            self.transaction_descriptor.clear();
            self.transaction_depth = 0;
        }
        Ok(tokens)
    }

    pub async fn exec_request(&mut self, request: &mut Request) -> Result<(), ConnectionError> {
        if self.close_handle.is_closed() || self.state == ConnectionState::Final {
            let error = RequestError::new(
                "Connection closed before request completed.",
                Some("ECLOSE".to_owned()),
            );
            request.fail(error.clone());
            return Err(ConnectionError::new(error.message, error.code));
        }
        let _lease = self.acquire_request(request).await?;
        request.set_row_collection_on_completion(self.config.row_collection_on_request_completion);
        if let Err(error) = request.begin() {
            request.fail(error.clone());
            return Err(ConnectionError::new(error.message, error.code));
        }
        let _active_request = self.register_active_request(request);
        self.transition_to(ConnectionState::SentClientRequest);
        let cancellation = request.cancellation_handle();
        let execution = self
            .exec_sql_batch_internal_with_cancellation_for_request(
                request.sql_text.clone(),
                false,
                Some(&cancellation),
                request,
            )
            .await;
        match execution {
            Ok(tokens) => {
                let _ = tokens;
                request.finish();
                Ok(())
            }
            Err(error) => {
                let error = if error.code.as_deref() == Some("ETIMEOUT") {
                    let timeout_error = Self::request_timeout_error(
                        request.timeout.unwrap_or(self.config.request_timeout),
                    );
                    request.error = Some(RequestError::new(
                        timeout_error.message.clone(),
                        timeout_error.code.clone(),
                    ));
                    request.cancel();
                    self.cancel_after_timeout().await;
                    timeout_error
                } else if error.code.as_deref() == Some("ECANCEL") {
                    request.cancel();
                    error
                } else {
                    error
                };
                let request_error = request.error.clone().unwrap_or_else(|| {
                    RequestError::new(error.message.clone(), error.code.clone())
                });
                request.fail(request_error);
                Err(error)
            }
        }
    }

    pub async fn exec_sql_batch_request(
        &mut self,
        request: &mut Request,
    ) -> Result<(), ConnectionError> {
        if self.close_handle.is_closed() || self.state == ConnectionState::Final {
            let error = RequestError::new(
                "Connection closed before request completed.",
                Some("ECLOSE".to_owned()),
            );
            request.fail(error.clone());
            return Err(ConnectionError::new(error.message, error.code));
        }
        let cancellation = request.cancellation_handle();
        let _lease = self.acquire_request(request).await?;
        let _active_request = self.register_active_request_with_handle(cancellation.clone());
        request.set_row_collection_on_completion(self.config.row_collection_on_request_completion);
        if let Err(error) = request.begin() {
            request.fail(error.clone());
            return Err(ConnectionError::new(error.message, error.code));
        }
        self.transition_to(ConnectionState::SentClientRequest);
        tokio::task::yield_now().await;
        let execution = self
            .exec_sql_batch_internal_with_cancellation_for_request(
                request.sql_text.clone(),
                false,
                Some(&cancellation),
                request,
            )
            .await;
        match execution {
            Ok(_) => {
                request.finish();
                Ok(())
            }
            Err(error) => {
                let error = if error.code.as_deref() == Some("ETIMEOUT") {
                    let timeout_error = Self::request_timeout_error(
                        request.timeout.unwrap_or(self.config.request_timeout),
                    );
                    request.error = Some(RequestError::new(
                        timeout_error.message.clone(),
                        timeout_error.code.clone(),
                    ));
                    request.cancel();
                    self.cancel_after_timeout().await;
                    timeout_error
                } else if error.code.as_deref() == Some("ECANCEL") {
                    request.cancel();
                    error
                } else {
                    error
                };
                request.fail(request.error.clone().unwrap_or_else(|| {
                    RequestError::new(error.message.clone(), error.code.clone())
                }));
                Err(error)
            }
        }
    }

    pub async fn exec_sql(&mut self, request: &mut Request) -> Result<(), ConnectionError> {
        if self.close_handle.is_closed() || self.state == ConnectionState::Final {
            let error = RequestError::new(
                "Connection closed before request completed.",
                Some("ECLOSE".to_owned()),
            );
            request.fail(error.clone());
            return Err(ConnectionError::new(error.message, error.code));
        }
        let _lease = self.acquire_request(request).await?;
        request.set_row_collection_on_completion(self.config.row_collection_on_request_completion);
        if let Err(error) = request.validate_parameters(self.database_collation.as_ref()) {
            request.fail_validation(error.clone());
            return Err(ConnectionError::new(error.message, error.code));
        }
        if self.config.read_only && !is_select_statement(&request.sql_text) {
            let error = ConnectionError::new(
                "read-only connection accepts SELECT statements only",
                Some("EREADONLY".to_owned()),
            );
            request.fail(RequestError::new(error.message.clone(), error.code.clone()));
            return Err(error);
        }
        if let Err(error) = self.maybe_encrypt_request_parameters(request).await {
            request.fail(RequestError::new(error.message.clone(), error.code.clone()));
            return Err(error);
        }
        request
            .begin()
            .map_err(|error| ConnectionError::new(error.message, error.code))?;
        let _active_request = self.register_active_request(request);
        let mut parameters = Vec::with_capacity(request.parameters.len() + 2);
        parameters.push(crate::payloads::RpcParameter {
            name: "statement".to_owned(),
            output: false,
            encoded: encode_nvarchar(&Value::Text(request.sql_text.clone()), 4000)
                .map_err(|error| ConnectionError::new(error.0, Some("EPARAM".to_owned())))?,
            encrypted_metadata: None,
        });
        if !request.parameters.is_empty() {
            let declaration = request
                .make_params_declaration()
                .map_err(|error| ConnectionError::new(error.0, Some("EPARAM".to_owned())))?;
            parameters.push(crate::payloads::RpcParameter {
                name: "params".to_owned(),
                output: false,
                encoded: encode_nvarchar(&Value::Text(declaration), 4000)
                    .map_err(|error| ConnectionError::new(error.0, Some("EPARAM".to_owned())))?,
                encrypted_metadata: None,
            });
        }
        parameters.extend(request.parameters.clone());
        let payload = RpcRequestPayload {
            procedure: Some("sp_executesql".to_owned()),
            procedure_id: None,
            parameters,
            transaction_descriptor: self.transaction_descriptor.clone(),
            tds_version: self.config.tds_version.clone(),
        };
        let request_timeout = request.timeout.unwrap_or(self.config.request_timeout);
        let execution = self
            .exec_rpc_tokens_with_request(&payload, request, request_timeout)
            .await;
        match execution {
            Ok(tokens) => {
                self.emit_stream_server_events(&tokens)?;
                dispatch_request_tokens(
                    request,
                    tokens,
                    self.config.row_collection_on_done,
                    self.config.lower_case_guids,
                    self.config.use_column_names,
                );
                self.transition_to(ConnectionState::LoggedIn);
                request.finish();
                Ok(())
            }
            Err(error) => {
                let error = if error.code.as_deref() == Some("ETIMEOUT") {
                    let timeout_error = Self::request_timeout_error(request_timeout);
                    request.error = Some(RequestError::new(
                        timeout_error.message.clone(),
                        timeout_error.code.clone(),
                    ));
                    request.cancel();
                    self.cancel_after_timeout().await;
                    timeout_error
                } else {
                    if error.code.as_deref() == Some("ECANCEL") {
                        request.cancel();
                    }
                    error
                };
                request.fail(request.error.clone().unwrap_or_else(|| {
                    RequestError::new(error.message.clone(), error.code.clone())
                }));
                Err(error)
            }
        }
    }

    pub async fn exec_rpc_request(&mut self, request: &mut Request) -> Result<(), ConnectionError> {
        let _lease = self.acquire_request(request).await?;
        request.set_row_collection_on_completion(self.config.row_collection_on_request_completion);
        if let Err(error) = request.validate_parameters(self.database_collation.as_ref()) {
            request.fail_validation(error.clone());
            return Err(ConnectionError::new(error.message, error.code));
        }
        if self.config.read_only && !is_select_statement(&request.sql_text) {
            let error = RequestError::new(
                "read-only connection accepts SELECT statements only",
                Some("EREADONLY".to_owned()),
            );
            request.fail(error.clone());
            return Err(ConnectionError::new(error.message, error.code));
        }
        if self.state != ConnectionState::LoggedIn {
            let error = RequestError::new(
                "connection is not logged in",
                Some("ENOTCONNECTED".to_owned()),
            );
            request.fail(error.clone());
            return Err(ConnectionError::new(error.message, error.code));
        }
        if let Err(error) = self.maybe_encrypt_request_parameters(request).await {
            request.fail(RequestError::new(error.message.clone(), error.code.clone()));
            return Err(error);
        }
        request
            .begin()
            .map_err(|error| ConnectionError::new(error.message, error.code))?;
        let _active_request = self.register_active_request(request);
        let payload = RpcRequestPayload {
            procedure: Some(request.sql_text.clone()),
            procedure_id: None,
            parameters: request.parameters.clone(),
            transaction_descriptor: self.transaction_descriptor.clone(),
            tds_version: self.config.tds_version.clone(),
        };
        let request_timeout = request.timeout.unwrap_or(self.config.request_timeout);
        let tokens = self
            .exec_rpc_tokens_with_request(&payload, request, request_timeout)
            .await;
        let tokens = match tokens {
            Ok(tokens) => tokens,
            Err(error) => {
                let error = if error.code.as_deref() == Some("ETIMEOUT") {
                    let timeout_error = Self::request_timeout_error(request_timeout);
                    request.error = Some(RequestError::new(
                        timeout_error.message.clone(),
                        timeout_error.code.clone(),
                    ));
                    request.cancel();
                    self.cancel_after_timeout().await;
                    timeout_error
                } else {
                    if error.code.as_deref() == Some("ECANCEL") {
                        request.cancel();
                    }
                    error
                };
                request.fail(request.error.clone().unwrap_or_else(|| {
                    RequestError::new(error.message.clone(), error.code.clone())
                }));
                return Err(error);
            }
        };
        self.emit_stream_server_events(&tokens)?;
        dispatch_request_tokens(
            request,
            tokens.clone(),
            self.config.row_collection_on_done,
            self.config.lower_case_guids,
            self.config.use_column_names,
        );
        self.transition_to(ConnectionState::LoggedIn);
        request.finish();
        Ok(())
    }

    pub async fn call_procedure(&mut self, request: &mut Request) -> Result<(), ConnectionError> {
        self.exec_rpc_request(request).await
    }

    pub async fn prepare(&mut self, request: &mut Request) -> Result<(), ConnectionError> {
        let _lease = self.acquire_request(request).await?;
        if self.config.read_only && !is_select_statement(&request.sql_text) {
            let error = ConnectionError::new(
                "read-only connection accepts SELECT statements only",
                Some("EREADONLY".to_owned()),
            );
            request.fail(RequestError::new(error.message.clone(), error.code.clone()));
            return Err(error);
        }
        request.begin_prepare();
        if self.state != ConnectionState::LoggedIn {
            let error = ConnectionError::new(
                "connection is not logged in",
                Some("ENOTCONNECTED".to_owned()),
            );
            request.fail(RequestError::new(error.message.clone(), error.code.clone()));
            return Err(error);
        }
        if let Err(error) = request.begin() {
            request.fail(error.clone());
            return Err(ConnectionError::new(error.message, error.code));
        }
        let _active_request = self.register_active_request(request);
        let declaration = match request.make_params_declaration() {
            Ok(declaration) => declaration,
            Err(error) => {
                let error = ConnectionError::new(error.0, Some("EPARAM".to_owned()));
                request
                    .fail_preparing(RequestError::new(error.message.clone(), error.code.clone()));
                return Err(error);
            }
        };
        let params_value = if request.parameters.is_empty() {
            Value::Null
        } else {
            Value::Text(declaration)
        };
        let handle = match crate::value_codec::encode_int(&Value::Null) {
            Ok(handle) => handle,
            Err(error) => {
                let error = ConnectionError::new(error.0, Some("EPARAM".to_owned()));
                request
                    .fail_preparing(RequestError::new(error.message.clone(), error.code.clone()));
                return Err(error);
            }
        };
        let params = match encode_nvarchar(&params_value, 4000) {
            Ok(params) => params,
            Err(error) => {
                let error = ConnectionError::new(error.0, Some("EPARAM".to_owned()));
                request
                    .fail_preparing(RequestError::new(error.message.clone(), error.code.clone()));
                return Err(error);
            }
        };
        let statement = match encode_nvarchar(&Value::Text(request.sql_text.clone()), 4000) {
            Ok(statement) => statement,
            Err(error) => {
                let error = ConnectionError::new(error.0, Some("EPARAM".to_owned()));
                request
                    .fail_preparing(RequestError::new(error.message.clone(), error.code.clone()));
                return Err(error);
            }
        };
        let parameters = vec![
            crate::payloads::RpcParameter {
                name: "handle".to_owned(),
                output: true,
                encoded: handle,
                encrypted_metadata: None,
            },
            crate::payloads::RpcParameter {
                name: "params".to_owned(),
                output: false,
                encoded: params,
                encrypted_metadata: None,
            },
            crate::payloads::RpcParameter {
                name: "stmt".to_owned(),
                output: false,
                encoded: statement,
                encrypted_metadata: None,
            },
        ];
        let payload = RpcRequestPayload {
            procedure: None,
            procedure_id: Some(
                crate::special_stored_procedure::SpecialStoredProcedure::SpPrepare.id(),
            ),
            parameters,
            transaction_descriptor: self.transaction_descriptor.clone(),
            tds_version: self.config.tds_version.clone(),
        };
        let result = self
            .exec_rpc_tokens_with_request(
                &payload,
                request,
                request.timeout.unwrap_or(self.config.request_timeout),
            )
            .await;
        match result {
            Ok(tokens) => {
                let has_error = tokens
                    .iter()
                    .any(|token| matches!(token, StreamToken::Control(Token::Error(_))));
                self.emit_stream_server_events(&tokens)?;
                dispatch_request_tokens(
                    request,
                    tokens.clone(),
                    self.config.row_collection_on_done,
                    self.config.lower_case_guids,
                    self.config.use_column_names,
                );
                self.transition_to(ConnectionState::LoggedIn);
                if let Some(unexpected) = tokens.iter().find_map(|token| match token {
                    StreamToken::Control(Token::ReturnValue(value)) if value.name != "handle" => {
                        Some(value.name.clone())
                    }
                    _ => None,
                }) {
                    let error = ConnectionError::new(
                        format!(
                            "Tedious > Unexpected output parameter {unexpected} from sp_prepare"
                        ),
                        Some("EPREPARE".to_owned()),
                    );
                    let request_error =
                        RequestError::new(error.message.clone(), error.code.clone());
                    request.error = Some(request_error.clone());
                    request.fail_preparing(request_error);
                    return Err(error);
                }
                if has_error {
                    let request_error = request.error.clone().unwrap_or_else(|| {
                        RequestError::new(
                            "sp_prepare returned an error",
                            Some("EREQUEST".to_owned()),
                        )
                    });
                    request.fail_preparing(request_error.clone());
                    let returned_error = request.errors.first().cloned().unwrap_or(request_error);
                    return Err(ConnectionError::new(
                        returned_error.message,
                        returned_error.code,
                    ));
                }
                let handle = tokens.iter().find_map(|token| match token {
                    StreamToken::Control(Token::ReturnValue(value)) if value.name == "handle" => {
                        match value.value {
                            Value::Integer(number) => i32::try_from(number).ok(),
                            _ => None,
                        }
                    }
                    _ => None,
                });
                if let Some(handle) = handle {
                    request.handle = Some(handle);
                }
                request.complete_prepare();
                Ok(())
            }
            Err(error) => {
                let error = if error.code.as_deref() == Some("ETIMEOUT") {
                    let timeout_error = Self::request_timeout_error(
                        request.timeout.unwrap_or(self.config.request_timeout),
                    );
                    request.error = Some(RequestError::new(
                        timeout_error.message.clone(),
                        timeout_error.code.clone(),
                    ));
                    request.cancel();
                    self.cancel_after_timeout().await;
                    timeout_error
                } else {
                    if error.code.as_deref() == Some("ECANCEL") {
                        request.cancel();
                    }
                    error
                };
                request
                    .fail_preparing(RequestError::new(error.message.clone(), error.code.clone()));
                Err(error)
            }
        }
    }

    pub async fn execute_prepared(&mut self, request: &mut Request) -> Result<(), ConnectionError> {
        self.execute_prepared_impl(request).await
    }

    async fn execute_prepared_impl(
        &mut self,
        request: &mut Request,
    ) -> Result<(), ConnectionError> {
        let _lease = self.acquire_request(request).await?;
        if let Err(error) = request.validate_parameters(self.database_collation.as_ref()) {
            request.fail_validation(error.clone());
            return Err(ConnectionError::new(error.message, error.code));
        }
        if self.config.read_only && !is_select_statement(&request.sql_text) {
            let error = RequestError::new(
                "read-only connection accepts SELECT statements only",
                Some("EREADONLY".to_owned()),
            );
            request.fail(error.clone());
            return Err(ConnectionError::new(error.message, error.code));
        }
        if self.state != ConnectionState::LoggedIn {
            let error = ConnectionError::new(
                "connection is not logged in",
                Some("ENOTCONNECTED".to_owned()),
            );
            request.fail(RequestError::new(error.message.clone(), error.code.clone()));
            return Err(error);
        }
        if let Err(error) = self.maybe_encrypt_request_parameters(request).await {
            request.fail(RequestError::new(error.message.clone(), error.code.clone()));
            return Err(error);
        }
        request.set_row_collection_on_completion(self.config.row_collection_on_request_completion);
        if let Err(error) = request.begin() {
            request.fail(error.clone());
            return Err(ConnectionError::new(error.message, error.code));
        }
        let _active_request = self.register_active_request(request);
        let mut parameters = Vec::with_capacity(request.parameters.len() + 1);
        parameters.push(crate::payloads::RpcParameter {
            name: String::new(),
            output: false,
            encoded: crate::value_codec::encode_int(
                &request
                    .handle
                    .map_or(Value::Null, |handle| Value::Integer(i64::from(handle))),
            )
            .map_err(|error| ConnectionError::new(error.0, Some("EPARAM".to_owned())))?,
            encrypted_metadata: None,
        });
        parameters.extend(request.parameters.clone());
        let payload = RpcRequestPayload {
            procedure: None,
            procedure_id: Some(
                crate::special_stored_procedure::SpecialStoredProcedure::SpExecute.id(),
            ),
            parameters,
            transaction_descriptor: self.transaction_descriptor.clone(),
            tds_version: self.config.tds_version.clone(),
        };
        let tokens = self
            .exec_rpc_tokens_with_request(
                &payload,
                request,
                request.timeout.unwrap_or(self.config.request_timeout),
            )
            .await;
        match tokens {
            Ok(tokens) => {
                self.emit_stream_server_events(&tokens)?;
                dispatch_request_tokens(
                    request,
                    tokens,
                    self.config.row_collection_on_done,
                    self.config.lower_case_guids,
                    self.config.use_column_names,
                );
                self.transition_to(ConnectionState::LoggedIn);
                request.finish();
                Ok(())
            }
            Err(error) => {
                let error = if error.code.as_deref() == Some("ETIMEOUT") {
                    let timeout_error = Self::request_timeout_error(
                        request.timeout.unwrap_or(self.config.request_timeout),
                    );
                    request.error = Some(RequestError::new(
                        timeout_error.message.clone(),
                        timeout_error.code.clone(),
                    ));
                    request.cancel();
                    self.cancel_after_timeout().await;
                    timeout_error
                } else {
                    if error.code.as_deref() == Some("ECANCEL") {
                        request.cancel();
                    }
                    error
                };
                request.fail(request.error.clone().unwrap_or_else(|| {
                    RequestError::new(error.message.clone(), error.code.clone())
                }));
                Err(error)
            }
        }
    }

    pub async fn execute_prepared_with_values(
        &mut self,
        request: &mut Request,
        values: &[(String, Value)],
    ) -> Result<(), ConnectionError> {
        let parameter_names = request
            .parameters
            .iter()
            .filter(|parameter| {
                request
                    .typed_parameters
                    .iter()
                    .any(|(name, _, _)| name == &parameter.name)
            })
            .map(|parameter| parameter.name.clone())
            .collect::<Vec<_>>();
        for name in parameter_names {
            let value = values
                .iter()
                .find(|(value_name, _)| value_name == &name)
                .map_or(Value::Null, |(_, value)| value.clone());
            if let Err(error) = request.set_typed_parameter_value(&name, value) {
                let error = ConnectionError::new(error.0, Some("EPARAM".to_owned()));
                request
                    .fail_validation(RequestError::new(error.message.clone(), error.code.clone()));
                return Err(error);
            }
        }
        self.execute_prepared_impl(request).await
    }

    pub async fn execute(
        &mut self,
        request: &mut Request,
        values: Option<&[(String, Value)]>,
    ) -> Result<(), ConnectionError> {
        if let Some(values) = values {
            self.execute_prepared_with_values(request, values).await
        } else {
            self.execute_prepared_with_values(request, &[]).await
        }
    }

    pub async fn unprepare(&mut self, request: &mut Request) -> Result<(), ConnectionError> {
        let _lease = self.acquire_request(request).await?;
        if matches!(
            request.state,
            crate::request::RequestState::Completed | crate::request::RequestState::Failed
        ) {
            request
                .begin()
                .map_err(|error| ConnectionError::new(error.message, error.code))?;
        }
        if self.config.read_only {
            let error = ConnectionError::new(
                "read-only connection does not allow unprepare",
                Some("EREADONLY".to_owned()),
            );
            request.fail(RequestError::new(error.message.clone(), error.code.clone()));
            return Err(error);
        }
        let handle = request.handle;
        if self.state != ConnectionState::LoggedIn {
            let error = ConnectionError::new(
                "connection is not logged in",
                Some("ENOTCONNECTED".to_owned()),
            );
            request.fail(RequestError::new(error.message.clone(), error.code.clone()));
            return Err(error);
        }
        let _active_request = self.register_active_request(request);
        let payload = RpcRequestPayload {
            procedure: None,
            procedure_id: Some(
                crate::special_stored_procedure::SpecialStoredProcedure::SpUnprepare.id(),
            ),
            parameters: vec![crate::payloads::RpcParameter {
                name: "handle".to_owned(),
                output: false,
                encoded: crate::value_codec::encode_int(
                    &handle.map_or(Value::Null, |handle| Value::Integer(i64::from(handle))),
                )
                .map_err(|error| ConnectionError::new(error.0, Some("EPARAM".to_owned())))?,
                encrypted_metadata: None,
            }],
            transaction_descriptor: self.transaction_descriptor.clone(),
            tds_version: self.config.tds_version.clone(),
        };
        let result = self
            .exec_rpc_tokens_with_request(
                &payload,
                request,
                request.timeout.unwrap_or(self.config.request_timeout),
            )
            .await;
        match result {
            Ok(tokens) => {
                self.emit_stream_server_events(&tokens)?;
                dispatch_request_tokens(
                    request,
                    tokens,
                    self.config.row_collection_on_done,
                    self.config.lower_case_guids,
                    self.config.use_column_names,
                );
                self.transition_to(ConnectionState::LoggedIn);
                request.finish();
                Ok(())
            }
            Err(error) => {
                let error = if error.code.as_deref() == Some("ETIMEOUT") {
                    let timeout_error = Self::request_timeout_error(
                        request.timeout.unwrap_or(self.config.request_timeout),
                    );
                    request.error = Some(RequestError::new(
                        timeout_error.message.clone(),
                        timeout_error.code.clone(),
                    ));
                    request.cancel();
                    self.cancel_after_timeout().await;
                    timeout_error
                } else {
                    if error.code.as_deref() == Some("ECANCEL") {
                        request.cancel();
                    }
                    error
                };
                request.fail(request.error.clone().unwrap_or_else(|| {
                    RequestError::new(error.message.clone(), error.code.clone())
                }));
                Err(error)
            }
        }
    }

    pub async fn exec_rpc_tokens(
        &mut self,
        payload: &RpcRequestPayload,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        self.exec_rpc_tokens_internal(payload, false).await
    }

    async fn exec_rpc_tokens_internal(
        &mut self,
        payload: &RpcRequestPayload,
        allow_read_only: bool,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        if self.config.read_only && !allow_read_only {
            return Err(ConnectionError::new(
                "read-only connection does not allow RPC requests",
                Some("EREADONLY".to_owned()),
            ));
        }
        self.exec_rpc_tokens_with_timeout(payload, Some(self.config.request_timeout))
            .await
    }

    async fn exec_rpc_tokens_with_timeout(
        &mut self,
        payload: &RpcRequestPayload,
        response_timeout: Option<Duration>,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        if self.close_handle.is_closed() {
            return Err(ConnectionError::new(
                "Connection closed before request completed.",
                Some("ECLOSE".to_owned()),
            ));
        }
        if self.state != ConnectionState::LoggedIn {
            return Err(ConnectionError::new(
                "connection is not logged in",
                Some("ENOTCONNECTED".to_owned()),
            ));
        }
        self.transition_to(ConnectionState::SentClientRequest);
        let buffers = payload
            .buffers()
            .map_err(|error| ConnectionError::new(error, Some("EPAYLOAD".to_owned())))?;
        self.emit_debug_payload("RPC Request");
        let first_packet_deadline = response_timeout
            .filter(|timeout| !timeout.is_zero())
            .map(|timeout| Instant::now() + timeout);
        match self
            .send_message_buffers_controlled(
                packet_type::RPC_REQUEST,
                false,
                &buffers,
                None,
                first_packet_deadline,
            )
            .await
        {
            ControlledSendOutcome::Completed(Ok(())) => {}
            ControlledSendOutcome::Completed(Err(error)) => {
                return Err(self.terminate_connection_with_error(connection_transport_error(error)));
            }
            ControlledSendOutcome::Closed => {
                return Err(self.terminate_connection_after_close(ConnectionError::new(
                    "Connection closed before request completed.",
                    Some("ECLOSE".to_owned()),
                )));
            }
            ControlledSendOutcome::TimedOut => {
                return Err(
                    self.terminate_connection_with_error(Self::request_timeout_error(
                        response_timeout.unwrap_or(Duration::ZERO),
                    )),
                );
            }
            ControlledSendOutcome::Canceled => {
                return Err(ConnectionError::new(
                    "Canceled.",
                    Some("ECANCEL".to_owned()),
                ));
            }
        }
        let tokens = self
            .read_response_tokens_incremental_with_cancellation_and_callback(
                "token stream failed",
                None,
                first_packet_deadline,
                |_, _| Ok(()),
            )
            .await;
        let tokens = match tokens {
            Ok(tokens) => tokens,
            Err(error) if error.code.as_deref() == Some("ETIMEOUT") => {
                self.cancel_after_timeout().await;
                return Err(Self::request_timeout_error(
                    response_timeout.unwrap_or(Duration::ZERO),
                ));
            }
            Err(error) => return Err(self.terminate_connection_with_error(error)),
        };
        self.emit_stream_server_events(&tokens)?;
        self.transition_to(ConnectionState::LoggedIn);
        Ok(tokens)
    }

    async fn exec_rpc_tokens_with_request(
        &mut self,
        payload: &RpcRequestPayload,
        request: &Request,
        response_timeout: Duration,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        if self.close_handle.is_closed() {
            return Err(ConnectionError::new(
                "Connection closed before request completed.",
                Some("ECLOSE".to_owned()),
            ));
        }
        if self.state != ConnectionState::LoggedIn {
            return Err(ConnectionError::new(
                "connection is not logged in",
                Some("ENOTCONNECTED".to_owned()),
            ));
        }
        let cancellation = request.cancellation_handle();
        if cancellation.is_cancelled() {
            return Err(ConnectionError::new(
                "Canceled.",
                Some("ECANCEL".to_owned()),
            ));
        }
        let first_packet_deadline =
            (!response_timeout.is_zero()).then(|| Instant::now() + response_timeout);
        self.transition_to(ConnectionState::SentClientRequest);
        let buffers = payload
            .buffers()
            .map_err(|error| ConnectionError::new(error, Some("EPAYLOAD".to_owned())))?;
        self.emit_debug_payload("RPC Request");
        let mut canceled_during_send = false;
        let send_result = match self
            .send_message_buffers_controlled(
                packet_type::RPC_REQUEST,
                false,
                &buffers,
                Some(&cancellation),
                first_packet_deadline,
            )
            .await
        {
            ControlledSendOutcome::Completed(result) => result,
            ControlledSendOutcome::Canceled => {
                canceled_during_send = true;
                Ok(())
            }
            ControlledSendOutcome::Closed => {
                return Err(self.terminate_connection_after_close(ConnectionError::new(
                    "Connection closed before request completed.",
                    Some("ECLOSE".to_owned()),
                )));
            }
            ControlledSendOutcome::TimedOut => {
                return Err(
                    self.terminate_connection_with_error(Self::request_timeout_error(
                        response_timeout,
                    )),
                );
            }
        };
        if let Err(error) = send_result {
            return Err(self.terminate_connection_with_error(connection_transport_error(error)));
        }
        let read: Pin<Box<dyn Future<Output = Result<Vec<StreamToken>, ConnectionError>> + '_>> =
            if canceled_during_send {
                Box::pin(self.read_response_tokens_incremental("token stream failed"))
            } else {
                Box::pin(
                    self.read_response_tokens_incremental_with_cancellation_and_callback(
                        "token stream failed",
                        Some(&cancellation),
                        first_packet_deadline,
                        |_, _| Ok(()),
                    ),
                )
            };
        let tokens = read.await?;
        self.emit_stream_server_events(&tokens)?;
        if canceled_during_send {
            return Err(ConnectionError::new(
                "Canceled.",
                Some("ECANCEL".to_owned()),
            ));
        }
        Ok(tokens)
    }

    pub async fn describe_parameter_encryption(
        &mut self,
        request: &Request,
        force_encrypt: &[bool],
    ) -> Result<Vec<crate::always_encrypted::types::CryptoMetadata>, ConnectionError> {
        let parameter_names = request
            .parameters
            .iter()
            .map(|parameter| parameter.name.trim_start_matches('@').to_owned())
            .collect::<Vec<_>>();
        if parameter_names.len() != force_encrypt.len() {
            return Err(ConnectionError::new(
                "parameter names and force-encrypt settings differ in length",
                Some("EAE_METADATA".to_owned()),
            ));
        }
        let declaration = request
            .make_params_declaration()
            .map_err(|error| ConnectionError::new(error.0, Some("EPARAM".to_owned())))?;
        let mut parameters = vec![crate::payloads::RpcParameter {
            name: "tsql".to_owned(),
            output: false,
            encoded: encode_nvarchar(&Value::Text(request.sql_text.clone()), 4000)
                .map_err(|error| ConnectionError::new(error.0, Some("EPARAM".to_owned())))?,
            encrypted_metadata: None,
        }];
        if !request.parameters.is_empty() {
            parameters.push(crate::payloads::RpcParameter {
                name: "params".to_owned(),
                output: false,
                encoded: encode_nvarchar(&Value::Text(declaration), 4000)
                    .map_err(|error| ConnectionError::new(error.0, Some("EPARAM".to_owned())))?,
                encrypted_metadata: None,
            });
        }
        let payload = RpcRequestPayload {
            procedure: Some("sp_describe_parameter_encryption".to_owned()),
            procedure_id: None,
            parameters,
            transaction_descriptor: self.transaction_descriptor.clone(),
            tds_version: self.config.tds_version.clone(),
        };
        let tokens = self.exec_rpc_tokens_internal(&payload, true).await?;
        let mut result_sets = Vec::new();
        let mut current = Vec::new();
        for token in tokens {
            match token {
                StreamToken::Row(row) | StreamToken::NbcRow(row) => current.push(row),
                StreamToken::Control(Token::Done(done)) if matches!(done.token, 0xFD..=0xFF) => {
                    result_sets.push(std::mem::take(&mut current));
                }
                _ => {}
            }
        }
        if !current.is_empty() {
            result_sets.push(current);
        }
        let cek_set = result_sets.iter().position(|rows| {
            rows.first().is_some_and(|row| {
                row.columns
                    .iter()
                    .any(|column| column.metadata.col_name.eq_ignore_ascii_case("database_id"))
            })
        });
        let parameter_set = result_sets.iter().position(|rows| {
            rows.first().is_some_and(|row| {
                row.columns.iter().any(|column| {
                    column
                        .metadata
                        .col_name
                        .eq_ignore_ascii_case("parameter_name")
                })
            })
        });
        let mut cek_rows = Vec::new();
        let mut parameter_rows = Vec::new();
        for (set_index, rows) in result_sets.iter().enumerate() {
            for row in rows {
                if Some(set_index) == cek_set {
                    if row.columns.len() < 9 {
                        return Err(ConnectionError::new(
                            "invalid first parameter-encryption result set",
                            Some("EAE_METADATA".to_owned()),
                        ));
                    }
                    cek_rows.push(CekMetadataRow {
                        key_ordinal: value_usize(&row.columns[0].value)?,
                        db_id: value_u32(&row.columns[1].value)?,
                        key_id: value_u32(&row.columns[2].value)?,
                        key_version: value_u32(&row.columns[3].value)?,
                        md_version: value_binary(&row.columns[4].value).map_err(|error| {
                            ConnectionError::new(
                                format!("{error} in first result column 4"),
                                Some("EAE_METADATA".to_owned()),
                            )
                        })?,
                        encrypted_key: value_binary(&row.columns[5].value).map_err(|error| {
                            ConnectionError::new(
                                format!("{error} in first result column 5"),
                                Some("EAE_METADATA".to_owned()),
                            )
                        })?,
                        provider_name: value_text(&row.columns[6].value)?,
                        key_path: value_text(&row.columns[7].value)?,
                        algorithm_name: value_text(&row.columns[8].value)?,
                    });
                } else if Some(set_index) == parameter_set {
                    if row.columns.len() < 6 {
                        return Err(ConnectionError::new(
                            "invalid second parameter-encryption result set",
                            Some("EAE_METADATA".to_owned()),
                        ));
                    }
                    parameter_rows.push(ParameterEncryptionMetadataRow {
                        parameter_ordinal: value_parameter_ordinal(&row.columns[0].value)?,
                        parameter_name: value_text(&row.columns[1].value)?,
                        cipher_algorithm_id: value_u32(&row.columns[2].value)? as u8,
                        encryption_type: value_u32(&row.columns[3].value)? as u8,
                        cek_ordinal: value_usize(&row.columns[4].value)?,
                        normalization_rule_version: vec![value_u8(&row.columns[5].value)?],
                    });
                }
            }
        }
        if result_sets.len() >= 2 && result_sets.iter().all(Vec::is_empty) {
            if force_encrypt.iter().any(|value| *value) {
                return Err(ConnectionError::new(
                    "parameter encryption metadata is missing for a forced-encryption parameter",
                    Some("EAE_METADATA".to_owned()),
                ));
            }
            return Ok(parameter_names
                .iter()
                .map(|_| plaintext_parameter_encryption_metadata())
                .collect());
        }
        build_parameter_encryption_metadata(
            &cek_rows,
            &parameter_rows,
            &parameter_names,
            force_encrypt,
        )
        .map_err(|error| ConnectionError::new(error, Some("EAE_METADATA".to_owned())))
    }

    pub async fn encrypt_request_parameters(
        &mut self,
        request: &mut Request,
        force_encrypt: &[bool],
        provider: &dyn KeyStoreProvider,
        cache_ttl: Duration,
    ) -> Result<(), ConnectionError> {
        let metadata = self
            .describe_parameter_encryption(request, force_encrypt)
            .await?;
        self.encrypt_request_parameters_with_metadata(request, metadata, provider, cache_ttl)
            .await
    }

    async fn encrypt_request_parameters_with_metadata(
        &mut self,
        request: &mut Request,
        mut metadata: Vec<crate::always_encrypted::types::CryptoMetadata>,
        provider: &dyn KeyStoreProvider,
        cache_ttl: Duration,
    ) -> Result<(), ConnectionError> {
        if metadata.len() != request.parameters.len() {
            return Err(ConnectionError::new(
                "parameter encryption metadata count does not match request parameters",
                Some("EAE_METADATA".to_owned()),
            ));
        }
        let mut result_metadata = Vec::with_capacity(metadata.len());
        for (parameter, mut parameter_metadata) in
            request.parameters.iter_mut().zip(metadata.drain(..))
        {
            if parameter_metadata.encryption_type == SqlServerEncryptionType::PlainText {
                result_metadata.push(parameter_metadata);
                continue;
            }
            let server_name = self
                .config
                .trusted_server_name_ae
                .as_deref()
                .ok_or_else(|| {
                    ConnectionError::new(
                        "Server name should not be null in EncryptWithKey",
                        Some("EAE_CONFIGURATION".to_owned()),
                    )
                })?;
            let base_type_info = parameter.encoded.type_info.clone();
            parameter.encoded = encrypt_encoded_parameter(
                &parameter.encoded,
                &mut parameter_metadata,
                server_name,
                provider,
                cache_ttl,
            )
            .map_err(|error| ConnectionError::new(error, Some("EAE_ENCRYPT".to_owned())))?;
            let key_info = parameter_metadata
                .encryption_key_info
                .as_ref()
                .ok_or_else(|| {
                    ConnectionError::new(
                        "encrypted parameter key metadata is missing",
                        Some("EAE_ENCRYPT".to_owned()),
                    )
                })?;
            let encryption_type = match parameter_metadata.encryption_type {
                SqlServerEncryptionType::Deterministic => 1,
                SqlServerEncryptionType::Randomized => 2,
                SqlServerEncryptionType::PlainText => 0,
            };
            parameter.encrypted_metadata = Some(crate::payloads::EncryptedParameterMetadata {
                base_type_info,
                algorithm_id: parameter_metadata.cipher_algorithm_id,
                encryption_type,
                normalization_rule_version: parameter_metadata
                    .normalization_rule_version
                    .first()
                    .copied()
                    .unwrap_or(1),
                database_id: key_info.db_id,
                cek_id: key_info.key_id,
                cek_version: key_info.key_version,
                cek_md_version: metadata_version_u64(&key_info.md_version),
            });
            result_metadata.push(parameter_metadata);
        }
        request.crypto_metadata = Some(result_metadata.clone());
        request.crypto_metadata_loaded = true;
        self.pending_result_crypto_metadata = Some(result_metadata);
        Ok(())
    }

    pub fn set_force_encrypt(&mut self, force_encrypt: Vec<bool>) {
        self.force_encrypt = Some(force_encrypt);
    }

    pub async fn exec_sql_with_force_encrypt(
        &mut self,
        request: &mut Request,
        force_encrypt: &[bool],
    ) -> Result<(), ConnectionError> {
        if force_encrypt.len() != request.parameters.len() {
            return Err(ConnectionError::new(
                "force-encrypt settings do not match parameter count",
                Some("EAE_CONFIGURATION".to_owned()),
            ));
        }
        self.force_encrypt = Some(force_encrypt.to_vec());
        let result = self.exec_sql(request).await;
        self.force_encrypt = None;
        result
    }

    pub async fn exec_bulk_load(
        &mut self,
        payload: &BulkLoadPayload,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        if self.config.read_only {
            return Err(ConnectionError::new(
                "read-only connection accepts SELECT statements only",
                Some("EREADONLY".to_owned()),
            ));
        }
        if self.state != ConnectionState::LoggedIn {
            return Err(ConnectionError::new(
                "connection is not logged in",
                Some("ENOTCONNECTED".to_owned()),
            ));
        }
        if self.close_handle.is_closed() {
            return Err(ConnectionError::new(
                "Connection closed before request completed.",
                Some("ECLOSE".to_owned()),
            ));
        }
        let mut request = Request::new(String::new(), |_| {});
        let _lease = self.acquire_request(&mut request).await?;
        if let Err(error) = request.begin() {
            let connection_error = ConnectionError::new(error.message.clone(), error.code.clone());
            request.fail(error);
            return Err(connection_error);
        }
        let _active_request = self.register_active_request(&request);
        let cancellation = request.cancellation_handle();
        let request_timeout = self.config.request_timeout;
        let first_packet_deadline =
            (!request_timeout.is_zero()).then(|| Instant::now() + request_timeout);
        let buffers = match payload
            .buffers()
            .map_err(|error| ConnectionError::new(error, Some("EPAYLOAD".to_owned())))
        {
            Ok(buffers) => buffers,
            Err(error) => {
                request.fail(RequestError::new(error.message.clone(), error.code.clone()));
                return Err(error);
            }
        };
        self.transition_to(ConnectionState::SentClientRequest);
        self.emit_debug_payload("Bulk Load");
        let mut canceled_during_send = false;
        match self
            .send_message_buffers_controlled(
                packet_type::BULK_LOAD,
                false,
                &buffers,
                Some(&cancellation),
                first_packet_deadline,
            )
            .await
        {
            ControlledSendOutcome::Completed(Ok(())) => {}
            ControlledSendOutcome::Completed(Err(error)) => {
                let error = self.terminate_connection_with_error(connection_transport_error(error));
                request.fail(RequestError::new(error.message.clone(), error.code.clone()));
                return Err(error);
            }
            ControlledSendOutcome::Closed => {
                let error = self.terminate_connection_after_close(ConnectionError::new(
                    "Connection closed before request completed.",
                    Some("ECLOSE".to_owned()),
                ));
                request.fail(RequestError::new(error.message.clone(), error.code.clone()));
                return Err(error);
            }
            ControlledSendOutcome::TimedOut => {
                let error = Self::request_timeout_error(request_timeout);
                request.error = Some(RequestError::new(error.message.clone(), error.code.clone()));
                request.cancel();
                self.cancel_after_timeout().await;
                request.fail(request.error.clone().unwrap_or_else(|| {
                    RequestError::new(error.message.clone(), error.code.clone())
                }));
                return Err(error);
            }
            ControlledSendOutcome::Canceled => canceled_during_send = true,
        }
        let tokens = match if canceled_during_send {
            self.read_response_tokens_incremental("token stream failed")
                .await
        } else {
            self.read_response_tokens_incremental_with_cancellation_and_callback(
                "token stream failed",
                Some(&cancellation),
                first_packet_deadline,
                |_, _| Ok(()),
            )
            .await
        } {
            Ok(tokens) => tokens,
            Err(error) if error.code.as_deref() == Some("ETIMEOUT") => {
                let error = Self::request_timeout_error(request_timeout);
                request.error = Some(RequestError::new(error.message.clone(), error.code.clone()));
                request.cancel();
                self.cancel_after_timeout().await;
                request.fail(request.error.clone().unwrap_or_else(|| {
                    RequestError::new(error.message.clone(), error.code.clone())
                }));
                return Err(error);
            }
            Err(error) if error.code.as_deref() == Some("ECANCEL") => {
                request.cancel();
                request.fail(RequestError::new(error.message.clone(), error.code.clone()));
                return Err(error);
            }
            Err(error) => {
                let error = self.terminate_connection_with_error(error);
                request.fail(RequestError::new(error.message.clone(), error.code.clone()));
                return Err(error);
            }
        };
        if let Some(error) = bulk_load_response_error(&tokens) {
            self.transition_to(ConnectionState::LoggedIn);
            request.fail(RequestError::new(error.message.clone(), error.code.clone()));
            return Err(error);
        }
        if let Err(error) = self.emit_stream_server_events(&tokens) {
            request.fail(RequestError::new(error.message.clone(), error.code.clone()));
            return Err(error);
        }
        self.transition_to(ConnectionState::LoggedIn);
        if canceled_during_send {
            let error = ConnectionError::new("Canceled.", Some("ECANCEL".to_owned()));
            request.cancel();
            request.fail(RequestError::new(error.message.clone(), error.code.clone()));
            return Err(error);
        }
        request.finish();
        Ok(tokens)
    }

    async fn exec_bulk_load_parts<I>(
        &mut self,
        parts: I,
        cancellation: &BulkLoadCancellationHandle,
    ) -> Result<Vec<StreamToken>, ConnectionError>
    where
        I: IntoIterator<Item = Result<Vec<u8>, TransportError>>,
    {
        if self.config.read_only {
            return Err(ConnectionError::new(
                "read-only connection accepts SELECT statements only",
                Some("EREADONLY".to_owned()),
            ));
        }
        if self.state != ConnectionState::LoggedIn {
            return Err(ConnectionError::new(
                "connection is not logged in",
                Some("ENOTCONNECTED".to_owned()),
            ));
        }
        self.transition_to(ConnectionState::SentClientRequest);
        let close_handle = self.close_handle.clone();
        let notified = close_handle.notify.notified();
        tokio::pin!(notified);
        let send_result = tokio::select! {
            result = async {
                let io = self.message_io.as_mut().ok_or_else(|| {
                    TransportError::InvalidPacket("connection is not logged in".to_owned())
                })?;
                io.send_message_parts_with_cancellation(packet_type::BULK_LOAD, parts, cancellation).await
            }
            => result,
            _ = &mut notified => {
                return Err(self.terminate_connection_with_error(ConnectionError::new(
                    "Connection closed before request completed.",
                    Some("ECLOSE".to_owned()),
                )));
            }
        };
        let canceled_during_send = match send_result {
            Ok(canceled) => canceled,
            Err(error) => {
                return Err(self.terminate_connection_with_error(connection_transport_error(error)));
            }
        };
        let tokens = match if canceled_during_send {
            self.read_response_tokens_after_bulk_cancellation("token stream failed")
                .await
        } else {
            self.read_response_tokens_incremental_with_bulk_cancellation(
                "token stream failed",
                cancellation,
            )
            .await
        } {
            Ok(tokens) => tokens,
            Err(error) => return Err(self.terminate_connection_with_error(error)),
        };
        if let Some(error) = bulk_load_response_error(&tokens) {
            self.transition_to(ConnectionState::LoggedIn);
            return Err(error);
        }
        self.emit_stream_server_events(&tokens)?;
        self.transition_to(ConnectionState::LoggedIn);
        if canceled_during_send {
            return Err(ConnectionError::new(
                "Canceled.",
                Some("ECANCEL".to_owned()),
            ));
        }
        Ok(tokens)
    }

    async fn exec_bulk_load_request(
        &mut self,
        bulk_load: &BulkLoad,
        payload: &BulkLoadPayload,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        let mut request = Request::new(bulk_load.get_bulk_insert_sql(), |_| {});
        let _lease = self.acquire_request(&mut request).await?;
        let _active_request = self.register_active_request(&request);
        request.set_row_collection_on_completion(self.config.row_collection_on_request_completion);
        if let Err(error) = request.begin() {
            request.fail(error.clone());
            return Err(ConnectionError::new(error.message, error.code));
        }
        let request_timeout = bulk_load.timeout.unwrap_or(self.config.request_timeout);
        request.timeout = Some(request_timeout);
        let request_cancellation = request.cancellation_handle();
        let cancellation = bulk_load.cancellation_handle();
        let result = {
            let execution = async {
                self.exec_bulk_load_preamble_with_request(
                    bulk_load,
                    &mut request,
                    &request_cancellation,
                    &cancellation,
                )
                .await?;
                let payload_execution = self.exec_bulk_load_cancellable(bulk_load, payload);
                tokio::pin!(payload_execution);
                tokio::select! {
                    result = &mut payload_execution => result,
                    _ = request_cancellation.cancelled() => {
                        cancellation.cancel();
                        payload_execution.await
                    }
                }
            };
            tokio::pin!(execution);
            if request_timeout.is_zero() {
                execution.await
            } else {
                tokio::select! {
                    biased;
                    result = &mut execution => result,
                    _ = tokio::time::sleep(request_timeout) => {
                        cancellation.cancel();
                        request_cancellation.cancel();
                        let _ = execution.await;
                        Err(Self::request_timeout_error(request_timeout))
                    }
                }
            }
        };
        drop(_active_request);
        drop(_lease);
        match result {
            Ok(tokens) => {
                request.finish();
                Ok(tokens)
            }
            Err(error) => {
                request.fail(RequestError::new(error.message.clone(), error.code.clone()));
                Err(error)
            }
        }
    }

    async fn exec_bulk_load_cancellable(
        &mut self,
        bulk_load: &BulkLoad,
        payload: &BulkLoadPayload,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        if bulk_load.canceled {
            return Err(ConnectionError::new(
                "Canceled.",
                Some("ECANCEL".to_owned()),
            ));
        }
        self.transition_to(ConnectionState::SentClientRequest);
        let buffers = payload
            .buffers()
            .map_err(|error| ConnectionError::new(error, Some("EPAYLOAD".to_owned())))?;
        let cancellation = bulk_load.cancellation_handle();
        let close_handle = self.close_handle.clone();
        let notified = close_handle.notify.notified();
        tokio::pin!(notified);
        let send_result = tokio::select! {
            result = async {
                let io = self.message_io.as_mut().ok_or_else(|| TransportError::InvalidPacket("connection is not logged in".to_owned()))?;
                io.send_message_buffers_with_cancellation(packet_type::BULK_LOAD, &buffers, &cancellation).await
            }
            => result,
            _ = &mut notified => {
                return Err(self.terminate_connection_with_error(ConnectionError::new("Connection closed before request completed.", Some("ECLOSE".to_owned()))));
            }
        };
        let canceled_during_send = match send_result {
            Ok(canceled) => canceled,
            Err(error) => {
                return Err(self.terminate_connection_with_error(connection_transport_error(error)));
            }
        };
        let tokens = if canceled_during_send {
            self.read_response_tokens_after_bulk_cancellation("token stream failed")
                .await?
        } else {
            self.read_response_tokens_incremental_with_bulk_cancellation(
                "token stream failed",
                &cancellation,
            )
            .await?
        };
        if let Some(error) = bulk_load_response_error(&tokens) {
            self.transition_to(ConnectionState::LoggedIn);
            return Err(error);
        }
        self.emit_stream_server_events(&tokens)?;
        self.transition_to(ConnectionState::LoggedIn);
        if canceled_during_send {
            return Err(ConnectionError::new(
                "Canceled.",
                Some("ECANCEL".to_owned()),
            ));
        }
        Ok(tokens)
    }

    async fn read_response_tokens_incremental_with_bulk_cancellation(
        &mut self,
        terminal_error_message: &str,
        cancellation: &BulkLoadCancellationHandle,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        tokio::select! {
            result = self.read_response_tokens_incremental(terminal_error_message) => result,
            _ = cancellation.canceled() => self.cancel_active_request(terminal_error_message).await,
        }
    }

    async fn read_response_tokens_after_bulk_cancellation(
        &mut self,
        terminal_error_message: &str,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        let result = if self.config.cancel_timeout.is_zero() {
            self.read_response_tokens_incremental(terminal_error_message)
                .await
        } else {
            match timeout(
                self.config.cancel_timeout,
                self.read_response_tokens_incremental(terminal_error_message),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => Err(ConnectionError::new(
                    format!(
                        "Failed to cancel request in {}ms",
                        self.config.cancel_timeout.as_millis()
                    ),
                    Some("ETIMEOUT".to_owned()),
                )),
            }
        };
        match result {
            Ok(tokens) => Ok(tokens),
            Err(error) => Err(self.terminate_connection_with_error(error)),
        }
    }

    async fn exec_bulk_load_preamble(
        &mut self,
        bulk_load: &BulkLoad,
    ) -> Result<(), ConnectionError> {
        if bulk_load.canceled {
            return Err(ConnectionError::new(
                "bulk load was canceled",
                Some("ECANCEL".to_owned()),
            ));
        }
        let mut request = Request::new(bulk_load.get_bulk_insert_sql(), |_| {});
        let cancellation = bulk_load.cancellation_handle();
        let request_cancellation = request.cancellation_handle();
        let execution_result = {
            let execution = self.exec_sql_batch_request(&mut request);
            tokio::pin!(execution);
            tokio::select! {
                result = &mut execution => result,
                _ = cancellation.canceled() => {
                    request_cancellation.cancel();
                    execution.await
                }
            }
        };
        execution_result?;
        if bulk_load.canceled {
            return Err(ConnectionError::new(
                "Canceled.",
                Some("ECANCEL".to_owned()),
            ));
        }
        if let Some(error) = request.error.as_ref() {
            return Err(ConnectionError::new(
                error.message.clone(),
                error.code.clone().or(Some("EREQUEST".to_owned())),
            ));
        }
        Ok(())
    }

    async fn exec_bulk_load_preamble_with_request(
        &mut self,
        bulk_load: &BulkLoad,
        request: &mut Request,
        request_cancellation: &RequestCancellationHandle,
        cancellation: &BulkLoadCancellationHandle,
    ) -> Result<(), ConnectionError> {
        if bulk_load.canceled {
            return Err(ConnectionError::new(
                "bulk load was canceled",
                Some("ECANCEL".to_owned()),
            ));
        }
        let execution_result = {
            let execution = self.exec_sql_batch_internal_with_cancellation_for_request(
                request.sql_text.clone(),
                false,
                Some(request_cancellation),
                request,
            );
            tokio::pin!(execution);
            tokio::select! {
                result = &mut execution => result,
                _ = cancellation.canceled() => {
                    request_cancellation.cancel();
                    execution.await
                }
            }
        };
        execution_result?;
        if bulk_load.canceled {
            return Err(ConnectionError::new(
                "Canceled.",
                Some("ECANCEL".to_owned()),
            ));
        }
        if let Some(error) = request.error.as_ref() {
            return Err(ConnectionError::new(
                error.message.clone(),
                error.code.clone().or(Some("EREQUEST".to_owned())),
            ));
        }
        Ok(())
    }

    pub fn new_bulk_load(
        &self,
        table: impl Into<String>,
        options: crate::bulk_load::BulkLoadOptions,
    ) -> Result<BulkLoad, String> {
        let mut bulk_load = BulkLoad::new(table, options)?;
        bulk_load.set_tds_version(self.config.tds_version.clone());
        bulk_load.set_collation(self.database_collation.clone());
        Ok(bulk_load)
    }

    pub async fn exec_bulk_load_object(
        &mut self,
        bulk_load: &mut BulkLoad,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        if let Err(error) = self.validate_bulk_load_start(bulk_load) {
            return bulk_load_error_result(bulk_load, error);
        }
        if let Err(error) = bulk_load.begin_execution() {
            return bulk_load_error_result(
                bulk_load,
                ConnectionError::new(error, Some("EINVALIDSTATE".to_owned())),
            );
        }
        let payload = bulk_load.payload();
        let result = self.exec_bulk_load_request(bulk_load, &payload).await;
        finish_bulk_load(bulk_load, result)
    }

    pub async fn exec_bulk_load_objects<I>(
        &mut self,
        bulk_load: &mut BulkLoad,
        rows: I,
    ) -> Result<Vec<StreamToken>, ConnectionError>
    where
        I: IntoIterator<Item = BTreeMap<String, Value>>,
    {
        let object_names = bulk_load
            .columns
            .iter()
            .map(|column| column.obj_name.clone())
            .collect::<Vec<_>>();
        let values = rows.into_iter().map(move |row| {
            object_names
                .iter()
                .map(|name| row.get(name).cloned().unwrap_or(Value::Null))
                .collect::<Vec<_>>()
        });
        let result = self.exec_bulk_load_value_iterator(bulk_load, values).await;
        finish_bulk_load(bulk_load, result)
    }

    pub async fn exec_bulk_load_values<I>(
        &mut self,
        bulk_load: &mut BulkLoad,
        rows: I,
    ) -> Result<Vec<StreamToken>, ConnectionError>
    where
        I: IntoIterator<Item = Vec<Value>>,
    {
        let result = self.exec_bulk_load_value_iterator(bulk_load, rows).await;
        finish_bulk_load(bulk_load, result)
    }

    pub async fn exec_bulk_load_value_stream<S>(
        &mut self,
        bulk_load: &mut BulkLoad,
        rows: S,
    ) -> Result<Vec<StreamToken>, ConnectionError>
    where
        S: Stream<Item = Vec<Value>>,
    {
        if bulk_load.execution_started {
            return bulk_load_error_result(
                bulk_load,
                ConnectionError::new(
                    "cannot add bulk-load rows after execution starts",
                    Some("EINVALIDSTATE".to_owned()),
                ),
            );
        }
        if bulk_load.canceled {
            return bulk_load_error_result(
                bulk_load,
                ConnectionError::new("bulk load was canceled", Some("ECANCEL".to_owned())),
            );
        }
        if self.config.read_only {
            return bulk_load_error_result(
                bulk_load,
                ConnectionError::new(
                    "read-only connection accepts SELECT statements only",
                    Some("EREADONLY".to_owned()),
                ),
            );
        }
        if self.state != ConnectionState::LoggedIn {
            return bulk_load_error_result(
                bulk_load,
                ConnectionError::new(
                    "connection is not logged in",
                    Some("ENOTCONNECTED".to_owned()),
                ),
            );
        }
        let mut request = Request::new(bulk_load.get_bulk_insert_sql(), |_| {});
        let _lease = self.acquire_request(&mut request).await?;
        if let Err(error) = request.begin() {
            let connection_error = ConnectionError::new(error.message.clone(), error.code.clone());
            request.fail(error);
            return bulk_load_error_result(bulk_load, connection_error);
        }
        request.timeout = Some(bulk_load.timeout.unwrap_or(self.config.request_timeout));
        let _active_request = self.register_active_request(&request);
        bulk_load.execution_started = true;
        let cancellation = bulk_load.cancellation_handle();
        let request_cancellation = request.cancellation_handle();
        if let Err(error) = self
            .exec_bulk_load_preamble_with_request(
                bulk_load,
                &mut request,
                &request_cancellation,
                &cancellation,
            )
            .await
        {
            request.fail(RequestError::new(error.message.clone(), error.code.clone()));
            return finish_bulk_load(bulk_load, Err(error));
        }
        let template = bulk_load.payload();
        self.emit_debug_payload("Bulk Load");
        let metadata = template
            .column_metadata_with_nullable_unknown(bulk_load.nullable_unknown_columns())
            .map_err(|error| ConnectionError::new(error, Some("EPAYLOAD".to_owned())))?;
        let done = bulk_load
            .create_done_token()
            .map_err(|error| ConnectionError::new(error, Some("EPAYLOAD".to_owned())))?;
        let request_timeout = bulk_load.timeout.unwrap_or(self.config.request_timeout);
        let execution =
            self.exec_bulk_load_value_stream_inner(bulk_load, template, metadata, done, rows);
        let result = if request_timeout.is_zero() {
            execution.await
        } else {
            tokio::pin!(execution);
            tokio::select! {
                result = &mut execution => result,
                _ = tokio::time::sleep(request_timeout) => {
                    cancellation.cancel();
                    let _ = execution.await;
                    Err(Self::request_timeout_error(request_timeout))
                }
            }
        };
        match &result {
            Ok(_) => request.finish(),
            Err(error) => {
                request.fail(RequestError::new(error.message.clone(), error.code.clone()))
            }
        }
        finish_bulk_load(bulk_load, result)
    }

    async fn exec_bulk_load_value_stream_inner<S>(
        &mut self,
        bulk_load: &BulkLoad,
        template: BulkLoadPayload,
        metadata: Vec<u8>,
        done: Vec<u8>,
        rows: S,
    ) -> Result<Vec<StreamToken>, ConnectionError>
    where
        S: Stream<Item = Vec<Value>>,
    {
        self.transition_to(ConnectionState::SentClientRequest);
        let producer_error = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let producer_error_seen = Arc::clone(&producer_error);
        let bulk_ref = bulk_load;
        let cancellation = bulk_load.cancellation_handle();
        let producer_cancellation = cancellation.clone();
        let rows = Box::pin(rows);
        let rows = stream::unfold(
            (rows, Vec::<Vec<u8>>::new(), 0usize),
            move |(mut rows, mut pending, mut index)| {
                let template = template.clone();
                let producer_error_seen = Arc::clone(&producer_error_seen);
                let producer_cancellation = producer_cancellation.clone();
                async move {
                    loop {
                        if producer_cancellation.is_canceled() {
                            return None;
                        }
                        if index < pending.len() {
                            let part = pending[index].clone();
                            index += 1;
                            return Some((Ok(part), (rows, pending, index)));
                        }
                        let row = rows.next().await?;
                        let values = match bulk_ref.encode_value_row(&row) {
                            Ok(values) => values,
                            Err(error) => {
                                producer_error_seen.store(true, Ordering::Release);
                                return Some((
                                    Err(TransportError::InvalidPacket(error)),
                                    (rows, pending, index),
                                ));
                            }
                        };
                        pending = match template.row_buffers(&BulkLoadRow { values }) {
                            Ok(buffers) => buffers,
                            Err(error) => {
                                producer_error_seen.store(true, Ordering::Release);
                                return Some((
                                    Err(TransportError::InvalidPacket(error)),
                                    (rows, pending, index),
                                ));
                            }
                        };
                        index = 0;
                    }
                }
            },
        );
        let parts = stream::once(async move { Ok(metadata) })
            .chain(rows)
            .chain(stream::once(async move { Ok(done) }));
        let close_handle = self.close_handle.clone();
        let notified = close_handle.notify.notified();
        tokio::pin!(notified);
        let send_result = tokio::select! {
            result = async {
                let io = self.message_io.as_mut().ok_or_else(|| {
                    TransportError::InvalidPacket("connection is not logged in".to_owned())
                })?;
                io.send_message_stream_with_cancellation(packet_type::BULK_LOAD, parts, &cancellation).await
            } => result,
            _ = &mut notified => {
                return Err(self.terminate_connection_with_error(ConnectionError::new(
                    "Connection closed before request completed.",
                    Some("ECLOSE".to_owned()),
                )));
            }
        };
        let canceled_during_send = match send_result {
            Ok(canceled) => canceled,
            Err(error) if producer_error.load(Ordering::Acquire) => {
                self.transition_to(ConnectionState::LoggedIn);
                return Err(ConnectionError::new(
                    match error {
                        TransportError::InvalidPacket(message) => message,
                        TransportError::Io(error) => error.to_string(),
                        TransportError::Timeout => "transport timeout".to_owned(),
                    },
                    Some("EPAYLOAD".to_owned()),
                ));
            }
            Err(error) => {
                return Err(self.terminate_connection_with_error(connection_transport_error(error)));
            }
        };
        if canceled_during_send {
            match self.cancel_active_request("token stream failed").await {
                Err(error) if error.code.as_deref() == Some("ECANCEL") => {
                    self.transition_to(ConnectionState::LoggedIn);
                    return Err(error);
                }
                Err(error) => return Err(error),
                Ok(_) => unreachable!(),
            }
        }
        let tokens = match if canceled_during_send {
            self.read_response_tokens_after_bulk_cancellation("token stream failed")
                .await
        } else {
            self.read_response_tokens_incremental_with_bulk_cancellation(
                "token stream failed",
                &cancellation,
            )
            .await
        } {
            Ok(tokens) => tokens,
            Err(error) => return Err(self.terminate_connection_with_error(error)),
        };
        if let Some(error) = bulk_load_response_error(&tokens) {
            self.transition_to(ConnectionState::LoggedIn);
            return Err(error);
        }
        self.emit_stream_server_events(&tokens)?;
        self.transition_to(ConnectionState::LoggedIn);
        if canceled_during_send {
            return Err(ConnectionError::new(
                "Canceled.",
                Some("ECANCEL".to_owned()),
            ));
        }
        Ok(tokens)
    }

    pub async fn exec_bulk_load_object_stream<S>(
        &mut self,
        bulk_load: &mut BulkLoad,
        rows: S,
    ) -> Result<Vec<StreamToken>, ConnectionError>
    where
        S: Stream<Item = BTreeMap<String, Value>>,
    {
        let object_names = bulk_load
            .columns
            .iter()
            .map(|column| column.obj_name.clone())
            .collect::<Vec<_>>();
        let values = rows.map(move |row| {
            object_names
                .iter()
                .map(|name| row.get(name).cloned().unwrap_or(Value::Null))
                .collect::<Vec<_>>()
        });
        self.exec_bulk_load_value_stream(bulk_load, values).await
    }

    async fn exec_bulk_load_value_iterator<I>(
        &mut self,
        bulk_load: &mut BulkLoad,
        rows: I,
    ) -> Result<Vec<StreamToken>, ConnectionError>
    where
        I: IntoIterator<Item = Vec<Value>>,
    {
        if let Err(error) = self.validate_bulk_load_start(bulk_load) {
            return bulk_load_error_result(bulk_load, error);
        }
        if let Err(error) = bulk_load.begin_execution() {
            return bulk_load_error_result(
                bulk_load,
                ConnectionError::new(error, Some("EINVALIDSTATE".to_owned())),
            );
        }
        let shape = bulk_load.payload();
        self.emit_debug_payload("Bulk Load");
        let metadata = shape
            .column_metadata()
            .map_err(|error| ConnectionError::new(error, Some("EPAYLOAD".to_owned())))?;
        let done = bulk_load
            .create_done_token()
            .map_err(|error| ConnectionError::new(error, Some("EPAYLOAD".to_owned())))?;
        let parts_cancellation = bulk_load.cancellation_handle();
        let parts = std::iter::once(Ok(metadata))
            .chain(rows.into_iter().flat_map(|row| {
                if parts_cancellation.is_canceled() {
                    return vec![Err(TransportError::InvalidPacket(
                        "bulk load was canceled".to_owned(),
                    ))];
                }
                match bulk_load
                    .encode_value_row(&row)
                    .and_then(|values| shape.row_buffers(&BulkLoadRow { values }))
                {
                    Ok(buffers) => buffers
                        .into_iter()
                        .map(Ok)
                        .collect::<Vec<Result<Vec<u8>, TransportError>>>(),
                    Err(error) => vec![Err(TransportError::InvalidPacket(error))],
                }
            }))
            .chain(std::iter::once(Ok(done)));
        let request_timeout = bulk_load.timeout.unwrap_or(self.config.request_timeout);
        self.exec_bulk_load_preamble(bulk_load).await?;
        let mut request = Request::new(String::new(), |_| {});
        let lease = self.acquire_request(&mut request).await?;
        if let Err(error) = request.begin() {
            request.fail(error.clone());
            return Err(ConnectionError::new(error.message, error.code));
        }
        let active_request =
            self.register_active_request_with_handle(request.cancellation_handle());
        let result = if request_timeout.is_zero() {
            self.exec_bulk_load_parts(parts, &parts_cancellation).await
        } else {
            match timeout(
                request_timeout,
                self.exec_bulk_load_parts(parts, &parts_cancellation),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => {
                    self.cancel_after_timeout().await;
                    Err(ConnectionError::new(
                        format!(
                            "Timeout: Bulk load failed to complete in {}ms",
                            request_timeout.as_millis()
                        ),
                        Some("ETIMEOUT".to_owned()),
                    ))
                }
            }
        };
        drop(active_request);
        drop(lease);
        match &result {
            Ok(_) => request.finish(),
            Err(error) => {
                request.fail(RequestError::new(error.message.clone(), error.code.clone()))
            }
        }
        result
    }

    fn validate_bulk_load_start(&self, bulk_load: &BulkLoad) -> Result<(), ConnectionError> {
        if bulk_load.execution_started {
            return Err(ConnectionError::new(
                "cannot add bulk-load rows after execution starts",
                Some("EINVALIDSTATE".to_owned()),
            ));
        }
        if bulk_load.canceled {
            return Err(ConnectionError::new(
                "bulk load was canceled",
                Some("ECANCEL".to_owned()),
            ));
        }
        if self.config.read_only {
            return Err(ConnectionError::new(
                "read-only connection accepts SELECT statements only",
                Some("EREADONLY".to_owned()),
            ));
        }
        if self.state != ConnectionState::LoggedIn || self.message_io.is_none() {
            return Err(ConnectionError::new(
                "connection is not logged in",
                Some("ENOTCONNECTED".to_owned()),
            ));
        }
        Ok(())
    }

    pub async fn begin_transaction(
        &mut self,
        name: impl Into<String>,
        isolation_level: IsolationLevel,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        if self.config.read_only {
            return Err(ConnectionError::new(
                "read-only connection accepts SELECT statements only",
                Some("EREADONLY".to_owned()),
            ));
        }
        if self.config.tds_version.as_str() < "7_2" {
            let transaction = Transaction::new(name, isolation_level);
            let sql = format!(
                "SET TRANSACTION ISOLATION LEVEL {};BEGIN TRAN {}",
                transaction.isolation_level_to_tsql(),
                transaction.name
            );
            let tokens = self.exec_sql_batch_internal(sql, false).await?;
            self.transaction_depth += 1;
            return Ok(tokens);
        }
        let transaction = Transaction::new(name, isolation_level);
        let tokens = self
            .exec_transaction_payload(
                transaction
                    .try_begin_payload(&self.transaction_descriptor)
                    .map_err(|error| {
                        ConnectionError::new(error.0, Some("ETRANSACTION".to_owned()))
                    })?,
            )
            .await?;
        self.transaction_depth += 1;
        Ok(tokens)
    }

    pub async fn begin_transaction_default(
        &mut self,
        name: impl Into<String>,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        self.begin_transaction(
            name,
            self.config
                .session
                .isolation_level
                .unwrap_or(IsolationLevel::NoChange),
        )
        .await
    }

    pub fn current_transaction_descriptor(&self) -> &[u8] {
        &self.transaction_descriptor
    }

    pub fn isolation_level_text(level: IsolationLevel) -> &'static str {
        Self::get_isolation_level_text(level)
    }

    pub fn get_isolation_level_text(level: IsolationLevel) -> &'static str {
        match level {
            IsolationLevel::ReadUncommitted => "read uncommitted",
            IsolationLevel::ReadCommitted => "read committed",
            IsolationLevel::RepeatableRead => "repeatable read",
            IsolationLevel::Serializable => "serializable",
            IsolationLevel::Snapshot => "snapshot",
            IsolationLevel::NoChange => "read committed",
        }
    }

    pub async fn commit_transaction(
        &mut self,
        name: impl Into<String>,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        if self.config.read_only {
            return Err(ConnectionError::new(
                "read-only connection accepts SELECT statements only",
                Some("EREADONLY".to_owned()),
            ));
        }
        if self.config.tds_version.as_str() < "7_2" {
            let transaction = Transaction::new(name, IsolationLevel::NoChange);
            let tokens = self
                .exec_sql_batch_internal(format!("COMMIT TRAN {}", transaction.name), false)
                .await?;
            self.transaction_depth = self.transaction_depth.saturating_sub(1);
            return Ok(tokens);
        }
        let transaction = Transaction::new(name, IsolationLevel::NoChange);
        let tokens = self
            .exec_transaction_payload(
                transaction
                    .try_commit_payload(&self.transaction_descriptor)
                    .map_err(|error| {
                        ConnectionError::new(error.0, Some("ETRANSACTION".to_owned()))
                    })?,
            )
            .await?;
        self.transaction_depth = self.transaction_depth.saturating_sub(1);
        if self.transaction_depth == 0 {
            self.transaction_descriptor = vec![0; 8];
        }
        Ok(tokens)
    }

    pub async fn rollback_transaction(
        &mut self,
        name: impl Into<String>,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        if self.config.read_only {
            return Err(ConnectionError::new(
                "read-only connection accepts SELECT statements only",
                Some("EREADONLY".to_owned()),
            ));
        }
        if self.config.tds_version.as_str() < "7_2" {
            let transaction = Transaction::new(name, IsolationLevel::NoChange);
            let tokens = self
                .exec_sql_batch_internal(format!("ROLLBACK TRAN {}", transaction.name), false)
                .await?;
            self.transaction_depth = self.transaction_depth.saturating_sub(1);
            return Ok(tokens);
        }
        let transaction = Transaction::new(name, IsolationLevel::NoChange);
        let tokens = self
            .exec_transaction_payload(
                transaction
                    .try_rollback_payload(&self.transaction_descriptor)
                    .map_err(|error| {
                        ConnectionError::new(error.0, Some("ETRANSACTION".to_owned()))
                    })?,
            )
            .await?;
        self.transaction_depth = 0;
        self.transaction_descriptor = vec![0; 8];
        Ok(tokens)
    }

    pub async fn save_transaction(
        &mut self,
        name: impl Into<String>,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        if self.config.read_only {
            return Err(ConnectionError::new(
                "read-only connection accepts SELECT statements only",
                Some("EREADONLY".to_owned()),
            ));
        }
        if self.config.tds_version.as_str() < "7_2" {
            let transaction = Transaction::new(name, IsolationLevel::NoChange);
            let tokens = self
                .exec_sql_batch_internal(format!("SAVE TRAN {}", transaction.name), false)
                .await?;
            self.transaction_depth += 1;
            return Ok(tokens);
        }
        let transaction = Transaction::new(name, IsolationLevel::NoChange);
        self.exec_transaction_payload(
            transaction
                .try_save_payload(&self.transaction_descriptor)
                .map_err(|error| ConnectionError::new(error.0, Some("ETRANSACTION".to_owned())))?,
        )
        .await
    }

    pub async fn transaction<F, Fut, T>(
        &mut self,
        name: impl Into<String>,
        isolation_level: IsolationLevel,
        operation: F,
    ) -> Result<T, ConnectionError>
    where
        F: FnOnce(&mut Self) -> Fut,
        Fut: Future<Output = Result<T, ConnectionError>>,
    {
        let nested = self.transaction_depth > 0;
        let name = name.into();
        if nested {
            self.save_transaction(name.clone()).await?;
        } else {
            self.begin_transaction(name.clone(), isolation_level)
                .await?;
        }

        match operation(self).await {
            Ok(value) => {
                if nested {
                    if self.config.tds_version.as_str() < "7_2" {
                        self.transaction_depth = self.transaction_depth.saturating_sub(1);
                    }
                } else {
                    self.commit_transaction(name).await?;
                }
                Ok(value)
            }
            Err(operation_error) => match self.rollback_transaction(name).await {
                Ok(_) => Err(operation_error),
                Err(rollback_error) => Err(rollback_error),
            },
        }
    }

    async fn exec_transaction_payload(
        &mut self,
        data: Vec<u8>,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        if self.close_handle.is_closed() || self.state == ConnectionState::Final {
            return Err(ConnectionError::new(
                "Connection closed before request completed.",
                Some("ECLOSE".to_owned()),
            ));
        }
        if self.state != ConnectionState::LoggedIn || self.message_io.is_none() {
            return Err(ConnectionError::new(
                "connection is not logged in",
                Some("ENOTCONNECTED".to_owned()),
            ));
        }
        let mut request = Request::new(String::new(), |_| {});
        let _lease = self.acquire_request(&mut request).await?;
        if let Err(error) = request.begin() {
            return Err(ConnectionError::new(error.message, error.code));
        }
        let _active_request = self.register_active_request(&request);
        self.transition_to(ConnectionState::SentClientRequest);
        let cancellation = request.cancellation_handle();
        let request_timeout = self.config.request_timeout;
        let first_packet_deadline =
            (!request_timeout.is_zero()).then(|| Instant::now() + request_timeout);
        let buffers = vec![data];
        let mut canceled_during_send = false;
        match self
            .send_message_buffers_controlled(
                packet_type::TRANSACTION_MANAGER,
                false,
                &buffers,
                Some(&cancellation),
                first_packet_deadline,
            )
            .await
        {
            ControlledSendOutcome::Completed(Ok(())) => {}
            ControlledSendOutcome::Completed(Err(error)) => {
                request.fail(RequestError::new(
                    error.to_string(),
                    Some("ETRANSPORT".to_owned()),
                ));
                return Err(self.terminate_connection_with_error(connection_transport_error(error)));
            }
            ControlledSendOutcome::Closed => {
                request.fail(RequestError::new(
                    "Connection closed before request completed.",
                    Some("ECLOSE".to_owned()),
                ));
                return Err(self.terminate_connection_after_close(ConnectionError::new(
                    "Connection closed before request completed.",
                    Some("ECLOSE".to_owned()),
                )));
            }
            ControlledSendOutcome::TimedOut => {
                let error = Self::request_timeout_error(request_timeout);
                request.fail(RequestError::new(error.message.clone(), error.code.clone()));
                return Err(self.terminate_connection_with_error(error));
            }
            ControlledSendOutcome::Canceled => {
                canceled_during_send = true;
                request.cancel();
            }
        }
        let tokens = match self
            .read_response_tokens_incremental_with_cancellation_and_callback(
                "token stream failed",
                (!canceled_during_send).then_some(&cancellation),
                first_packet_deadline,
                |_, _| Ok(()),
            )
            .await
        {
            Ok(_tokens) if canceled_during_send => {
                self.transition_to(ConnectionState::LoggedIn);
                request.fail(RequestError::new("Canceled.", Some("ECANCEL".to_owned())));
                return Err(ConnectionError::new(
                    "Canceled.",
                    Some("ECANCEL".to_owned()),
                ));
            }
            Ok(tokens) => tokens,
            Err(error) if error.code.as_deref() == Some("ETIMEOUT") => {
                request.fail(RequestError::new(error.message.clone(), error.code.clone()));
                self.cancel_after_timeout().await;
                return Err(Self::request_timeout_error(request_timeout));
            }
            Err(error) => {
                request.fail(RequestError::new(error.message.clone(), error.code.clone()));
                return Err(error);
            }
        };
        self.emit_stream_server_events(&tokens)?;
        for token in &tokens {
            if let StreamToken::Control(crate::token::parser::Token::EnvChange(change)) = token
                && let new_value = match &change.kind {
                    crate::token::parser::EnvChangeKind::BeginTransaction { new_value, .. }
                    | crate::token::parser::EnvChangeKind::CommitTransaction {
                        new_value, ..
                    }
                    | crate::token::parser::EnvChangeKind::RollbackTransaction {
                        new_value, ..
                    }
                    | crate::token::parser::EnvChangeKind::Binary { new_value, .. } => new_value,
                    _ => continue,
                }
                && matches!(change.change_type, 8..=10)
                && new_value.len() <= 8
            {
                self.transaction_descriptor = new_value.clone();
            }
        }
        self.transition_to(ConnectionState::LoggedIn);
        request.finish();
        Ok(tokens)
    }

    fn request_timeout_error(request_timeout: Duration) -> ConnectionError {
        ConnectionError::new(
            format!(
                "Timeout: Request failed to complete in {}ms",
                request_timeout.as_millis()
            ),
            Some("ETIMEOUT".to_owned()),
        )
    }

    async fn cancel_after_timeout(&mut self) {
        let cancellation = if self.config.cancel_timeout.is_zero() {
            self.cancel_inner().await
        } else {
            match timeout(self.config.cancel_timeout, self.cancel_inner()).await {
                Ok(result) => result,
                Err(_) => Err(ConnectionError::new(
                    format!(
                        "Failed to cancel request in {}ms",
                        self.config.cancel_timeout.as_millis()
                    ),
                    Some("ETIMEOUT".to_owned()),
                )),
            }
        };
        if cancellation.is_err() {
            self.message_io.take();
            if self.state != ConnectionState::Final {
                self.transition_to(ConnectionState::Final);
                self.emit_end_or_defer();
            }
        }
    }

    fn terminate_connection_with_error(&mut self, error: ConnectionError) -> ConnectionError {
        if self.state != ConnectionState::Final {
            self.message_io.take();
            self.transition_to(ConnectionState::Final);
            self.emit(ConnectionEvent::Error(error.clone()));
            self.emit_end_or_defer();
        }
        error
    }

    fn emit_end_or_defer(&self) {
        let end_pending = self.active_request.lock().ok().and_then(|active_request| {
            active_request
                .as_ref()
                .map(|request| Arc::clone(&request.end_pending))
        });
        if let Some(end_pending) = end_pending {
            end_pending.store(true, Ordering::Release);
        } else {
            self.emit(ConnectionEvent::End);
        }
    }

    fn terminate_connection_after_close(&mut self, error: ConnectionError) -> ConnectionError {
        if self.state != ConnectionState::Final {
            self.message_io.take();
            self.transition_to(ConnectionState::Final);
            let end_pending = self.active_request.lock().ok().and_then(|active_request| {
                active_request
                    .as_ref()
                    .map(|request| Arc::clone(&request.end_pending))
            });
            if let Some(end_pending) = end_pending {
                end_pending.store(true, Ordering::Release);
            } else {
                self.emit(ConnectionEvent::End);
            }
        }
        error
    }

    async fn cancel_inner(&mut self) -> Result<(), ConnectionError> {
        if self.state != ConnectionState::SentClientRequest {
            return Err(ConnectionError::new(
                "connection has no active request",
                Some("EINVALIDSTATE".to_owned()),
            ));
        }
        self.cancel_active_request_inner().await
    }

    pub fn cancel_request(&self) -> bool {
        let Ok(active_request) = self.active_request.lock() else {
            return false;
        };
        let Some(active_request) = active_request.as_ref() else {
            return false;
        };
        if active_request.cancellation.is_cancelled() {
            return false;
        }
        active_request.cancellation.cancel();
        true
    }

    pub fn cancel(&self) -> bool {
        self.cancel_request()
    }

    pub async fn close(&mut self) {
        self.close_now();
    }

    pub(crate) fn close_now(&mut self) {
        self.close_handle.close();
        if self.state == ConnectionState::Final {
            return;
        }
        self.terminate_connection_after_close(ConnectionError::new(
            "Connection closed before request completed.",
            Some("ECLOSE".to_owned()),
        ));
    }
}

fn connection_event_name(event: &ConnectionEvent) -> &'static str {
    match event {
        ConnectionEvent::Connect | ConnectionEvent::ConnectError(_) => "connect",
        ConnectionEvent::Error(_) => "error",
        ConnectionEvent::End => "end",
        ConnectionEvent::NewListener { .. } => "newListener",
        ConnectionEvent::RemoveListener { .. } => "removeListener",
        ConnectionEvent::Rerouting => "rerouting",
        ConnectionEvent::Retry => "retry",
        ConnectionEvent::RollbackTransaction => "rollbackTransaction",
        ConnectionEvent::CharsetChange(_) => "charsetChange",
        ConnectionEvent::DatabaseChange(_) => "databaseChange",
        ConnectionEvent::LanguageChange(_) => "languageChange",
        ConnectionEvent::InfoMessage(_) => "infoMessage",
        ConnectionEvent::ErrorMessage(_) => "errorMessage",
        ConnectionEvent::ResetConnection => "resetConnection",
        ConnectionEvent::Secure => "secure",
        ConnectionEvent::DatabaseMirroringPartner(_) => "databaseMirroringPartner",
        ConnectionEvent::PacketSizeChange(_) => "packetSizeChange",
        ConnectionEvent::CollationChange(_) => "collationChange",
        ConnectionEvent::Debug(_) => "debug",
    }
}

fn plaintext_parameter_encryption_metadata() -> crate::always_encrypted::types::CryptoMetadata {
    crate::always_encrypted::types::CryptoMetadata {
        cek_entry: None,
        cipher_algorithm_id: 0,
        cipher_algorithm_name: None,
        normalization_rule_version: Vec::new(),
        encryption_key_info: None,
        ordinal: 0,
        encryption_type: SqlServerEncryptionType::PlainText,
        cipher_algorithm: None,
    }
}

pub fn attention_acknowledged(tokens: &[StreamToken]) -> bool {
    tokens.iter().any(|token| {
        matches!(
            token,
            StreamToken::Control(Token::Done(done)) if done.attention
        )
    })
}

fn bulk_load_response_error(tokens: &[StreamToken]) -> Option<ConnectionError> {
    tokens.iter().find_map(|token| match token {
        StreamToken::Control(Token::Error(error)) => Some(ConnectionError::new(
            error.message.clone(),
            Some("EREQUEST".to_owned()),
        )),
        StreamToken::Control(Token::Done(done)) if done.sql_error => Some(ConnectionError::new(
            "An unknown error has occurred. This is likely because the schema of the BulkLoad does not match the schema of the table you are attempting to insert into.",
            Some("UNKNOWN".to_owned()),
        )),
        _ => None,
    })
}

fn finish_bulk_load(
    bulk_load: &mut BulkLoad,
    result: Result<Vec<StreamToken>, ConnectionError>,
) -> Result<Vec<StreamToken>, ConnectionError> {
    match &result {
        Ok(tokens) => {
            let row_count = tokens
                .iter()
                .filter_map(|token| match token {
                    StreamToken::Control(Token::Done(done)) => done.row_count,
                    _ => None,
                })
                .try_fold(0usize, |count, value| {
                    usize::try_from(value)
                        .ok()
                        .and_then(|value| count.checked_add(value))
                })
                .unwrap_or(usize::MAX);
            bulk_load.complete(row_count);
        }
        Err(error) => {
            bulk_load.fail(error.message.clone());
        }
    }
    result
}

fn bulk_load_error_result(
    bulk_load: &mut BulkLoad,
    error: ConnectionError,
) -> Result<Vec<StreamToken>, ConnectionError> {
    bulk_load.fail(error.message.clone());
    Err(error)
}

pub fn dispatch_request_tokens(
    request: &mut Request,
    tokens: Vec<StreamToken>,
    row_collection_on_done: bool,
    lower_case_guids: bool,
    use_column_names: bool,
) {
    let mut resultset_rows = Vec::new();
    for token in tokens {
        dispatch_request_token(
            request,
            token,
            row_collection_on_done,
            lower_case_guids,
            use_column_names,
            &mut resultset_rows,
        );
    }
}

fn dispatch_request_token(
    request: &mut Request,
    token: StreamToken,
    row_collection_on_done: bool,
    lower_case_guids: bool,
    use_column_names: bool,
    resultset_rows: &mut Vec<crate::token::rows::Row>,
) {
    if request.canceled {
        return;
    }
    match token {
        StreamToken::ColMetadata(columns) => {
            if use_column_names {
                let mut named_columns = crate::request::NamedColumnMetadata::new();
                for column in columns {
                    named_columns.insert(column.col_name.clone(), column);
                }
                request.emit(RequestEvent::NamedColumnMetadata(named_columns));
            } else {
                request.emit(RequestEvent::ColumnMetadata(columns));
            }
        }
        StreamToken::Row(row) | StreamToken::NbcRow(row) => {
            let mut row = row;
            if lower_case_guids {
                normalize_guid_case(&mut row);
            }
            if request.row_collection_on_completion {
                request.rows.push(row.clone());
            }
            if row_collection_on_done {
                resultset_rows.push(row.clone());
            }
            if use_column_names {
                request.emit(RequestEvent::NamedRow(row.named_values()));
            } else {
                request.emit(RequestEvent::Row(row));
            }
        }
        StreamToken::Control(Token::Done(done)) => {
            if done.sql_error && request.error.is_none() && !request.canceled {
                let error =
                    RequestError::new("An unknown error has occurred.", Some("UNKNOWN".to_owned()));
                request.error = Some(error);
            }
            request.row_count = request
                .row_count
                .saturating_add(done.row_count.unwrap_or(0));
            let rows = Some(resultset_rows.clone());
            match done.token {
                0xFE => {
                    request.emit(RequestEvent::DoneProc {
                        row_count: done.row_count,
                        more: done.more,
                        return_status: request.return_status,
                        rows,
                    });
                    request.return_status = None;
                }
                0xFF => request.emit(RequestEvent::DoneInProc {
                    row_count: done.row_count,
                    more: done.more,
                    rows,
                }),
                _ => request.emit(RequestEvent::Done {
                    row_count: done.row_count,
                    more: done.more,
                    rows,
                }),
            }
            if row_collection_on_done {
                resultset_rows.clear();
            }
        }
        StreamToken::Control(Token::Error(error)) => {
            let mut request_error = RequestError::new(error.message, Some("EREQUEST".to_owned()));
            request_error.number = i32::try_from(error.number).ok();
            request_error.state = Some(error.state);
            request_error.class = Some(error.class);
            request_error.server_name = Some(error.server_name);
            request_error.proc_name = Some(error.proc_name);
            request_error.line_number = i32::try_from(error.line_number).ok();
            request_error.serverName = request_error.server_name.clone();
            request_error.procName = request_error.proc_name.clone();
            request_error.lineNumber = request_error.line_number;
            request.errors.push(request_error.clone());
            request.error = if request.errors.len() == 1 {
                Some(request_error)
            } else {
                Some(RequestError::aggregate(request.errors.clone()))
            };
        }
        StreamToken::Control(Token::ReturnStatus(value)) => {
            request.return_status = Some(value.value);
        }
        StreamToken::Control(Token::ReturnValue(value)) => {
            request.emit(RequestEvent::ReturnValue {
                name: value.name,
                value: value.value,
                metadata: Box::new(value.metadata),
            })
        }
        StreamToken::Control(Token::Order(order)) => request.emit(RequestEvent::Order(order)),
        _ => {}
    }
}

fn normalize_guid_case(row: &mut crate::token::rows::Row) {
    for column in &mut row.columns {
        if let Value::Guid(value) = &mut column.value {
            *value = value.to_ascii_lowercase();
        }
    }
}

pub(crate) fn parameter_sql_declaration_for_public(
    parameter: &crate::value_codec::EncodedParameter,
) -> String {
    match parameter.type_info.first().copied() {
        Some(0x26) => match parameter.type_info.get(1).copied() {
            Some(1) => "tinyint".to_owned(),
            Some(2) => "smallint".to_owned(),
            Some(4) => "int".to_owned(),
            Some(8) => "bigint".to_owned(),
            _ => "int".to_owned(),
        },
        Some(0x68) => "bit".to_owned(),
        Some(0x6D) => match parameter.type_info.get(1).copied() {
            Some(4) => "real".to_owned(),
            _ => "float".to_owned(),
        },
        Some(0xA5) => {
            let length = parameter_length(parameter, 1);
            if length == usize::from(u16::MAX) {
                "varbinary(max)".to_owned()
            } else {
                format!("varbinary({length})")
            }
        }
        Some(0xE7) => {
            let length = parameter_length(parameter, 1) / 2;
            if length == usize::from(u16::MAX) / 2 {
                "nvarchar(max)".to_owned()
            } else {
                format!("nvarchar({length})")
            }
        }
        Some(0xA7) => {
            let length = parameter_length(parameter, 1);
            if length == usize::from(u16::MAX) {
                "varchar(max)".to_owned()
            } else {
                format!("varchar({length})")
            }
        }
        Some(0xAD) => format!("binary({})", parameter_length(parameter, 1)),
        Some(0xAF) => format!("char({})", parameter_length(parameter, 1)),
        Some(0xEF) => format!("nchar({})", parameter_length(parameter, 1) / 2),
        Some(0x23) => "text".to_owned(),
        Some(0x63) => "ntext".to_owned(),
        Some(0x22) => "image".to_owned(),
        Some(0xF1) => "xml".to_owned(),
        Some(0x28) => "date".to_owned(),
        Some(0x29) => format!("time({})", parameter.type_info.get(1).copied().unwrap_or(7)),
        Some(0x2A) => format!(
            "datetime2({})",
            parameter.type_info.get(1).copied().unwrap_or(7)
        ),
        Some(0x2B) => format!(
            "datetimeoffset({})",
            parameter.type_info.get(1).copied().unwrap_or(7)
        ),
        Some(0x37) | Some(0x3F) | Some(0x6A) | Some(0x6C) => format!(
            "decimal({},{})",
            parameter.type_info.get(2).copied().unwrap_or(38),
            parameter.type_info.get(3).copied().unwrap_or(0)
        ),
        Some(0x24) => "uniqueidentifier".to_owned(),
        Some(0x62) => "sql_variant".to_owned(),
        _ => "varbinary(max)".to_owned(),
    }
}

fn parameter_length(parameter: &crate::value_codec::EncodedParameter, offset: usize) -> usize {
    parameter
        .type_info
        .get(offset..offset + 2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]) as usize)
        .unwrap_or(usize::from(u16::MAX))
}

fn parse_all_tokens(data: &[u8], tds_version: &str) -> Result<Vec<Token>, ConnectionError> {
    let mut offset = 0;
    let mut tokens = Vec::new();
    while offset < data.len() {
        let parsed =
            parse_token(data, offset, tds_version).map_err(|error| token_error(error, offset))?;
        if parsed.offset <= offset {
            return Err(ConnectionError::new(
                "token parser made no progress",
                Some("ETOKEN".to_owned()),
            ));
        }
        offset = parsed.offset;
        tokens.push(parsed.value);
    }
    Ok(tokens)
}

pub fn negotiated_tds_version(tokens: &[Token]) -> Result<String, ConnectionError> {
    let login_ack = tokens.iter().find_map(|token| match token {
        Token::LoginAck(value) => Some(value),
        _ => None,
    });
    let login_ack = login_ack
        .ok_or_else(|| ConnectionError::new("Login failed.", Some("ELOGIN".to_owned())))?;
    if login_ack.interface.is_none() {
        return Err(ConnectionError::new(
            "Server responded with unsupported interface.",
            Some("EINTERFACENOTSUPP".to_owned()),
        ));
    }
    login_ack.tds_version.clone().ok_or_else(|| {
        ConnectionError::new(
            "Server responded with unknown TDS version.",
            Some("ETDS".to_owned()),
        )
    })
}

pub fn supports_required_fed_auth(authentication: &Authentication) -> bool {
    matches!(
        authentication,
        Authentication::FedAuth { .. }
            | Authentication::AzureActiveDirectoryAccessToken { .. }
            | Authentication::AzureActiveDirectoryPassword { .. }
            | Authentication::AzureActiveDirectoryMsiVm { .. }
            | Authentication::AzureActiveDirectoryMsiAppService { .. }
            | Authentication::AzureActiveDirectoryServicePrincipalSecret { .. }
            | Authentication::AzureActiveDirectoryDefault { .. }
            | Authentication::AzureActiveDirectoryTokenCredential
    )
}

pub fn validate_feature_ext_ack(
    tokens: &[Token],
    fed_auth_requested: bool,
) -> Result<(), ConnectionError> {
    for token in tokens {
        let Token::FeatureExtAck(value) = token else {
            continue;
        };
        if fed_auth_requested {
            if value.fed_auth.is_none() {
                return Err(ConnectionError::new(
                    "Did not receive Active Directory authentication acknowledgement",
                    Some("ELOGIN".to_owned()),
                ));
            }
            if value.fed_auth.as_ref().is_some_and(|data| !data.is_empty()) {
                return Err(ConnectionError::new(
                    "Active Directory authentication acknowledgement includes extra data",
                    Some("ELOGIN".to_owned()),
                ));
            }
        } else if value.fed_auth.is_none() && value.utf8_support.is_none() {
            return Err(ConnectionError::new(
                "Received acknowledgement for unknown feature",
                Some("ELOGIN".to_owned()),
            ));
        } else if value.fed_auth.is_some() {
            return Err(ConnectionError::new(
                "Did not request Active Directory authentication, but received the acknowledgment",
                Some("ELOGIN".to_owned()),
            ));
        }
    }
    Ok(())
}

fn token_error(error: TokenParseError, offset: usize) -> ConnectionError {
    ConnectionError::new(
        format!("token parsing failed at {offset}: {error:?}"),
        Some("ETOKEN".to_owned()),
    )
}

fn connection_transport_error(error: TransportError) -> ConnectionError {
    let message = error.to_string();
    ConnectionError::with_cause(message, Some("ESOCKET".to_owned()), error)
}

fn is_recoverable_stream_parse_error(error: &StreamParseError) -> bool {
    matches!(
        error,
        StreamParseError::NotEnoughData
            | StreamParseError::Token(crate::token::parser::TokenParseError::NotEnoughData(_))
            | StreamParseError::Row(crate::token::rows::RowParseError::NotEnoughData(_))
            | StreamParseError::Row(crate::token::rows::RowParseError::Metadata(
                crate::metadata_parser::MetadataError::NotEnoughData(_),
            ))
            | StreamParseError::Row(crate::token::rows::RowParseError::Value(
                crate::value_parser::ValueParserError::NotEnoughData(_),
            ))
    )
}

fn connector_error(error: ConnectorError) -> ConnectionError {
    ConnectionError::new(
        format!("connector failed: {error}"),
        Some("ECONNECT".to_owned()),
    )
}

fn connection_state_name(state: ConnectionState) -> &'static str {
    match state {
        ConnectionState::Initialized => "Initialized",
        ConnectionState::Connecting => "Connecting",
        ConnectionState::SentPrelogin => "SentPrelogin",
        ConnectionState::SentTlsSslNegotiation => "SentTLSSSLNegotiation",
        ConnectionState::SentLogin7WithStandardLogin => "SentLogin7WithStandardLogin",
        ConnectionState::SentLogin7WithNtlm => "SentLogin7WithNTLMLogin",
        ConnectionState::SentLogin7WithFedAuth => "SentLogin7WithFedauth",
        ConnectionState::LoggedInSendingInitialSql => "LoggedInSendingInitialSql",
        ConnectionState::SentClientRequest => "SentClientRequest",
        ConnectionState::LoggedIn => "LoggedIn",
        ConnectionState::SentAttention => "SentAttention",
        ConnectionState::Rerouting => "ReRouting",
        ConnectionState::TransientFailureRetry => "TRANSIENT_FAILURE_RETRY",
        ConnectionState::Final => "Final",
    }
}

fn is_select_statement(sql: &str) -> bool {
    let normalized = strip_leading_sql_comments(sql);
    let tokens = sql_tokens(normalized);
    let Some(first) = tokens.first() else {
        return false;
    };
    if first != "SELECT" && first != "WITH" {
        return false;
    }
    if tokens.iter().filter(|token| token.as_str() == ";").count() > 1 {
        return false;
    }
    if tokens.last().is_some_and(|token| token == ";") {
        if tokens.len() == 1 {
            return false;
        }
    } else if tokens.iter().any(|token| token == ";") {
        return false;
    }
    if tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "INSERT"
                | "UPDATE"
                | "DELETE"
                | "MERGE"
                | "EXEC"
                | "EXECUTE"
                | "TRUNCATE"
                | "ALTER"
                | "DROP"
                | "CREATE"
                | "GRANT"
                | "REVOKE"
                | "INTO"
        )
    }) {
        return false;
    }
    first == "SELECT" || tokens.iter().any(|token| token == "SELECT")
}

fn strip_leading_sql_comments(mut sql: &str) -> &str {
    loop {
        sql = sql.trim_start();
        if let Some(rest) = sql.strip_prefix("--") {
            sql = rest.split_once('\n').map_or("", |(_, remaining)| remaining);
            continue;
        }
        if let Some(rest) = sql.strip_prefix("/*") {
            sql = rest.split_once("*/").map_or("", |(_, remaining)| remaining);
            continue;
        }
        return sql;
    }
}

fn sql_tokens(sql: &str) -> Vec<String> {
    let bytes = sql.as_bytes();
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut index = 0;
    let flush = |current: &mut String, tokens: &mut Vec<String>| {
        if !current.is_empty() {
            tokens.push(std::mem::take(current));
        }
    };
    while index < bytes.len() {
        match bytes[index] {
            b'\'' => {
                flush(&mut current, &mut tokens);
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == b'\'' {
                        if bytes.get(index + 1) == Some(&b'\'') {
                            index += 2;
                        } else {
                            index += 1;
                            break;
                        }
                    } else {
                        index += 1;
                    }
                }
            }
            b'-' if bytes.get(index + 1) == Some(&b'-') => {
                flush(&mut current, &mut tokens);
                index += 2;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                flush(&mut current, &mut tokens);
                index += 2;
                while index + 1 < bytes.len() && !(bytes[index] == b'*' && bytes[index + 1] == b'/')
                {
                    index += 1;
                }
                index = (index + 2).min(bytes.len());
            }
            b';' => {
                flush(&mut current, &mut tokens);
                tokens.push(";".to_owned());
                index += 1;
            }
            byte if byte.is_ascii_alphanumeric() || byte == b'_' => {
                current.push(byte.to_ascii_uppercase() as char);
                index += 1;
            }
            _ => {
                flush(&mut current, &mut tokens);
                index += 1;
            }
        }
    }
    flush(&mut current, &mut tokens);
    tokens
}

fn environment_value(name: &str) -> Result<String, ConnectionError> {
    std::env::var(name).map_err(|_| {
        ConnectionError::new(
            format!("required environment variable {name} is missing"),
            Some("EENV".to_owned()),
        )
    })
}

pub fn encode_fed_auth_token_payload(token: &str) -> Result<Vec<u8>, ConnectionError> {
    let utf16 = token
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let token_length = u32::try_from(utf16.len()).map_err(|_| {
        ConnectionError::new(
            "federated token exceeds the protocol length limit",
            Some("EFEDAUTH".to_owned()),
        )
    })?;
    let total_length = token_length.checked_add(4).ok_or_else(|| {
        ConnectionError::new(
            "federated token length overflows the protocol",
            Some("EFEDAUTH".to_owned()),
        )
    })?;
    let mut data = Vec::with_capacity(8 + utf16.len());
    data.extend(total_length.to_le_bytes());
    data.extend(token_length.to_le_bytes());
    data.extend(utf16);
    Ok(data)
}

pub fn fed_auth_scope(spn: &str) -> Result<String, ConnectionError> {
    let mut value = url::Url::parse(spn).map_err(|error| {
        ConnectionError::new(
            format!("invalid federated authentication service principal name: {error}"),
            Some("EFEDAUTH".to_owned()),
        )
    })?;
    value.set_path("/.default");
    value.set_query(None);
    value.set_fragment(None);
    Ok(value.to_string())
}

fn decode_ntlm_field(value: &[u8]) -> String {
    let units = value
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    String::from_utf16_lossy(&units)
}

fn value_usize(value: &Value) -> Result<usize, ConnectionError> {
    match value {
        Value::Integer(number) if *number >= 0 => Ok(*number as usize),
        Value::BigInteger(number) => number
            .parse::<u128>()
            .ok()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                ConnectionError::new(
                    "metadata integer exceeds usize",
                    Some("EAE_METADATA".to_owned()),
                )
            }),
        _ => Err(ConnectionError::new(
            "metadata value is not a non-negative integer",
            Some("EAE_METADATA".to_owned()),
        )),
    }
}

fn value_u32(value: &Value) -> Result<u32, ConnectionError> {
    u32::try_from(value_usize(value)?).map_err(|_| {
        ConnectionError::new(
            "metadata integer exceeds u32",
            Some("EAE_METADATA".to_owned()),
        )
    })
}

fn value_u8(value: &Value) -> Result<u8, ConnectionError> {
    u8::try_from(value_u32(value)?).map_err(|_| {
        ConnectionError::new(
            "metadata integer exceeds u8",
            Some("EAE_METADATA".to_owned()),
        )
    })
}

fn value_parameter_ordinal(value: &Value) -> Result<usize, ConnectionError> {
    value_usize(value)?.checked_sub(1).ok_or_else(|| {
        ConnectionError::new(
            "metadata parameter ordinal must be one-based",
            Some("EAE_METADATA".to_owned()),
        )
    })
}

fn metadata_version_u64(value: &[u8]) -> u64 {
    let mut bytes = [0u8; 8];
    let length = value.len().min(bytes.len());
    bytes[..length].copy_from_slice(&value[..length]);
    u64::from_le_bytes(bytes)
}

fn value_text(value: &Value) -> Result<String, ConnectionError> {
    match value {
        Value::Text(text) => Ok(text.clone()),
        _ => Err(ConnectionError::new(
            "metadata value is not text",
            Some("EAE_METADATA".to_owned()),
        )),
    }
}

fn value_binary(value: &Value) -> Result<Vec<u8>, ConnectionError> {
    match value {
        Value::Binary(bytes) => Ok(bytes.clone()),
        _ => Err(ConnectionError::new(
            "metadata value is not binary",
            Some("EAE_METADATA".to_owned()),
        )),
    }
}

fn _message_type_name(value: u8) -> &'static str {
    message_type_name(value)
}
