use crate::backend::domain::connection::ConnectionOperationError;
use crate::backend::ports::{SecretStore, SecretStoreProvider, VaultControl, VaultStatus};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use futures_util::future::BoxFuture;
use rand::TryRngCore;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use zeroize::Zeroizing;

const VAULT_VERSION: u32 = 1;
const KDF_ALGORITHM: &str = "argon2id";
const ENCRYPTION_ALGORITHM: &str = "xchacha20-poly1305";
const KEY_LENGTH: usize = 32;
const SALT_LENGTH: usize = 16;
const NONCE_LENGTH: usize = 24;
const MEMORY_COST: u32 = 19_456;
const TIME_COST: u32 = 2;
const PARALLELISM: u32 = 1;

fn secret_error(
    code: &str,
    message: impl Into<String>,
    recoverable: bool,
) -> ConnectionOperationError {
    ConnectionOperationError::new(code, message, recoverable)
}

fn secret_ref(connection_id: &str) -> String {
    format!("connection:{connection_id}:password")
}

#[derive(Serialize, Deserialize)]
struct VaultFile {
    version: u32,
    kdf: KdfMetadata,
    encryption: EncryptionMetadata,
}

#[derive(Serialize, Deserialize)]
struct KdfMetadata {
    algorithm: String,
    salt: Vec<u8>,
    memory_cost: u32,
    time_cost: u32,
    parallelism: u32,
}

#[derive(Serialize, Deserialize)]
struct EncryptionMetadata {
    algorithm: String,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

struct UnlockedVault {
    key: Zeroizing<[u8; KEY_LENGTH]>,
    salt: Vec<u8>,
    memory_cost: u32,
    time_cost: u32,
    parallelism: u32,
    secrets: BTreeMap<String, String>,
}

enum VaultState {
    Locked,
    Unlocked(UnlockedVault),
    Error,
}

pub struct LocalEncryptedVaultSecretStore {
    path: PathBuf,
    state: Arc<Mutex<VaultState>>,
}

impl LocalEncryptedVaultSecretStore {
    pub fn new(app: &AppHandle) -> Result<Self, ConnectionOperationError> {
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|error| secret_error("SECRET_STORE_IO", error.to_string(), true))?;
        Ok(Self {
            path: directory.join("vault.enc"),
            state: Arc::new(Mutex::new(VaultState::Locked)),
        })
    }

    pub fn new_for_path(path: PathBuf) -> Self {
        Self {
            path,
            state: Arc::new(Mutex::new(VaultState::Locked)),
        }
    }

