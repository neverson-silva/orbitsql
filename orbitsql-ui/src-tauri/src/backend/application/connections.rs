use crate::backend::domain::catalog::DialectId;
use crate::backend::domain::connection::{
    ConnectionAuthMethod, ConnectionDiagnostic, ConnectionDraft, ConnectionOperationError,
    ConnectionProfileRecord, ConnectionSessionResult, ConnectionUpdateDraft,
};
use crate::backend::domain::connection_profile_definition::ConnectionProfileDefinition;
use crate::backend::domain::query::{QueryResult, QueryResultSessionStart};
use crate::backend::ports::{
    CatalogProviderFactory, CatalogRefreshRequest, CatalogScope, CatalogSnapshotCache,
    ConnectionDriverFactory, ConnectionProfileDefinitionStore, ConnectionRuntimeRegistry,
    ProfileStore, ResolvedConnection, ResultSessionRegistry, RuntimeResultSessionStart,
    SecretStore,
};
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use uuid::Uuid;

fn validation_error(message: impl Into<String>) -> ConnectionOperationError {
    ConnectionOperationError::new("VALIDATION_FAILED", message, false)
}

fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut start = 0;
    let mut index = 0;
    let bytes = sql.as_bytes();
    let mut quote = None;
    let mut line_comment = false;
    let mut block_comment = false;
    let mut dollar_tag: Option<String> = None;

    while index < bytes.len() {
        if line_comment {
            if bytes[index] == b'\n' {
                line_comment = false;
            }
            index += 1;
            continue;
        }
        if block_comment {
            if index + 1 < bytes.len() && bytes[index] == b'*' && bytes[index + 1] == b'/' {
                block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(tag) = &dollar_tag {
            if sql[index..].starts_with(tag) {
                index += tag.len();
                dollar_tag = None;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(current_quote) = quote {
            if bytes[index] == current_quote {
                if index + 1 < bytes.len() && bytes[index + 1] == current_quote {
                    index += 2;
                } else {
                    quote = None;
                    index += 1;
                }
            } else if bytes[index] == b'\\' && current_quote == b'\'' {
                index = (index + 2).min(bytes.len());
            } else {
                index += 1;
            }
            continue;
        }
        if index + 1 < bytes.len() && bytes[index] == b'-' && bytes[index + 1] == b'-' {
            line_comment = true;
            index += 2;
            continue;
        }
        if index + 1 < bytes.len() && bytes[index] == b'/' && bytes[index + 1] == b'*' {
            block_comment = true;
            index += 2;
            continue;
        }
        if bytes[index] == b'\'' || bytes[index] == b'"' || bytes[index] == b'`' {
            quote = Some(bytes[index]);
            index += 1;
            continue;
        }
        if bytes[index] == b'$' {
            if let Some(end) = sql[index + 1..].find('$') {
                let tag = &sql[index..=index + end + 1];
                if tag[1..tag.len() - 1]
                    .chars()
                    .all(|character| character == '_' || character.is_ascii_alphanumeric())
                {
                    dollar_tag = Some(tag.to_owned());
                    index += tag.len();
                    continue;
                }
            }
        }
        if bytes[index] == b';' {
            let statement = sql[start..index].trim();
            if !statement.is_empty() {
                statements.push(statement.to_owned());
            }
            start = index + 1;
        }
        index += 1;
    }
    let statement = sql[start..].trim();
    if !statement.is_empty() {
        statements.push(statement.to_owned());
    }
    statements
}

async fn execute_sql_statements(
    connection: &Arc<dyn crate::backend::ports::RuntimeConnection>,
    sql: String,
) -> Result<QueryResult, ConnectionOperationError> {
    let mut result = None;
    for statement in split_sql_statements(&sql) {
        result = Some(connection.execute(statement).await?);
    }
    result.ok_or_else(|| validation_error("SQL query is required"))
}

fn supported_dialect(dialect: &DialectId) -> bool {
    matches!(dialect, DialectId::Postgresql | DialectId::Sqlserver)
}

fn validate_common(
    name: &str,
    dialect: &DialectId,
    server: &str,
    port: u16,
    user_name: &str,
    auth_method: &ConnectionAuthMethod,
) -> Result<(), ConnectionOperationError> {
    if !supported_dialect(dialect) {
        return Err(validation_error(
            "Only PostgreSQL and SQL Server are enabled",
        ));
    }
    if name.trim().is_empty() {
        return Err(validation_error("Connection name is required"));
    }
    if server.trim().is_empty() {
        return Err(validation_error("Server is required"));
    }
    if port == 0 {
        return Err(validation_error("Port must be greater than zero"));
    }
    if user_name.trim().is_empty() {
        return Err(validation_error("Username is required"));
    }
    if matches!(dialect, DialectId::Postgresql)
        && matches!(auth_method, ConnectionAuthMethod::WindowsNtlm)
    {
        return Err(validation_error(
            "Windows authentication is only available for SQL Server",
        ));
    }
    Ok(())
}

fn redact(message: String, secret: &str) -> String {
    if secret.is_empty() {
        message
    } else {
        message.replace(secret, "[REDACTED]")
    }
}

async fn close_runtime(
    connection: &Arc<dyn crate::backend::ports::RuntimeConnection>,
) -> Result<(), ConnectionOperationError> {
    tokio::time::timeout(Duration::from_secs(15), connection.close())
        .await
        .map_err(|_| {
            ConnectionOperationError::new("CONNECTION_TIMEOUT", "Connection close timed out", true)
        })?
}

fn profile_from_draft(
    id: String,
    draft: &ConnectionDraft,
    timestamp: &str,
) -> ConnectionProfileRecord {
    ConnectionProfileRecord {
        id,
        name: draft.name.trim().to_owned(),
        dialect: draft.dialect.clone(),
        server: draft.server.trim().to_owned(),
        port: draft.port,
        user_name: draft.user_name.trim().to_owned(),
        database: draft.database.trim().to_owned(),
        trust_server_certificate: draft.trust_server_certificate,
        auth_method: draft.auth_method.clone(),
        profile_id: draft.profile_id.trim().to_owned(),
        created_at: timestamp.to_owned(),
        updated_at: timestamp.to_owned(),
        show_all_catalog_items: draft.show_all_catalog_items,
        transport: draft.transport.clone(),
        driver_properties: draft.driver_properties.clone(),
        keep_alive_enabled: draft.keep_alive_enabled,
        keep_alive_interval_seconds: draft.keep_alive_interval_seconds.max(1),
        idle_timeout_seconds: draft.idle_timeout_seconds,
    }
}

fn timestamp() -> String {
    format!("{}", chrono_like_now())
}

fn chrono_like_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

pub async fn test_connection<F>(
    factory: &F,
    secret_store: &dyn SecretStore,
    draft: ConnectionDraft,
    connection_id: Option<String>,
) -> Result<ConnectionDiagnostic, ConnectionOperationError>
where
    F: ConnectionDriverFactory,
{
    validate_common(
        &draft.name,
        &draft.dialect,
        &draft.server,
        draft.port,
        &draft.user_name,
        &draft.auth_method,
    )?;
    let password = if draft.password.is_empty() {
        match connection_id.as_deref() {
            Some(connection_id) => secret_store.get(connection_id).await.map_err(|_| {
                validation_error("Saved password is unavailable; enter a new password")
            })?,
            None => {
                return Err(validation_error(
                    "Password is required for a connection test",
                ));
            }
        }
    } else {
        draft.password.clone()
    };
    let started = Instant::now();
    let driver = factory.driver(draft.dialect.clone())?;
    let capabilities = driver.capabilities();
    let profile = profile_from_draft("test".to_owned(), &draft, "test");
    let connection = match driver
        .connect(ResolvedConnection {
            profile,
            password: password.clone(),
        })
        .await
    {
        Ok(connection) => connection,
        Err(error) => {
            return Ok(ConnectionDiagnostic {
                success: false,
                latency_ms: started.elapsed().as_millis(),
                server_version: None,
                database: (!draft.database.trim().is_empty()).then_some(draft.database),
                error_code: Some(error.code),
                error_message: Some(redact(error.message, &password)),
                recoverable: error.recoverable,
                capabilities: None,
            });
        }
    };
    let server_version = match connection.server_version().await {
        Ok(server_version) => server_version,
        Err(error) => {
            let _ = close_runtime(&connection).await;
            return Ok(ConnectionDiagnostic {
                success: false,
                latency_ms: started.elapsed().as_millis(),
                server_version: None,
                database: (!draft.database.trim().is_empty()).then_some(draft.database),
                error_code: Some(error.code),
                error_message: Some(redact(error.message, &password)),
                recoverable: error.recoverable,
                capabilities: None,
            });
        }
    };
    let close_result = close_runtime(&connection).await;
    if let Err(error) = close_result {
        return Ok(ConnectionDiagnostic {
            success: false,
            latency_ms: started.elapsed().as_millis(),
            server_version: None,
            database: (!draft.database.trim().is_empty()).then_some(draft.database),
            error_code: Some(error.code),
            error_message: Some(redact(error.message, &password)),
            recoverable: error.recoverable,
            capabilities: None,
        });
    }
    Ok(ConnectionDiagnostic {
        success: true,
        latency_ms: started.elapsed().as_millis(),
        server_version,
        database: (!draft.database.trim().is_empty()).then_some(draft.database),
        error_code: None,
        error_message: None,
        recoverable: false,
        capabilities: Some(capabilities),
    })
}

pub async fn save_and_connect<F>(
    profiles: &dyn ProfileStore,
    factory: &F,
    catalog_factory: &dyn CatalogProviderFactory,
    registry: &ConnectionRuntimeRegistry,
    sessions: &ResultSessionRegistry,
    catalog_cache: &CatalogSnapshotCache,
    secret_store: &dyn SecretStore,
    draft: ConnectionDraft,
) -> Result<ConnectionSessionResult, ConnectionOperationError>
where
    F: ConnectionDriverFactory,
{
    let profile = save_profile(profiles, secret_store, draft.clone()).await?;
    let driver = match factory.driver(draft.dialect.clone()) {
        Ok(driver) => driver,
        Err(error) => {
            let _ = profiles.remove(&profile.id).await;
            let _ = secret_store.delete(&profile.id).await;
            return Err(error);
        }
    };
    let capabilities = driver.capabilities();
    let connection = match driver
        .connect(ResolvedConnection {
            profile: profile.clone(),
            password: draft.password,
        })
        .await
    {
        Ok(connection) => connection,
        Err(error) => {
            let _ = profiles.remove(&profile.id).await;
            let _ = secret_store.delete(&profile.id).await;
            return Err(error);
        }
    };
    if let Some(previous) = registry.insert_with_settings(
        profile.id.clone(),
        connection,
        profile.keep_alive_enabled,
        profile.keep_alive_interval_seconds,
        profile.idle_timeout_seconds,
    ) {
        sessions.close_for_connection(&profile.id).await;
        let _ = close_runtime(&previous).await;
    }
    let catalog = match initial_catalog_for_connection(
        profiles,
        factory,
        catalog_factory,
        registry,
        sessions,
        catalog_cache,
        secret_store,
        profile.id.clone(),
        timestamp(),
        timestamp(),
    )
    .await
    {
        Ok(catalog) => catalog,
        Err(error) => {
            let _ = disconnect_existing(registry, sessions, &profile.id).await;
            let _ = profiles.remove(&profile.id).await;
            let _ = secret_store.delete(&profile.id).await;
            return Err(error);
        }
    };
    Ok(ConnectionSessionResult {
        profile,
        catalog,
        capabilities,
    })
}

pub async fn save_profile(
    profiles: &dyn ProfileStore,
    secret_store: &dyn SecretStore,
    draft: ConnectionDraft,
) -> Result<ConnectionProfileRecord, ConnectionOperationError> {
    validate_common(
        &draft.name,
        &draft.dialect,
        &draft.server,
        draft.port,
        &draft.user_name,
        &draft.auth_method,
    )?;
    if draft.password.is_empty() {
        return Err(validation_error("Password is required"));
    }
    let id = Uuid::new_v4().to_string();
    let now = timestamp();
    let profile = profile_from_draft(id.clone(), &draft, &now);
    secret_store.set(&id, draft.password.clone()).await?;
    if let Err(error) = profiles.upsert(profile.clone()).await {
        let _ = secret_store.delete(&id).await;
        return Err(error);
    }
    Ok(profile)
}

pub async fn list_connection_profiles(
    profiles: &dyn ProfileStore,
) -> Result<Vec<ConnectionProfileRecord>, ConnectionOperationError> {
    profiles.list().await
}

pub async fn list_connection_profile_definitions(
    profiles: &dyn ConnectionProfileDefinitionStore,
) -> Result<Vec<ConnectionProfileDefinition>, ConnectionOperationError> {
    profiles.list().await
}

pub async fn upsert_connection_profile_definition(
    profiles: &dyn ConnectionProfileDefinitionStore,
    profile: ConnectionProfileDefinition,
) -> Result<ConnectionProfileDefinition, ConnectionOperationError> {
    if profile.id.trim().is_empty()
        || profile.name.trim().is_empty()
        || !is_profile_color(&profile.color)
    {
        return Err(validation_error(
            "Profile id, name and valid color are required",
        ));
    }
    let profile = ConnectionProfileDefinition {
        id: profile.id.trim().to_owned(),
        name: profile.name.trim().to_owned(),
        color: profile.color.trim().to_owned(),
        description: profile.description.trim().to_owned(),
    };
    profiles.upsert(profile.clone()).await?;
    Ok(profile)
}

fn is_profile_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub async fn delete_connection_profile_definition(
    profiles: &dyn ConnectionProfileDefinitionStore,
    profile_id: String,
) -> Result<(), ConnectionOperationError> {
    if profile_id.trim().is_empty() {
        return Err(validation_error("Profile id is required"));
    }
    profiles.remove(profile_id.trim()).await
}

pub async fn execute_query(
    registry: &ConnectionRuntimeRegistry,
    catalog_cache: &CatalogSnapshotCache,
    connection_id: String,
    sql: String,
) -> Result<QueryResult, ConnectionOperationError> {
    if sql.trim().is_empty() {
        return Err(validation_error("SQL query is required"));
    }
    let connection = registry
        .get(&connection_id)
        .ok_or_else(|| validation_error("Connection is not connected"))?;
    registry.touch(&connection_id);
    let result = execute_sql_statements(&connection, sql).await;
    if result.is_ok() {
        catalog_cache.invalidate_connection(&connection_id);
    }
    result
}

pub async fn start_query_session(
    registry: &ConnectionRuntimeRegistry,
    sessions: &ResultSessionRegistry,
    connection_id: String,
    sql: String,
    initial_rows: usize,
) -> Result<QueryResultSessionStart, ConnectionOperationError> {
    validate_result_session_sql(&sql)?;
    let connection = registry
        .get(&connection_id)
        .ok_or_else(|| validation_error("Connection is not connected"))?;
    registry.touch(&connection_id);
    let RuntimeResultSessionStart { result, session } = connection
        .start_result_session(sql, initial_rows.max(1))
        .await?;
    if result.first_batch.has_more {
        sessions.insert(result.session_id.clone(), connection_id, session);
    } else {
        session.close().await?;
    }
    Ok(result)
}

fn validate_result_session_sql(sql: &str) -> Result<(), ConnectionOperationError> {
    if sql.trim().is_empty() {
        return Err(validation_error("SQL query is required"));
    }
    if split_sql_statements(sql).len() != 1 {
        return Err(validation_error(
            "Incremental result sessions require exactly one SQL statement",
        ));
    }
    Ok(())
}

pub async fn start_query_session_in_context<F>(
    profiles: &dyn ProfileStore,
    factory: &F,
    secret_store: &dyn SecretStore,
    registry: &ConnectionRuntimeRegistry,
    sessions: &ResultSessionRegistry,
    connection_id: String,
    sql: String,
    database: Option<String>,
    schema: Option<String>,
    initial_rows: usize,
) -> Result<QueryResultSessionStart, ConnectionOperationError>
where
    F: ConnectionDriverFactory,
{
    validate_result_session_sql(&sql)?;
    let profiles = profiles.list().await?;
    let profile = profiles
        .iter()
        .find(|profile| profile.id == connection_id)
        .cloned()
        .ok_or_else(|| validation_error("Connection profile was not found"))?;
    let requested_database = database.filter(|value| !value.trim().is_empty());
    let mut temporary = None;
    let runtime = if let Some(database) = requested_database {
        if profile.database != database {
            let password = secret_store.get(&connection_id).await?;
            let driver = factory.driver(profile.dialect.clone())?;
            let mut contextual_profile = profile.clone();
            contextual_profile.database = database;
            let runtime = driver
                .connect(ResolvedConnection {
                    profile: contextual_profile,
                    password,
                })
                .await?;
            temporary = Some(runtime.clone());
            runtime
        } else {
            registry
                .get(&connection_id)
                .ok_or_else(|| validation_error("Connection is not connected"))?
        }
    } else {
        registry
            .get(&connection_id)
            .ok_or_else(|| validation_error("Connection is not connected"))?
    };
    if temporary.is_none() {
        registry.touch(&connection_id);
    }
    if matches!(profile.dialect, DialectId::Postgresql)
        && let Some(schema) = schema.filter(|value| !value.trim().is_empty())
    {
        let escaped = schema.replace('"', "\"\"");
        runtime
            .execute(format!("SET search_path TO \"{escaped}\";"))
            .await?;
    }
    let started = runtime.start_result_session(sql, initial_rows.max(1)).await;
    let RuntimeResultSessionStart { result, session } = match started {
        Ok(started) => started,
        Err(error) => {
            if let Some(runtime) = temporary {
                let _ = close_runtime(&runtime).await;
            }
            return Err(error);
        }
    };
    if result.first_batch.has_more {
        sessions.insert(result.session_id.clone(), connection_id, session);
    } else {
        session.close().await?;
    }
    Ok(result)
}

pub async fn fetch_query_session(
    sessions: &ResultSessionRegistry,
    session_id: String,
    requested_rows: usize,
) -> Result<crate::backend::domain::query::QueryResultBatch, ConnectionOperationError> {
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| validation_error("Result session was not found"))?;
    let batch = session.fetch_next(requested_rows.max(1)).await?;
    if !batch.has_more {
        if let Some(session) = sessions.remove(&session_id) {
            session.close().await?;
        }
    }
    Ok(batch)
}

pub async fn cancel_query_session(
    sessions: &ResultSessionRegistry,
    session_id: String,
) -> Result<(), ConnectionOperationError> {
    let session = sessions
        .remove(&session_id)
        .ok_or_else(|| validation_error("Result session was not found"))?;
    session.cancel().await
}

pub async fn close_query_session(
    sessions: &ResultSessionRegistry,
    session_id: String,
) -> Result<(), ConnectionOperationError> {
    if let Some(session) = sessions.remove(&session_id) {
        session.close().await?;
    }
    Ok(())
}

pub async fn execute_query_in_context<F>(
    profiles: &dyn ProfileStore,
    factory: &F,
    secret_store: &dyn SecretStore,
    registry: &ConnectionRuntimeRegistry,
    connection_id: String,
    sql: String,
    database: Option<String>,
    schema: Option<String>,
) -> Result<QueryResult, ConnectionOperationError>
where
    F: ConnectionDriverFactory,
{
    let requested_database = database.filter(|value| !value.trim().is_empty());
    let profile_dialect = profiles
        .list()
        .await?
        .into_iter()
        .find(|profile| profile.id == connection_id)
        .map(|profile| profile.dialect);
    let mut temporary = None;
    let runtime = if let Some(database) = requested_database {
        let profile = profiles
            .list()
            .await?
            .into_iter()
            .find(|profile| profile.id == connection_id)
            .ok_or_else(|| validation_error("Connection profile was not found"))?;
        if profile.database != database {
            let password = secret_store.get(&connection_id).await?;
            let driver = factory.driver(profile.dialect.clone())?;
            let mut contextual_profile = profile;
            contextual_profile.database = database;
            let runtime = driver
                .connect(ResolvedConnection {
                    profile: contextual_profile,
                    password,
                })
                .await?;
            temporary = Some(runtime.clone());
            runtime
        } else {
            registry
                .get(&connection_id)
                .ok_or_else(|| validation_error("Connection is not connected"))?
        }
    } else {
        registry
            .get(&connection_id)
            .ok_or_else(|| validation_error("Connection is not connected"))?
    };
    if temporary.is_none() {
        registry.touch(&connection_id);
    }
    if matches!(profile_dialect, Some(DialectId::Postgresql))
        && let Some(schema) = schema.filter(|value| !value.trim().is_empty())
    {
        let escaped = schema.replace('"', "\"\"");
        runtime
            .execute(format!("SET search_path TO \"{escaped}\";"))
            .await?;
    }
    let result = execute_sql_statements(&runtime, sql).await;
    if let Some(runtime) = temporary {
        let _ = close_runtime(&runtime).await;
    }
    result
}

pub async fn connect_existing<F>(
    profiles: &dyn ProfileStore,
    factory: &F,
    catalog_factory: &dyn CatalogProviderFactory,
    registry: &ConnectionRuntimeRegistry,
    sessions: &ResultSessionRegistry,
    catalog_cache: &CatalogSnapshotCache,
    secret_store: &dyn SecretStore,
    connection_id: String,
) -> Result<ConnectionSessionResult, ConnectionOperationError>
where
    F: ConnectionDriverFactory,
{
    let profile = profiles
        .list()
        .await?
        .into_iter()
        .find(|profile| profile.id == connection_id)
        .ok_or_else(|| validation_error("Connection profile was not found"))?;
    let password = secret_store.get(&connection_id).await?;
    catalog_cache.invalidate_connection(&connection_id);
    let driver = factory.driver(profile.dialect.clone())?;
    let capabilities = driver.capabilities();
    let connection = driver
        .connect(ResolvedConnection {
            profile: profile.clone(),
            password,
        })
        .await?;
    if let Some(previous) = registry.insert_with_settings(
        connection_id,
        connection,
        profile.keep_alive_enabled,
        profile.keep_alive_interval_seconds,
        profile.idle_timeout_seconds,
    ) {
        sessions.close_for_connection(&profile.id).await;
        let _ = close_runtime(&previous).await;
    }
    let catalog = match initial_catalog_for_connection(
        profiles,
        factory,
        catalog_factory,
        registry,
        sessions,
        catalog_cache,
        secret_store,
        profile.id.clone(),
        timestamp(),
        timestamp(),
    )
    .await
    {
        Ok(catalog) => catalog,
        Err(error) => {
            let _ = disconnect_existing(registry, sessions, &profile.id).await;
            return Err(error);
        }
    };
    Ok(ConnectionSessionResult {
        profile,
        catalog,
        capabilities,
    })
}

pub async fn connect_existing_without_catalog<F>(
    profiles: &dyn ProfileStore,
    factory: &F,
    registry: &ConnectionRuntimeRegistry,
    sessions: &ResultSessionRegistry,
    catalog_cache: &CatalogSnapshotCache,
    secret_store: &dyn SecretStore,
    connection_id: String,
) -> Result<ConnectionProfileRecord, ConnectionOperationError>
where
    F: ConnectionDriverFactory,
{
    let profile = profiles
        .list()
        .await?
        .into_iter()
        .find(|profile| profile.id == connection_id)
        .ok_or_else(|| validation_error("Connection profile was not found"))?;
    let password = secret_store.get(&connection_id).await?;
    catalog_cache.invalidate_connection(&connection_id);
    let driver = factory.driver(profile.dialect.clone())?;
    let connection = driver
        .connect(ResolvedConnection {
            profile: profile.clone(),
            password,
        })
        .await?;
    if let Some(previous) = registry.insert_with_settings(
        connection_id,
        connection,
        profile.keep_alive_enabled,
        profile.keep_alive_interval_seconds,
        profile.idle_timeout_seconds,
    ) {
        sessions.close_for_connection(&profile.id).await;
        let _ = close_runtime(&previous).await;
    }
    Ok(profile)
}

pub async fn disconnect_existing(
    registry: &ConnectionRuntimeRegistry,
    sessions: &ResultSessionRegistry,
    connection_id: &str,
) -> Result<(), ConnectionOperationError> {
    sessions.close_for_connection(connection_id).await;
    if let Some(connection) = registry.remove(connection_id) {
        close_runtime(&connection).await?;
    }
    Ok(())
}

pub async fn ping_existing_connection(
    registry: &ConnectionRuntimeRegistry,
    sessions: &ResultSessionRegistry,
    connection_id: &str,
) -> bool {
    let Some(runtime) = registry.get(connection_id) else {
        return false;
    };
    if matches!(
        tokio::time::timeout(Duration::from_secs(5), runtime.ping()).await,
        Ok(Ok(()))
    ) {
        registry.touch(connection_id);
        return true;
    }
    if let Some(stale_runtime) = registry.remove_if_same(connection_id, &runtime) {
        sessions.close_for_connection(connection_id).await;
        let _ = close_runtime(&stale_runtime).await;
    }
    false
}

pub async fn delete_connection(
    profiles: &dyn ProfileStore,
    registry: &ConnectionRuntimeRegistry,
    sessions: &ResultSessionRegistry,
    catalog_cache: &CatalogSnapshotCache,
    secret_store: &dyn SecretStore,
    connection_id: String,
) -> Result<(), ConnectionOperationError> {
    let mut first_error = disconnect_existing(registry, sessions, &connection_id)
        .await
        .err();
    let previous_secret = secret_store.get(&connection_id).await.ok();
    if let Err(error) = secret_store.delete(&connection_id).await {
        if first_error.is_none() {
            first_error = Some(error);
        }
        catalog_cache.invalidate_connection(&connection_id);
        return first_error.map_or(Ok(()), Err);
    }
    if let Err(error) = profiles.remove(&connection_id).await {
        if let Some(secret) = previous_secret {
            let _ = secret_store.set(&connection_id, secret).await;
        }
        if first_error.is_none() {
            first_error = Some(error);
        }
    }
    catalog_cache.invalidate_connection(&connection_id);
    first_error.map_or(Ok(()), Err)
}

pub async fn update_connection<F>(
    profiles: &dyn ProfileStore,
    factory: &F,
    registry: &ConnectionRuntimeRegistry,
    catalog_cache: &CatalogSnapshotCache,
    secret_store: &dyn SecretStore,
    draft: ConnectionUpdateDraft,
    reconnect: bool,
) -> Result<ConnectionProfileRecord, ConnectionOperationError>
where
    F: ConnectionDriverFactory,
{
    validate_common(
        &draft.name,
        &draft.dialect,
        &draft.server,
        draft.port,
        &draft.user_name,
        &draft.auth_method,
    )?;
    let existing = profiles
        .list()
        .await?
        .into_iter()
        .find(|profile| profile.id == draft.id)
        .ok_or_else(|| validation_error("Connection profile was not found"))?;
    catalog_cache.invalidate_connection(&draft.id);
    let previous_password = secret_store.get(&draft.id).await.ok();
    let password_changed = draft
        .password
        .as_ref()
        .is_some_and(|password| !password.is_empty());
    let password = match draft.password {
        Some(password) if !password.is_empty() => {
            secret_store.set(&draft.id, password.clone()).await?;
            password
        }
        _ => previous_password.clone().ok_or_else(|| {
            validation_error("Saved password is unavailable; enter a new password")
        })?,
    };
    let now = timestamp();
    let profile = ConnectionProfileRecord {
        id: draft.id.clone(),
        name: draft.name.trim().to_owned(),
        dialect: draft.dialect,
        server: draft.server.trim().to_owned(),
        port: draft.port,
        user_name: draft.user_name.trim().to_owned(),
        database: draft.database.trim().to_owned(),
        trust_server_certificate: draft.trust_server_certificate,
        auth_method: draft.auth_method,
        profile_id: draft.profile_id.trim().to_owned(),
        created_at: existing.created_at,
        updated_at: now,
        show_all_catalog_items: draft.show_all_catalog_items,
        transport: existing.transport,
        driver_properties: existing.driver_properties,
        keep_alive_enabled: draft.keep_alive_enabled,
        keep_alive_interval_seconds: draft.keep_alive_interval_seconds.max(1),
        idle_timeout_seconds: draft.idle_timeout_seconds,
    };
    if !reconnect || !registry.contains(&draft.id) {
        if let Err(error) = profiles.upsert(profile.clone()).await {
            if password_changed {
                if let Some(previous_password) = previous_password.as_ref() {
                    let _ = secret_store.set(&draft.id, previous_password.clone()).await;
                }
            }
            return Err(error);
        }
        return Ok(profile);
    }
    let driver = match factory.driver(profile.dialect.clone()) {
        Ok(driver) => driver,
        Err(error) => {
            if password_changed {
                if let Some(previous_password) = previous_password.as_ref() {
                    let _ = secret_store.set(&draft.id, previous_password.clone()).await;
                }
            }
            return Err(error);
        }
    };
    let connection = match driver
        .connect(ResolvedConnection {
            profile: profile.clone(),
            password,
        })
        .await
    {
        Ok(connection) => connection,
        Err(error) => {
            if password_changed {
                if let Some(previous_password) = previous_password.as_ref() {
                    let _ = secret_store.set(&draft.id, previous_password.clone()).await;
                }
            }
            return Err(error);
        }
    };
    if let Err(error) = profiles.upsert(profile.clone()).await {
        let _ = close_runtime(&connection).await;
        if password_changed {
            if let Some(previous_password) = previous_password.as_ref() {
                let _ = secret_store.set(&draft.id, previous_password.clone()).await;
            }
        }
        return Err(error);
    }
    if let Some(previous) = registry.insert_with_settings(
        draft.id,
        connection,
        profile.keep_alive_enabled,
        profile.keep_alive_interval_seconds,
        profile.idle_timeout_seconds,
    ) {
        let _ = close_runtime(&previous).await;
    }
    Ok(profile)
}

pub async fn initial_catalog_for_connection(
    profiles: &dyn ProfileStore,
    connection_factory: &dyn ConnectionDriverFactory,
    factory: &dyn CatalogProviderFactory,
    registry: &ConnectionRuntimeRegistry,
    sessions: &ResultSessionRegistry,
    catalog_cache: &CatalogSnapshotCache,
    secret_store: &dyn SecretStore,
    connection_id: String,
    version: String,
    generated_at: String,
) -> Result<crate::backend::domain::catalog::CatalogSnapshot, ConnectionOperationError> {
    if let Some(snapshot) = catalog_cache.get(&connection_id, "initial") {
        return Ok(snapshot);
    }
    let profile = profiles
        .list()
        .await?
        .into_iter()
        .find(|profile| profile.id == connection_id)
        .ok_or_else(|| validation_error("Connection profile was not found"))?;
    let password = secret_store.get(&connection_id).await?;
    ensure_runtime_connection(connection_factory, registry, sessions, &profile, &password).await?;
    let request = CatalogRefreshRequest {
        dialect: profile.dialect.clone(),
        connection_id: profile.id.clone(),
        version,
        generated_at,
        server: profile.server.clone(),
        port: profile.port,
        user_name: profile.user_name.clone(),
        password: password.clone(),
        database: profile.database.clone(),
        trust_server_certificate: profile.trust_server_certificate,
        auth_method: Some(profile.auth_method.clone()),
        show_all_catalog_items: profile.show_all_catalog_items,
        transport: profile.transport.clone(),
        driver_properties: profile.driver_properties.clone(),
    };
    let secret = request.password.clone();
    let snapshot = match run_catalog_initial(factory, request.clone(), &secret).await {
        Ok(snapshot) => snapshot,
        Err(introspection_error) => {
            let runtime_is_healthy = if let Some(runtime) = registry.get(&connection_id) {
                matches!(
                    tokio::time::timeout(Duration::from_secs(5), runtime.ping()).await,
                    Ok(Ok(()))
                )
            } else {
                false
            };
            if runtime_is_healthy {
                return Err(introspection_error);
            }
            ensure_runtime_connection(connection_factory, registry, sessions, &profile, &password)
                .await?;
            run_catalog_initial(factory, request, &secret).await?
        }
    };
    catalog_cache.put(&connection_id, "initial", snapshot.clone());
    Ok(snapshot)
}

async fn run_catalog_initial(
    factory: &dyn CatalogProviderFactory,
    request: CatalogRefreshRequest,
    secret: &str,
) -> Result<crate::backend::domain::catalog::CatalogSnapshot, ConnectionOperationError> {
    let provider = factory.provider(request.dialect.clone()).map_err(|error| {
        ConnectionOperationError::new(
            "INTROSPECTION_FAILED",
            redact(error.to_string(), secret),
            true,
        )
    })?;
    tokio::time::timeout(Duration::from_secs(60), provider.initial(request))
        .await
        .map_err(|_| {
            ConnectionOperationError::new(
                "INTROSPECTION_TIMEOUT",
                "Initial catalog introspection timed out",
                true,
            )
        })?
        .map_err(|error| {
            ConnectionOperationError::new(
                "INTROSPECTION_FAILED",
                redact(error.to_string(), secret),
                true,
            )
        })
}

pub async fn refresh_catalog_for_connection(
    profiles: &dyn ProfileStore,
    connection_factory: &dyn ConnectionDriverFactory,
    factory: &dyn CatalogProviderFactory,
    registry: &ConnectionRuntimeRegistry,
    sessions: &ResultSessionRegistry,
    catalog_cache: &CatalogSnapshotCache,
    secret_store: &dyn SecretStore,
    connection_id: String,
    version: String,
    generated_at: String,
) -> Result<crate::backend::domain::catalog::CatalogSnapshot, ConnectionOperationError> {
    let profile = profiles
        .list()
        .await?
        .into_iter()
        .find(|profile| profile.id == connection_id)
        .ok_or_else(|| validation_error("Connection profile was not found"))?;
    let password = secret_store.get(&connection_id).await?;
    ensure_runtime_connection(connection_factory, registry, sessions, &profile, &password).await?;
    let request = CatalogRefreshRequest {
        dialect: profile.dialect.clone(),
        connection_id: profile.id.clone(),
        version,
        generated_at,
        server: profile.server.clone(),
        port: profile.port,
        user_name: profile.user_name.clone(),
        password: password.clone(),
        database: profile.database.clone(),
        trust_server_certificate: profile.trust_server_certificate,
        auth_method: Some(profile.auth_method.clone()),
        show_all_catalog_items: profile.show_all_catalog_items,
        transport: profile.transport.clone(),
        driver_properties: profile.driver_properties.clone(),
    };
    let secret = request.password.clone();
    let snapshot = match run_catalog_refresh(factory, request.clone(), &secret).await {
        Ok(snapshot) => snapshot,
        Err(introspection_error) => {
            let runtime_is_healthy = if let Some(runtime) = registry.get(&connection_id) {
                matches!(
                    tokio::time::timeout(Duration::from_secs(5), runtime.ping()).await,
                    Ok(Ok(()))
                )
            } else {
                false
            };
            if runtime_is_healthy {
                return Err(introspection_error);
            }
            ensure_runtime_connection(connection_factory, registry, sessions, &profile, &password)
                .await?;
            run_catalog_refresh(factory, request, &secret).await?
        }
    };
    catalog_cache.put(&connection_id, "full", snapshot.clone());
    Ok(snapshot)
}

async fn ensure_runtime_connection(
    factory: &dyn ConnectionDriverFactory,
    registry: &ConnectionRuntimeRegistry,
    sessions: &ResultSessionRegistry,
    profile: &ConnectionProfileRecord,
    password: &str,
) -> Result<(), ConnectionOperationError> {
    if let Some(runtime) = registry.get(&profile.id) {
        if matches!(
            tokio::time::timeout(Duration::from_secs(5), runtime.ping()).await,
            Ok(Ok(()))
        ) {
            registry.touch(&profile.id);
            return Ok(());
        }
        if let Some(stale_runtime) = registry.remove_if_same(&profile.id, &runtime) {
            sessions.close_for_connection(&profile.id).await;
            let _ = close_runtime(&stale_runtime).await;
        }
    }
    sessions.close_for_connection(&profile.id).await;
    let driver = factory.driver(profile.dialect.clone()).map_err(|error| {
        ConnectionOperationError::new(
            "CONNECTION_RECONNECT_FAILED",
            redact(error.to_string(), password),
            true,
        )
    })?;
    let runtime = driver
        .connect(ResolvedConnection {
            profile: profile.clone(),
            password: password.to_owned(),
        })
        .await
        .map_err(|error| {
            ConnectionOperationError::new(
                "CONNECTION_RECONNECT_FAILED",
                redact(error.to_string(), password),
                true,
            )
        })?;
    registry.insert_with_settings(
        profile.id.clone(),
        runtime,
        profile.keep_alive_enabled,
        profile.keep_alive_interval_seconds,
        profile.idle_timeout_seconds,
    );
    Ok(())
}

async fn run_catalog_refresh(
    factory: &dyn CatalogProviderFactory,
    request: CatalogRefreshRequest,
    secret: &str,
) -> Result<crate::backend::domain::catalog::CatalogSnapshot, ConnectionOperationError> {
    let provider = factory.provider(request.dialect.clone()).map_err(|error| {
        ConnectionOperationError::new(
            "INTROSPECTION_FAILED",
            redact(error.to_string(), secret),
            true,
        )
    })?;
    tokio::time::timeout(Duration::from_secs(60), provider.refresh(request))
        .await
        .map_err(|_| {
            ConnectionOperationError::new(
                "INTROSPECTION_TIMEOUT",
                "Catalog refresh timed out",
                true,
            )
        })?
        .map_err(|error| {
            ConnectionOperationError::new(
                "INTROSPECTION_FAILED",
                redact(error.to_string(), secret),
                true,
            )
        })
}

#[allow(clippy::too_many_arguments)]
pub async fn expand_catalog_for_connection(
    profiles: &dyn ProfileStore,
    connection_factory: &dyn ConnectionDriverFactory,
    factory: &dyn CatalogProviderFactory,
    registry: &ConnectionRuntimeRegistry,
    sessions: &ResultSessionRegistry,
    catalog_cache: &CatalogSnapshotCache,
    secret_store: &dyn SecretStore,
    connection_id: String,
    scope: CatalogScope,
    version: String,
    generated_at: String,
) -> Result<crate::backend::domain::catalog::CatalogSnapshot, ConnectionOperationError> {
    let scope_key = scope.cache_key();
    if let Some(snapshot) = catalog_cache.get(&connection_id, &scope_key) {
        return Ok(snapshot);
    }
    let profile = profiles
        .list()
        .await?
        .into_iter()
        .find(|profile| profile.id == connection_id)
        .ok_or_else(|| validation_error("Connection profile was not found"))?;
    let password = secret_store.get(&connection_id).await?;
    ensure_runtime_connection(connection_factory, registry, sessions, &profile, &password).await?;
    let request = CatalogRefreshRequest {
        dialect: profile.dialect.clone(),
        connection_id: profile.id.clone(),
        version,
        generated_at,
        server: profile.server.clone(),
        port: profile.port,
        user_name: profile.user_name.clone(),
        password: password.clone(),
        database: profile.database.clone(),
        trust_server_certificate: profile.trust_server_certificate,
        auth_method: Some(profile.auth_method.clone()),
        show_all_catalog_items: profile.show_all_catalog_items,
        transport: profile.transport.clone(),
        driver_properties: profile.driver_properties.clone(),
    };
    let secret = request.password.clone();
    let snapshot = match run_catalog_expand(factory, request.clone(), scope.clone(), &secret).await
    {
        Ok(snapshot) => snapshot,
        Err(introspection_error) => {
            let runtime_is_healthy = if let Some(runtime) = registry.get(&connection_id) {
                matches!(
                    tokio::time::timeout(Duration::from_secs(5), runtime.ping()).await,
                    Ok(Ok(()))
                )
            } else {
                false
            };
            if runtime_is_healthy {
                return Err(introspection_error);
            }
            ensure_runtime_connection(connection_factory, registry, sessions, &profile, &password)
                .await?;
            run_catalog_expand(factory, request, scope, &secret).await?
        }
    };
    catalog_cache.put(&connection_id, &scope_key, snapshot.clone());
    Ok(snapshot)
}

async fn run_catalog_expand(
    factory: &dyn CatalogProviderFactory,
    request: CatalogRefreshRequest,
    scope: CatalogScope,
    secret: &str,
) -> Result<crate::backend::domain::catalog::CatalogSnapshot, ConnectionOperationError> {
    let provider = factory.provider(request.dialect.clone()).map_err(|error| {
        ConnectionOperationError::new(
            "INTROSPECTION_FAILED",
            redact(error.to_string(), secret),
            true,
        )
    })?;
    tokio::time::timeout(Duration::from_secs(60), provider.expand(request, scope))
        .await
        .map_err(|_| {
            ConnectionOperationError::new(
                "INTROSPECTION_TIMEOUT",
                "Catalog expansion timed out",
                true,
            )
        })?
        .map_err(|error| {
            ConnectionOperationError::new(
                "INTROSPECTION_FAILED",
                redact(error.to_string(), secret),
                true,
            )
        })
}

pub async fn rename_connection(
    profiles: &dyn ProfileStore,
    connection_id: String,
    name: String,
) -> Result<ConnectionProfileRecord, ConnectionOperationError> {
    if name.trim().is_empty() {
        return Err(validation_error("Connection name is required"));
    }
    let mut profile = profiles
        .list()
        .await?
        .into_iter()
        .find(|profile| profile.id == connection_id)
        .ok_or_else(|| validation_error("Connection profile was not found"))?;
    profile.name = name.trim().to_owned();
    profile.updated_at = timestamp();
    profiles.upsert(profile.clone()).await?;
    Ok(profile)
}
