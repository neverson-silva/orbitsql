use super::transport::{TransportGuard, prepare_endpoint};
use crate::backend::domain::catalog::{DatabaseCapabilities, DialectId};
use crate::backend::domain::connection::{ConnectionAuthMethod, ConnectionOperationError};
use crate::backend::domain::query::{QueryColumn, QueryResult, QueryResultBatch};
use crate::backend::domain::session_manager::{
    DatabaseSession, SessionKillResult, SessionTelemetry, annotate_blocking_graph,
    extract_current_statement, transaction_isolation_name, validate_kill_session_id,
};
use crate::backend::ports::connection::{
    ConnectionDriver, ConnectionDriverFactory, ResolvedConnection, RuntimeConnection,
    RuntimeResultSession, RuntimeResultSessionStart,
};
use futures_util::future::BoxFuture;
use futures_util::{Stream, StreamExt};
use serde_json::Value as JsonValue;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgRow, PgSslMode, Postgres};
use sqlx::{Column, Executor, Row, SqlSafeStr, Statement, TypeInfo};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tedious_rs::{Connection, Request, StreamPauseHandle};
use tokio::sync::mpsc;

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(15);
const QUERY_TIMEOUT: Duration = Duration::from_secs(60);
const RESULT_SESSION_CLOSE_TIMEOUT: Duration = Duration::from_millis(750);

fn driver_error(message: impl Into<String>) -> ConnectionOperationError {
    let message = message.into();
    let normalized = message.to_ascii_lowercase();
    let code = if normalized.contains("dns")
        || normalized.contains("resolve")
        || normalized.contains("name or service")
    {
        "DNS_RESOLUTION_FAILED"
    } else if normalized.contains("refused") {
        "CONNECTION_REFUSED"
    } else if normalized.contains("timeout") || normalized.contains("timed out") {
        "CONNECTION_TIMEOUT"
    } else if normalized.contains("certificate")
        || normalized.contains("tls")
        || normalized.contains("ssl")
    {
        "TLS_ERROR"
    } else if normalized.contains("login failed")
        || normalized.contains("authentication")
        || normalized.contains("password")
    {
        "AUTHENTICATION_FAILED"
    } else if normalized.contains("database") && normalized.contains("not") {
        "DATABASE_NOT_FOUND"
    } else if normalized.contains("permission") || normalized.contains("access denied") {
        "PERMISSION_DENIED"
    } else {
        "DRIVER_ERROR"
    };
    let recoverable = !matches!(
        code,
        "AUTHENTICATION_FAILED" | "DATABASE_NOT_FOUND" | "PERMISSION_DENIED"
    );
    ConnectionOperationError::new(code, message, recoverable)
}

fn redact_secret(message: String, secret: &str) -> String {
    if secret.is_empty() {
        message
    } else {
        message.replace(secret, "[REDACTED]")
    }
}

fn binary_text(value: &[u8]) -> String {
    value
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn postgres_value(row: &PgRow, index: usize, type_name: &str) -> JsonValue {
    let normalized_type = type_name.to_ascii_lowercase();
    match normalized_type.as_str() {
        "bool" => row
            .try_get::<Option<bool>, _>(index)
            .ok()
            .flatten()
            .map(JsonValue::Bool)
            .unwrap_or(JsonValue::Null),
        "int2" => row
            .try_get::<Option<i16>, _>(index)
            .ok()
            .flatten()
            .map(|value| JsonValue::from(i64::from(value)))
            .unwrap_or(JsonValue::Null),
        "int4" => row
            .try_get::<Option<i32>, _>(index)
            .ok()
            .flatten()
            .map(|value| JsonValue::from(i64::from(value)))
            .unwrap_or(JsonValue::Null),
        "int8" => row
            .try_get::<Option<i64>, _>(index)
            .ok()
            .flatten()
            .map(JsonValue::from)
            .unwrap_or(JsonValue::Null),
        "float4" => row
            .try_get::<Option<f32>, _>(index)
            .ok()
            .flatten()
            .and_then(|value| serde_json::Number::from_f64(f64::from(value)))
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        "float8" => row
            .try_get::<Option<f64>, _>(index)
            .ok()
            .flatten()
            .and_then(serde_json::Number::from_f64)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        "json" | "jsonb" => row
            .try_get::<Option<JsonValue>, _>(index)
            .ok()
            .flatten()
            .unwrap_or(JsonValue::Null),
        "bytea" => row
            .try_get::<Option<Vec<u8>>, _>(index)
            .ok()
            .flatten()
            .map(|value| JsonValue::String(binary_text(&value)))
            .unwrap_or(JsonValue::Null),
        "date" => row
            .try_get::<Option<chrono::NaiveDate>, _>(index)
            .ok()
            .flatten()
            .map(|value| JsonValue::String(value.to_string()))
            .unwrap_or(JsonValue::Null),
        "time" | "timetz" => row
            .try_get::<Option<chrono::NaiveTime>, _>(index)
            .ok()
            .flatten()
            .map(|value| JsonValue::String(value.to_string()))
            .unwrap_or(JsonValue::Null),
        "timestamp" => row
            .try_get::<Option<chrono::NaiveDateTime>, _>(index)
            .ok()
            .flatten()
            .map(|value| JsonValue::String(value.to_string()))
            .unwrap_or(JsonValue::Null),
        "timestamptz" => row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(index)
            .ok()
            .flatten()
            .map(|value| JsonValue::String(value.to_rfc3339()))
            .unwrap_or(JsonValue::Null),
        "uuid" => row
            .try_get::<Option<uuid::Uuid>, _>(index)
            .ok()
            .flatten()
            .map(|value| JsonValue::String(value.to_string()))
            .unwrap_or(JsonValue::Null),
        _ => row
            .try_get::<Option<String>, _>(index)
            .ok()
            .flatten()
            .map(JsonValue::String)
            .unwrap_or(JsonValue::Null),
    }
}

fn sqlserver_value(value: &tedious_rs::value_codec::Value) -> JsonValue {
    use tedious_rs::value_codec::Value;
    match value {
        Value::Null => JsonValue::Null,
        Value::Integer(value) => JsonValue::from(*value),
        Value::BigInteger(value) | Value::Text(value) | Value::Guid(value) => {
            JsonValue::String(value.clone())
        }
        Value::Boolean(value) => JsonValue::Bool(*value),
        Value::Real(value) => serde_json::Number::from_f64(f64::from(*value))
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        Value::Float(value) => serde_json::Number::from_f64(*value)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        Value::Binary(value) => JsonValue::String(binary_text(value)),
        Value::Date { year, month, day } | Value::LocalDate { year, month, day } => {
            JsonValue::String(format!("{year:04}-{month:02}-{day:02}"))
        }
        Value::Time {
            hour,
            minute,
            second,
            nanosecond,
        }
        | Value::LocalTime {
            hour,
            minute,
            second,
            nanosecond,
        } => JsonValue::String(format!("{hour:02}:{minute:02}:{second:02}.{nanosecond:09}")),
        Value::DateTime { date, time } | Value::LocalDateTime { date, time } => {
            let date = sqlserver_value(date);
            let time = sqlserver_value(time);
            JsonValue::String(format!(
                "{}T{}",
                date.as_str().unwrap_or_default(),
                time.as_str().unwrap_or_default()
            ))
        }
        Value::DateTimeOffset {
            date,
            time,
            offset_minutes,
        } => {
            let date = sqlserver_value(date);
            let time = sqlserver_value(time);
            let sign = if *offset_minutes < 0 { '-' } else { '+' };
            let minutes = offset_minutes.unsigned_abs();
            JsonValue::String(format!(
                "{}T{}{}{:02}:{:02}",
                date.as_str().unwrap_or_default(),
                time.as_str().unwrap_or_default(),
                sign,
                minutes / 60,
                minutes % 60
            ))
        }
        Value::Decimal { unscaled, scale } => {
            if *scale == 0 {
                JsonValue::String(unscaled.to_string())
            } else {
                let negative = *unscaled < 0;
                let digits = unscaled.unsigned_abs().to_string();
                let scale = usize::from(*scale);
                let value = if digits.len() <= scale {
                    format!("0.{}{}", "0".repeat(scale - digits.len()), digits)
                } else {
                    let split = digits.len() - scale;
                    format!("{}.{}", &digits[..split], &digits[split..])
                };
                JsonValue::String(if negative { format!("-{value}") } else { value })
            }
        }
        Value::Tvp(_) | Value::Udt { .. } => JsonValue::String("[unsupported value]".to_owned()),
    }
}

fn property<'a>(
    profile: &'a crate::backend::domain::connection::ConnectionProfileRecord,
    key: &str,
) -> Option<&'a str> {
    profile.driver_properties.get(key).map(String::as_str)
}

