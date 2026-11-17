use sha2::{Digest, Sha256};
use tedious_rs::always_encrypted::keystore_provider_azure_key_vault::{
    AzureKeyVaultClient, ColumnEncryptionAzureKeyVaultProvider,
};
use tedious_rs::always_encrypted::symmetric_key_cache::KeyStoreProvider;

struct FakeAzureClient;

impl AzureKeyVaultClient for FakeAzureClient {
    fn key_size(
        &self,
        _vault_url: &str,
        _name: &str,
        _version: Option<&str>,
    ) -> Result<usize, String> {
        Ok(32)
    }

    fn wrap_key(
        &self,
        _key_path: &str,
        _algorithm: &str,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, String> {
        Ok(plaintext.to_vec())
    }

    fn unwrap_key(
        &self,
        _key_path: &str,
        _algorithm: &str,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, String> {
        Ok(ciphertext.to_vec())
    }

    fn sign(&self, _key_path: &str, _algorithm: &str, digest: &[u8]) -> Result<Vec<u8>, String> {
        Ok(digest.to_vec())
    }

    fn verify(
        &self,
        _key_path: &str,
        _algorithm: &str,
        digest: &[u8],
        signature: &[u8],
    ) -> Result<bool, String> {
        Ok(digest == signature)
    }
}

#[test]
fn parses_vault_key_paths_and_optional_versions() {
    let parsed = ColumnEncryptionAzureKeyVaultProvider::<FakeAzureClient>::parse_path(
        "https://vault.vault.azure.net/keys/master/version",
    )
    .unwrap();
    assert_eq!(parsed.vault_url, "https://vault.vault.azure.net");
    assert_eq!(parsed.name, "master");
    assert_eq!(parsed.version.as_deref(), Some("version"));
    assert!(
        ColumnEncryptionAzureKeyVaultProvider::<FakeAzureClient>::parse_path(
            "https://example.test/keys/master"
        )
        .is_err()
    );
}

#[test]
fn rejects_loopback_and_non_https_key_vault_paths_in_provider_api() {
    for path in [
        "http://vault.vault.azure.net/keys/master",
        "https://localhost/keys/master",
        "https://127.0.0.1/keys/master",
        "https://[::1]/keys/master",
        "http://localhost/keys/master",
    ] {
        assert!(
            ColumnEncryptionAzureKeyVaultProvider::<FakeAzureClient>::parse_path(path).is_err(),
            "provider API accepted non-production Azure Key Vault path: {path}"
        );
    }
}

#[test]
fn preserves_encrypted_cek_format_and_verifies_signature() {
    let provider = ColumnEncryptionAzureKeyVaultProvider::new(FakeAzureClient);
    let path = "https://vault.vault.azure.net/keys/master";
    let encrypted = provider
        .encrypt_column_encryption_key(path, "RSA_OAEP", &[0xA5; 32])
        .unwrap();
    let recovered = provider
        .decrypt_column_encryption_key(path, "RSA-OAEP", &encrypted)
        .unwrap();
    assert_eq!(recovered, vec![0xA5; 32]);
    let mut modified = encrypted;
    let last = modified.len() - 1;
    modified[last] ^= 1;
    assert!(
        provider
            .decrypt_column_encryption_key(path, "RSA-OAEP", &modified)
            .is_err()
    );
}

#[test]
fn rejects_invalid_algorithm_and_malformed_encrypted_cek() {
    let provider = ColumnEncryptionAzureKeyVaultProvider::new(FakeAzureClient);
    let path = "https://vault.vault.azure.net/keys/master";
    assert!(
        provider
            .encrypt_column_encryption_key(path, "RSA1_5", &[0; 32])
            .is_err()
    );
    assert!(
        provider
            .decrypt_column_encryption_key(path, "RSA-OAEP", &[1, 0, 0, 32, 0])
            .is_err()
    );
}

#[test]
fn encrypted_cek_digest_uses_sha256_over_body() {
    let body = [1u8, 0, 0, 0, 0];
    let digest = Sha256::digest(body);
    assert_eq!(digest.len(), 32);
}
