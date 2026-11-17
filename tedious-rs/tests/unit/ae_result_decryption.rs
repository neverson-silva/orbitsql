use std::time::Duration;

use tedious_rs::always_encrypted::key_crypto::decrypt_with_key;
use tedious_rs::always_encrypted::result_decryption::{
    associate_base_type_info, associate_result_set_metadata, associate_return_value_metadata,
    decrypt_result_column, decrypt_result_column_with_provider, decrypt_result_value,
};
use tedious_rs::always_encrypted::symmetric_key_cache::KeyStoreProvider;
use tedious_rs::always_encrypted::{
    AeadAes256CbcHmac256Algorithm, AeadAes256CbcHmac256EncryptionKey, CryptoMetadata,
    EncryptionType, SqlServerEncryptionType,
};
use tedious_rs::data_type::type_by_id;
use tedious_rs::metadata_parser::Metadata;
use tedious_rs::value_codec::{Value, encode_nvarchar};

fn metadata() -> CryptoMetadata {
    let key =
        AeadAes256CbcHmac256EncryptionKey::new(vec![0x11; 32], "AEAD_AES_256_CBC_HMAC_SHA256")
            .unwrap();
    CryptoMetadata {
        cek_entry: None,
        cipher_algorithm_id: 2,
        cipher_algorithm_name: None,
        normalization_rule_version: vec![1],
        encryption_key_info: None,
        ordinal: 0,
        encryption_type: SqlServerEncryptionType::Deterministic,
        cipher_algorithm: Some(AeadAes256CbcHmac256Algorithm::new(
            key,
            EncryptionType::Deterministic,
        )),
    }
}

fn base_type_info() -> Metadata {
    Metadata {
        user_type: 0,
        flags: 0,
        data_type: type_by_id(0xE7).unwrap(),
        collation: None,
        precision: None,
        scale: None,
        data_length: Some(40),
        schema: None,
        udt_info: None,
    }
}

fn encrypted_value() -> Vec<u8> {
    let plain = encode_nvarchar(&Value::Text("segredo".to_owned()), 20)
        .unwrap()
        .data;
    let key =
        AeadAes256CbcHmac256EncryptionKey::new(vec![0x11; 32], "AEAD_AES_256_CBC_HMAC_SHA256")
            .unwrap();
    AeadAes256CbcHmac256Algorithm::new(key, EncryptionType::Deterministic)
        .encrypt_data(&plain)
        .unwrap()
}

#[test]
fn decrypts_a_result_using_its_base_type_metadata() {
    let metadata = associate_base_type_info(metadata(), base_type_info());
    assert_eq!(
        decrypt_result_value(&encrypted_value(), &metadata, "server").unwrap(),
        Value::Text("segredo".to_owned())
    );
}

#[test]
fn decrypt_result_value_rejects_missing_server_name_with_node_message() {
    let metadata = associate_base_type_info(metadata(), base_type_info());
    let error = decrypt_result_value(&encrypted_value(), &metadata, "").unwrap_err();
    assert_eq!(error, "Server name should not be null in DecryptWithKey");
}

#[test]
fn decrypt_with_key_rejects_missing_server_name_before_algorithm_lookup() {
    let metadata = metadata();
    let error = decrypt_with_key(b"ciphertext", &metadata, "").unwrap_err();
    assert_eq!(error, "Server name should not be null in DecryptWithKey");
}

#[test]
fn decrypts_binary_result_and_preserves_null_result() {
    let metadata = associate_base_type_info(metadata(), base_type_info());
    assert_eq!(
        decrypt_result_column(&Value::Binary(encrypted_value()), &metadata, "server").unwrap(),
        Value::Text("segredo".to_owned())
    );
    assert_eq!(
        decrypt_result_column(&Value::Null, &metadata, "server").unwrap(),
        Value::Null
    );
}

#[test]
fn rejects_result_plaintext_with_invalid_base_type_payload() {
    let plaintext = vec![1, 2, 3];
    let key =
        AeadAes256CbcHmac256EncryptionKey::new(vec![0x11; 32], "AEAD_AES_256_CBC_HMAC_SHA256")
            .unwrap();
    let encrypted = AeadAes256CbcHmac256Algorithm::new(key, EncryptionType::Deterministic)
        .encrypt_data(&plaintext)
        .unwrap();
    let mut base = base_type_info();
    base.data_type = type_by_id(0x24).unwrap();
    let metadata = associate_base_type_info(metadata(), base);
    assert!(decrypt_result_value(&encrypted, &metadata, "server").is_err());
}

