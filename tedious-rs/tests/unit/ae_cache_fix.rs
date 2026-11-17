use std::sync::{
    Arc, Barrier,
    atomic::{AtomicUsize, Ordering},
};
use std::thread;
use std::time::Duration;

use tedious_rs::always_encrypted::symmetric_key_cache::{KeyStoreProvider, clear_cache, get_key};
use tedious_rs::always_encrypted::types::EncryptionKeyInfo;

struct Provider {
    calls: Arc<AtomicUsize>,
    gate: Option<Arc<Barrier>>,
}

impl KeyStoreProvider for Provider {
    fn decrypt_column_encryption_key(
        &self,
        _: &str,
        _: &str,
        encrypted_key: &[u8],
    ) -> Result<Vec<u8>, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(gate) = &self.gate {
            gate.wait();
        }
        let mut key = vec![0; 32];
        for (target, source) in key.iter_mut().zip(encrypted_key.iter().copied().cycle()) {
            *target = source;
        }
        Ok(key)
    }
}

fn info(index: usize) -> EncryptionKeyInfo {
    EncryptionKeyInfo {
        encrypted_key: index.to_le_bytes()[..3].to_vec(),
        db_id: 1,
        key_id: index as u32,
        key_version: 1,
        md_version: vec![1],
        key_path: format!("key-{index}"),
        key_store_name: "TEST".to_owned(),
        algorithm_name: "AEAD_AES_256_CBC_HMAC_SHA256".to_owned(),
    }
}

#[test]
fn cache_uses_unbounded_capacity_like_node_lru() {
    let _guard = crate::cache_test_lock();
    clear_cache().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = Provider {
        calls: Arc::clone(&calls),
        gate: None,
    };
    get_key(
        "ae-cache-fix-capacity",
        &info(0),
        &provider,
        Duration::from_secs(60),
    )
    .unwrap();
    get_key(
        "ae-cache-fix-capacity",
        &info(0),
        &provider,
        Duration::from_secs(60),
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn clear_forces_one_new_provider_call_after_cached_access() {
    let _guard = crate::cache_test_lock();
    clear_cache().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let gate = Arc::new(Barrier::new(4));
    let provider = Arc::new(Provider {
        calls: Arc::clone(&calls),
        gate: Some(Arc::clone(&gate)),
    });
    let mut handles = Vec::new();
    for _ in 0..4 {
        let provider = Arc::clone(&provider);
        handles.push(thread::spawn(move || {
            get_key(
                "ae-cache-fix-concurrent",
                &info(7),
                provider.as_ref(),
                Duration::from_secs(60),
            )
        }));
    }
    for handle in handles {
        handle.join().unwrap().unwrap();
    }
    let concurrent_calls = calls.load(Ordering::SeqCst);
    assert!(concurrent_calls > 1);
    clear_cache().unwrap();
    let provider = Provider {
        calls: Arc::clone(&calls),
        gate: None,
    };
    get_key(
        "ae-cache-fix-concurrent",
        &info(7),
        &provider,
        Duration::from_secs(60),
    )
    .unwrap();
    assert!(calls.load(Ordering::SeqCst) > concurrent_calls);
}
