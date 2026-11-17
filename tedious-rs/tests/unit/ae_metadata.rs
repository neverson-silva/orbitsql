use tedious_rs::always_encrypted::key_crypto::validate_and_get_encryption_algorithm_name;

#[test]
fn only_server_aead_algorithm_id_is_accepted() {
    assert_eq!(
        validate_and_get_encryption_algorithm_name(2, Some("unexpected")),
        Ok("AEAD_AES_256_CBC_HMAC_SHA256")
    );
    assert_eq!(
        validate_and_get_encryption_algorithm_name(1, None),
        Err("Custom cipher algorithm not supported.".to_owned())
    );
}
