use crate::backend::adapters::{
    DatabaseDriverFactory, JsonProfileStore, RealConnectionDriverFactory, SecretStoreRuntime,
};
use crate::backend::application;
use crate::backend::domain::catalog::CatalogSnapshot;
use crate::backend::domain::connection::{
    ConnectionDiagnostic, ConnectionDraft, ConnectionOperationError, ConnectionProfileRecord,
    ConnectionSessionResult, ConnectionUpdateDraft,
};
use crate::backend::domain::connection_profile_definition::ConnectionProfileDefinition;
use crate::backend::domain::query::QueryResult;
use crate::backend::domain::session_manager::{SessionKillResult, SessionTelemetry};
use crate::backend::ports::{
    CatalogProviderFactory, CatalogScope, CatalogSnapshotCache, ConnectionRuntimeRegistry,
    QueryCancellationRegistry, ResultSessionRegistry, SecretStoreProvider, VaultStatus,
};
use futures_util::future::{AbortHandle, Abortable};
use serde::Deserialize;
use std::collections::HashMap;
use tauri::{AppHandle, State};

#[derive(Debug, Deserialize)]
pub struct MenuCommandState {
    pub enabled: bool,
    pub checked: Option<bool>,
}

fn query_cancelled() -> ConnectionOperationError {
    ConnectionOperationError::new("QUERY_CANCELLED", "Query cancelled", true)
}

#[tauri::command]
pub fn sync_menu_state(
    app: AppHandle,
    states: HashMap<String, MenuCommandState>,
    version: u64,
) -> Result<(), String> {
    let _ = version;
    #[cfg(desktop)]
    {
        #[cfg(target_os = "macos")]
        if let Some(menu) = app.menu() {
            for (id, state) in states {
                let _ =
                    crate::native_menu::set_command_state(&menu, &id, state.enabled, state.checked);
            }
        }
    }

    #[cfg(not(desktop))]
    {
        let _ = (app, states);
    }

    Ok(())
}

#[tauri::command]
pub async fn get_database_sessions(
    registry: State<'_, ConnectionRuntimeRegistry>,
    connection_id: String,
) -> Result<SessionTelemetry, ConnectionOperationError> {
    application::session_manager::get_database_sessions(registry.inner(), connection_id).await
}

#[tauri::command]
pub async fn kill_database_session(
    registry: State<'_, ConnectionRuntimeRegistry>,
    connection_id: String,
    session_id: i32,
) -> Result<SessionKillResult, ConnectionOperationError> {
    application::session_manager::kill_database_session(registry.inner(), connection_id, session_id)
        .await
}

#[tauri::command]
pub async fn test_connection(
    factory: State<'_, RealConnectionDriverFactory>,
    secret_store: State<'_, SecretStoreRuntime>,
    draft: ConnectionDraft,
    connection_id: Option<String>,
) -> Result<ConnectionDiagnostic, ConnectionOperationError> {
    application::connections::test_connection(
        factory.inner(),
        secret_store.inner().store(),
        draft,
        connection_id,
    )
    .await
}

#[tauri::command]
pub async fn save_connection(
    app: AppHandle,
    factory: State<'_, RealConnectionDriverFactory>,
    catalog_factory: State<'_, DatabaseDriverFactory>,
    registry: State<'_, ConnectionRuntimeRegistry>,
    sessions: State<'_, ResultSessionRegistry>,
    catalog_cache: State<'_, CatalogSnapshotCache>,
    secret_store: State<'_, SecretStoreRuntime>,
    draft: ConnectionDraft,
) -> Result<ConnectionSessionResult, ConnectionOperationError> {
    let profiles = JsonProfileStore::new(app.clone());
    application::connections::save_and_connect(
        &profiles,
        factory.inner(),
        catalog_factory.inner(),
        registry.inner(),
        sessions.inner(),
        catalog_cache.inner(),
        secret_store.inner().store(),
        draft,
    )
    .await
}

#[tauri::command]
pub async fn save_connection_profile(
    app: AppHandle,
    secret_store: State<'_, SecretStoreRuntime>,
    draft: ConnectionDraft,
) -> Result<ConnectionProfileRecord, ConnectionOperationError> {
    let profiles = JsonProfileStore::new(app);
    application::connections::save_profile(&profiles, secret_store.inner().store(), draft).await
}

#[tauri::command]
pub async fn list_connection_profiles(
    app: AppHandle,
) -> Result<Vec<ConnectionProfileRecord>, ConnectionOperationError> {
    let profiles = JsonProfileStore::new(app);
    application::connections::list_connection_profiles(&profiles).await
}

#[tauri::command]
pub async fn list_connection_profile_definitions(
    app: AppHandle,
) -> Result<Vec<ConnectionProfileDefinition>, ConnectionOperationError> {
    let profiles = JsonProfileStore::new(app);
    application::connections::list_connection_profile_definitions(&profiles).await
}

