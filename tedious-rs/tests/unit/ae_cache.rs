use std::sync::{
    Arc, Barrier,
    atomic::{AtomicUsize, Ordering},
};
use std::thread;
use std::time::Duration;

use tedious_rs::always_encrypted::symmetric_key_cache::{
    KeyStoreProvider, cache_key, clear_cache, get_key,
};
use tedious_rs::always_encrypted::types::EncryptionKeyInfo;

struct Provider {
    calls: Arc<AtomicUsize>,
    delay: Duration,
    result: Result<Vec<u8>, String>,
    gate: Option<Arc<Barrier>>,
}

impl KeyStoreProvider for Provider {
    fn decrypt_column_encryption_key(&self, _: &str, _: &str, _: &[u8]) -> Result<Vec<u8>, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        thread::sleep(self.delay);
        if let Some(gate) = &self.gate {
            gate.wait();
        }
        self.result.clone()
    }
}

fn provider(
    calls: &Arc<AtomicUsize>,
    result: Result<Vec<u8>, String>,
    delay: Duration,
) -> Provider {
    Provider {
        calls: Arc::clone(calls),
        delay,
        result,
        gate: None,
    }
}

fn gated_provider(
    calls: &Arc<AtomicUsize>,
    result: Result<Vec<u8>, String>,
    delay: Duration,
    gate: &Arc<Barrier>,
) -> Provider {
    Provider {
        calls: Arc::clone(calls),
        delay,
        result,
        gate: Some(Arc::clone(gate)),
    }
}

fn info(encrypted_key: Vec<u8>) -> EncryptionKeyInfo {
    EncryptionKeyInfo {
        encrypted_key,
        db_id: 1,
        key_id: 2,
        key_version: 1,
        md_version: vec![3],
        key_path: "https://vault.vault.azure.net/keys/k".to_owned(),
        key_store_name: "P".to_owned(),
        algorithm_name: "AEAD_AES_256_CBC_HMAC_SHA256".to_owned(),
    }
}

#[test]
fn cache_key_uses_standard_base64_for_all_remainder_lengths() {
    let _guard = crate::cache_test_lock();
    let expected = [
        (vec![], "server::P"),
        (vec![0], "server:AA==:P"),
        (vec![1], "server:AQ==:P"),
        (vec![1, 2], "server:AQI=:P"),
        (vec![1, 2, 3], "server:AQID:P"),
        (vec![255], "server:/w==:P"),
        (vec![255, 255], "server://8=:P"),
        (vec![1, 2, 255], "server:AQL/:P"),
    ];
    for (bytes, expected) in expected {
        assert_eq!(cache_key("server", &info(bytes)), expected);
    }
}

#[test]
fn concurrent_successful_misses_do_not_coalesce_provider_calls() {
    let _guard = crate::cache_test_lock();
    clear_cache().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let gate = Arc::new(Barrier::new(4));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let calls = Arc::clone(&calls);
        let gate = Arc::clone(&gate);
        handles.push(thread::spawn(move || {
            get_key(
                "coalesced-success",
                &info(vec![1, 2]),
                &gated_provider(&calls, Ok(vec![7; 32]), Duration::from_millis(25), &gate),
                Duration::from_secs(1),
            )
            .unwrap()
        }));
    }
    for handle in handles {
        assert_eq!(handle.join().unwrap().root_key(), &[7; 32]);
    }
    assert!(calls.load(Ordering::SeqCst) > 1);
}

#[test]
fn concurrent_failed_misses_call_provider_for_each_request_and_do_not_cache() {
    let _guard = crate::cache_test_lock();
    clear_cache().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for _ in 0..3 {
        let calls = Arc::clone(&calls);
        handles.push(thread::spawn(move || {
            get_key(
                "coalesced-failure",
                &info(vec![3, 4]),
                &provider(
                    &calls,
                    Err("provider failed".to_owned()),
                    Duration::from_millis(25),
                ),
                Duration::from_secs(1),
            )
        }));
    }
    for handle in handles {
        assert_eq!(handle.join().unwrap().unwrap_err(), "provider failed");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    get_key(
        "coalesced-failure",
        &info(vec![3, 4]),
        &provider(&calls, Ok(vec![8; 32]), Duration::ZERO),
        Duration::from_secs(1),
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 4);
}

#[test]
fn ttl_expiration_reloads_key_and_zero_ttl_does_not_cache() {
    let _guard = crate::cache_test_lock();
    clear_cache().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let value = info(vec![5, 6, 7]);
    get_key(
        "ttl",
        &value,
        &provider(&calls, Ok(vec![1; 32]), Duration::ZERO),
        Duration::from_millis(25),
    )
    .unwrap();
    get_key(
        "ttl",
        &value,
        &provider(&calls, Ok(vec![1; 32]), Duration::ZERO),
        Duration::from_millis(25),
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    thread::sleep(Duration::from_millis(40));
    get_key(
        "ttl",
        &value,
        &provider(&calls, Ok(vec![1; 32]), Duration::ZERO),
        Duration::from_millis(25),
    )
    .unwrap();
    get_key(
        "no-ttl",
        &value,
        &provider(&calls, Ok(vec![1; 32]), Duration::ZERO),
        Duration::ZERO,
    )
    .unwrap();
    get_key(
        "no-ttl",
        &value,
        &provider(&calls, Ok(vec![1; 32]), Duration::ZERO),
        Duration::ZERO,
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 4);
}

#[test]
fn positive_ttl_reuses_entries_with_node_unbounded_capacity() {
    let _guard = crate::cache_test_lock();
    clear_cache().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = provider(&calls, Ok(vec![9; 32]), Duration::ZERO);
    let value = info(vec![0, 1, 2]);
    get_key(
        "unbounded-capacity",
        &value,
        &provider,
        Duration::from_secs(60),
    )
    .unwrap();
    get_key(
        "unbounded-capacity",
        &value,
        &provider,
        Duration::from_secs(60),
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn empty_server_name_is_rejected_without_provider_call() {
    let _guard = crate::cache_test_lock();
    clear_cache().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let result = get_key(
        "",
        &info(vec![1]),
        &provider(&calls, Ok(vec![1; 32]), Duration::ZERO),
        Duration::from_secs(1),
    );
    assert_eq!(
        result.unwrap_err(),
        "Server name should not be null in getKey"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn empty_plaintext_key_uses_node_error_and_is_not_cached() {
    let _guard = crate::cache_test_lock();
    clear_cache().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let value = info(vec![9, 9]);
    let error = get_key(
        "empty-key",
        &value,
        &provider(&calls, Ok(Vec::new()), Duration::ZERO),
        Duration::from_secs(1),
    )
    .unwrap_err();
    assert_eq!(error, "Empty column encryption key specified.");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn cache_clear_removes_cached_entries() {
    let _guard = crate::cache_test_lock();
    clear_cache().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let value = info(vec![8, 9]);
    get_key(
        "clear",
        &value,
        &provider(&calls, Ok(vec![2; 32]), Duration::ZERO),
        Duration::from_secs(60),
    )
    .unwrap();
    clear_cache().unwrap();
    get_key(
        "clear",
        &value,
        &provider(&calls, Ok(vec![2; 32]), Duration::ZERO),
        Duration::from_secs(60),
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
