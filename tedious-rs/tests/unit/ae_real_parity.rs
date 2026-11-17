use tedious_rs::always_encrypted::get_parameter_encryption_metadata::build_parameter_encryption_metadata;
use tedious_rs::always_encrypted::result_decryption::{
    associate_result_set_metadata, decrypt_result_column, decrypt_result_set_with_provider,
    parse_cek_metadata_row, parse_encrypted_result_column_metadata,
    parse_parameter_encryption_metadata_row, serialize_encrypted_result_column_metadata,
};
use tedious_rs::always_encrypted::types::{EncryptedResultColumnMetadata, SqlServerEncryptionType};
use tedious_rs::value_codec::Value;

const REAL_SCENARIO: &str = include_str!("../fixtures/ae/real_scenario.json");

fn cek_values() -> Vec<Value> {
    vec![
        Value::Integer(0),
        Value::Integer(12),
        Value::Integer(34),
        Value::Integer(56),
        Value::Binary(vec![1; 16]),
        Value::Binary(vec![2, 3, 4]),
        Value::Text("TEST_PROVIDER".to_owned()),
        Value::Text("key/path".to_owned()),
        Value::Text("RSA_OAEP".to_owned()),
    ]
}

fn parameter_values() -> Vec<Value> {
    vec![
        Value::Integer(1),
        Value::Text("@value".to_owned()),
        Value::Integer(2),
        Value::Integer(1),
        Value::Integer(0),
        Value::Integer(1),
    ]
}

struct Provider;