#[tauri::command]
pub async fn upsert_connection_profile_definition(
    app: AppHandle,
    profile: ConnectionProfileDefinition,
) -> Result<ConnectionProfileDefinition, ConnectionOperationError> {
    let profiles = JsonProfileStore::new(app);
    application::connections::upsert_connection_profile_definition(&profiles, profile).await
}

#[tauri::command]
pub async fn delete_connection_profile_definition(
    app: AppHandle,
    profile_id: String,
) -> Result<(), ConnectionOperationError> {
    let profiles = JsonProfileStore::new(app);
    application::connections::delete_connection_profile_definition(&profiles, profile_id).await
}

#[tauri::command]
pub async fn list_connection_statuses(
    registry: State<'_, ConnectionRuntimeRegistry>,
) -> Result<Vec<String>, String> {
    Ok(registry.inner().connected_ids())
}

#[tauri::command]
pub async fn ping_connection(
    registry: State<'_, ConnectionRuntimeRegistry>,
    sessions: State<'_, ResultSessionRegistry>,
    connection_id: String,
) -> Result<bool, String> {
    Ok(application::connections::ping_existing_connection(
        registry.inner(),
        sessions.inner(),
        &connection_id,
    )
    .await)
}

#[tauri::command]
pub async fn connect_connection(
    app: AppHandle,
    factory: State<'_, RealConnectionDriverFactory>,
    catalog_factory: State<'_, DatabaseDriverFactory>,
    registry: State<'_, ConnectionRuntimeRegistry>,
    sessions: State<'_, ResultSessionRegistry>,
    catalog_cache: State<'_, CatalogSnapshotCache>,
    secret_store: State<'_, SecretStoreRuntime>,
    connection_id: String,
) -> Result<ConnectionSessionResult, ConnectionOperationError> {
    let profiles = JsonProfileStore::new(app.clone());
    catalog_factory.inner().invalidate(&connection_id);
    application::connections::connect_existing(
        &profiles,
        factory.inner(),
        catalog_factory.inner(),
        registry.inner(),
        sessions.inner(),
        catalog_cache.inner(),
        secret_store.inner().store(),
        connection_id,
    )
    .await
}

#[tauri::command]
pub async fn connect_connection_only(
    app: AppHandle,
    factory: State<'_, RealConnectionDriverFactory>,
    catalog_factory: State<'_, DatabaseDriverFactory>,
    registry: State<'_, ConnectionRuntimeRegistry>,
    sessions: State<'_, ResultSessionRegistry>,
    catalog_cache: State<'_, CatalogSnapshotCache>,
    secret_store: State<'_, SecretStoreRuntime>,
    connection_id: String,
) -> Result<ConnectionProfileRecord, ConnectionOperationError> {
    let profiles = JsonProfileStore::new(app);
    catalog_factory.inner().invalidate(&connection_id);
    application::connections::connect_existing_without_catalog(
        &profiles,
        factory.inner(),
        registry.inner(),
        sessions.inner(),
        catalog_cache.inner(),
        secret_store.inner().store(),
        connection_id,
    )
    .await
}

#[tauri::command]
pub async fn disconnect_connection(
    catalog_factory: State<'_, DatabaseDriverFactory>,
    registry: State<'_, ConnectionRuntimeRegistry>,
    sessions: State<'_, ResultSessionRegistry>,
    catalog_cache: State<'_, CatalogSnapshotCache>,
    connection_id: String,
) -> Result<(), ConnectionOperationError> {
    let result = application::connections::disconnect_existing(
        registry.inner(),
        sessions.inner(),
        &connection_id,
    )
    .await;
    catalog_factory.inner().invalidate(&connection_id);
    catalog_cache.inner().invalidate_connection(&connection_id);
    result
}

#[tauri::command]
pub async fn delete_connection(
    app: AppHandle,
    catalog_factory: State<'_, DatabaseDriverFactory>,
    registry: State<'_, ConnectionRuntimeRegistry>,
    sessions: State<'_, ResultSessionRegistry>,
    catalog_cache: State<'_, CatalogSnapshotCache>,
    secret_store: State<'_, SecretStoreRuntime>,
    connection_id: String,
) -> Result<(), ConnectionOperationError> {
    let profiles = JsonProfileStore::new(app.clone());
    let connection_id_for_cache = connection_id.clone();
    let result = application::connections::delete_connection(
        &profiles,
        registry.inner(),
        sessions.inner(),
        catalog_cache.inner(),
        secret_store.inner().store(),
        connection_id,
    )
    .await;
    if result.is_ok() {
        catalog_factory.inner().invalidate(&connection_id_for_cache);
    }
    result
}

