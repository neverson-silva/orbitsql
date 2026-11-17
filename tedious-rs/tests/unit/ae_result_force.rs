use std::sync::{Arc, Mutex};
use std::time::Duration;

use tedious_rs::always_encrypted::result_decryption::build_parameter_encryption_metadata_with_force;
use tedious_rs::always_encrypted::symmetric_key_cache::{KeyStoreProvider, clear_cache, get_key};
use tedious_rs::always_encrypted::types::StatementColumnEncryptionSetting;
use tedious_rs::always_encrypted::utils::{should_decrypt_results, should_encrypt_parameters};
use tedious_rs::always_encrypted::{
    CryptoMetadata, EncryptionKeyInfo, ParameterEncryptionMetadata, SqlServerEncryptionType,
};

fn plaintext_metadata() -> CryptoMetadata {
    CryptoMetadata {
        cek_entry: None,
        cipher_algorithm_id: 0,
        cipher_algorithm_name: None,
        normalization_rule_version: Vec::new(),
        encryption_key_info: None,
        ordinal: 0,
        encryption_type: SqlServerEncryptionType::PlainText,
        cipher_algorithm: None,
    }
}

#[test]
fn keeps_force_encrypt_per_parameter() {
    let _guard = crate::cache_test_lock();
    let metadata = build_parameter_encryption_metadata_with_force(
        vec![plaintext_metadata(), plaintext_metadata()],
        vec![false, true],
    )
    .unwrap();
    assert!(!metadata.force_encrypt(0).unwrap());
    assert!(metadata.force_encrypt(1).unwrap());
    assert_eq!(metadata.iter().count(), 2);
}

#[test]
fn rejects_force_encrypt_settings_with_wrong_cardinality() {
    let _guard = crate::cache_test_lock();
    let result: Result<ParameterEncryptionMetadata, String> =
        build_parameter_encryption_metadata_with_force(vec![plaintext_metadata()], vec![]);
    assert!(result.is_err());
}

#[test]
fn result_set_only_decrypts_results_without_encrypting_parameters() {
    let _guard = crate::cache_test_lock();
    assert!(!should_encrypt_parameters(
        StatementColumnEncryptionSetting::ResultSetOnly,
        true
    ));
    assert!(should_decrypt_results(
        StatementColumnEncryptionSetting::ResultSetOnly,
        false
    ));
}

struct CountingProvider {
    calls: Arc<Mutex<usize>>,
}

impl KeyStoreProvider for CountingProvider {
    fn decrypt_column_encryption_key(
        &self,
        _key_path: &str,
        _algorithm_name: &str,
        _encrypted_key: &[u8],
    ) -> Result<Vec<u8>, String> {
        let mut calls = self.calls.lock().unwrap();
        *calls += 1;
        Ok(vec![0x21; 32])
    }
}

fn cache_key_info(index: usize) -> EncryptionKeyInfo {
    let encoded_index = [
        (index & 0xff) as u8,
        ((index >> 8) & 0xff) as u8,
        ((index >> 16) & 0xff) as u8,
    ];
    EncryptionKeyInfo {
        encrypted_key: encoded_index.to_vec(),
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
fn reuses_result_keys_with_node_unbounded_cache_capacity() {
    let _guard = crate::cache_test_lock();
    clear_cache().unwrap();
    let calls = Arc::new(Mutex::new(0));
    let provider = CountingProvider {
        calls: Arc::clone(&calls),
    };
    let ttl = Duration::from_secs(60);
    for index in 0..1024 {
        get_key("eviction-test", &cache_key_info(index), &provider, ttl).unwrap();
    }
    get_key("eviction-test", &cache_key_info(0), &provider, ttl).unwrap();
    for index in 1024..1025 {
        get_key("eviction-test", &cache_key_info(index), &provider, ttl).unwrap();
    }
    let calls_before_reuse = *calls.lock().unwrap();
    get_key("eviction-test", &cache_key_info(1), &provider, ttl).unwrap();
    let calls_after_reuse = *calls.lock().unwrap();

    assert_eq!(calls_before_reuse, 1025);
    assert_eq!(calls_after_reuse, 1025);
    get_key("eviction-test", &cache_key_info(0), &provider, ttl).unwrap();
    assert_eq!(*calls.lock().unwrap(), 1025);
}
