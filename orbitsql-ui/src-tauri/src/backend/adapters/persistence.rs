use crate::backend::domain::connection::{ConnectionOperationError, ConnectionProfileRecord};
use crate::backend::domain::connection_profile_definition::{
    ConnectionProfileDefinition, built_in_connection_profile_definitions,
};
use crate::backend::ports::{ConnectionProfileDefinitionStore, ProfileStore, SecretStore};
use futures_util::future::BoxFuture;
use keyring::Entry;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

fn persistence_error(message: impl Into<String>) -> ConnectionOperationError {
    ConnectionOperationError::new("PERSISTENCE_FAILED", message, true)
}

fn profiles_path(app: &AppHandle) -> Result<PathBuf, ConnectionOperationError> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("connection-profiles.json"))
        .map_err(|error| persistence_error(error.to_string()))
}

fn connection_profile_definitions_path(
    app: &AppHandle,
) -> Result<PathBuf, ConnectionOperationError> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("connection-environment-profiles.json"))
        .map_err(|error| persistence_error(error.to_string()))
}

pub async fn load_profiles(
    app: &AppHandle,
) -> Result<Vec<ConnectionProfileRecord>, ConnectionOperationError> {
    let path = profiles_path(app)?;
    match tokio::fs::metadata(&path).await {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Vec::new());
        }
        Err(error) => return Err(persistence_error(error.to_string())),
    }
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|error| persistence_error(error.to_string()))?;
    serde_json::from_slice(&bytes).map_err(|error| persistence_error(error.to_string()))
}

pub async fn save_profiles(
    app: &AppHandle,
    profiles: &[ConnectionProfileRecord],
) -> Result<(), ConnectionOperationError> {
    let path = profiles_path(app)?;
    let directory = path
        .parent()
        .ok_or_else(|| persistence_error("Profile storage directory is unavailable"))?;
    tokio::fs::create_dir_all(directory)
        .await
        .map_err(|error| persistence_error(error.to_string()))?;
    let bytes = serde_json::to_vec_pretty(profiles)
        .map_err(|error| persistence_error(error.to_string()))?;
    let temporary_path = path.with_extension("json.tmp");
    tokio::fs::write(&temporary_path, bytes)
        .await
        .map_err(|error| persistence_error(error.to_string()))?;
    tokio::fs::rename(temporary_path, path)
        .await
        .map_err(|error| persistence_error(error.to_string()))
}

async fn load_connection_profile_definitions(
    app: &AppHandle,
) -> Result<Vec<ConnectionProfileDefinition>, ConnectionOperationError> {
    let path = connection_profile_definitions_path(app)?;
    match tokio::fs::metadata(&path).await {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(built_in_connection_profile_definitions());
        }
        Err(error) => return Err(persistence_error(error.to_string())),
    }
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|error| persistence_error(error.to_string()))?;
    serde_json::from_slice(&bytes).map_err(|error| persistence_error(error.to_string()))
}

async fn save_connection_profile_definitions(
    app: &AppHandle,
    profiles: &[ConnectionProfileDefinition],
) -> Result<(), ConnectionOperationError> {
    let path = connection_profile_definitions_path(app)?;
    let directory = path
        .parent()
        .ok_or_else(|| persistence_error("Profile storage directory is unavailable"))?;
    tokio::fs::create_dir_all(directory)
        .await
        .map_err(|error| persistence_error(error.to_string()))?;
    let bytes = serde_json::to_vec_pretty(profiles)
        .map_err(|error| persistence_error(error.to_string()))?;
    let temporary_path = path.with_extension("json.tmp");
    tokio::fs::write(&temporary_path, bytes)
        .await
        .map_err(|error| persistence_error(error.to_string()))?;
    tokio::fs::rename(temporary_path, path)
        .await
        .map_err(|error| persistence_error(error.to_string()))
}

async fn upsert_connection_profile_definition(
    app: &AppHandle,
    profile: ConnectionProfileDefinition,
) -> Result<(), ConnectionOperationError> {
    if profile.name.trim().is_empty() || profile.color.trim().is_empty() {
        return Err(persistence_error("Profile name and color are required"));
    }
    let mut profiles = load_connection_profile_definitions(app).await?;
    if let Some(existing) = profiles.iter_mut().find(|item| item.id == profile.id) {
        *existing = profile;
    } else {
        profiles.push(profile);
    }
    save_connection_profile_definitions(app, &profiles).await
}