#[tauri::command]
pub async fn update_connection(
    app: AppHandle,
    factory: State<'_, RealConnectionDriverFactory>,
    catalog_factory: State<'_, DatabaseDriverFactory>,
    registry: State<'_, ConnectionRuntimeRegistry>,
    catalog_cache: State<'_, CatalogSnapshotCache>,
    secret_store: State<'_, SecretStoreRuntime>,
    draft: ConnectionUpdateDraft,
    reconnect: bool,
) -> Result<ConnectionProfileRecord, ConnectionOperationError> {
    let profiles = JsonProfileStore::new(app.clone());
    let connection_id = draft.id.clone();
    catalog_factory.inner().invalidate(&connection_id);
    let result = application::connections::update_connection(
        &profiles,
        factory.inner(),
        registry.inner(),
        catalog_cache.inner(),
        secret_store.inner().store(),
        draft,
        reconnect,
    )
    .await;
    if result.is_ok() {
        catalog_factory.inner().invalidate(&connection_id);
    }
    result
}

#[tauri::command]
pub async fn refresh_connection_catalog(
    app: AppHandle,
    connection_factory: State<'_, RealConnectionDriverFactory>,
    factory: State<'_, DatabaseDriverFactory>,
    registry: State<'_, ConnectionRuntimeRegistry>,
    sessions: State<'_, ResultSessionRegistry>,
    catalog_cache: State<'_, CatalogSnapshotCache>,
    secret_store: State<'_, SecretStoreRuntime>,
    connection_id: String,
    version: String,
    generated_at: String,
) -> Result<CatalogSnapshot, ConnectionOperationError> {
    let profiles = JsonProfileStore::new(app.clone());
    catalog_cache.inner().invalidate_connection(&connection_id);
    application::connections::refresh_catalog_for_connection(
        &profiles,
        connection_factory.inner(),
        factory.inner(),
        registry.inner(),
        sessions.inner(),
        catalog_cache.inner(),
        secret_store.inner().store(),
        connection_id,
        version,
        generated_at,
    )
    .await
}

#[tauri::command]
pub async fn expand_connection_node(
    app: AppHandle,
    connection_factory: State<'_, RealConnectionDriverFactory>,
    factory: State<'_, DatabaseDriverFactory>,
    registry: State<'_, ConnectionRuntimeRegistry>,
    sessions: State<'_, ResultSessionRegistry>,
    catalog_cache: State<'_, CatalogSnapshotCache>,
    secret_store: State<'_, SecretStoreRuntime>,
    connection_id: String,
    scope: CatalogScope,
) -> Result<CatalogSnapshot, ConnectionOperationError> {
    let profiles = JsonProfileStore::new(app.clone());
    application::connections::expand_catalog_for_connection(
        &profiles,
        connection_factory.inner(),
        factory.inner(),
        registry.inner(),
        sessions.inner(),
        catalog_cache.inner(),
        secret_store.inner().store(),
        connection_id,
        scope,
        chrono::Utc::now().to_rfc3339(),
        chrono::Utc::now().to_rfc3339(),
    )
    .await
}

#[tauri::command]
pub async fn rename_connection(
    app: AppHandle,
    connection_id: String,
    name: String,
) -> Result<ConnectionProfileRecord, ConnectionOperationError> {
    let profiles = JsonProfileStore::new(app);
    application::connections::rename_connection(&profiles, connection_id, name).await
}

#[tauri::command]
pub async fn execute_query(
    registry: State<'_, ConnectionRuntimeRegistry>,
    catalog_cache: State<'_, CatalogSnapshotCache>,
    cancellation: State<'_, QueryCancellationRegistry>,
    connection_id: String,
    sql: String,
) -> Result<QueryResult, ConnectionOperationError> {
    let (handle, registration) = AbortHandle::new_pair();
    cancellation.begin(connection_id.clone(), handle);
    let result = match Abortable::new(
        application::connections::execute_query(
            registry.inner(),
            catalog_cache.inner(),
            connection_id.clone(),
            sql,
        ),
        registration,
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err(query_cancelled()),
    };
    cancellation.finish(&connection_id);
    result
}

#[tauri::command]
pub async fn start_query_session(
    registry: State<'_, ConnectionRuntimeRegistry>,
    sessions: State<'_, ResultSessionRegistry>,
    connection_id: String,
    sql: String,
    initial_rows: Option<usize>,
) -> Result<crate::backend::domain::query::QueryResultSessionStart, ConnectionOperationError> {
    application::connections::start_query_session(
        registry.inner(),
        sessions.inner(),
        connection_id,
        sql,
        initial_rows.unwrap_or(200),
    )
    .await
}