    fn lock_state(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, VaultState>, ConnectionOperationError> {
        self.state
            .lock()
            .map_err(|_| secret_error("SECRET_STORE_ERROR", "Vault state is unavailable", false))
    }

    fn derive_key(
        master_password: &str,
        salt: &[u8],
        memory_cost: u32,
        time_cost: u32,
        parallelism: u32,
    ) -> Result<Zeroizing<[u8; KEY_LENGTH]>, ConnectionOperationError> {
        let params = argon2::Params::new(memory_cost, time_cost, parallelism, Some(KEY_LENGTH))
            .map_err(|_| {
                secret_error("VAULT_CONFIGURATION", "Invalid vault KDF parameters", false)
            })?;
        let argon =
            argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
        let mut key = Zeroizing::new([0_u8; KEY_LENGTH]);
        argon
            .hash_password_into(master_password.as_bytes(), salt, key.as_mut())
            .map_err(|_| {
                secret_error(
                    "VAULT_UNLOCK_FAILED",
                    "Unable to unlock vault. Check your master password.",
                    true,
                )
            })?;
        Ok(key)
    }

    fn encrypt(
        key: &[u8; KEY_LENGTH],
        salt: &[u8],
        memory_cost: u32,
        time_cost: u32,
        parallelism: u32,
        secrets: &BTreeMap<String, String>,
    ) -> Result<Vec<u8>, ConnectionOperationError> {
        let plaintext = serde_json::to_vec(secrets)
            .map_err(|error| secret_error("VAULT_ENCRYPTION", error.to_string(), false))?;
        let mut nonce = [0_u8; NONCE_LENGTH];
        OsRng
            .try_fill_bytes(&mut nonce)
            .map_err(|error| secret_error("VAULT_RANDOMNESS", error.to_string(), false))?;
        let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
        let ciphertext = cipher
            .encrypt(XNonce::from_slice(&nonce), plaintext.as_ref())
            .map_err(|_| secret_error("VAULT_ENCRYPTION", "Unable to encrypt vault", false))?;
        serde_json::to_vec(&VaultFile {
            version: VAULT_VERSION,
            kdf: KdfMetadata {
                algorithm: KDF_ALGORITHM.to_owned(),
                salt: salt.to_vec(),
                memory_cost,
                time_cost,
                parallelism,
            },
            encryption: EncryptionMetadata {
                algorithm: ENCRYPTION_ALGORITHM.to_owned(),
                nonce: nonce.to_vec(),
                ciphertext,
            },
        })
        .map_err(|error| secret_error("VAULT_SERIALIZATION", error.to_string(), false))
    }

    fn decrypt(
        key: &[u8; KEY_LENGTH],
        file: VaultFile,
    ) -> Result<BTreeMap<String, String>, ConnectionOperationError> {
        if file.version != VAULT_VERSION {
            return Err(secret_error(
                "VAULT_UNSUPPORTED_VERSION",
                "This vault version is not supported",
                false,
            ));
        }
        if file.kdf.algorithm != KDF_ALGORITHM
            || file.encryption.algorithm != ENCRYPTION_ALGORITHM
            || file.kdf.salt.len() != SALT_LENGTH
            || file.encryption.nonce.len() != NONCE_LENGTH
        {
            return Err(secret_error(
                "VAULT_CORRUPTED",
                "Vault format is invalid",
                false,
            ));
        }
        let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
        let plaintext = cipher
            .decrypt(
                XNonce::from_slice(&file.encryption.nonce),
                file.encryption.ciphertext.as_ref(),
            )
            .map_err(|_| {
                secret_error(
                    "VAULT_UNLOCK_FAILED",
                    "Unable to unlock vault. Check your master password.",
                    true,
                )
            })?;
        serde_json::from_slice(&plaintext)
            .map_err(|_| secret_error("VAULT_CORRUPTED", "Vault contents are invalid", false))
    }

    fn write_atomic(&self, bytes: &[u8]) -> Result<(), ConnectionOperationError> {
        let directory = self.path.parent().ok_or_else(|| {
            secret_error("SECRET_STORE_IO", "Vault directory is unavailable", false)
        })?;
        std::fs::create_dir_all(directory)
            .map_err(|error| secret_error("SECRET_STORE_IO", error.to_string(), true))?;
        let temporary_path = self.path.with_extension("enc.tmp");
        let mut options = std::fs::OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary_path)
            .map_err(|error| secret_error("SECRET_STORE_IO", error.to_string(), true))?;
        use std::io::Write;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|error| secret_error("SECRET_STORE_IO", error.to_string(), true))?;
        std::fs::rename(&temporary_path, &self.path)
            .map_err(|error| secret_error("SECRET_STORE_IO", error.to_string(), true))
    }

    fn save_unlocked(&self, vault: &UnlockedVault) -> Result<(), ConnectionOperationError> {
        let bytes = Self::encrypt(
            &*vault.key,
            &vault.salt,
            vault.memory_cost,
            vault.time_cost,
            vault.parallelism,
            &vault.secrets,
        )?;
        self.write_atomic(&bytes)
    }

    fn load_file(&self) -> Result<VaultFile, ConnectionOperationError> {
        let bytes = std::fs::read(&self.path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                secret_error("VAULT_UNINITIALIZED", "Vault has not been created", true)
            } else {
                secret_error("SECRET_STORE_IO", error.to_string(), true)
            }
        })?;
        serde_json::from_slice(&bytes)
            .map_err(|_| secret_error("VAULT_CORRUPTED", "Vault format is invalid", false))
    }
}