fn property_u64(
    profile: &crate::backend::domain::connection::ConnectionProfileRecord,
    key: &str,
) -> Option<u64> {
    property(profile, key)?.parse().ok()
}

fn property_bool(
    profile: &crate::backend::domain::connection::ConnectionProfileRecord,
    key: &str,
) -> Option<bool> {
    property(profile, key).and_then(|value| match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" => Some(true),
        "false" | "no" | "0" => Some(false),
        _ => None,
    })
}

fn postgres_ssl_mode(
    profile: &crate::backend::domain::connection::ConnectionProfileRecord,
) -> PgSslMode {
    match property(profile, "sslMode") {
        Some("disable") => PgSslMode::Disable,
        Some("require") => PgSslMode::Require,
        Some("verifyCa") => PgSslMode::VerifyCa,
        Some("verifyFull") => PgSslMode::VerifyFull,
        _ if profile.trust_server_certificate => PgSslMode::Require,
        _ => PgSslMode::Prefer,
    }
}

fn apply_postgres_properties(
    mut options: PgConnectOptions,
    profile: &crate::backend::domain::connection::ConnectionProfileRecord,
) -> PgConnectOptions {
    if let Some(application_name) = property(profile, "applicationName")
        && !application_name.trim().is_empty()
    {
        options = options.application_name(application_name);
    }
    if let Some(capacity) = property_u64(profile, "statementCacheCapacity") {
        options = options.statement_cache_capacity(capacity as usize);
    }
    if let Some(timeout) = property_u64(profile, "statementTimeoutMilliseconds")
        && timeout > 0
    {
        options = options.options([(String::from("statement_timeout"), timeout.to_string())]);
    }
    if let Some(extra_float_digits) =
        property(profile, "extraFloatDigits").and_then(|value| value.parse::<i8>().ok())
    {
        options = options.extra_float_digits(extra_float_digits);
    }
    if let Some(search_path) = property(profile, "searchPath")
        && !search_path.trim().is_empty()
    {
        options = options.options([(String::from("search_path"), search_path)]);
    }
    options
}

fn apply_sqlserver_properties(
    mut config: tedious_rs::ConnectionConfig,
    profile: &crate::backend::domain::connection::ConnectionProfileRecord,
) -> tedious_rs::ConnectionConfig {
    if let Some(application_name) = property(profile, "applicationName")
        && !application_name.trim().is_empty()
    {
        config.app_name = application_name.to_owned();
    }
    if let Some(timeout) = property_u64(profile, "connectTimeoutSeconds") {
        config.connect_timeout = Duration::from_secs(timeout);
    }
    if let Some(timeout) = property_u64(profile, "requestTimeoutSeconds") {
        config.request_timeout = Duration::from_secs(timeout);
    }
    if let Some(packet_size) = property_u64(profile, "packetSize") {
        config.packet_size = packet_size as usize;
    }
    if let Some(multi_subnet_failover) = property_bool(profile, "multiSubnetFailover") {
        config.multi_subnet_failover = multi_subnet_failover;
    }
    if let Some(encrypt) = property_bool(profile, "encrypt") {
        config.encrypt = encrypt;
    }
    if let Some(fallback) = property_bool(profile, "fallbackToDefaultDb") {
        config.fallback_to_default_db = fallback;
    }
    if let Some(use_utc) = property_bool(profile, "useUtc") {
        config.use_utc = use_utc;
    }
    if let Some(row_collection_on_done) = property_bool(profile, "rowCollectionOnDone") {
        config.row_collection_on_done = row_collection_on_done;
    }
    config
}

struct PostgresqlRuntimeConnection {
    pool: sqlx::PgPool,
    _transport_guard: Option<Arc<TransportGuard>>,
}

type PostgresqlRowStream = Pin<Box<dyn Stream<Item = Result<PgRow, sqlx::Error>> + Send>>;

struct PostgresqlResultSession {
    columns: Vec<QueryColumn>,
    state: tokio::sync::Mutex<PostgresqlResultSessionState>,
}

struct PostgresqlResultSessionState {
    stream: PostgresqlRowStream,
    pending: Option<PgRow>,
    total_fetched: AtomicUsize,
    terminal: Option<&'static str>,
}

impl PostgresqlResultSession {
    async fn read_batch(
        &self,
        requested_rows: usize,
    ) -> Result<QueryResultBatch, ConnectionOperationError> {
        let mut state = self.state.lock().await;
        if let Some(terminal) = state.terminal {
            return Err(ConnectionOperationError::new(
                terminal,
                "Result session is no longer available",
                false,
            ));
        }
        let requested_rows = requested_rows.max(1);
        let mut rows = Vec::with_capacity(requested_rows);
        while rows.len() < requested_rows {
            let row = if let Some(row) = state.pending.take() {
                Some(row)
            } else {
                state.stream.next().await.transpose().map_err(|error| {
                    state.terminal = Some("RESULT_SESSION_FETCH_FAILED");
                    driver_error(error.to_string())
                })?
            };
            let Some(row) = row else {
                state.terminal = Some("RESULT_SESSION_EXHAUSTED");
                return Ok(QueryResultBatch {
                    row_count: rows.len(),
                    has_more: false,
                    total_fetched: state.total_fetched.load(Ordering::Acquire) + rows.len(),
                    rows: rows
                        .iter()
                        .map(|row: &PgRow| {
                            self.columns
                                .iter()
                                .enumerate()
                                .map(|(index, column)| {
                                    postgres_value(row, index, &column.type_name)
                                })
                                .collect()
                        })
                        .collect(),
                });
            };
            rows.push(row);
        }
        let has_more = match state.stream.next().await.transpose().map_err(|error| {
            state.terminal = Some("RESULT_SESSION_FETCH_FAILED");
            driver_error(error.to_string())
        })? {
            Some(row) => {
                state.pending = Some(row);
                true
            }
            None => {
                state.terminal = Some("RESULT_SESSION_EXHAUSTED");
                false
            }
        };
        let total_fetched =
            state.total_fetched.fetch_add(rows.len(), Ordering::AcqRel) + rows.len();
        let converted_rows = rows
            .iter()
            .map(|row| {
                self.columns
                    .iter()
                    .enumerate()
                    .map(|(index, column)| postgres_value(row, index, &column.type_name))
                    .collect()
            })
            .collect();
        Ok(QueryResultBatch {
            row_count: rows.len(),
            rows: converted_rows,
            has_more,
            total_fetched,
        })
    }
}

