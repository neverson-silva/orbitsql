use crate::backend::domain::catalog::{CatalogSnapshot, DatabaseCapabilities};
use crate::backend::domain::connection::{ConnectionOperationError, ConnectionProfileRecord};
use crate::backend::domain::connection_profile_definition::ConnectionProfileDefinition;
use crate::backend::domain::query::{QueryResult, QueryResultBatch, QueryResultSessionStart};
use crate::backend::domain::session_manager::{SessionKillResult, SessionTelemetry};
use futures_util::future::AbortHandle;
use futures_util::future::BoxFuture;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct ResolvedConnection {
    pub profile: ConnectionProfileRecord,
    pub password: String,
}

pub trait RuntimeConnection: Send + Sync {
    fn close(&self) -> BoxFuture<'_, Result<(), ConnectionOperationError>>;
    fn ping(&self) -> BoxFuture<'_, Result<(), ConnectionOperationError>>;
    fn server_version(&self) -> BoxFuture<'_, Result<Option<String>, ConnectionOperationError>>;
    fn execute(&self, sql: String) -> BoxFuture<'_, Result<QueryResult, ConnectionOperationError>>;
    fn start_result_session(
        &self,
        sql: String,
        initial_rows: usize,
    ) -> BoxFuture<'_, Result<RuntimeResultSessionStart, ConnectionOperationError>>;
    fn session_telemetry(
        &self,
    ) -> BoxFuture<'_, Result<SessionTelemetry, ConnectionOperationError>> {
        Box::pin(async {
            Err(ConnectionOperationError::new(
                "SESSION_TELEMETRY_UNSUPPORTED",
                "Session telemetry is not available for this database provider yet",
                false,
            ))
        })
    }
    fn kill_session(
        &self,
        _session_id: i32,
    ) -> BoxFuture<'_, Result<SessionKillResult, ConnectionOperationError>> {
        Box::pin(async {
            Err(ConnectionOperationError::new(
                "SESSION_TELEMETRY_UNSUPPORTED",
                "Session termination is not available for this database provider yet",
                false,
            ))
        })
    }
}

pub struct RuntimeResultSessionStart {
    pub result: QueryResultSessionStart,
    pub session: Arc<dyn RuntimeResultSession>,
}

pub trait RuntimeResultSession: Send + Sync {
    fn fetch_next(
        &self,
        requested_rows: usize,
    ) -> BoxFuture<'_, Result<QueryResultBatch, ConnectionOperationError>>;
    fn cancel(&self) -> BoxFuture<'_, Result<(), ConnectionOperationError>>;
    fn close(&self) -> BoxFuture<'_, Result<(), ConnectionOperationError>>;
}

pub trait ConnectionDriver: Send + Sync {
    fn capabilities(&self) -> DatabaseCapabilities;
    fn connect(
        &self,
        connection: ResolvedConnection,
    ) -> BoxFuture<'static, Result<Arc<dyn RuntimeConnection>, ConnectionOperationError>>;
}

pub trait ConnectionDriverFactory: Send + Sync {
    fn driver(
        &self,
        dialect: crate::backend::domain::catalog::DialectId,
    ) -> Result<Arc<dyn ConnectionDriver>, ConnectionOperationError>;
}