impl VaultControl for LocalEncryptedVaultSecretStore {
    fn status(&self) -> VaultStatus {
        let state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => return VaultStatus::Error,
        };
        match &*state {
            VaultState::Unlocked(_) => VaultStatus::Unlocked,
            VaultState::Error => VaultStatus::Error,
            VaultState::Locked if self.path.exists() => VaultStatus::Locked,
            VaultState::Locked => VaultStatus::Uninitialized,
        }
    }

    fn create(
        &self,
        master_password: String,
    ) -> BoxFuture<'static, Result<(), ConnectionOperationError>> {
        let path = self.path.clone();
        let state = self.state.clone();
        Box::pin(async move {
            if master_password.is_empty() {
                return Err(secret_error(
                    "VAULT_PASSWORD_REQUIRED",
                    "Master password is required",
                    true,
                ));
            }
            if path.exists() {
                return Err(secret_error(
                    "VAULT_ALREADY_EXISTS",
                    "Vault already exists",
                    true,
                ));
            }
            let mut salt = vec![0_u8; SALT_LENGTH];
            OsRng
                .try_fill_bytes(&mut salt)
                .map_err(|error| secret_error("VAULT_RANDOMNESS", error.to_string(), false))?;
            let key = LocalEncryptedVaultSecretStore::derive_key(
                &master_password,
                &salt,
                MEMORY_COST,
                TIME_COST,
                PARALLELISM,
            )?;
            let vault = UnlockedVault {
                key,
                salt,
                memory_cost: MEMORY_COST,
                time_cost: TIME_COST,
                parallelism: PARALLELISM,
                secrets: BTreeMap::new(),
            };
            let store = LocalEncryptedVaultSecretStore { path, state };
            store.save_unlocked(&vault)?;
            *store.lock_state()? = VaultState::Unlocked(vault);
            Ok(())
        })
    }

    fn unlock(
        &self,
        master_password: String,
    ) -> BoxFuture<'static, Result<(), ConnectionOperationError>> {
        let path = self.path.clone();
        let state = self.state.clone();
        Box::pin(async move {
            let store = LocalEncryptedVaultSecretStore { path, state };
            let result = async {
                let file = store.load_file()?;
                if file.kdf.algorithm != KDF_ALGORITHM {
                    return Err(secret_error(
                        "VAULT_UNSUPPORTED_KDF",
                        "This vault KDF is not supported",
                        false,
                    ));
                }
                let key = Self::derive_key(
                    &master_password,
                    &file.kdf.salt,
                    file.kdf.memory_cost,
                    file.kdf.time_cost,
                    file.kdf.parallelism,
                )?;
                let salt = file.kdf.salt.clone();
                let memory_cost = file.kdf.memory_cost;
                let time_cost = file.kdf.time_cost;
                let parallelism = file.kdf.parallelism;
                let secrets = Self::decrypt(&key, file)?;
                let vault = UnlockedVault {
                    key,
                    salt,
                    memory_cost,
                    time_cost,
                    parallelism,
                    secrets,
                };
                *store.lock_state()? = VaultState::Unlocked(vault);
                Ok(())
            }
            .await;
            if matches!(
                result
                    .as_ref()
                    .map(|_| ())
                    .err()
                    .map(|error| error.code.as_str()),
                Some("VAULT_CORRUPTED")
                    | Some("VAULT_UNSUPPORTED_KDF")
                    | Some("VAULT_UNSUPPORTED_VERSION")
            ) {
                *store.lock_state()? = VaultState::Error;
            }
            result
        })
    }

    fn lock(&self) -> Result<(), ConnectionOperationError> {
        *self.lock_state()? = VaultState::Locked;
        Ok(())
    }
}

impl SecretStore for LocalEncryptedVaultSecretStore {
    fn set(
        &self,
        connection_id: &str,
        secret: String,
    ) -> BoxFuture<'static, Result<(), ConnectionOperationError>> {
        let path = self.path.clone();
        let state = self.state.clone();
        let key = secret_ref(connection_id);
        Box::pin(async move {
            let store = Self { path, state };
            let mut guard = store.lock_state()?;
            let VaultState::Unlocked(vault) = &mut *guard else {
                return Err(secret_error(
                    "SECRET_STORE_LOCKED",
                    "Unlock the vault before saving credentials",
                    true,
                ));
            };
            let mut next = BTreeMap::clone(&vault.secrets);
            next.insert(key, secret);
            let candidate = UnlockedVault {
                key: Zeroizing::new(*vault.key),
                salt: vault.salt.clone(),
                memory_cost: vault.memory_cost,
                time_cost: vault.time_cost,
                parallelism: vault.parallelism,
                secrets: next,
            };
            store.save_unlocked(&candidate)?;
            vault.secrets = candidate.secrets;
            Ok(())
        })
    }

    fn get(
        &self,
        connection_id: &str,
    ) -> BoxFuture<'static, Result<String, ConnectionOperationError>> {
        let state = self.state.clone();
        let key = secret_ref(connection_id);
        Box::pin(async move {
            let store = Self {
                path: PathBuf::new(),
                state,
            };
            let guard = store.lock_state()?;
            let VaultState::Unlocked(vault) = &*guard else {
                return Err(secret_error(
                    "SECRET_STORE_LOCKED",
                    "Unlock the vault before using credentials",
                    true,
                ));
            };
            vault
                .secrets
                .get(&key)
                .cloned()
                .ok_or_else(|| secret_error("SECRET_NOT_FOUND", "Credential was not found", true))
        })
    }

    fn delete(
        &self,
        connection_id: &str,
    ) -> BoxFuture<'static, Result<(), ConnectionOperationError>> {
        let path = self.path.clone();
        let state = self.state.clone();
        let key = secret_ref(connection_id);
        Box::pin(async move {
            let store = Self { path, state };
            let mut guard = store.lock_state()?;
            let VaultState::Unlocked(vault) = &mut *guard else {
                return Err(secret_error(
                    "SECRET_STORE_LOCKED",
                    "Unlock the vault before deleting credentials",
                    true,
                ));
            };
            if !vault.secrets.contains_key(&key) {
                return Ok(());
            }
            let mut next = BTreeMap::clone(&vault.secrets);
            next.remove(&key);
            let candidate = UnlockedVault {
                key: Zeroizing::new(*vault.key),
                salt: vault.salt.clone(),
                memory_cost: vault.memory_cost,
                time_cost: vault.time_cost,
                parallelism: vault.parallelism,
                secrets: next,
            };
            store.save_unlocked(&candidate)?;
            vault.secrets = candidate.secrets;
            Ok(())
        })
    }
}

