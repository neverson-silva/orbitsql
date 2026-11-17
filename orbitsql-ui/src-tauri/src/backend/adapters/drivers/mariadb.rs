use super::mysql;
use crate::backend::domain::catalog::DialectId;
use crate::backend::ports::{
    CatalogProvider, CatalogProviderError, CatalogProviderFuture, CatalogRefreshRequest,
};

pub struct MariaDbAdapter;

impl CatalogProvider for MariaDbAdapter {
    fn dialect(&self) -> DialectId {
        DialectId::Mariadb
    }

    fn refresh(&self, request: CatalogRefreshRequest) -> CatalogProviderFuture {
        Box::pin(async move {
            mysql::introspect(request)
                .await
                .map_err(|message| CatalogProviderError::Driver {
                    dialect: DialectId::Mariadb,
                    message,
                })
        })
    }
}
