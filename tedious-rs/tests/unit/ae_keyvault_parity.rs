use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use tedious_rs::always_encrypted::keystore_provider_azure_key_vault::{
    AzureKeyVaultClient, AzureKeyVaultHttpClient, ColumnEncryptionAzureKeyVaultProvider,
};
use tedious_rs::always_encrypted::symmetric_key_cache::KeyStoreProvider;

#[derive(Clone)]
struct RecordingClient {
    calls: Arc<Mutex<Vec<String>>>,
    verify_result: bool,
}

impl RecordingClient {
    fn new(verify_result: bool) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            verify_result,
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, value: &str) {
        self.calls.lock().unwrap().push(value.to_owned());
    }
}

impl AzureKeyVaultClient for RecordingClient {
    fn key_size(&self, _: &str, _: &str, _: Option<&str>) -> Result<usize, String> {
        self.record("key_size");
        Ok(32)
    }

    fn wrap_key(&self, _: &str, algorithm: &str, value: &[u8]) -> Result<Vec<u8>, String> {
        assert_eq!(algorithm, "RSA-OAEP");
        self.record("wrap_key");
        Ok(vec![value[0]; 32])
    }

    fn unwrap_key(&self, _: &str, algorithm: &str, value: &[u8]) -> Result<Vec<u8>, String> {
        assert_eq!(algorithm, "RSA-OAEP");
        self.record("unwrap_key");
        Ok(value[..0].to_vec())
    }

    fn sign(&self, _: &str, algorithm: &str, _: &[u8]) -> Result<Vec<u8>, String> {
        assert_eq!(algorithm, "RS256");
        self.record("sign");
        Ok(vec![7; 32])
    }

    fn verify(&self, _: &str, algorithm: &str, _: &[u8], _: &[u8]) -> Result<bool, String> {
        assert_eq!(algorithm, "RS256");
        self.record("verify");
        Ok(self.verify_result)
    }
}

#[test]
fn encrypt_and_decrypt_preserve_node_operation_order_and_container_layout() {
    let client = RecordingClient::new(true);
    let calls = client.calls.clone();
    let provider = ColumnEncryptionAzureKeyVaultProvider::new(client);
    let path = "https://vault.vault.azure.net/keys/master/version";
    let encrypted = provider
        .encrypt_column_encryption_key(path, "RSA_OAEP", &[3; 32])
        .unwrap();
    assert_eq!(&encrypted[..5], &[1, path.len() as u8, 0, 32, 0]);
    assert_eq!(
        &encrypted[5..5 + path.len()],
        path.to_ascii_lowercase().as_bytes()
    );
    assert_eq!(encrypted.len(), 5 + path.len() + 64);
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        ["key_size", "wrap_key", "sign", "verify"]
    );
    let recovered = provider
        .decrypt_column_encryption_key(path, "RSA-OAEP", &encrypted)
        .unwrap();
    assert!(recovered.is_empty());
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        [
            "key_size",
            "wrap_key",
            "sign",
            "verify",
            "key_size",
            "verify",
            "unwrap_key"
        ]
    );
}

#[test]
fn invalid_signature_from_sign_is_rejected_before_container_is_returned() {
    let valid = ColumnEncryptionAzureKeyVaultProvider::new(RecordingClient::new(true))
        .encrypt_column_encryption_key(
            "https://vault.vault.azure.net/keys/master",
            "RSA-OAEP",
            &[3; 32],
        )
        .unwrap();
    let client = RecordingClient::new(false);
    let provider = ColumnEncryptionAzureKeyVaultProvider::new(client.clone());
    let result = provider.decrypt_column_encryption_key(
        "https://vault.vault.azure.net/keys/master",
        "RSA-OAEP",
        &valid,
    );
    assert_eq!(result.unwrap_err(), "encrypted CEK signature is invalid");
    assert_eq!(client.calls(), ["key_size", "verify"]);
}

#[test]
fn malformed_headers_are_rejected_without_panicking_or_calling_crypto() {
    let client = RecordingClient::new(true);
    let provider = ColumnEncryptionAzureKeyVaultProvider::new(client.clone());
    for encrypted in [vec![], vec![2], vec![1], vec![1, 0, 0, 32, 0]] {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            provider.decrypt_column_encryption_key(
                "https://vault.vault.azure.net/keys/master",
                "RSA-OAEP",
                &encrypted,
            )
        }));
        assert!(result.is_ok());
        assert!(result.unwrap().is_err());
    }
    assert!(client.calls().iter().all(|call| call == "key_size"));
}

#[test]
fn algorithm_validation_matches_rsa_oaep_alias_and_whitespace_rules() {
    let provider = ColumnEncryptionAzureKeyVaultProvider::new(RecordingClient::new(true));
    let path = "https://vault.vault.azure.net/keys/master";
    assert!(
        provider
            .encrypt_column_encryption_key(path, "RSA_OAEP", &[1; 32])
            .is_ok()
    );
    assert!(
        provider
            .encrypt_column_encryption_key(path, " rsa_oaep ", &[1; 32])
            .is_err()
    );
    assert!(
        provider
            .encrypt_column_encryption_key(path, " RSA-OAEP ", &[1; 32])
            .is_ok()
    );
}

#[test]
fn http_client_sends_keyvault_urls_and_bearer_headers_for_each_operation() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    let worker = thread::spawn(move || {
        for _ in 0..6 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 8192];
            let size = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..size]).into_owned();
            captured.lock().unwrap().push(request.clone());
            let body = if request.starts_with("POST /tenant/") {
                r#"{"access_token":"test-token","expires_in":3600}"#
            } else if request.contains("/wrapkey?api-version=7.4")
                || request.contains("/sign?api-version=7.4")
            {
                r#"{"value":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}"#
            } else {
                r#"{"key":{"kty":"RSA","n":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}}"#
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    let vault = format!("http://{address}");
    let client = AzureKeyVaultHttpClient::new_with_endpoints(
        "client-id",
        "client-secret",
        "tenant",
        vault.clone(),
        false,
    )
    .unwrap();
    let path = format!("{vault}/keys/master/v1");
    assert_eq!(client.key_size(&vault, "master", Some("v1")).unwrap(), 32);
    assert_eq!(
        client.wrap_key(&path, "RSA-OAEP", &[1; 32]).unwrap().len(),
        32
    );
    assert_eq!(client.sign(&path, "RS256", &[1; 32]).unwrap().len(), 32);
    worker.join().unwrap();
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 6);
    assert!(requests[0].starts_with("POST /tenant/oauth2/v2.0/token HTTP/1.1"));
    assert!(requests[0].contains("client_id=client-id"));
    assert!(requests[1].starts_with("GET /keys/master/v1?api-version=7.4 HTTP/1.1"));
    assert!(requests[2].starts_with("POST /tenant/oauth2/v2.0/token HTTP/1.1"));
    assert!(requests[3].starts_with("POST /keys/master/v1/wrapkey?api-version=7.4 HTTP/1.1"));
    assert!(requests[4].starts_with("POST /tenant/oauth2/v2.0/token HTTP/1.1"));
    assert!(requests[5].starts_with("POST /keys/master/v1/sign?api-version=7.4 HTTP/1.1"));
    assert!(
        requests[1]
            .to_ascii_lowercase()
            .contains("authorization: bearer test-token")
    );
    assert!(
        requests[3]
            .to_ascii_lowercase()
            .contains("authorization: bearer test-token")
    );
    assert!(
        requests[5]
            .to_ascii_lowercase()
            .contains("authorization: bearer test-token")
    );
}