impl RuntimeResultSession for PostgresqlResultSession {
    fn fetch_next(
        &self,
        requested_rows: usize,
    ) -> BoxFuture<'_, Result<QueryResultBatch, ConnectionOperationError>> {
        Box::pin(self.read_batch(requested_rows))
    }

    fn cancel(&self) -> BoxFuture<'_, Result<(), ConnectionOperationError>> {
        Box::pin(async {
            let mut state = self.state.lock().await;
            state.terminal = Some("RESULT_SESSION_CANCELLED");
            state.pending = None;
            state.stream = Box::pin(futures_util::stream::empty());
            Ok(())
        })
    }

    fn close(&self) -> BoxFuture<'_, Result<(), ConnectionOperationError>> {
        Box::pin(async {
            let mut state = self.state.lock().await;
            state.terminal = Some("RESULT_SESSION_CLOSED");
            state.pending = None;
            state.stream = Box::pin(futures_util::stream::empty());
            Ok(())
        })
    }
}

impl RuntimeConnection for PostgresqlRuntimeConnection {
    fn close(&self) -> BoxFuture<'_, Result<(), ConnectionOperationError>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            pool.close().await;
            Ok(())
        })
    }

    fn server_version(&self) -> BoxFuture<'_, Result<Option<String>, ConnectionOperationError>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            tokio::time::timeout(
                QUERY_TIMEOUT,
                sqlx::query_scalar::<_, String>("SELECT version()").fetch_optional(&pool),
            )
            .await
            .map_err(|_| driver_error("server version timeout"))?
            .map_err(|error| driver_error(error.to_string()))
        })
    }

    fn ping(&self) -> BoxFuture<'_, Result<(), ConnectionOperationError>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            tokio::time::timeout(
                Duration::from_secs(5),
                sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&pool),
            )
            .await
            .map_err(|_| driver_error("connection health check timeout"))?
            .map(|_| ())
            .map_err(|error| driver_error(error.to_string()))
        })
    }

    fn execute(&self, sql: String) -> BoxFuture<'_, Result<QueryResult, ConnectionOperationError>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let started = Instant::now();
            let statement = tokio::time::timeout(
                QUERY_TIMEOUT,
                pool.prepare(sqlx::AssertSqlSafe(sql.clone()).into_sql_str()),
            )
            .await
            .map_err(|_| driver_error("query timeout"))?
            .map_err(|error| driver_error(error.to_string()))?;
            let columns: Vec<QueryColumn> = statement
                .columns()
                .iter()
                .map(|column| QueryColumn {
                    name: column.name().to_owned(),
                    type_name: column.type_info().name().to_owned(),
                })
                .collect();
            let rows = tokio::time::timeout(
                QUERY_TIMEOUT,
                sqlx::query(sqlx::AssertSqlSafe(sql)).fetch_all(&pool),
            )
            .await
            .map_err(|_| driver_error("query timeout"))?
            .map_err(|error| driver_error(error.to_string()))?;
            let result_rows = rows
                .iter()
                .map(|row| {
                    columns
                        .iter()
                        .enumerate()
                        .map(|(index, column)| postgres_value(row, index, &column.type_name))
                        .collect()
                })
                .collect::<Vec<Vec<JsonValue>>>();
            Ok(QueryResult {
                row_count: result_rows.len(),
                columns,
                rows: result_rows,
                affected_rows: None,
                elapsed_ms: started.elapsed().as_millis(),
            })
        })
    }

    fn start_result_session(
        &self,
        sql: String,
        initial_rows: usize,
    ) -> BoxFuture<'_, Result<RuntimeResultSessionStart, ConnectionOperationError>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let started = Instant::now();
            let statement = tokio::time::timeout(
                QUERY_TIMEOUT,
                pool.prepare(sqlx::AssertSqlSafe(sql.clone()).into_sql_str()),
            )
            .await
            .map_err(|_| driver_error("query timeout"))?
            .map_err(|error| driver_error(error.to_string()))?;
            let columns = statement
                .columns()
                .iter()
                .map(|column| QueryColumn {
                    name: column.name().to_owned(),
                    type_name: column.type_info().name().to_owned(),
                })
                .collect::<Vec<_>>();
            let stream: PostgresqlRowStream =
                Box::pin(sqlx::query::<Postgres>(sqlx::AssertSqlSafe(sql)).fetch(&pool));
            let session = Arc::new(PostgresqlResultSession {
                columns: columns.clone(),
                state: tokio::sync::Mutex::new(PostgresqlResultSessionState {
                    stream,
                    pending: None,
                    total_fetched: AtomicUsize::new(0),
                    terminal: None,
                }),
            });
            let first_batch = session.read_batch(initial_rows).await?;
            let session_id = uuid::Uuid::new_v4().to_string();
            Ok(RuntimeResultSessionStart {
                result: crate::backend::domain::query::QueryResultSessionStart {
                    session_id,
                    columns,
                    first_batch,
                    elapsed_ms: started.elapsed().as_millis(),
                },
                session,
            })
        })
    }
}

enum SqlServerStreamEvent {
    Batch {
        columns: Vec<QueryColumn>,
        batch: QueryResultBatch,
    },
    Error(ConnectionOperationError),
}

struct SqlServerStreamState {
    columns: Vec<QueryColumn>,
    rows: Vec<Vec<JsonValue>>,
    terminal: bool,
    finished: bool,
    total_fetched: usize,
    requested_rows: Arc<AtomicUsize>,
}

impl SqlServerStreamState {
    fn drain_batch(&mut self, pause_handle: &StreamPauseHandle) -> Option<QueryResultBatch> {
        if self.finished {
            return None;
        }
        let requested_rows = self.requested_rows.load(Ordering::Acquire).max(1);
        if self.rows.len() <= requested_rows && !self.terminal {
            return None;
        }
        let row_count = self.rows.len().min(requested_rows);
        let rows = self.rows.drain(..row_count).collect::<Vec<_>>();
        let has_more = !self.rows.is_empty();
        if has_more {
            pause_handle.pause();
        } else if self.terminal {
            self.finished = true;
        }
        self.total_fetched += rows.len();
        Some(QueryResultBatch {
            row_count: rows.len(),
            rows,
            has_more,
            total_fetched: self.total_fetched,
        })
    }
}

