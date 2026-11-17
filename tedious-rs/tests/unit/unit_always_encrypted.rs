use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tedious_rs::always_encrypted::cek_entry::CekEntry;
use tedious_rs::always_encrypted::key_crypto::{
    decrypt_symmetric_key, encrypt_encoded_parameter, encrypt_with_key,
    validate_and_get_encryption_algorithm_name,
};
use tedious_rs::always_encrypted::symmetric_key_cache::KeyStoreProvider;
use tedious_rs::always_encrypted::types::{
    CryptoMetadata, EncryptionKeyInfo, SqlServerEncryptionType, StatementColumnEncryptionSetting,
};
use tedious_rs::always_encrypted::utils::should_honor_ae;
use tedious_rs::always_encrypted::{
    AeadAes256CbcHmac256Algorithm, AeadAes256CbcHmac256EncryptionKey, EncryptionType,
};
use tedious_rs::value_codec::{Value, encode_nvarchar};

fn algorithm(encryption_type: EncryptionType) -> AeadAes256CbcHmac256Algorithm {
    let key =
        AeadAes256CbcHmac256EncryptionKey::new(vec![0x11; 32], "AEAD_AES_256_CBC_HMAC_SHA256")
            .unwrap();
    AeadAes256CbcHmac256Algorithm::new(key, encryption_type)
}

#[test]
fn deterministic_encryption_round_trips_and_is_stable() {
    let algorithm = algorithm(EncryptionType::Deterministic);
    let first = algorithm.encrypt_data(b"secret value").unwrap();
    let second = algorithm.encrypt_data(b"secret value").unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        "015f5c06c1010bcb10ffa8f968f56271341386d34fa305788c74ce196168df9fed389908914df008ebafc313373ef1b0df224f6e4f462795c7e2afef1afcc32d1a"
    );
    assert_eq!(algorithm.decrypt_data(&first).unwrap(), b"secret value");
}

#[test]
fn randomized_encryption_changes_iv_and_round_trips() {
    let algorithm = algorithm(EncryptionType::Randomized);
    let first = algorithm.encrypt_data(b"secret value").unwrap();
    let second = algorithm.encrypt_data(b"secret value").unwrap();
    assert_ne!(&first[33..49], &second[33..49]);
    assert_eq!(algorithm.decrypt_data(&first).unwrap(), b"secret value");
    assert_eq!(algorithm.decrypt_data(&second).unwrap(), b"secret value");
}

#[test]
fn rejects_modified_ciphertext_and_invalid_root_key_length() {
    let algorithm = algorithm(EncryptionType::Deterministic);
    let mut ciphertext = algorithm.encrypt_data(b"secret value").unwrap();
    ciphertext[10] ^= 1;
    assert!(algorithm.decrypt_data(&ciphertext).is_err());
    assert!(
        AeadAes256CbcHmac256EncryptionKey::new(vec![0; 31], "AEAD_AES_256_CBC_HMAC_SHA256")
            .is_err()
    );
}

#[test]
fn algorithm_name_is_ignored_when_the_node_algorithm_id_is_supported() {
    assert_eq!(
        validate_and_get_encryption_algorithm_name(2, Some("server-name")),
        Ok("AEAD_AES_256_CBC_HMAC_SHA256")
    );
    assert!(validate_and_get_encryption_algorithm_name(1, None).is_err());
}

struct TestProvider {
    calls: Arc<AtomicUsize>,
}