#[derive(Clone)]
pub struct SecretStoreRuntime {
    provider: SecretStoreProvider,
    store: Arc<dyn SecretStore>,
    vault: Option<Arc<dyn VaultControl>>,
}

impl SecretStoreRuntime {
    pub fn provider(&self) -> SecretStoreProvider {
        self.provider
    }

    pub fn store(&self) -> &dyn SecretStore {
        self.store.as_ref()
    }

    pub fn vault(&self) -> Option<&dyn VaultControl> {
        self.vault.as_deref()
    }
}

impl SecretStore for SecretStoreRuntime {
    fn set(
        &self,
        connection_id: &str,
        secret: String,
    ) -> BoxFuture<'static, Result<(), ConnectionOperationError>> {
        self.store.set(connection_id, secret)
    }

    fn get(
        &self,
        connection_id: &str,
    ) -> BoxFuture<'static, Result<String, ConnectionOperationError>> {
        self.store.get(connection_id)
    }

    fn delete(
        &self,
        connection_id: &str,
    ) -> BoxFuture<'static, Result<(), ConnectionOperationError>> {
        self.store.delete(connection_id)
    }
}

#[derive(Clone, Default)]
pub struct InMemorySecretStore {
    secrets: Arc<Mutex<BTreeMap<String, String>>>,
}

impl SecretStore for InMemorySecretStore {
    fn set(
        &self,
        connection_id: &str,
        secret: String,
    ) -> BoxFuture<'static, Result<(), ConnectionOperationError>> {
        let secrets = Arc::clone(&self.secrets);
        let key = secret_ref(connection_id);
        Box::pin(async move {
            secrets
                .lock()
                .map_err(|_| {
                    secret_error("SECRET_STORE_ERROR", "Secret store is unavailable", false)
                })?
                .insert(key, secret);
            Ok(())
        })
    }

    fn get(
        &self,
        connection_id: &str,
    ) -> BoxFuture<'static, Result<String, ConnectionOperationError>> {
        let secrets = Arc::clone(&self.secrets);
        let key = secret_ref(connection_id);
        Box::pin(async move {
            secrets
                .lock()
                .map_err(|_| {
                    secret_error("SECRET_STORE_ERROR", "Secret store is unavailable", false)
                })?
                .get(&key)
                .cloned()
                .ok_or_else(|| secret_error("SECRET_NOT_FOUND", "Credential was not found", true))
        })
    }

    fn delete(
        &self,
        connection_id: &str,
    ) -> BoxFuture<'static, Result<(), ConnectionOperationError>> {
        let secrets = Arc::clone(&self.secrets);
        let key = secret_ref(connection_id);
        Box::pin(async move {
            secrets
                .lock()
                .map_err(|_| {
                    secret_error("SECRET_STORE_ERROR", "Secret store is unavailable", false)
                })?
                .remove(&key);
            Ok(())
        })
    }
}

pub fn configured_provider(
    value: Option<&str>,
) -> Result<SecretStoreProvider, ConnectionOperationError> {
    match value {
        None => Ok(SecretStoreProvider::Keychain),
        Some("vault") if cfg!(target_os = "macos") => Ok(SecretStoreProvider::Keychain),
        Some("vault") => Ok(SecretStoreProvider::Vault),
        Some("keychain") => Ok(SecretStoreProvider::Keychain),
        Some(value) => Err(secret_error(
            "SECRET_STORE_CONFIGURATION",
            format!("Unsupported ORBIT_SECRET_STORE value: {value}"),
            false,
        )),
    }
}