struct SqlServerResultSession {
    receiver: tokio::sync::Mutex<mpsc::UnboundedReceiver<SqlServerStreamEvent>>,
    state: Arc<StdMutex<SqlServerStreamState>>,
    pause_handle: Arc<StdMutex<Option<StreamPauseHandle>>>,
    cancellation: tedious_rs::request::RequestCancellationHandle,
    task: tokio::sync::Mutex<Option<tokio::task::JoinHandle<Result<(), ConnectionOperationError>>>>,
    operation: tokio::sync::Mutex<()>,
    connection: Arc<tokio::sync::Mutex<Option<Connection>>>,
    standby_connection: Arc<tokio::sync::Mutex<Option<Connection>>>,
    standby_available: Arc<AtomicBool>,
    config: Arc<tedious_rs::ConnectionConfig>,
    active_result_session: Arc<AtomicBool>,
    stopped: AtomicBool,
}

impl SqlServerResultSession {
    fn take_buffered_batch(&self) -> Option<QueryResultBatch> {
        let pause_handle = self
            .pause_handle
            .lock()
            .expect("SQL Server result pause handle poisoned")
            .clone();
        let pause_handle = pause_handle?;
        let mut state = self.state.lock().expect("SQL Server result state poisoned");
        let batch = state.drain_batch(&pause_handle);
        drop(state);
        if batch.as_ref().is_some_and(|batch| !batch.has_more) {
            if let Some(pause_handle) = self
                .pause_handle
                .lock()
                .expect("SQL Server result pause handle poisoned")
                .take()
            {
                pause_handle.resume();
            }
        }
        batch
    }

    async fn stop(&self) -> Result<(), ConnectionOperationError> {
        if self.stopped.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        if let Some(pause_handle) = self
            .pause_handle
            .lock()
            .expect("SQL Server result pause handle poisoned")
            .take()
        {
            pause_handle.resume();
        }
        self.cancellation.cancel();
        let _operation = tokio::time::timeout(RESULT_SESSION_CLOSE_TIMEOUT, self.operation.lock())
            .await
            .ok();
        let mut task = self.task.lock().await.take();
        let mut timed_out = false;
        if let Some(task_handle) = task.as_mut() {
            match tokio::time::timeout(RESULT_SESSION_CLOSE_TIMEOUT, &mut *task_handle).await {
                Ok(result) => {
                    result.map_err(|error| {
                        ConnectionOperationError::new(
                            "RESULT_SESSION_TASK_FAILED",
                            error.to_string(),
                            false,
                        )
                    })??;
                }
                Err(_) => {
                    timed_out = true;
                    task_handle.abort();
                    let _ = task_handle.await;
                }
            }
        }
        drop(task);
        if timed_out {
            let mut connection = self.connection.lock().await;
            if connection.is_none() {
                if let Some(replacement) = self.standby_connection.lock().await.take() {
                    self.standby_available.store(false, Ordering::Release);
                    *connection = Some(replacement);
                } else {
                    let mut replacement = Connection::new((*self.config).clone());
                    replacement
                        .connect()
                        .await
                        .map_err(|error| driver_error(error.to_string()))?;
                    *connection = Some(replacement);
                }
            }
        }
        self.active_result_session.store(false, Ordering::Release);
        Ok(())
    }

    async fn receive_batch(&self) -> Result<QueryResultBatch, ConnectionOperationError> {
        let mut receiver = self.receiver.lock().await;
        match receiver.recv().await {
            Some(SqlServerStreamEvent::Batch { batch, .. }) => Ok(batch),
            Some(SqlServerStreamEvent::Error(error)) => Err(error),
            None => Err(ConnectionOperationError::new(
                "RESULT_SESSION_CLOSED",
                "SQL Server result stream ended before a batch was produced",
                false,
            )),
        }
    }
}

impl RuntimeResultSession for SqlServerResultSession {
    fn fetch_next(
        &self,
        requested_rows: usize,
    ) -> BoxFuture<'_, Result<QueryResultBatch, ConnectionOperationError>> {
        Box::pin(async move {
            if self.stopped.load(Ordering::Acquire) {
                return Err(ConnectionOperationError::new(
                    "RESULT_SESSION_CLOSED",
                    "SQL Server result session is closed",
                    true,
                ));
            }
            let _operation = self.operation.lock().await;
            if self.stopped.load(Ordering::Acquire) {
                return Err(ConnectionOperationError::new(
                    "RESULT_SESSION_CLOSED",
                    "SQL Server result session is closed",
                    true,
                ));
            }
            let requested_rows = requested_rows.max(1);
            if let Ok(state) = self.state.lock() {
                state
                    .requested_rows
                    .store(requested_rows, Ordering::Release);
            }
            if let Some(batch) = self.take_buffered_batch() {
                return Ok(batch);
            }
            if let Some(pause_handle) = self
                .pause_handle
                .lock()
                .expect("SQL Server result pause handle poisoned")
                .take()
            {
                pause_handle.resume();
            }
            self.receive_batch().await
        })
    }

    fn cancel(&self) -> BoxFuture<'_, Result<(), ConnectionOperationError>> {
        Box::pin(self.stop())
    }

    fn close(&self) -> BoxFuture<'_, Result<(), ConnectionOperationError>> {
        Box::pin(self.stop())
    }
}

fn sqlserver_stream_connection_error(message: impl Into<String>) -> tedious_rs::ConnectionError {
    tedious_rs::ConnectionError::new(message, Some("ERESULTSTREAM".to_owned()))
}

fn process_sqlserver_stream_tokens(
    tokens: &[tedious_rs::token::stream::StreamToken],
    pause_handle: &StreamPauseHandle,
    state: &Arc<StdMutex<SqlServerStreamState>>,
    pause_slot: &Arc<StdMutex<Option<StreamPauseHandle>>>,
    sender: &mpsc::UnboundedSender<SqlServerStreamEvent>,
    cancellation: &tedious_rs::request::RequestCancellationHandle,
) -> Result<(), tedious_rs::ConnectionError> {
    let mut error = None;
    let mut event = None;
    {
        let mut state = state.lock().expect("SQL Server result state poisoned");
        if state.finished {
            return Ok(());
        }
        for token in tokens {
            match token {
                tedious_rs::token::stream::StreamToken::ColMetadata(metadata) => {
                    state.columns = metadata
                        .iter()
                        .map(|column| QueryColumn {
                            name: column.col_name.clone(),
                            type_name: format!("{:?}", column.metadata.data_type),
                        })
                        .collect();
                }
                tedious_rs::token::stream::StreamToken::Row(row)
                | tedious_rs::token::stream::StreamToken::NbcRow(row) => {
                    state.rows.push(
                        row.columns
                            .iter()
                            .map(|column| sqlserver_value(&column.value))
                            .collect(),
                    );
                }
                tedious_rs::token::stream::StreamToken::Control(
                    tedious_rs::token::parser::Token::Error(message),
                ) => {
                    error = Some(driver_error(format_sqlserver_error(message)));
                    break;
                }
                tedious_rs::token::stream::StreamToken::Control(
                    tedious_rs::token::parser::Token::Done(done),
                ) if done.more => {
                    error = Some(ConnectionOperationError::new(
                        "RESULT_MULTIPLE_SETS_UNSUPPORTED",
                        "SQL Server result sessions currently support one result set per query",
                        false,
                    ));
                    break;
                }
                tedious_rs::token::stream::StreamToken::Control(
                    tedious_rs::token::parser::Token::Done(done),
                ) => {
                    if done.sql_error {
                        error = Some(driver_error(
                            "SQL Server reported an execution error without details",
                        ));
                        break;
                    }
                    state.terminal = true;
                }
                tedious_rs::token::stream::StreamToken::Control(_) => {}
            }
        }
        if let Some(operation_error) = error.clone() {
            state.finished = true;
            event = Some(SqlServerStreamEvent::Error(operation_error));
        } else if let Some(batch) = state.drain_batch(pause_handle) {
            if batch.has_more {
                pause_slot
                    .lock()
                    .expect("SQL Server result pause handle poisoned")
                    .replace(pause_handle.clone());
            }
            event = Some(SqlServerStreamEvent::Batch {
                columns: state.columns.clone(),
                batch,
            });
        }
    }
    if let Some(event) = event {
        sender
            .send(event)
            .map_err(|error| sqlserver_stream_connection_error(error.to_string()))?;
    }
    if error.is_some() {
        cancellation.cancel();
    }
    Ok(())
}