impl tedious_rs::always_encrypted::symmetric_key_cache::KeyStoreProvider for Provider {
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
fn real_scenario_fixture_requires_a_real_provider_and_sql_server_round_trip() {
    let fixture: serde_json::Value = serde_json::from_str(REAL_SCENARIO).unwrap();
    assert_eq!(fixture["key_store_provider"], "AZURE_KEY_VAULT");
    assert_eq!(fixture["key_encryption_algorithm"], "RSA_OAEP");
    assert_eq!(
        fixture["column_encryption_algorithm"],
        "AEAD_AES_256_CBC_HMAC_SHA256"
    );
    assert_eq!(fixture["encryption_type"], "DETERMINISTIC");
    let required_environment = fixture["required_environment"].as_array().unwrap();
    for name in [
        "TEDIOUS_RS_RUN_INTEGRATION",
        "TEDIOUS_RS_RUN_AE_INTEGRATION",
        "AZURE_CLIENT_ID",
        "AZURE_CLIENT_SECRET",
        "AZURE_TENANT_ID",
        "TEDIOUS_RS_AE_KEY_PATH",
    ] {
        assert!(required_environment.iter().any(|value| value == name));
    }
    assert!(
        fixture["assertions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "parameterized insert encrypts a value")
    );
    assert!(
        fixture["assertions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "select decrypts the value")
    );
}

#[test]
fn parses_tedious_parameter_encryption_resultset_rows_by_wire_position() {
    let cek = parse_cek_metadata_row(&cek_values()).unwrap();
    assert_eq!(cek.key_ordinal, 0);
    assert_eq!(cek.db_id, 12);
    assert_eq!(cek.encrypted_key, vec![2, 3, 4]);
    assert_eq!(cek.provider_name, "TEST_PROVIDER");

    let parameter = parse_parameter_encryption_metadata_row(&parameter_values()).unwrap();
    assert_eq!(parameter.parameter_ordinal, 0);
    assert_eq!(parameter.parameter_name, "@value");
    assert_eq!(parameter.cek_ordinal, 0);
    assert_eq!(parameter.encryption_type, 1);

    let metadata =
        build_parameter_encryption_metadata(&[cek], &[parameter], &["value".to_owned()], &[false])
            .unwrap();
    assert_eq!(
        metadata[0].encryption_type,
        SqlServerEncryptionType::Deterministic
    );
    assert_eq!(metadata[0].ordinal, 0);
}

#[test]
fn serializes_and_parses_encrypted_result_column_metadata_exactly() {
    let metadata =
        EncryptedResultColumnMetadata::new(513, 2, SqlServerEncryptionType::Randomized, vec![1])
            .unwrap();
    let wire = serialize_encrypted_result_column_metadata(&metadata).unwrap();
    assert_eq!(wire, vec![1, 2, 2, 2, 1]);
    let (parsed, offset) = parse_encrypted_result_column_metadata(&wire, 0).unwrap();
    assert_eq!(parsed, metadata);
    assert_eq!(offset, wire.len());
}

#[test]
fn rejects_truncated_or_invalid_encrypted_result_column_metadata() {
    assert!(parse_encrypted_result_column_metadata(&[0, 0, 2, 1], 0).is_err());
    assert!(parse_encrypted_result_column_metadata(&[0, 0, 2, 3, 1], 0).is_err());
    assert!(parse_encrypted_result_column_metadata(&[0, 0, 2, 1, 1], 1).is_err());
    assert!(
        EncryptedResultColumnMetadata::new(0, 2, SqlServerEncryptionType::Deterministic, vec![])
            .is_err()
    );
}

#[test]
fn associates_encrypted_and_plain_result_columns_without_crossing_ordinals() {
    let encrypted =
        EncryptedResultColumnMetadata::new(3, 2, SqlServerEncryptionType::Deterministic, vec![1])
            .unwrap();
    let crypto = tedious_rs::always_encrypted::CryptoMetadata {
        cek_entry: None,
        cipher_algorithm_id: encrypted.cipher_algorithm_id,
        cipher_algorithm_name: None,
        normalization_rule_version: encrypted.normalization_rule_version.clone(),
        encryption_key_info: None,
        ordinal: encrypted.ordinal,
        encryption_type: encrypted.encryption_type,
        cipher_algorithm: None,
    };
    let base = tedious_rs::metadata_parser::Metadata {
        user_type: 0,
        flags: 0,
        data_type: tedious_rs::data_type::type_by_id(0x38).unwrap(),
        collation: None,
        precision: None,
        scale: None,
        data_length: Some(8),
        schema: None,
        udt_info: None,
    };
    let result = associate_result_set_metadata(vec![None, Some((crypto, base))]);
    assert!(result.column(0).is_none());
    assert_eq!(result.column(1).unwrap().crypto_metadata.ordinal, 3);
    assert_eq!(
        decrypt_result_column(&Value::Null, result.column(1).unwrap(), "server").unwrap(),
        Value::Null
    );
}

#[test]
fn decrypts_only_encrypted_resultset_slots_and_preserves_plain_slots() {
    let key = tedious_rs::always_encrypted::AeadAes256CbcHmac256EncryptionKey::new(
        vec![0x11; 32],
        "AEAD_AES_256_CBC_HMAC_SHA256",
    )
    .unwrap();
    let algorithm = tedious_rs::always_encrypted::AeadAes256CbcHmac256Algorithm::new(
        key,
        tedious_rs::always_encrypted::EncryptionType::Deterministic,
    );
    let encrypted = algorithm.encrypt_data(&[7, 0, 0, 0]).unwrap();
    let crypto = tedious_rs::always_encrypted::CryptoMetadata {
        cek_entry: None,
        cipher_algorithm_id: 2,
        cipher_algorithm_name: None,
        normalization_rule_version: vec![1],
        encryption_key_info: None,
        ordinal: 0,
        encryption_type: SqlServerEncryptionType::Deterministic,
        cipher_algorithm: Some(algorithm),
    };
    let base = tedious_rs::metadata_parser::Metadata {
        user_type: 0,
        flags: 0,
        data_type: tedious_rs::data_type::type_by_id(0x38).unwrap(),
        collation: None,
        precision: None,
        scale: None,
        data_length: Some(4),
        schema: None,
        udt_info: None,
    };
    let mut result_metadata = associate_result_set_metadata(vec![Some((crypto, base)), None]);
    let mut values = vec![Value::Binary(encrypted), Value::Text("plain".to_owned())];
    decrypt_result_set_with_provider(
        &mut values,
        &mut result_metadata,
        "server",
        &Provider,
        std::time::Duration::ZERO,
    )
    .unwrap();
    assert_eq!(values[0], Value::Integer(7));
    assert_eq!(values[1], Value::Text("plain".to_owned()));
}