#[tauri::command]
pub async fn start_query_session_in_context(
    app: AppHandle,
    factory: State<'_, RealConnectionDriverFactory>,
    registry: State<'_, ConnectionRuntimeRegistry>,
    sessions: State<'_, ResultSessionRegistry>,
    secret_store: State<'_, SecretStoreRuntime>,
    connection_id: String,
    sql: String,
    database: Option<String>,
    schema: Option<String>,
    initial_rows: Option<usize>,
) -> Result<crate::backend::domain::query::QueryResultSessionStart, ConnectionOperationError> {
    let profiles = JsonProfileStore::new(app);
    application::connections::start_query_session_in_context(
        &profiles,
        factory.inner(),
        secret_store.inner().store(),
        registry.inner(),
        sessions.inner(),
        connection_id,
        sql,
        database,
        schema,
        initial_rows.unwrap_or(200),
    )
    .await
}

#[tauri::command]
pub async fn fetch_query_session(
    sessions: State<'_, ResultSessionRegistry>,
    session_id: String,
    requested_rows: Option<usize>,
) -> Result<crate::backend::domain::query::QueryResultBatch, ConnectionOperationError> {
    application::connections::fetch_query_session(
        sessions.inner(),
        session_id,
        requested_rows.unwrap_or(200),
    )
    .await
}

#[tauri::command]
pub async fn cancel_query_session(
    sessions: State<'_, ResultSessionRegistry>,
    session_id: String,
) -> Result<(), ConnectionOperationError> {
    application::connections::cancel_query_session(sessions.inner(), session_id).await
}

#[tauri::command]
pub async fn close_query_session(
    sessions: State<'_, ResultSessionRegistry>,
    session_id: String,
) -> Result<(), ConnectionOperationError> {
    application::connections::close_query_session(sessions.inner(), session_id).await
}

#[tauri::command]
pub async fn execute_query_in_context(
    app: AppHandle,
    factory: State<'_, RealConnectionDriverFactory>,
    registry: State<'_, ConnectionRuntimeRegistry>,
    secret_store: State<'_, SecretStoreRuntime>,
    cancellation: State<'_, QueryCancellationRegistry>,
    connection_id: String,
    sql: String,
    database: Option<String>,
    schema: Option<String>,
) -> Result<QueryResult, ConnectionOperationError> {
    let profiles = JsonProfileStore::new(app);
    let (handle, registration) = AbortHandle::new_pair();
    cancellation.begin(connection_id.clone(), handle);
    let result = match Abortable::new(
        application::connections::execute_query_in_context(
            &profiles,
            factory.inner(),
            secret_store.inner().store(),
            registry.inner(),
            connection_id.clone(),
            sql,
            database,
            schema,
        ),
        registration,
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err(query_cancelled()),
    };
    cancellation.finish(&connection_id);
    result
}

#[tauri::command]
pub fn cancel_query(
    cancellation: State<'_, QueryCancellationRegistry>,
    connection_id: String,
) -> Result<bool, ConnectionOperationError> {
    Ok(cancellation.cancel(&connection_id))
}

#[tauri::command]
pub fn secret_store_status(
    secret_store: State<'_, SecretStoreRuntime>,
) -> Result<String, ConnectionOperationError> {
    let provider = match secret_store.provider() {
        SecretStoreProvider::Vault => "vault",
        SecretStoreProvider::Keychain => "keychain",
    };
    let status = match secret_store.vault().map(|vault| vault.status()) {
        Some(VaultStatus::Uninitialized) => "uninitialized",
        Some(VaultStatus::Locked) => "locked",
        Some(VaultStatus::Unlocked) => "unlocked",
        Some(VaultStatus::Error) => "error",
        None => "available",
    };
    Ok(format!("{provider}:{status}"))
}

#[tauri::command]
pub async fn create_secret_vault(
    secret_store: State<'_, SecretStoreRuntime>,
    master_password: String,
) -> Result<(), ConnectionOperationError> {
    let vault = secret_store.vault().ok_or_else(|| {
        ConnectionOperationError::new(
            "SECRET_STORE_CONFIGURATION",
            "The selected secret store is not a vault",
            false,
        )
    })?;
    vault.create(master_password).await
}

#[tauri::command]
pub async fn unlock_secret_vault(
    secret_store: State<'_, SecretStoreRuntime>,
    master_password: String,
) -> Result<(), ConnectionOperationError> {
    let vault = secret_store.vault().ok_or_else(|| {
        ConnectionOperationError::new(
            "SECRET_STORE_CONFIGURATION",
            "The selected secret store is not a vault",
            false,
        )
    })?;
    vault.unlock(master_password).await
}

#[tauri::command]
pub fn lock_secret_vault(
    secret_store: State<'_, SecretStoreRuntime>,
) -> Result<(), ConnectionOperationError> {
    let vault = secret_store.vault().ok_or_else(|| {
        ConnectionOperationError::new(
            "SECRET_STORE_CONFIGURATION",
            "The selected secret store is not a vault",
            false,
        )
    })?;
    vault.lock()
}