struct SqlServerRuntimeConnection {
    connection: Arc<tokio::sync::Mutex<Option<Connection>>>,
    standby_connection: Arc<tokio::sync::Mutex<Option<Connection>>>,
    standby_available: Arc<AtomicBool>,
    standby_requested: Arc<AtomicBool>,
    config: Arc<tedious_rs::ConnectionConfig>,
    active_result_session: Arc<AtomicBool>,
    closed: Arc<AtomicBool>,
    _transport_guard: Option<Arc<TransportGuard>>,
}

impl SqlServerRuntimeConnection {
    fn prewarm_standby(&self) {
        if self.closed.load(Ordering::Acquire)
            || self.standby_available.load(Ordering::Acquire)
            || self.standby_requested.swap(true, Ordering::AcqRel)
        {
            return;
        }
        let standby_connection = self.standby_connection.clone();
        let standby_available = self.standby_available.clone();
        let standby_requested = self.standby_requested.clone();
        let config = self.config.clone();
        let closed = self.closed.clone();
        tokio::spawn(async move {
            let mut standby = Connection::new((*config).clone());
            let result = tokio::time::timeout(CONNECTION_TIMEOUT, standby.connect())
                .await
                .map_err(|_| driver_error("standby connection timeout"))
                .and_then(|result| result.map_err(|error| driver_error(error.to_string())));
            if let Ok(()) = result {
                if closed.load(Ordering::Acquire) {
                    standby.close().await;
                } else {
                    let mut slot = standby_connection.lock().await;
                    if slot.is_none() {
                        *slot = Some(standby);
                        standby_available.store(true, Ordering::Release);
                    } else {
                        standby.close().await;
                    }
                }
            }
            standby_requested.store(false, Ordering::Release);
        });
    }
}