#[test]
fn decodes_decrypted_plp_and_legacy_lob_results_with_their_wire_envelopes() {
    let key =
        AeadAes256CbcHmac256EncryptionKey::new(vec![0x11; 32], "AEAD_AES_256_CBC_HMAC_SHA256")
            .unwrap();
    let algorithm = AeadAes256CbcHmac256Algorithm::new(key, EncryptionType::Deterministic);
    let encrypted = algorithm.encrypt_data(b"abc").unwrap();
    let mut base = base_type_info();
    base.data_type = type_by_id(0xF0).unwrap();
    base.data_length = None;
    let mut crypto = metadata();
    crypto.cipher_algorithm = Some(algorithm);
    let result_metadata = associate_base_type_info(crypto, base);
    assert_eq!(
        decrypt_result_value(&encrypted, &result_metadata, "server").unwrap(),
        Value::Binary(b"abc".to_vec())
    );

    let mut base = base_type_info();
    base.data_type = type_by_id(0x22).unwrap();
    base.data_length = None;
    let mut crypto = metadata();
    crypto.cipher_algorithm = Some(AeadAes256CbcHmac256Algorithm::new(
        AeadAes256CbcHmac256EncryptionKey::new(vec![0x11; 32], "AEAD_AES_256_CBC_HMAC_SHA256")
            .unwrap(),
        EncryptionType::Deterministic,
    ));
    let result_metadata = associate_base_type_info(crypto, base);
    assert_eq!(
        decrypt_result_value(&encrypted, &result_metadata, "server").unwrap(),
        Value::Binary(b"abc".to_vec())
    );
}

#[test]
fn keeps_result_columns_and_return_values_in_separate_metadata_maps() {
    let column_crypto = metadata();
    let column_base = base_type_info();
    let mut result = associate_result_set_metadata(vec![Some((column_crypto, column_base))]);
    let return_crypto = metadata();
    let return_base = base_type_info();
    associate_return_value_metadata(&mut result, 7, "@answer", return_crypto, return_base);

    assert!(result.column(0).is_some());
    assert!(result.return_value_by_ordinal(7).is_some());
    assert!(result.return_value_by_name("@answer").is_some());
    assert!(result.return_value_by_ordinal(0).is_none());
}

#[test]
fn rejects_odd_wide_plaintext_before_building_plp_or_lob_wire() {
    let key =
        AeadAes256CbcHmac256EncryptionKey::new(vec![0x11; 32], "AEAD_AES_256_CBC_HMAC_SHA256")
            .unwrap();
    let algorithm = AeadAes256CbcHmac256Algorithm::new(key, EncryptionType::Deterministic);
    let encrypted = algorithm.encrypt_data(&[0x61]).unwrap();
    let mut crypto = metadata();
    crypto.cipher_algorithm = Some(algorithm);
    let mut base = base_type_info();
    base.data_type = type_by_id(0xF1).unwrap();
    base.data_length = None;
    let result_metadata = associate_base_type_info(crypto, base);
    let error = decrypt_result_value(&encrypted, &result_metadata, "server").unwrap_err();
    assert!(error.contains("odd byte length"));
}

struct Provider;

impl KeyStoreProvider for Provider {
    fn decrypt_column_encryption_key(
        &self,
        _key_path: &str,
        _algorithm_name: &str,
        _encrypted_key: &[u8],
    ) -> Result<Vec<u8>, String> {
        Ok(vec![0x11; 32])
    }
}

#[test]
fn resolves_result_key_through_the_existing_provider() {
    let mut metadata = metadata();
    let mut entry = tedious_rs::always_encrypted::cek_entry::CekEntry::new(0);
    entry
        .add(
            vec![1],
            1,
            1,
            1,
            vec![1],
            "test-key".to_owned(),
            "TEST".to_owned(),
            "AEAD_AES_256_CBC_HMAC_SHA256".to_owned(),
        )
        .unwrap();
    metadata.cek_entry = Some(entry);
    let mut metadata = associate_base_type_info(metadata, base_type_info());
    assert_eq!(
        decrypt_result_column_with_provider(
            &Value::Binary(encrypted_value()),
            &mut metadata,
            "server",
            &Provider,
            Duration::ZERO,
        )
        .unwrap(),
        Value::Text("segredo".to_owned())
    );
}
