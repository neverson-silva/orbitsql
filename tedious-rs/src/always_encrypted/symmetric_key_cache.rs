use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::symmetric_key::SymmetricKey;
use super::types::EncryptionKeyInfo;

struct CacheEntry {
    key: SymmetricKey,
    expires_at: Option<Instant>,
    last_used: u64,
}

struct CacheState {
    entries: HashMap<String, CacheEntry>,
    sequence: u64,
    generation: u64,
}

static CACHE: OnceLock<Mutex<CacheState>> = OnceLock::new();
pub const CACHE_CAPACITY: usize = usize::MAX;

pub trait KeyStoreProvider: Send + Sync {
    fn decrypt_column_encryption_key(
        &self,
        key_path: &str,
        algorithm_name: &str,
        encrypted_key: &[u8],
    ) -> Result<Vec<u8>, String>;
}

pub fn cache_key(server_name: &str, key_info: &EncryptionKeyInfo) -> String {
    format!(
        "{server_name}:{}:{}",
        base64(&key_info.encrypted_key),
        key_info.key_store_name
    )
}

pub fn get_key(
    server_name: &str,
    key_info: &EncryptionKeyInfo,
    provider: &dyn KeyStoreProvider,
    ttl: Duration,
) -> Result<SymmetricKey, String> {
    if server_name.is_empty() {
        return Err("Server name should not be null in getKey".to_owned());
    }
    let key = cache_key(server_name, key_info);
    let cache = CACHE.get_or_init(|| {
        Mutex::new(CacheState {
            entries: HashMap::new(),
            sequence: 0,
            generation: 0,
        })
    });
    let cache_generation = {
        let mut state = cache.lock().map_err(|_| "key cache poisoned".to_owned())?;
        let now = Instant::now();
        let cached = state
            .entries
            .get(&key)
            .filter(|entry| entry.expires_at.is_none_or(|expires| expires > now))
            .map(|entry| entry.key.clone());
        if let Some(cached) = cached {
            state.sequence = state.sequence.wrapping_add(1);
            let sequence = state.sequence;
            if let Some(entry) = state.entries.get_mut(&key) {
                entry.last_used = sequence;
            }
            return Ok(cached);
        }
        if state.entries.contains_key(&key) {
            state.entries.remove(&key);
        }
        state.generation
    };
    let plaintext = provider.decrypt_column_encryption_key(
        &key_info.key_path,
        &key_info.algorithm_name,
        &key_info.encrypted_key,
    );
    let result = plaintext.and_then(SymmetricKey::new);
    if let Ok(symmetric) = &result
        && !ttl.is_zero()
    {
        let mut state = cache.lock().map_err(|_| "key cache poisoned".to_owned())?;
        if state.generation != cache_generation {
            return result;
        }
        state.sequence = state.sequence.wrapping_add(1);
        let sequence = state.sequence;
        state.entries.insert(
            key.clone(),
            CacheEntry {
                key: symmetric.clone(),
                expires_at: Some(Instant::now() + ttl),
                last_used: sequence,
            },
        );
    }
    result
}

pub fn clear_cache() -> Result<(), String> {
    let cache = CACHE.get_or_init(|| {
        Mutex::new(CacheState {
            entries: HashMap::new(),
            sequence: 0,
            generation: 0,
        })
    });
    let mut state = cache.lock().map_err(|_| "key cache poisoned".to_owned())?;
    state.entries.clear();
    state.generation = state.generation.wrapping_add(1);
    Ok(())
}

fn base64(value: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(value.len().div_ceil(3) * 4);
    for chunk in value.chunks(3) {
        let first = u32::from(chunk[0]);
        let second = u32::from(*chunk.get(1).unwrap_or(&0));
        let third = u32::from(*chunk.get(2).unwrap_or(&0));
        let packed = (first << 16) | (second << 8) | third;
        output.push(TABLE[((packed >> 18) & 0x3f) as usize] as char);
        output.push(TABLE[((packed >> 12) & 0x3f) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[((packed >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(packed & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    output
}