impl RuntimeConnection for SqlServerRuntimeConnection {
    fn close(&self) -> BoxFuture<'_, Result<(), ConnectionOperationError>> {
        let connection = &self.connection;
        let standby_connection = &self.standby_connection;
        self.closed.store(true, Ordering::Release);
        Box::pin(async move {
            let mut connection = connection.lock().await;
            if let Some(connection) = connection.as_mut() {
                connection.close().await;
            }
            *connection = None;
            let mut standby = standby_connection.lock().await;
            if let Some(connection) = standby.as_mut() {
                connection.close().await;
            }
            *standby = None;
            Ok(())
        })
    }

    fn server_version(&self) -> BoxFuture<'_, Result<Option<String>, ConnectionOperationError>> {
        Box::pin(async move {
            let tokens = tokio::time::timeout(QUERY_TIMEOUT, async {
                let mut connection = self.connection.lock().await;
                let Some(connection) = connection.as_mut() else {
                    return Ok::<_, ConnectionOperationError>(None);
                };
                connection
                    .exec_sql_batch("SELECT CAST(SERVERPROPERTY('ProductVersion') AS VARCHAR(128))")
                    .await
                    .map(Some)
                    .map_err(|error| driver_error(error.to_string()))
            })
            .await
            .map_err(|_| driver_error("server version timeout"))??;
            let Some(tokens) = tokens else {
                return Ok(None);
            };
            for token in tokens {
                if let tedious_rs::token::stream::StreamToken::Row(row)
                | tedious_rs::token::stream::StreamToken::NbcRow(row) = token
                    && let Some(column) = row.columns.first()
                {
                    return match &column.value {
                        tedious_rs::value_codec::Value::Text(value) => Ok(Some(value.clone())),
                        value => Ok(Some(format!("{value:?}"))),
                    };
                }
            }
            Ok(None)
        })
    }

    fn ping(&self) -> BoxFuture<'_, Result<(), ConnectionOperationError>> {
        Box::pin(async move {
            tokio::time::timeout(Duration::from_secs(5), async {
                let mut connection = self.connection.lock().await;
                let Some(connection) = connection.as_mut() else {
                    if self.active_result_session.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    return Err(driver_error("Connection is closed"));
                };
                connection
                    .exec_sql_batch("SELECT 1")
                    .await
                    .map(|_| ())
                    .map_err(|error| driver_error(error.to_string()))
            })
            .await
            .map_err(|_| driver_error("connection health check timeout"))?
        })
    }

    fn session_telemetry(
        &self,
    ) -> BoxFuture<'_, Result<SessionTelemetry, ConnectionOperationError>> {
        Box::pin(async move {
            let tokens = tokio::time::timeout(QUERY_TIMEOUT, async {
                let mut connection = self.connection.lock().await;
                if let Some(connection) = connection.as_mut() {
                    return connection
                        .exec_sql_batch(SESSION_TELEMETRY_SQL)
                        .await
                        .map_err(|error| driver_error(error.to_string()));
                }
                drop(connection);
                let mut standby = self.standby_connection.lock().await;
                let connection = standby
                    .as_mut()
                    .ok_or_else(|| driver_error("Connection is closed"))?;
                connection
                    .exec_sql_batch(SESSION_TELEMETRY_SQL)
                    .await
                    .map_err(|error| driver_error(error.to_string()))
            })
            .await
            .map_err(|_| driver_error("Session telemetry query timed out"))??;
            let mut sessions = Vec::new();
            let mut monitor_session_id = None;
            for token in tokens {
                match token {
                    tedious_rs::token::stream::StreamToken::Row(row)
                    | tedious_rs::token::stream::StreamToken::NbcRow(row) => {
                        let values = row
                            .columns
                            .iter()
                            .map(|column| &column.value)
                            .collect::<Vec<_>>();
                        monitor_session_id =
                            session_i32(values.first().copied()).or(monitor_session_id);
                        if let Some(session) = map_database_session_row(&values) {
                            sessions.push(session);
                        }
                    }
                    tedious_rs::token::stream::StreamToken::Control(
                        tedious_rs::token::parser::Token::Error(error),
                    ) => return Err(session_sql_error(&error)),
                    tedious_rs::token::stream::StreamToken::Control(
                        tedious_rs::token::parser::Token::Done(done),
                    ) if done.sql_error => {
                        return Err(ConnectionOperationError::new(
                            "SESSION_TELEMETRY_QUERY_FAILED",
                            "SQL Server could not complete the session telemetry query",
                            true,
                        ));
                    }
                    _ => {}
                }
            }
            let monitor_session_id = monitor_session_id.ok_or_else(|| {
                ConnectionOperationError::new(
                    "SESSION_TELEMETRY_RESULT_INVALID",
                    "SQL Server did not return the monitor session id",
                    true,
                )
            })?;
            annotate_blocking_graph(&mut sessions);
            Ok(SessionTelemetry {
                sessions,
                monitor_session_id,
                captured_at: chrono::Utc::now().to_rfc3339(),
            })
        })
    }

    fn kill_session(
        &self,
        session_id: i32,
    ) -> BoxFuture<'_, Result<SessionKillResult, ConnectionOperationError>> {
        Box::pin(async move {
            tokio::time::timeout(QUERY_TIMEOUT, async {
                let mut connection = self.connection.lock().await;
                if let Some(connection) = connection.as_mut() {
                    return kill_sqlserver_session(connection, session_id).await;
                }
                drop(connection);
                let mut standby = self.standby_connection.lock().await;
                let connection = standby
                    .as_mut()
                    .ok_or_else(|| driver_error("Connection is closed"))?;
                kill_sqlserver_session(connection, session_id).await
            })
            .await
            .map_err(|_| driver_error("Killing the SQL Server session timed out"))?
        })
    }

    fn execute(&self, sql: String) -> BoxFuture<'_, Result<QueryResult, ConnectionOperationError>> {
        let connection = &self.connection;
        Box::pin(async move {
            let started = Instant::now();
            let tokens = tokio::time::timeout(QUERY_TIMEOUT, async {
                let mut connection = connection.lock().await;
                let Some(connection) = connection.as_mut() else {
                    return Err(driver_error("Connection is closed"));
                };
                connection
                    .exec_sql_batch(sql)
                    .await
                    .map_err(|error| driver_error(error.to_string()))
            })
            .await
            .map_err(|_| driver_error("query timeout"))??;
            let mut columns = Vec::new();
            let mut rows = Vec::new();
            let mut affected_rows = None;
            for token in tokens {
                match token {
                    tedious_rs::token::stream::StreamToken::ColMetadata(metadata) => {
                        columns = metadata
                            .into_iter()
                            .map(|column| QueryColumn {
                                name: column.col_name,
                                type_name: format!("{:?}", column.metadata.data_type),
                            })
                            .collect();
                    }
                    tedious_rs::token::stream::StreamToken::Row(row)
                    | tedious_rs::token::stream::StreamToken::NbcRow(row) => {
                        rows.push(
                            row.columns
                                .into_iter()
                                .map(|column| sqlserver_value(&column.value))
                                .collect(),
                        );
                    }
                    tedious_rs::token::stream::StreamToken::Control(
                        tedious_rs::token::parser::Token::Error(error),
                    ) => {
                        return Err(driver_error(format_sqlserver_error(&error)));
                    }
                    tedious_rs::token::stream::StreamToken::Control(
                        tedious_rs::token::parser::Token::Done(done),
                    ) => {
                        if done.sql_error {
                            return Err(driver_error(
                                "SQL Server reported an execution error without details",
                            ));
                        }
                        affected_rows = done.row_count.or(affected_rows);
                    }
                    tedious_rs::token::stream::StreamToken::Control(_) => {}
                }
            }
            Ok(QueryResult {
                row_count: rows.len(),
                columns,
                rows,
                affected_rows,
                elapsed_ms: started.elapsed().as_millis(),
            })
        })
    }

    fn start_result_session(
        &self,
        sql: String,
        initial_rows: usize,
    ) -> BoxFuture<'_, Result<RuntimeResultSessionStart, ConnectionOperationError>> {
        let connection = self.connection.clone();
        Box::pin(async move {
            let started = Instant::now();
            let mut source_slot = connection.lock().await;
            if self.active_result_session.swap(true, Ordering::AcqRel) {
                return Err(ConnectionOperationError::new(
                    "CONNECTION_BUSY",
                    "The SQL Server connection is already executing a result session",
                    true,
                ));
            }
            let Some(mut source) = source_slot.take() else {
                self.active_result_session.store(false, Ordering::Release);
                return Err(ConnectionOperationError::new(
                    "CONNECTION_BUSY",
                    "The SQL Server connection is already executing a result session",
                    true,
                ));
            };
            drop(source_slot);
            let (sender, mut receiver) = mpsc::unbounded_channel();
            let requested_rows = Arc::new(AtomicUsize::new(initial_rows.max(1)));
            let state = Arc::new(StdMutex::new(SqlServerStreamState {
                columns: Vec::new(),
                rows: Vec::new(),
                terminal: false,
                finished: false,
                total_fetched: 0,
                requested_rows: requested_rows.clone(),
            }));
            let pause_slot = Arc::new(StdMutex::new(None));
            let cancellation = Request::new("", |_| {}).cancellation_handle();
            let task_connection = connection.clone();
            let task_config = self.config.clone();
            let task_active_result_session = self.active_result_session.clone();
            let task_state = state.clone();
            let task_pause_slot = pause_slot.clone();
            let task_cancellation = cancellation.clone();
            let task = tokio::spawn(async move {
                let result = source
                    .exec_sql_batch_stream(
                        &sql,
                        Some(&task_cancellation),
                        |tokens, pause_handle| {
                            process_sqlserver_stream_tokens(
                                tokens,
                                pause_handle,
                                &task_state,
                                &task_pause_slot,
                                &sender,
                                &task_cancellation,
                            )
                        },
                    )
                    .await;
                let task_result = match result {
                    Ok(()) => Ok(()),
                    Err(error) if error.code.as_deref() == Some("ECANCEL") => Ok(()),
                    Err(error) => {
                        let operation_error = driver_error(error.to_string());
                        let _ = sender.send(SqlServerStreamEvent::Error(operation_error.clone()));
                        Err(operation_error)
                    }
                };
                let mut slot = task_connection.lock().await;
                let task_result = match task_result {
                    Ok(()) => {
                        if slot.is_none() {
                            *slot = Some(source);
                        }
                        Ok(())
                    }
                    Err(error) => {
                        source.close().await;
                        let mut replacement = Connection::new((*task_config).clone());
                        if replacement.connect().await.is_ok() {
                            *slot = Some(replacement);
                            Ok(())
                        } else {
                            Err(error)
                        }
                    }
                };
                task_active_result_session.store(false, Ordering::Release);
                task_result
            });
            let event = receiver.recv().await.ok_or_else(|| {
                ConnectionOperationError::new(
                    "RESULT_SESSION_START_FAILED",
                    "SQL Server did not produce a result batch",
                    false,
                )
            })?;
            let (columns, first_batch) = match event {
                SqlServerStreamEvent::Batch { columns, batch } => (columns, batch),
                SqlServerStreamEvent::Error(error) => {
                    cancellation.cancel();
                    let _ = task.await;
                    return Err(error);
                }
            };
            let session_id = uuid::Uuid::new_v4().to_string();
            let session = Arc::new(SqlServerResultSession {
                receiver: tokio::sync::Mutex::new(receiver),
                state,
                pause_handle: pause_slot,
                cancellation,
                task: tokio::sync::Mutex::new(Some(task)),
                operation: tokio::sync::Mutex::new(()),
                connection: connection.clone(),
                standby_connection: self.standby_connection.clone(),
                standby_available: self.standby_available.clone(),
                config: self.config.clone(),
                active_result_session: self.active_result_session.clone(),
                stopped: AtomicBool::new(false),
            });
            if first_batch.has_more {
                self.prewarm_standby();
            }
            Ok(RuntimeResultSessionStart {
                result: crate::backend::domain::query::QueryResultSessionStart {
                    session_id,
                    columns,
                    first_batch,
                    elapsed_ms: started.elapsed().as_millis(),
                },
                session,
            })
        })
    }
}

