use tedious_rs::always_encrypted::cek_entry::CekEntry;
use tedious_rs::always_encrypted::get_parameter_encryption_metadata::{
    CekMetadataRow, ParameterEncryptionMetadataRow, build_parameter_encryption_metadata,
};
use tedious_rs::always_encrypted::symmetric_key::SymmetricKey;
use tedious_rs::always_encrypted::types::EncryptionKeyInfo;

fn cek_row() -> CekMetadataRow {
    CekMetadataRow {
        key_ordinal: 0,
        db_id: 1,
        key_id: 2,
        key_version: 3,
        md_version: vec![4],
        encrypted_key: vec![5],
        provider_name: "provider".to_owned(),
        key_path: "path".to_owned(),
        algorithm_name: "RSA-OAEP".to_owned(),
    }
}

fn encrypted_parameter(normalization_rule_version: Vec<u8>) -> ParameterEncryptionMetadataRow {
    ParameterEncryptionMetadataRow {
        parameter_ordinal: 0,
        parameter_name: "value".to_owned(),
        cipher_algorithm_id: 2,
        encryption_type: 1,
        cek_ordinal: 0,
        normalization_rule_version,
    }
}

fn key_info() -> EncryptionKeyInfo {
    EncryptionKeyInfo {
        encrypted_key: vec![1],
        db_id: 1,
        key_id: 2,
        key_version: 3,
        md_version: vec![4],
        key_path: "path".to_owned(),
        key_store_name: "provider".to_owned(),
        algorithm_name: "RSA-OAEP".to_owned(),
    }
}

#[test]
fn rejects_incomplete_encrypted_metadata_before_provider_use() {
    let result = build_parameter_encryption_metadata(
        &[CekMetadataRow {
            encrypted_key: Vec::new(),
            ..cek_row()
        }],
        &[encrypted_parameter(vec![1])],
        &["value".to_owned()],
        &[false],
    );
    assert_eq!(
        result.err().unwrap(),
        "Empty encrypted column encryption key specified."
    );
}

#[test]
fn rejects_missing_normalization_rule_for_encrypted_parameter() {
    let result = build_parameter_encryption_metadata(
        &[cek_row()],
        &[encrypted_parameter(Vec::new())],
        &["value".to_owned()],
        &[false],
    );
    assert_eq!(
        result.err().unwrap(),
        "normalization rule version for parameter 1 must contain one byte"
    );
}

#[test]
fn rejects_empty_key_provider_fields() {
    let mut entry = CekEntry::new(0);
    let mut invalid = key_info();
    invalid.key_store_name.clear();
    assert_eq!(
        entry
            .add(
                invalid.encrypted_key,
                invalid.db_id,
                invalid.key_id,
                invalid.key_version,
                invalid.md_version,
                invalid.key_path,
                invalid.key_store_name,
                invalid.algorithm_name,
            )
            .unwrap_err(),
        "Column encryption key store provider cannot be empty."
    );
}

#[test]
fn accepts_explicit_key_zeroing_and_empty_keys_are_never_constructed() {
    let mut key = SymmetricKey::new(vec![7; 32]).unwrap();
    key.zero_out_key();
    assert!(key.root_key().iter().all(|value| *value == 0));
    assert!(SymmetricKey::new(Vec::new()).is_err());
}
