use super::drivers::{PostgresqlAdapter, SqlServerAdapter};
use crate::backend::domain::catalog::DialectId;
use crate::backend::ports::{CatalogProvider, CatalogProviderError, CatalogProviderFactory};
use std::sync::Arc;

#[derive(Clone)]
pub struct DatabaseDriverFactory {
    postgresql: Arc<PostgresqlAdapter>,
    sqlserver: Arc<SqlServerAdapter>,
}

impl Default for DatabaseDriverFactory {
    fn default() -> Self {
        Self {
            postgresql: Arc::new(PostgresqlAdapter::default()),
            sqlserver: Arc::new(SqlServerAdapter::default()),
        }
    }
}

impl CatalogProviderFactory for DatabaseDriverFactory {
    fn invalidate(&self, connection_id: &str) {
        self.postgresql.invalidate(connection_id);
        self.sqlserver.invalidate(connection_id);
    }

    fn provider(
        &self,
        dialect: DialectId,
    ) -> Result<Arc<dyn CatalogProvider>, CatalogProviderError> {
        match dialect {
            DialectId::Postgresql => Ok(self.postgresql.clone()),
            DialectId::Sqlserver => Ok(self.sqlserver.clone()),
            unsupported => Err(CatalogProviderError::UnsupportedDialect(unsupported)),
        }
    }
}