const SESSION_TELEMETRY_SQL: &str = r#"
SELECT
    CAST(@@SPID AS int) AS monitor_session_id,
    s.session_id,
    s.status,
    r.status AS request_status,
    DB_NAME(COALESCE(r.database_id, s.database_id)) AS database_name,
    s.login_name,
    s.original_login_name,
    s.host_name,
    s.program_name,
    c.client_interface_name,
    c.client_net_address,
    c.local_net_address,
    c.local_tcp_port,
    CONVERT(nvarchar(33), s.login_time, 127) AS login_time,
    r.command,
    CAST(DATEDIFF(DAY, telemetry_clock.started_at, telemetry_clock.captured_at) AS bigint) * 86400000
        + DATEDIFF(MILLISECOND, DATEADD(DAY, DATEDIFF(DAY, 0, telemetry_clock.captured_at), 0), telemetry_clock.captured_at)
        - DATEDIFF(MILLISECOND, DATEADD(DAY, DATEDIFF(DAY, 0, telemetry_clock.started_at), 0), telemetry_clock.started_at) AS duration_ms,
    COALESCE(r.cpu_time, 0) AS cpu_time_ms,
    COALESCE(r.reads, 0) AS reads,
    COALESCE(r.writes, 0) AS writes,
    COALESCE(r.logical_reads, 0) AS logical_reads,
    COALESCE(r.row_count, 0) AS row_count,
    r.wait_type,
    COALESCE(r.wait_time, 0) AS wait_time_ms,
    r.wait_resource,
    r.last_wait_type,
    r.blocking_session_id,
    s.open_transaction_count,
    s.transaction_isolation_level,
    CAST(st.text AS nvarchar(max)) AS batch_text,
    COALESCE(r.statement_start_offset, -1) AS statement_start_offset,
    COALESCE(r.statement_end_offset, -1) AS statement_end_offset,
    COALESCE(r.percent_complete, 0) AS percent_complete,
    COALESCE(r.estimated_completion_time, 0) AS estimated_completion_time_ms
FROM sys.dm_exec_sessions AS s
LEFT JOIN sys.dm_exec_requests AS r ON r.session_id = s.session_id
LEFT JOIN sys.dm_exec_connections AS c ON c.session_id = s.session_id
OUTER APPLY sys.dm_exec_sql_text(COALESCE(r.sql_handle, c.most_recent_sql_handle)) AS st
CROSS APPLY (
    SELECT COALESCE(r.start_time, s.last_request_start_time, s.login_time) AS started_at,
           GETDATE() AS captured_at
) AS telemetry_clock
WHERE s.is_user_process = 1
ORDER BY CASE WHEN r.blocking_session_id > 0 THEN 0 WHEN r.session_id IS NOT NULL THEN 1 ELSE 2 END,
         COALESCE(r.total_elapsed_time, CAST(DATEDIFF(DAY, s.last_request_start_time, GETDATE()) AS bigint) * 86400000, 0) DESC,
         s.session_id
"#;

fn session_sql_error(error: &tedious_rs::token::parser::MessageToken) -> ConnectionOperationError {
    let permission_denied = error.number == 229
        || error.number == 297
        || error.message.to_ascii_lowercase().contains("permission")
        || error
            .message
            .to_ascii_lowercase()
            .contains("not have permission");
    ConnectionOperationError::new(
        if permission_denied {
            "SESSION_TELEMETRY_PERMISSION_DENIED"
        } else {
            "SESSION_TELEMETRY_SQL_ERROR"
        },
        format!(
            "SQL Server error {} (state {}, class {}, server '{}', procedure '{}', line {}): {}",
            error.number,
            error.state,
            error.class,
            error.server_name,
            error.proc_name,
            error.line_number,
            error.message
        ),
        !permission_denied,
    )
}

async fn kill_sqlserver_session(
    connection: &mut Connection,
    session_id: i32,
) -> Result<SessionKillResult, ConnectionOperationError> {
    let monitor_tokens = connection
        .exec_sql_batch("SELECT CAST(@@SPID AS int)")
        .await
        .map_err(|error| driver_error(error.to_string()))?;
    let mut monitor_session_id = None;
    for token in monitor_tokens {
        match token {
            tedious_rs::token::stream::StreamToken::Row(row)
            | tedious_rs::token::stream::StreamToken::NbcRow(row) => {
                monitor_session_id = row
                    .columns
                    .first()
                    .and_then(|column| session_i32(Some(&column.value)));
            }
            tedious_rs::token::stream::StreamToken::Control(
                tedious_rs::token::parser::Token::Error(error),
            ) => return Err(session_sql_error(&error)),
            _ => {}
        }
    }
    let monitor_session_id = monitor_session_id.ok_or_else(|| {
        ConnectionOperationError::new(
            "SESSION_TELEMETRY_RESULT_INVALID",
            "SQL Server did not return the monitor session id",
            true,
        )
    })?;
    validate_kill_session_id(session_id, monitor_session_id)?;
    let tokens = connection
        .exec_sql_batch(format!("KILL {session_id}"))
        .await
        .map_err(|error| driver_error(error.to_string()))?;
    for token in tokens {
        if let tedious_rs::token::stream::StreamToken::Control(
            tedious_rs::token::parser::Token::Error(error),
        ) = token
        {
            return Err(session_sql_error(&error));
        }
    }
    Ok(SessionKillResult {
        session_id,
        message: format!("KILL was sent to session {session_id}"),
    })
}

pub fn map_database_session_row(
    values: &[&tedious_rs::value_codec::Value],
) -> Option<DatabaseSession> {
    use tedious_rs::value_codec::Value;
    let session_id = session_i32(values.get(1).copied())?;
    let text = |index: usize| match values.get(index).copied()? {
        Value::Text(value) => Some(value.clone()),
        Value::Integer(value) => Some(value.to_string()),
        _ => None,
    };
    let number = |index: usize| session_i64(values.get(index).copied()).unwrap_or_default();
    let batch_text = text(28);
    let statement_start_offset = number(29);
    let statement_end_offset = number(30);
    let current_statement = batch_text.as_ref().and_then(|batch| {
        extract_current_statement(batch, statement_start_offset, statement_end_offset)
    });
    let isolation = number(27) as i32;
    Some(DatabaseSession {
        session_id,
        status: text(2).unwrap_or_else(|| "unknown".to_owned()),
        request_status: text(3),
        database: text(4),
        login: text(5),
        original_login: text(6),
        host: text(7),
        application: text(8),
        client_interface: text(9),
        client_address: text(10),
        local_address: text(11),
        local_port: session_i32(values.get(12).copied()),
        login_time: text(13),
        command: text(14),
        duration_ms: number(15).to_string(),
        cpu_time_ms: number(16),
        reads: number(17).to_string(),
        writes: number(18).to_string(),
        logical_reads: number(19).to_string(),
        row_count: number(20).to_string(),
        wait_type: text(21),
        wait_time_ms: number(22),
        wait_resource: text(23),
        last_wait_type: text(24),
        blocking_session_id: session_i32(values.get(25).copied()).filter(|value| *value != 0),
        blocker_session_id: None,
        root_blocker_session_id: None,
        is_blocking: false,
        is_root_blocker: false,
        blocked_session_count: 0,
        blocking_depth: 0,
        open_transaction_count: number(26) as i32,
        transaction_isolation_level: isolation,
        transaction_isolation_name: transaction_isolation_name(isolation),
        batch_text,
        current_statement,
        percent_complete: session_f64(values.get(31).copied()).unwrap_or_default(),
        estimated_completion_time_ms: number(32).to_string(),
    })
}

