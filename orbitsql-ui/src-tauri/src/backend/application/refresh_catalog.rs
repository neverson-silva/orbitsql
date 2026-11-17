use crate::backend::domain::catalog::CatalogSnapshot;
use crate::backend::ports::{CatalogProviderError, CatalogProviderFactory, CatalogRefreshRequest};

pub struct RefreshCatalogUseCase<F> {
    factory: F,
}

impl<F> RefreshCatalogUseCase<F>
where
    F: CatalogProviderFactory,
{
    pub fn new(factory: F) -> Self {
        Self { factory }
    }

    pub async fn execute(
        &self,
        request: CatalogRefreshRequest,
    ) -> Result<CatalogSnapshot, CatalogProviderError> {
        let provider = self.factory.provider(request.dialect.clone())?;
        provider.refresh(request).await
    }
}
