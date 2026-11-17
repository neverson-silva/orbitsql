use tedious_rs::always_encrypted::get_parameter_encryption_metadata::{
    CekMetadataRow, ParameterEncryptionMetadataRow, build_parameter_encryption_metadata,
};
use tedious_rs::always_encrypted::types::SqlServerEncryptionType;

fn cek_row(ordinal: usize) -> CekMetadataRow {
    CekMetadataRow {
        key_ordinal: ordinal,
        db_id: 1,
        key_id: 2,
        key_version: 3,
        md_version: vec![4; 16],
        encrypted_key: vec![5, 6],
        provider_name: "TEST".to_owned(),
        key_path: "path".to_owned(),
        algorithm_name: "AEAD_AES_256_CBC_HMAC_SHA256".to_owned(),
    }
}

#[test]
fn groups_ceks_and_associates_metadata_by_parameter_name() {
    let rows = vec![
        ParameterEncryptionMetadataRow {
            parameter_ordinal: 1,
            parameter_name: "@second".to_owned(),
            cipher_algorithm_id: 2,
            encryption_type: 2,
            cek_ordinal: 1,
            normalization_rule_version: vec![1],
        },
        ParameterEncryptionMetadataRow {
            parameter_ordinal: 0,
            parameter_name: "first".to_owned(),
            cipher_algorithm_id: 2,
            encryption_type: 1,
            cek_ordinal: 0,
            normalization_rule_version: vec![1],
        },
    ];
    let metadata = build_parameter_encryption_metadata(
        &[cek_row(0), cek_row(1)],
        &rows,
        &["first".to_owned(), "second".to_owned()],
        &[false, false],
    )
    .unwrap();
    assert_eq!(metadata.len(), 2);
    assert_eq!(
        metadata[0].encryption_type,
        SqlServerEncryptionType::Deterministic
    );
    assert_eq!(
        metadata[1].encryption_type,
        SqlServerEncryptionType::Randomized
    );
}

#[test]
fn fails_closed_for_force_encrypt_plaintext_and_missing_metadata() {
    let plaintext = ParameterEncryptionMetadataRow {
        parameter_ordinal: 0,
        parameter_name: "p".to_owned(),
        cipher_algorithm_id: 0,
        encryption_type: 0,
        cek_ordinal: 0,
        normalization_rule_version: Vec::new(),
    };
    assert!(
        build_parameter_encryption_metadata(
            &[],
            std::slice::from_ref(&plaintext),
            &["p".to_owned()],
            &[true],
        )
        .is_err()
    );
    assert!(build_parameter_encryption_metadata(&[], &[], &["p".to_owned()], &[false]).is_err());
}

#[test]
fn associates_metadata_by_name_when_server_ordinal_differs() {
    let first = ParameterEncryptionMetadataRow {
        parameter_ordinal: 1,
        parameter_name: "first".to_owned(),
        cipher_algorithm_id: 0,
        encryption_type: 0,
        cek_ordinal: 0,
        normalization_rule_version: Vec::new(),
    };
    let second = ParameterEncryptionMetadataRow {
        parameter_ordinal: 0,
        parameter_name: "second".to_owned(),
        cipher_algorithm_id: 0,
        encryption_type: 0,
        cek_ordinal: 0,
        normalization_rule_version: Vec::new(),
    };
    let result = build_parameter_encryption_metadata(
        &[],
        &[first, second],
        &["first".to_owned(), "second".to_owned()],
        &[false, false],
    );
    assert!(result.is_ok());
}
