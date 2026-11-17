use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use tedious_rs::always_encrypted::keystore_provider_azure_key_vault::{
    AzureKeyVaultClient, AzureKeyVaultHttpClient, ColumnEncryptionAzureKeyVaultProvider,
};
use tedious_rs::always_encrypted::symmetric_key_cache::KeyStoreProvider;

struct Fake;
impl AzureKeyVaultClient for Fake {
    fn key_size(&self, _: &str, _: &str, _: Option<&str>) -> Result<usize, String> {
        Ok(32)
    }
    fn wrap_key(&self, _: &str, _: &str, value: &[u8]) -> Result<Vec<u8>, String> {
        Ok([value, &[0; 0]].concat())
    }
    fn unwrap_key(&self, _: &str, _: &str, value: &[u8]) -> Result<Vec<u8>, String> {
        Ok(value.to_vec())
    }
    fn sign(&self, _: &str, _: &str, _: &[u8]) -> Result<Vec<u8>, String> {
        Ok(vec![1; 32])
    }
    fn verify(&self, _: &str, _: &str, _: &[u8], _: &[u8]) -> Result<bool, String> {
        Ok(true)
    }
}

#[test]
fn validates_uri_and_round_trips_the_encrypted_cek_container() {
    let path = "https://vault.vault.azure.net/keys/master/v1";
    let parsed = ColumnEncryptionAzureKeyVaultProvider::<Fake>::parse_path(path).unwrap();
    assert_eq!(parsed.name, "master");
    assert_eq!(parsed.version.as_deref(), Some("v1"));
    let provider = ColumnEncryptionAzureKeyVaultProvider::new(Fake);
    let encrypted = provider
        .encrypt_column_encryption_key(path, "RSA_OAEP", &[9; 32])
        .unwrap();
    assert_eq!(
        provider
            .decrypt_column_encryption_key(path, "RSA-OAEP", &encrypted)
            .unwrap(),
        vec![9; 32]
    );
    assert!(
        ColumnEncryptionAzureKeyVaultProvider::<Fake>::parse_path(
            "http://vault.vault.azure.net/keys/master"
        )
        .is_err()
    );
}

#[test]
fn preserves_vault_port_in_parsed_key_path() {
    let parsed = ColumnEncryptionAzureKeyVaultProvider::<Fake>::parse_path(
        "https://vault.vault.azure.net:9443/keys/master/v1",
    )
    .unwrap();
    assert_eq!(parsed.vault_url, "https://vault.vault.azure.net:9443");
}

#[test]
fn exercises_the_http_client_against_a_fake_http_endpoint() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let worker = thread::spawn(move || {
        for _ in 0..10 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut chunk = [0; 1024];
            loop {
                let size = stream.read(&mut chunk).unwrap();
                if size == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..size]);
                let headers_end = request
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                    .map(|position| position + 4);
                let Some(headers_end) = headers_end else {
                    continue;
                };
                let header_text = String::from_utf8_lossy(&request[..headers_end]);
                let content_length = header_text
                    .lines()
                    .find_map(|line| line.strip_prefix("Content-Length: "))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if request.len() >= headers_end + content_length {
                    break;
                }
            }
            let request = String::from_utf8_lossy(&request);
            let body = if request.starts_with("POST /tenant/") {
                r#"{"access_token":"fake-token"}"#
            } else if request.contains("/verify?") {
                r#"{"value":true}"#
            } else if request.contains("/wrapkey?") || request.contains("/sign?") {
                r#"{"value":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}"#
            } else {
                r#"{"key":{"kty":"RSA","n":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}} "#
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    let client = AzureKeyVaultHttpClient::new_with_endpoints(
        "client",
        "secret",
        "tenant",
        format!("http://{address}"),
        false,
    )
    .unwrap();
    let vault = format!("http://{address}");
    let size = client.key_size(&vault, "master", Some("v1")).unwrap();
    let provider = ColumnEncryptionAzureKeyVaultProvider::new(client);
    let path = format!("{vault}/keys/master/v1");
    let encrypted = provider
        .encrypt_column_encryption_key(&path, "RSA-OAEP", &[9; 32])
        .unwrap();
    worker.join().unwrap();
    assert_eq!(size, 32);
    assert_eq!(encrypted.len(), 5 + path.len() + 32 + 32);
}