pub trait SecretStore: Send + Sync {
    fn set(
        &self,
        connection_id: &str,
        secret: String,
    ) -> BoxFuture<'static, Result<(), ConnectionOperationError>>;
    fn get(
        &self,
        connection_id: &str,
    ) -> BoxFuture<'static, Result<String, ConnectionOperationError>>;
    fn delete(
        &self,
        connection_id: &str,
    ) -> BoxFuture<'static, Result<(), ConnectionOperationError>>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretStoreProvider {
    Vault,
    Keychain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VaultStatus {
    Uninitialized,
    Locked,
    Unlocked,
    Error,
}

pub trait VaultControl: Send + Sync {
    fn status(&self) -> VaultStatus;
    fn create(
        &self,
        master_password: String,
    ) -> BoxFuture<'static, Result<(), ConnectionOperationError>>;
    fn unlock(
        &self,
        master_password: String,
    ) -> BoxFuture<'static, Result<(), ConnectionOperationError>>;
    fn lock(&self) -> Result<(), ConnectionOperationError>;
}

pub trait ProfileStore: Send + Sync {
    fn list(&self)
    -> BoxFuture<'_, Result<Vec<ConnectionProfileRecord>, ConnectionOperationError>>;
    fn upsert(
        &self,
        profile: ConnectionProfileRecord,
    ) -> BoxFuture<'_, Result<(), ConnectionOperationError>>;
    fn remove(&self, connection_id: &str) -> BoxFuture<'_, Result<(), ConnectionOperationError>>;
}

pub trait ConnectionProfileDefinitionStore: Send + Sync {
    fn list(
        &self,
    ) -> BoxFuture<'_, Result<Vec<ConnectionProfileDefinition>, ConnectionOperationError>>;
    fn upsert(
        &self,
        profile: ConnectionProfileDefinition,
    ) -> BoxFuture<'_, Result<(), ConnectionOperationError>>;
    fn remove(&self, profile_id: &str) -> BoxFuture<'_, Result<(), ConnectionOperationError>>;
}

#[derive(Default)]
pub struct ConnectionRuntimeRegistry {
    connections: Mutex<HashMap<String, Arc<dyn RuntimeConnection>>>,
    settings: Mutex<HashMap<String, ConnectionRuntimeSettings>>,
    last_activity: Mutex<HashMap<String, Instant>>,
    last_health_check: Mutex<HashMap<String, Instant>>,
}

#[derive(Clone, Copy)]
pub struct ConnectionRuntimeSettings {
    pub keep_alive_enabled: bool,
    pub keep_alive_interval: Duration,
    pub idle_timeout: Option<Duration>,
}

impl Default for ConnectionRuntimeSettings {
    fn default() -> Self {
        Self {
            keep_alive_enabled: true,
            keep_alive_interval: Duration::from_secs(15),
            idle_timeout: None,
        }
    }
}

#[derive(Default)]
pub struct QueryCancellationRegistry {
    handles: Mutex<HashMap<String, AbortHandle>>,
}

#[derive(Default)]
pub struct ResultSessionRegistry {
    sessions: Mutex<HashMap<String, ResultSessionEntry>>,
}

struct ResultSessionEntry {
    connection_id: String,
    session: Arc<dyn RuntimeResultSession>,
}

impl ResultSessionRegistry {
    pub fn insert(
        &self,
        session_id: String,
        connection_id: String,
        session: Arc<dyn RuntimeResultSession>,
    ) {
        self.sessions
            .lock()
            .expect("result session registry poisoned")
            .insert(
                session_id,
                ResultSessionEntry {
                    connection_id,
                    session,
                },
            );
    }

    pub fn get(&self, session_id: &str) -> Option<Arc<dyn RuntimeResultSession>> {
        self.sessions
            .lock()
            .expect("result session registry poisoned")
            .get(session_id)
            .map(|entry| entry.session.clone())
    }

    pub fn remove(&self, session_id: &str) -> Option<Arc<dyn RuntimeResultSession>> {
        self.sessions
            .lock()
            .expect("result session registry poisoned")
            .remove(session_id)
            .map(|entry| entry.session)
    }

    pub async fn close_for_connection(&self, connection_id: &str) {
        let sessions = {
            let mut entries = self
                .sessions
                .lock()
                .expect("result session registry poisoned");
            let session_ids = entries
                .iter()
                .filter(|(_, entry)| entry.connection_id == connection_id)
                .map(|(session_id, _)| session_id.clone())
                .collect::<Vec<_>>();
            session_ids
                .into_iter()
                .filter_map(|session_id| entries.remove(&session_id))
                .map(|entry| entry.session)
                .collect::<Vec<_>>()
        };
        for session in sessions {
            let _ = session.close().await;
        }
    }

    pub async fn close_all(&self) {
        let sessions = std::mem::take(
            &mut *self
                .sessions
                .lock()
                .expect("result session registry poisoned"),
        )
        .into_values()
        .map(|entry| entry.session)
        .collect::<Vec<_>>();
        for session in sessions {
            let _ = session.close().await;
        }
    }
}

impl QueryCancellationRegistry {
    pub fn begin(&self, connection_id: String, handle: AbortHandle) {
        if let Some(previous) = self
            .handles
            .lock()
            .expect("query cancellation registry poisoned")
            .insert(connection_id, handle)
        {
            previous.abort();
        }
    }

