use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tedious_rs::always_encrypted::cek_entry::CekEntry;
use tedious_rs::always_encrypted::key_crypto::{decrypt_symmetric_key, encrypt_with_key};
use tedious_rs::always_encrypted::symmetric_key_cache::KeyStoreProvider;
use tedious_rs::always_encrypted::types::{CryptoMetadata, SqlServerEncryptionType};

struct Provider {
    calls: Arc<AtomicUsize>,
}

impl KeyStoreProvider for Provider {
    fn decrypt_column_encryption_key(
        &self,
        _key_path: &str,
        _algorithm_name: &str,
        _encrypted_key: &[u8],
    ) -> Result<Vec<u8>, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![0x11; 32])
    }
}

fn metadata() -> CryptoMetadata {
    let mut entry = CekEntry::new(0);
    entry
        .add(
            vec![1],
            1,
            2,
            3,
            vec![4],
            "path".to_owned(),
            "provider".to_owned(),
            "RSA-OAEP".to_owned(),
        )
        .unwrap();
    CryptoMetadata {
        cek_entry: Some(entry),
        cipher_algorithm_id: 2,
        cipher_algorithm_name: None,
        normalization_rule_version: vec![1],
        encryption_key_info: None,
        ordinal: 0,
        encryption_type: SqlServerEncryptionType::Deterministic,
        cipher_algorithm: None,
    }
}

#[test]
fn encrypt_with_key_rejects_missing_trusted_server_name_before_provider_use() {
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = Provider {
        calls: Arc::clone(&calls),
    };
    let result = encrypt_with_key(
        b"value",
        &mut metadata(),
        "",
        &provider,
        Duration::from_secs(1),
    );
    assert_eq!(
        result.unwrap_err(),
        "Server name should not be null in EncryptWithKey"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn decrypt_symmetric_key_rejects_missing_server_name_before_provider_use() {
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = Provider {
        calls: Arc::clone(&calls),
    };
    let result = decrypt_symmetric_key(&mut metadata(), "", &provider, Duration::from_secs(1));
    assert_eq!(
        result.unwrap_err(),
        "Server name should not be null in getKey"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn decrypt_symmetric_key_rejects_missing_encryption_info_with_node_message() {
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = Provider {
        calls: Arc::clone(&calls),
    };
    let mut metadata = metadata();
    metadata.cek_entry = None;
    let result = decrypt_symmetric_key(&mut metadata, "server", &provider, Duration::from_secs(1));
    assert_eq!(
        result.unwrap_err(),
        "md.EncryptionInfo should not be null in DecryptSymmetricKey."
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
