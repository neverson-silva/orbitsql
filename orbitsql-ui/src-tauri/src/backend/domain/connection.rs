use super::catalog::{CatalogSnapshot, DatabaseCapabilities, DialectId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

fn default_keep_alive_enabled() -> bool {
    true
}

fn default_keep_alive_interval_seconds() -> u64 {
    15
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionAuthMethod {
    #[default]
    Password,
    WindowsNtlm,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionTransportMode {
    #[default]
    Direct,
    Ssh,
    Proxy,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProxyType {
    #[default]
    Socks5,
    HttpConnect,
}

fn default_ssh_port() -> u16 {
    22
}

fn default_proxy_port() -> u16 {
    1080
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTransport {
    #[serde(default)]
    pub mode: ConnectionTransportMode,
    #[serde(default)]
    pub ssh_host: String,
    #[serde(default = "default_ssh_port")]
    pub ssh_port: u16,
    #[serde(default)]
    pub ssh_user_name: String,
    #[serde(default)]
    pub ssh_private_key_path: String,
    #[serde(default)]
    pub proxy_host: String,
    #[serde(default = "default_proxy_port")]
    pub proxy_port: u16,
    #[serde(default)]
    pub proxy_type: ProxyType,
}

impl Default for ConnectionTransport {
    fn default() -> Self {
        Self {
            mode: ConnectionTransportMode::Direct,
            ssh_host: String::new(),
            ssh_port: default_ssh_port(),
            ssh_user_name: String::new(),
            ssh_private_key_path: String::new(),
            proxy_host: String::new(),
            proxy_port: default_proxy_port(),
            proxy_type: ProxyType::Socks5,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProfileRecord {
    pub id: String,
    pub name: String,
    pub dialect: DialectId,
    pub server: String,
    pub port: u16,
    pub user_name: String,
    #[serde(default)]
    pub database: String,
    pub trust_server_certificate: bool,
    #[serde(default)]
    pub auth_method: ConnectionAuthMethod,
    #[serde(default, alias = "environment")]
    pub profile_id: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub show_all_catalog_items: bool,
    #[serde(default)]
    pub transport: ConnectionTransport,
    #[serde(default)]
    pub driver_properties: BTreeMap<String, String>,
    #[serde(default = "default_keep_alive_enabled")]
    pub keep_alive_enabled: bool,
    #[serde(default = "default_keep_alive_interval_seconds")]
    pub keep_alive_interval_seconds: u64,
    #[serde(default)]
    pub idle_timeout_seconds: Option<u64>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionDraft {
    pub name: String,
    pub dialect: DialectId,
    pub server: String,
    pub port: u16,
    pub user_name: String,
    pub password: String,
    #[serde(default)]
    pub database: String,
    pub trust_server_certificate: bool,
    pub auth_method: ConnectionAuthMethod,
    #[serde(default, alias = "environment")]
    pub profile_id: String,
    #[serde(default)]
    pub show_all_catalog_items: bool,
    #[serde(default)]
    pub transport: ConnectionTransport,
    #[serde(default)]
    pub driver_properties: BTreeMap<String, String>,
    #[serde(default = "default_keep_alive_enabled")]
    pub keep_alive_enabled: bool,
    #[serde(default = "default_keep_alive_interval_seconds")]
    pub keep_alive_interval_seconds: u64,
    #[serde(default)]
    pub idle_timeout_seconds: Option<u64>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionUpdateDraft {
    pub id: String,
    pub name: String,
    pub dialect: DialectId,
    pub server: String,
    pub port: u16,
    pub user_name: String,
    pub password: Option<String>,
    #[serde(default)]
    pub database: String,
    pub trust_server_certificate: bool,
    pub auth_method: ConnectionAuthMethod,
    #[serde(default, alias = "environment")]
    pub profile_id: String,
    #[serde(default)]
    pub show_all_catalog_items: bool,
    #[serde(default = "default_keep_alive_enabled")]
    pub keep_alive_enabled: bool,
    #[serde(default = "default_keep_alive_interval_seconds")]
    pub keep_alive_interval_seconds: u64,
    #[serde(default)]
    pub idle_timeout_seconds: Option<u64>,
}

impl std::fmt::Debug for ConnectionDraft {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConnectionDraft")
            .field("name", &self.name)
            .field("dialect", &self.dialect)
            .field("server", &self.server)
            .field("port", &self.port)
            .field("user_name", &self.user_name)
            .field("password", &"[REDACTED]")
            .field("database", &self.database)
            .field("trust_server_certificate", &self.trust_server_certificate)
            .field("auth_method", &self.auth_method)
            .field("profile_id", &self.profile_id)
            .field("show_all_catalog_items", &self.show_all_catalog_items)
            .field("transport", &self.transport)
            .field("driver_properties", &self.driver_properties)
            .finish()
    }
}

impl std::fmt::Debug for ConnectionUpdateDraft {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConnectionUpdateDraft")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("dialect", &self.dialect)
            .field("server", &self.server)
            .field("port", &self.port)
            .field("user_name", &self.user_name)
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .field("database", &self.database)
            .field("trust_server_certificate", &self.trust_server_certificate)
            .field("auth_method", &self.auth_method)
            .field("profile_id", &self.profile_id)
            .finish()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionDiagnostic {
    pub success: bool,
    pub latency_ms: u128,
    pub server_version: Option<String>,
    pub database: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub recoverable: bool,
    pub capabilities: Option<DatabaseCapabilities>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSessionResult {
    pub profile: ConnectionProfileRecord,
    pub catalog: CatalogSnapshot,
    pub capabilities: DatabaseCapabilities,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionOperationError {
    pub code: String,
    pub message: String,
    pub recoverable: bool,
}

impl ConnectionOperationError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, recoverable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            recoverable,
        }
    }
}

impl Display for ConnectionOperationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl Error for ConnectionOperationError {}