    pub fn finish(&self, connection_id: &str) {
        self.handles
            .lock()
            .expect("query cancellation registry poisoned")
            .remove(connection_id);
    }

    pub fn cancel(&self, connection_id: &str) -> bool {
        self.handles
            .lock()
            .expect("query cancellation registry poisoned")
            .remove(connection_id)
            .map(|handle| {
                handle.abort();
                true
            })
            .unwrap_or(false)
    }
}

struct CatalogSnapshotCacheEntry {
    snapshot: CatalogSnapshot,
    inserted_at: Instant,
}

#[derive(Default)]
pub struct CatalogSnapshotCache {
    snapshots: Mutex<HashMap<String, CatalogSnapshotCacheEntry>>,
}

impl CatalogSnapshotCache {
    const TTL: Duration = Duration::from_secs(300);

    fn key(connection_id: &str, scope: &str) -> String {
        format!("{connection_id}:{scope}")
    }

    pub fn get(&self, connection_id: &str, scope: &str) -> Option<CatalogSnapshot> {
        let key = Self::key(connection_id, scope);
        let mut snapshots = self.snapshots.lock().expect("catalog cache poisoned");
        let entry = snapshots.get(&key)?;
        if entry.inserted_at.elapsed() > Self::TTL {
            snapshots.remove(&key);
            return None;
        }
        Some(entry.snapshot.clone())
    }

    pub fn put(&self, connection_id: &str, scope: &str, snapshot: CatalogSnapshot) {
        self.snapshots
            .lock()
            .expect("catalog cache poisoned")
            .insert(
                Self::key(connection_id, scope),
                CatalogSnapshotCacheEntry {
                    snapshot,
                    inserted_at: Instant::now(),
                },
            );
    }

    pub fn invalidate_connection(&self, connection_id: &str) {
        let prefix = format!("{connection_id}:");
        self.snapshots
            .lock()
            .expect("catalog cache poisoned")
            .retain(|key, _| !key.starts_with(&prefix));
    }
}

impl ConnectionRuntimeRegistry {
    pub async fn check_health(&self) -> Vec<(String, ConnectionOperationError)> {
        let connections = self
            .connections
            .lock()
            .expect("connection registry poisoned")
            .iter()
            .map(|(id, connection)| (id.clone(), Arc::clone(connection)))
            .collect::<Vec<_>>();
        let mut failures = Vec::new();
        for (connection_id, connection) in connections {
            let settings = self
                .settings
                .lock()
                .expect("connection settings poisoned")
                .get(&connection_id)
                .copied()
                .unwrap_or_default();
            let now = Instant::now();
            let last_activity = self
                .last_activity
                .lock()
                .expect("connection activity poisoned")
                .get(&connection_id)
                .copied()
                .unwrap_or(now);
            if settings
                .idle_timeout
                .is_some_and(|timeout| last_activity.elapsed() >= timeout)
            {
                if self.remove_if_same(&connection_id, &connection).is_some() {
                    failures.push((
                        connection_id,
                        ConnectionOperationError::new(
                            "CONNECTION_IDLE_TIMEOUT",
                            "Connection closed after exceeding the idle timeout",
                            true,
                        ),
                    ));
                }
                continue;
            }
            if !settings.keep_alive_enabled {
                continue;
            }
            let should_check = self
                .last_health_check
                .lock()
                .expect("connection health state poisoned")
                .get(&connection_id)
                .is_none_or(|last_check| last_check.elapsed() >= settings.keep_alive_interval);
            if !should_check {
                continue;
            }
            self.last_health_check
                .lock()
                .expect("connection health state poisoned")
                .insert(connection_id.clone(), now);
            let result = tokio::time::timeout(Duration::from_secs(5), connection.ping()).await;
            let error = match result {
                Ok(Ok(())) => continue,
                Ok(Err(error)) => error,
                Err(_) => ConnectionOperationError::new(
                    "CONNECTION_HEALTH_TIMEOUT",
                    "Connection health check timed out",
                    true,
                ),
            };
            if self.remove_if_same(&connection_id, &connection).is_some() {
                failures.push((connection_id, error));
            }
        }
        failures
    }