fn session_i32(value: Option<&tedious_rs::value_codec::Value>) -> Option<i32> {
    session_i64(value).and_then(|value| i32::try_from(value).ok())
}

fn session_i64(value: Option<&tedious_rs::value_codec::Value>) -> Option<i64> {
    match value? {
        tedious_rs::value_codec::Value::Integer(value) => Some(*value),
        tedious_rs::value_codec::Value::BigInteger(value)
        | tedious_rs::value_codec::Value::Text(value) => value.parse().ok(),
        tedious_rs::value_codec::Value::Decimal { unscaled, scale: 0 } => {
            i64::try_from(*unscaled).ok()
        }
        _ => None,
    }
}

fn session_f64(value: Option<&tedious_rs::value_codec::Value>) -> Option<f64> {
    match value? {
        tedious_rs::value_codec::Value::Integer(value) => Some(*value as f64),
        tedious_rs::value_codec::Value::BigInteger(value)
        | tedious_rs::value_codec::Value::Text(value) => value.parse().ok(),
        tedious_rs::value_codec::Value::Real(value) => Some(f64::from(*value)),
        tedious_rs::value_codec::Value::Float(value) => Some(*value),
        tedious_rs::value_codec::Value::Decimal { unscaled, scale } => {
            Some(*unscaled as f64 / 10_f64.powi(i32::from(*scale)))
        }
        _ => None,
    }
}

fn format_sqlserver_error(error: &tedious_rs::token::parser::MessageToken) -> String {
    format!(
        "SQL Server error {} at line {}: {}",
        error.number, error.line_number, error.message
    )
}

struct PostgresqlConnectionDriver;

impl ConnectionDriver for PostgresqlConnectionDriver {
    fn capabilities(&self) -> DatabaseCapabilities {
        DatabaseCapabilities {
            databases: true,
            schemas: true,
            tables: true,
            views: true,
            materialized_views: true,
            procedures: true,
            functions: true,
            triggers: true,
            sequences: true,
            extensions: true,
            partitions: true,
            users: true,
            roles: true,
            permissions: true,
            synonyms: false,
            indexes: true,
            foreign_keys: true,
            custom_types: true,
        }
    }

    fn connect(
        &self,
        resolved: ResolvedConnection,
    ) -> BoxFuture<'static, Result<Arc<dyn RuntimeConnection>, ConnectionOperationError>> {
        Box::pin(async move {
            let password = resolved.password;
            let profile = resolved.profile;
            let endpoint = prepare_endpoint(&profile).await?;
            let options = PgConnectOptions::new()
                .host(&endpoint.host)
                .port(endpoint.port)
                .username(&profile.user_name)
                .password(&password)
                .ssl_mode(postgres_ssl_mode(&profile));
            let options = apply_postgres_properties(options, &profile);
            let options = if profile.database.trim().is_empty() {
                options
            } else {
                options.database(&profile.database)
            };
            let pool = tokio::time::timeout(
                CONNECTION_TIMEOUT,
                PgPoolOptions::new()
                    .max_connections(1)
                    .min_connections(0)
                    .acquire_timeout(CONNECTION_TIMEOUT)
                    .connect_with(options),
            )
            .await
            .map_err(|_| driver_error("connection timeout"))?
            .map_err(|error| driver_error(redact_secret(error.to_string(), &password)))?;
            Ok(Arc::new(PostgresqlRuntimeConnection {
                pool,
                _transport_guard: endpoint.guard,
            }) as Arc<dyn RuntimeConnection>)
        })
    }
}

struct SqlServerConnectionDriver;

impl ConnectionDriver for SqlServerConnectionDriver {
    fn capabilities(&self) -> DatabaseCapabilities {
        DatabaseCapabilities {
            databases: true,
            schemas: true,
            tables: true,
            views: true,
            materialized_views: false,
            procedures: true,
            functions: true,
            triggers: true,
            sequences: true,
            extensions: false,
            partitions: true,
            users: true,
            roles: true,
            permissions: true,
            synonyms: true,
            indexes: true,
            foreign_keys: true,
            custom_types: true,
        }
    }

    fn connect(
        &self,
        resolved: ResolvedConnection,
    ) -> BoxFuture<'static, Result<Arc<dyn RuntimeConnection>, ConnectionOperationError>> {
        Box::pin(async move {
            let password = resolved.password;
            let profile = resolved.profile;
            let endpoint = prepare_endpoint(&profile).await?;
            let mut config = match profile.auth_method {
                ConnectionAuthMethod::Password => tedious_rs::ConnectionConfig::sql(
                    endpoint.host.clone(),
                    profile.user_name.clone(),
                    password.clone(),
                ),
                ConnectionAuthMethod::WindowsNtlm => tedious_rs::ConnectionConfig::ntlm(
                    endpoint.host.clone(),
                    profile.user_name.clone(),
                    password.clone(),
                    None,
                ),
            };
            config.port = endpoint.port;
            config.database =
                (!profile.database.trim().is_empty()).then(|| profile.database.clone());
            config.trust_server_certificate = profile.trust_server_certificate;
            config = apply_sqlserver_properties(config, &profile);
            let runtime_config = Arc::new(config.clone());
            let mut connection = Connection::new(config);
            tokio::time::timeout(CONNECTION_TIMEOUT, connection.connect())
                .await
                .map_err(|_| driver_error("connection timeout"))?
                .map_err(|error| driver_error(redact_secret(error.to_string(), &password)))?;
            Ok(Arc::new(SqlServerRuntimeConnection {
                connection: Arc::new(tokio::sync::Mutex::new(Some(connection))),
                standby_connection: Arc::new(tokio::sync::Mutex::new(None)),
                standby_available: Arc::new(AtomicBool::new(false)),
                standby_requested: Arc::new(AtomicBool::new(false)),
                config: runtime_config,
                active_result_session: Arc::new(AtomicBool::new(false)),
                closed: Arc::new(AtomicBool::new(false)),
                _transport_guard: endpoint.guard,
            }) as Arc<dyn RuntimeConnection>)
        })
    }
}

#[derive(Default, Clone)]
pub struct RealConnectionDriverFactory;

impl ConnectionDriverFactory for RealConnectionDriverFactory {
    fn driver(
        &self,
        dialect: DialectId,
    ) -> Result<Arc<dyn ConnectionDriver>, ConnectionOperationError> {
        match dialect {
            DialectId::Postgresql => Ok(Arc::new(PostgresqlConnectionDriver)),
            DialectId::Sqlserver => Ok(Arc::new(SqlServerConnectionDriver)),
            unsupported => Err(ConnectionOperationError::new(
                "DRIVER_UNSUPPORTED",
                format!("No connection driver is enabled for {unsupported:?}"),
                false,
            )),
        }
    }
}