async fn remove_connection_profile_definition(
    app: &AppHandle,
    profile_id: &str,
) -> Result<(), ConnectionOperationError> {
    let mut profiles = load_connection_profile_definitions(app).await?;
    profiles.retain(|profile| profile.id != profile_id);
    save_connection_profile_definitions(app, &profiles).await
}

pub async fn upsert_profile(
    app: &AppHandle,
    profile: ConnectionProfileRecord,
) -> Result<(), ConnectionOperationError> {
    let mut profiles = load_profiles(app).await?;
    if let Some(existing) = profiles.iter_mut().find(|item| item.id == profile.id) {
        *existing = profile;
    } else {
        profiles.push(profile);
    }
    save_profiles(app, &profiles).await
}

pub async fn remove_profile(
    app: &AppHandle,
    connection_id: &str,
) -> Result<(), ConnectionOperationError> {
    let mut profiles = load_profiles(app).await?;
    profiles.retain(|profile| profile.id != connection_id);
    save_profiles(app, &profiles).await
}

#[derive(Clone)]
pub struct JsonProfileStore {
    app: AppHandle,
}

impl JsonProfileStore {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl ProfileStore for JsonProfileStore {
    fn list(
        &self,
    ) -> BoxFuture<'_, Result<Vec<ConnectionProfileRecord>, ConnectionOperationError>> {
        let app = self.app.clone();
        Box::pin(async move { load_profiles(&app).await })
    }

    fn upsert(
        &self,
        profile: ConnectionProfileRecord,
    ) -> BoxFuture<'_, Result<(), ConnectionOperationError>> {
        let app = self.app.clone();
        Box::pin(async move { upsert_profile(&app, profile).await })
    }

    fn remove(&self, connection_id: &str) -> BoxFuture<'_, Result<(), ConnectionOperationError>> {
        let app = self.app.clone();
        let connection_id = connection_id.to_owned();
        Box::pin(async move { remove_profile(&app, &connection_id).await })
    }
}

impl ConnectionProfileDefinitionStore for JsonProfileStore {
    fn list(
        &self,
    ) -> BoxFuture<'_, Result<Vec<ConnectionProfileDefinition>, ConnectionOperationError>> {
        let app = self.app.clone();
        Box::pin(async move { load_connection_profile_definitions(&app).await })
    }

    fn upsert(
        &self,
        profile: ConnectionProfileDefinition,
    ) -> BoxFuture<'_, Result<(), ConnectionOperationError>> {
        let app = self.app.clone();
        Box::pin(async move { upsert_connection_profile_definition(&app, profile).await })
    }

    fn remove(&self, profile_id: &str) -> BoxFuture<'_, Result<(), ConnectionOperationError>> {
        let app = self.app.clone();
        let profile_id = profile_id.to_owned();
        Box::pin(async move { remove_connection_profile_definition(&app, &profile_id).await })
    }
}

#[derive(Clone, Default)]
pub struct OsSecretStore;

impl OsSecretStore {
    fn entry(connection_id: String) -> Result<Entry, ConnectionOperationError> {
        Entry::new("OrbitSQL", &format!("connection:{connection_id}"))
            .map_err(|error| persistence_error(error.to_string()))
    }
}

impl SecretStore for OsSecretStore {
    fn set(
        &self,
        connection_id: &str,
        secret: String,
    ) -> BoxFuture<'static, Result<(), ConnectionOperationError>> {
        let connection_id = connection_id.to_owned();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                Self::entry(connection_id)?
                    .set_password(&secret)
                    .map_err(|error| persistence_error(error.to_string()))
            })
            .await
            .map_err(|error| persistence_error(error.to_string()))?
        })
    }

    fn get(
        &self,
        connection_id: &str,
    ) -> BoxFuture<'static, Result<String, ConnectionOperationError>> {
        let connection_id = connection_id.to_owned();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                Self::entry(connection_id)?
                    .get_password()
                    .map_err(|error| persistence_error(error.to_string()))
            })
            .await
            .map_err(|error| persistence_error(error.to_string()))?
        })
    }

    fn delete(
        &self,
        connection_id: &str,
    ) -> BoxFuture<'static, Result<(), ConnectionOperationError>> {
        let connection_id = connection_id.to_owned();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                match Self::entry(connection_id)?.delete_credential() {
                    Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                    Err(error) => Err(persistence_error(error.to_string())),
                }
            })
            .await
            .map_err(|error| persistence_error(error.to_string()))?
        })
    }
}