    pub fn insert(
        &self,
        connection_id: String,
        connection: Arc<dyn RuntimeConnection>,
    ) -> Option<Arc<dyn RuntimeConnection>> {
        self.insert_with_settings(connection_id, connection, true, 15, None)
    }

    pub fn insert_with_settings(
        &self,
        connection_id: String,
        connection: Arc<dyn RuntimeConnection>,
        keep_alive_enabled: bool,
        keep_alive_interval_seconds: u64,
        idle_timeout_seconds: Option<u64>,
    ) -> Option<Arc<dyn RuntimeConnection>> {
        let previous = self
            .connections
            .lock()
            .expect("connection registry poisoned")
            .insert(connection_id.clone(), connection);
        self.settings
            .lock()
            .expect("connection settings poisoned")
            .insert(
                connection_id.clone(),
                ConnectionRuntimeSettings {
                    keep_alive_enabled,
                    keep_alive_interval: Duration::from_secs(keep_alive_interval_seconds.max(1)),
                    idle_timeout: idle_timeout_seconds
                        .filter(|seconds| *seconds > 0)
                        .map(Duration::from_secs),
                },
            );
        self.last_activity
            .lock()
            .expect("connection activity poisoned")
            .insert(connection_id, Instant::now());
        previous
    }

    pub fn remove(&self, connection_id: &str) -> Option<Arc<dyn RuntimeConnection>> {
        let result = self
            .connections
            .lock()
            .expect("connection registry poisoned")
            .remove(connection_id);
        self.remove_metadata(connection_id);
        result
    }

    pub fn remove_if_same(
        &self,
        connection_id: &str,
        expected: &Arc<dyn RuntimeConnection>,
    ) -> Option<Arc<dyn RuntimeConnection>> {
        let mut connections = self
            .connections
            .lock()
            .expect("connection registry poisoned");
        let matches = connections
            .get(connection_id)
            .is_some_and(|current| Arc::ptr_eq(current, expected));
        let removed = matches.then(|| connections.remove(connection_id)).flatten();
        drop(connections);
        if removed.is_some() {
            self.remove_metadata(connection_id);
        }
        removed
    }

    fn remove_metadata(&self, connection_id: &str) {
        self.settings
            .lock()
            .expect("connection settings poisoned")
            .remove(connection_id);
        self.last_activity
            .lock()
            .expect("connection activity poisoned")
            .remove(connection_id);
        self.last_health_check
            .lock()
            .expect("connection health state poisoned")
            .remove(connection_id);
    }

    pub fn touch(&self, connection_id: &str) {
        if self.contains(connection_id) {
            self.last_activity
                .lock()
                .expect("connection activity poisoned")
                .insert(connection_id.to_owned(), Instant::now());
        }
    }

    pub fn contains(&self, connection_id: &str) -> bool {
        self.connections
            .lock()
            .expect("connection registry poisoned")
            .contains_key(connection_id)
    }

    pub fn get(&self, connection_id: &str) -> Option<Arc<dyn RuntimeConnection>> {
        self.connections
            .lock()
            .expect("connection registry poisoned")
            .get(connection_id)
            .cloned()
    }

    pub fn connected_ids(&self) -> Vec<String> {
        self.connections
            .lock()
            .expect("connection registry poisoned")
            .keys()
            .cloned()
            .collect()
    }

    pub async fn close_all(&self) -> Result<(), ConnectionOperationError> {
        let connections = std::mem::take(
            &mut *self
                .connections
                .lock()
                .expect("connection registry poisoned"),
        )
        .into_values()
        .collect::<Vec<_>>();
        let mut first_error = None;
        for connection in connections {
            let result = tokio::time::timeout(Duration::from_secs(15), connection.close()).await;
            let error = match result {
                Ok(result) => result.err(),
                Err(_) => Some(ConnectionOperationError::new(
                    "CONNECTION_TIMEOUT",
                    "Connection close timed out",
                    true,
                )),
            };
            if first_error.is_none() {
                first_error = error;
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}
