use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tedious_rs::connection::{Connection, ConnectionConfig};
use tedious_rs::errors::ConnectionError;

#[path = "../integration/ntlm_read_only_harness.rs"]
mod ntlm_read_only_harness;

#[path = "../integration/sql_auth_harness.rs"]
mod sql_auth_harness;

fn required(name: &str) -> Result<String, ConnectionError> {
    std::env::var(name).map_err(|_| {
        ConnectionError::new(
            format!("required integration environment variable {name} is missing"),
            Some("EENV".to_owned()),
        )
    })
}

fn required_non_empty(name: &str) -> Result<String, ConnectionError> {
    let value = required(name)?;

    if value.is_empty() {
        return Err(ConnectionError::new(
            format!("required integration environment variable {name} is empty"),
            Some("EENV".to_owned()),
        ));
    }

    Ok(value)
}

pub fn integration_enabled() -> bool {
    matches!(
        std::env::var("TEDIOUS_RS_RUN_INTEGRATION").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

pub fn sql_config() -> Result<ConnectionConfig, ConnectionError> {
    let mut config = ConnectionConfig::sql("localhost", "sa", "DevStr0ng@Pass");
    config.database = Some("tedious_rs".to_owned());
    config.trust_server_certificate = true;
    Ok(config)
}

pub fn ntlm_config(domain: Option<String>) -> Result<ConnectionConfig, ConnectionError> {
    let mut config = ConnectionConfig::ntlm(
        required_non_empty("TEST_DB_HOST")?,
        required_non_empty("VPN_USER")?,
        required_non_empty("VPN_PWD")?,
        domain,
    );
    config.database = Some(required_non_empty("TEST_DB_NAME")?);
    config.trust_server_certificate = true;
    config.read_only = true;
    Ok(config)
}

pub fn ntlm_config_from_environment() -> Result<ConnectionConfig, ConnectionError> {
    ntlm_config(std::env::var("VPN_DOMAIN").ok())
}

pub fn ntlm_domain_variants() -> Vec<Option<String>> {
    let mut variants = vec![None, Some(String::new())];

    if let Ok(domain) = std::env::var("VPN_DOMAIN")
        && !domain.is_empty()
    {
        variants.push(Some(domain));
    }

    variants
}

pub async fn with_sql_connection<F, T>(operation: F) -> Result<T, Box<dyn std::error::Error>>
where
    F: for<'a> FnOnce(
        &'a mut Connection,
    )
        -> Pin<Box<dyn Future<Output = Result<T, Box<dyn std::error::Error>>> + 'a>>,
{
    let mut connection = Connection::new(sql_config()?);

    if let Err(error) = connection.connect().await {
        connection.close().await;
        return Err(error.into());
    }

    let result = operation(&mut connection).await;
    connection.close().await;
    result
}

pub async fn with_ntlm_connection<F, T>(
    domain: Option<String>,
    operation: F,
) -> Result<T, Box<dyn std::error::Error>>
where
    F: for<'a> FnOnce(
        &'a mut Connection,
    )
        -> Pin<Box<dyn Future<Output = Result<T, Box<dyn std::error::Error>>> + 'a>>,
{
    let mut connection = Connection::new(ntlm_config(domain)?);

    if let Err(error) = connection.connect().await {
        connection.close().await;
        return Err(error.into());
    }

    let result = operation(&mut connection).await;
    connection.close().await;
    result
}

static SQL_AUTH_OBJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn sql_auth_object_name(prefix: &str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let counter = SQL_AUTH_OBJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "tedious_rs_{prefix}_{}_{}_{}",
        std::process::id(),
        timestamp,
        counter
    )
}

#[derive(Debug)]
struct CleanupError {
    operation: Option<Box<dyn std::error::Error>>,
    cleanup: Box<dyn std::error::Error>,
}

impl Display for CleanupError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.operation {
            Some(operation) => write!(
                formatter,
                "operation failed: {operation}; cleanup failed: {}",
                self.cleanup
            ),
            None => write!(formatter, "cleanup failed: {}", self.cleanup),
        }
    }
}

impl std::error::Error for CleanupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.operation
            .as_deref()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

fn combine_operation_and_cleanup<T>(
    operation: Result<T, Box<dyn std::error::Error>>,
    cleanup: Result<(), Box<dyn std::error::Error>>,
) -> Result<T, Box<dyn std::error::Error>> {
    match (operation, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Err(operation), Err(cleanup)) => Err(Box::new(CleanupError {
            operation: Some(operation),
            cleanup,
        })),
    }
}

pub async fn with_sql_connection_cleanup<F, T>(
    cleanup_sql: String,
    operation: F,
) -> Result<T, Box<dyn std::error::Error>>
where
    F: for<'a> FnOnce(
        &'a mut Connection,
    )
        -> Pin<Box<dyn Future<Output = Result<T, Box<dyn std::error::Error>>> + 'a>>,
{
    with_sql_connection_cleanup_with_config(sql_config()?, cleanup_sql, operation).await
}

pub async fn with_sql_connection_cleanup_with_config<F, T>(
    config: ConnectionConfig,
    cleanup_sql: String,
    operation: F,
) -> Result<T, Box<dyn std::error::Error>>
where
    F: for<'a> FnOnce(
        &'a mut Connection,
    )
        -> Pin<Box<dyn Future<Output = Result<T, Box<dyn std::error::Error>>> + 'a>>,
{
    let mut connection = Connection::new(config);

    if let Err(error) = connection.connect().await {
        connection.close().await;
        return Err(error.into());
    }

    let operation = operation(&mut connection).await;
    let cleanup = connection
        .exec_sql_batch(format!(
            "IF @@TRANCOUNT > 0 ROLLBACK TRANSACTION; {cleanup_sql}"
        ))
        .await
        .map(|_| ())
        .map_err(|error| Box::new(error) as Box<dyn std::error::Error>);
    connection.close().await;
    combine_operation_and_cleanup(operation, cleanup)
}