impl KeyStoreProvider for TestProvider {
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

fn crypto_metadata() -> CryptoMetadata {
    let mut entry = CekEntry::new(0);
    entry
        .add(
            vec![1, 2, 3],
            5,
            7,
            1,
            vec![8; 16],
            "key/path".to_owned(),
            "TEST_PROVIDER".to_owned(),
            "AEAD_AES_256_CBC_HMAC_SHA256".to_owned(),
        )
        .unwrap();
    CryptoMetadata {
        cek_entry: Some(entry),
        cipher_algorithm_id: 2,
        cipher_algorithm_name: Some("AEAD_AES_256_CBC_HMAC_SHA256".to_owned()),
        normalization_rule_version: vec![1],
        encryption_key_info: None,
        ordinal: 0,
        encryption_type: SqlServerEncryptionType::Deterministic,
        cipher_algorithm: None,
    }
}

#[test]
fn reuses_the_cek_for_each_operation_when_node_cache_is_enabled() {
    let _guard = crate::cache_test_lock();
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = TestProvider {
        calls: Arc::clone(&calls),
    };
    let mut first = crypto_metadata();
    decrypt_symmetric_key(&mut first, "server-a", &provider, Duration::from_secs(30)).unwrap();
    let mut second = crypto_metadata();
    let encrypted = encrypt_with_key(
        b"value",
        &mut second,
        "server-a",
        &provider,
        Duration::from_secs(30),
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        first
            .cipher_algorithm
            .as_ref()
            .unwrap()
            .decrypt_data(&encrypted)
            .unwrap(),
        b"value"
    );
}

#[test]
fn rejects_inconsistent_cek_identity_and_applies_statement_policy() {
    let mut entry = CekEntry::new(1);
    let value = EncryptionKeyInfo {
        encrypted_key: vec![1],
        db_id: 1,
        key_id: 2,
        key_version: 3,
        md_version: vec![4],
        key_path: "path".to_owned(),
        key_store_name: "provider".to_owned(),
        algorithm_name: "algorithm".to_owned(),
    };
    entry
        .add(
            value.encrypted_key.clone(),
            value.db_id,
            value.key_id,
            value.key_version,
            value.md_version.clone(),
            value.key_path.clone(),
            value.key_store_name.clone(),
            value.algorithm_name.clone(),
        )
        .unwrap();
    let mut changed = value;
    changed.key_id = 8;
    assert!(
        entry
            .add(
                changed.encrypted_key,
                changed.db_id,
                changed.key_id,
                changed.key_version,
                changed.md_version,
                changed.key_path,
                changed.key_store_name,
                changed.algorithm_name,
            )
            .is_err()
    );
    assert!(!should_honor_ae(
        StatementColumnEncryptionSetting::Disabled,
        true
    ));
    assert!(should_honor_ae(
        StatementColumnEncryptionSetting::Enabled,
        false
    ));
    assert!(!should_honor_ae(
        StatementColumnEncryptionSetting::UseConnectionSetting,
        false
    ));
}

#[test]
fn replaces_plain_parameter_data_with_encrypted_varbinary_max() {
    let _guard = crate::cache_test_lock();
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = TestProvider {
        calls: Arc::clone(&calls),
    };
    let mut metadata = crypto_metadata();
    let parameter = tedious_rs::value_codec::EncodedParameter {
        type_info: vec![0x26, 4],
        length: vec![4],
        data: vec![42, 0, 0, 0],
    };
    let encrypted = encrypt_encoded_parameter(
        &parameter,
        &mut metadata,
        "server-b",
        &provider,
        Duration::from_secs(30),
    )
    .unwrap();
    assert_eq!(encrypted.type_info, vec![0xA5, 0xFF, 0xFF]);
    assert_eq!(encrypted.length.len(), 8);
    assert_eq!(
        u32::from_le_bytes(encrypted.data[..4].try_into().unwrap()) as usize,
        encrypted.data.len() - 8
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn encrypts_logical_plp_chunks_without_encrypting_their_framing() {
    let _guard = crate::cache_test_lock();
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = TestProvider {
        calls: Arc::clone(&calls),
    };
    let mut metadata = crypto_metadata();
    let parameter = encode_nvarchar(&Value::Text("hi".to_owned()), 4001).unwrap();
    let encrypted = encrypt_encoded_parameter(
        &parameter,
        &mut metadata,
        "server-c",
        &provider,
        Duration::from_secs(30),
    )
    .unwrap();
    let length = u32::from_le_bytes(encrypted.data[..4].try_into().unwrap()) as usize;
    let plaintext = metadata
        .cipher_algorithm
        .as_ref()
        .unwrap()
        .decrypt_data(&encrypted.data[4..4 + length])
        .unwrap();
    assert_eq!(
        plaintext,
        "hi".encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    );
}