pub fn resolve_secret_store(
    app: &AppHandle,
) -> Result<SecretStoreRuntime, ConnectionOperationError> {
    let provider = configured_provider(std::env::var("ORBIT_SECRET_STORE").ok().as_deref())?;
    match provider {
        SecretStoreProvider::Vault => {
            let vault = Arc::new(LocalEncryptedVaultSecretStore::new(app)?);
            Ok(SecretStoreRuntime {
                provider,
                store: vault.clone(),
                vault: Some(vault),
            })
        }
        SecretStoreProvider::Keychain => Ok(SecretStoreRuntime {
            provider,
            store: Arc::new(super::persistence::OsSecretStore),
            vault: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::ports::SecretStore;

    #[tokio::test]
    async fn vault_round_trip_is_encrypted_and_lockable() {
        let directory = tempfile::tempdir().expect("temporary vault directory");
        let path = directory.path().join("vault.enc");
        let store = LocalEncryptedVaultSecretStore::new_for_path(path.clone());

        assert_eq!(store.status(), VaultStatus::Uninitialized);
        store
            .create("correct horse battery staple".to_owned())
            .await
            .expect("create vault");
        assert_eq!(store.status(), VaultStatus::Unlocked);
        store
            .set("connection-a", "secret-a".to_owned())
            .await
            .expect("save secret");
        assert_eq!(
            store.get("connection-a").await.expect("read secret"),
            "secret-a"
        );
        let encrypted = std::fs::read(&path).expect("read vault file");
        assert!(!String::from_utf8_lossy(&encrypted).contains("secret-a"));
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;
        #[cfg(unix)]
        assert_eq!(
            std::fs::metadata(&path)
                .expect("vault metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        store.lock().expect("lock vault");
        assert_eq!(store.status(), VaultStatus::Locked);
        assert_eq!(
            store
                .get("connection-a")
                .await
                .expect_err("locked get")
                .code,
            "SECRET_STORE_LOCKED"
        );
        store
            .unlock("wrong password".to_owned())
            .await
            .expect_err("wrong password");
        assert_eq!(
            std::fs::read(&path).expect("read unchanged vault"),
            encrypted
        );
        store
            .unlock("correct horse battery staple".to_owned())
            .await
            .expect("unlock vault");
        assert_eq!(
            store.get("connection-a").await.expect("read after unlock"),
            "secret-a"
        );

        store.delete("connection-a").await.expect("delete secret");
        assert_eq!(
            store
                .get("connection-a")
                .await
                .expect_err("missing secret")
                .code,
            "SECRET_NOT_FOUND"
        );
    }

    #[tokio::test]
    async fn corrupted_vault_is_rejected_without_becoming_empty() {
        let directory = tempfile::tempdir().expect("temporary vault directory");
        let path = directory.path().join("vault.enc");
        let store = LocalEncryptedVaultSecretStore::new_for_path(path.clone());
        store
            .create("master password".to_owned())
            .await
            .expect("create vault");
        let original = std::fs::read(&path).expect("read vault");
        std::fs::write(&path, b"not a vault").expect("corrupt vault");
        let error = store
            .unlock("master password".to_owned())
            .await
            .expect_err("reject corruption");
        assert_eq!(error.code, "VAULT_CORRUPTED");
        assert_ne!(
            std::fs::read(&path).expect("read corrupted vault"),
            original
        );
    }

    #[test]
    fn invalid_provider_is_rejected() {
        let error = configured_provider(Some("banana")).expect_err("invalid provider");
        assert_eq!(error.code, "SECRET_STORE_CONFIGURATION");
    }

    #[test]
    fn provider_values_are_explicit() {
        #[cfg(not(target_os = "macos"))]
        assert_eq!(
            configured_provider(Some("vault")).expect("vault"),
            SecretStoreProvider::Vault
        );
        assert_eq!(
            configured_provider(Some("keychain")).expect("keychain"),
            SecretStoreProvider::Keychain
        );
        #[cfg(target_os = "macos")]
        assert_eq!(
            configured_provider(None).expect("macOS default"),
            SecretStoreProvider::Keychain
        );
        #[cfg(target_os = "macos")]
        assert_eq!(
            configured_provider(Some("vault")).expect("macOS vault override"),
            SecretStoreProvider::Keychain
        );
        #[cfg(not(target_os = "macos"))]
        assert_eq!(
            configured_provider(None).expect("platform default"),
            SecretStoreProvider::Keychain
        );
    }
}
