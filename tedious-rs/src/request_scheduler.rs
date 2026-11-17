use std::sync::Arc;

use crate::errors::RequestError;

#[derive(Debug)]
pub struct RequestScheduler {
    semaphore: Arc<tokio::sync::Semaphore>,
}

#[derive(Debug)]
pub struct RequestLease {
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl RequestScheduler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
        })
    }

    pub async fn acquire(self: &Arc<Self>) -> Result<RequestLease, RequestError> {
        let permit = Arc::clone(&self.semaphore)
            .acquire_owned()
            .await
            .map_err(|_| {
                RequestError::new("request scheduler is closed", Some("ELOCK".to_owned()))
            })?;
        Ok(RequestLease { _permit: permit })
    }

    pub fn is_active(&self) -> bool {
        self.semaphore.available_permits() == 0
    }
}
