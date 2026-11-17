use crate::backend::domain::catalog::{CatalogSnapshot, DialectId};
use crate::backend::domain::connection::{ConnectionAuthMethod, ConnectionTransport};
use serde::Deserialize;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogRefreshRequest {
    pub dialect: DialectId,
    pub connection_id: String,
    pub version: String,
    pub generated_at: String,
    pub server: String,
    pub port: u16,
    pub user_name: String,
    pub password: String,
    pub database: String,
    pub trust_server_certificate: bool,
    #[serde(default)]
    pub auth_method: Option<ConnectionAuthMethod>,
    #[serde(default)]
    pub show_all_catalog_items: bool,
    #[serde(default)]
    pub transport: ConnectionTransport,
    #[serde(default)]
    pub driver_properties: std::collections::BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum CatalogScope {
    Table {
        #[serde(rename = "tableId")]
        table_id: String,
        #[serde(default, rename = "databaseId")]
        database_id: Option<String>,
    },
    Schema {
        #[serde(rename = "schemaId")]
        schema_id: String,
        #[serde(default, rename = "databaseId")]
        database_id: Option<String>,
    },
    Database {
        #[serde(rename = "databaseId")]
        database_id: String,
    },
}

impl CatalogScope {
    pub fn cache_key(&self) -> String {
        match self {
            Self::Database { database_id } => format!("database:{database_id}"),
            Self::Schema {
                schema_id,
                database_id,
            } => format!(
                "schema:{}:{schema_id}",
                database_id.as_deref().unwrap_or_default()
            ),
            Self::Table {
                table_id,
                database_id,
            } => format!(
                "table:{}:{table_id}",
                database_id.as_deref().unwrap_or_default()
            ),
        }
    }
}

impl std::fmt::Debug for CatalogRefreshRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CatalogRefreshRequest")
            .field("dialect", &self.dialect)
            .field("connection_id", &self.connection_id)
            .field("version", &self.version)
            .field("generated_at", &self.generated_at)
            .field("server", &self.server)
            .field("port", &self.port)
            .field("user_name", &self.user_name)
            .field("password", &"[REDACTED]")
            .field("database", &self.database)
            .field("trust_server_certificate", &self.trust_server_certificate)
            .field("auth_method", &self.auth_method)
            .finish()
    }
}

pub type ProviderResult<T> = Result<T, Box<dyn Error + Send + Sync>>;
pub type CatalogProviderFuture =
    Pin<Box<dyn Future<Output = Result<CatalogSnapshot, CatalogProviderError>> + Send + 'static>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogProviderError {
    UnsupportedDialect(DialectId),
    Driver { dialect: DialectId, message: String },
}

impl Display for CatalogProviderError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedDialect(dialect) => {
                write!(
                    formatter,
                    "No catalog provider is registered for {dialect:?}"
                )
            }
            Self::Driver { dialect, message } => {
                write!(formatter, "{dialect:?} catalog provider failed: {message}")
            }
        }
    }
}

impl Error for CatalogProviderError {}

pub trait CatalogProvider: Send + Sync {
    fn dialect(&self) -> DialectId;
    fn invalidate(&self, _connection_id: &str) {}
    fn refresh(&self, request: CatalogRefreshRequest) -> CatalogProviderFuture;
    fn initial(&self, request: CatalogRefreshRequest) -> CatalogProviderFuture {
        self.refresh(request)
    }
    fn expand(
        &self,
        _request: CatalogRefreshRequest,
        scope: CatalogScope,
    ) -> CatalogProviderFuture {
        let dialect = self.dialect();
        Box::pin(async move {
            Err(CatalogProviderError::Driver {
                dialect,
                message: format!("Catalog scope is not supported: {scope:?}"),
            })
        })
    }
}

pub trait CatalogProviderFactory: Send + Sync {
    fn invalidate(&self, connection_id: &str);
    fn provider(
        &self,
        dialect: DialectId,
    ) -> Result<Arc<dyn CatalogProvider>, CatalogProviderError>;
}

#[cfg(test)]
mod tests {
    use super::CatalogScope;

    #[test]
    fn deserializes_overlapping_catalog_scopes_by_their_specific_id() {
        let schema: CatalogScope = serde_json::from_value(serde_json::json!({
            "schemaId": "schema:42",
            "databaseId": "plataforma"
        }))
        .expect("schema scope should deserialize");
        assert!(matches!(schema, CatalogScope::Schema { .. }));

        let table: CatalogScope = serde_json::from_value(serde_json::json!({
            "tableId": "object:84",
            "databaseId": "plataforma"
        }))
        .expect("table scope should deserialize");
        assert!(matches!(table, CatalogScope::Table { .. }));

        let database: CatalogScope = serde_json::from_value(serde_json::json!({
            "databaseId": "plataforma"
        }))
        .expect("database scope should deserialize");
        assert!(matches!(database, CatalogScope::Database { .. }));
    }
}
