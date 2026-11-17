use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use tedious_rs::always_encrypted::keystore_provider_azure_key_vault::{
    AzureKeyVaultClient, AzureKeyVaultHttpClient,
};

fn response(body: &str, status: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn endpoint(
    requests: usize,
    handler: impl Fn(&str) -> (&'static str, &'static str) + Send + 'static,
) -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let output = Arc::clone(&captured);
    let worker = thread::spawn(move || {
        for _ in 0..requests {
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = vec![0; 16384];
            let length = stream.read(&mut bytes).unwrap();
            bytes.truncate(length);
            let request = String::from_utf8(bytes).unwrap();
            output.lock().unwrap().push(request.clone());
            let (body, status) = handler(&request);
            stream.write_all(response(body, status).as_bytes()).unwrap();
        }
    });
    (format!("http://{address}"), captured, worker)
}

#[test]
fn follows_keyvault_sdk_paths_methods_headers_and_json_contract() {
    let (vault, captured, worker) = endpoint(10, |request| {
        if request.starts_with("POST /tenant/oauth2/v2.0/token") {
            (r#"{"access_token":"access-token"}"#, "200 OK")
        } else if request.starts_with("GET /keys/master/v1?") {
            (
                r#"{"key":{"kty":"RSA","n":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}}"#,
                "200 OK",
            )
        } else if request.contains("/wrapkey?")
            || request.contains("/unwrapkey?")
            || request.contains("/sign?")
        {
            (
                r#"{"value":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}"#,
                "200 OK",
            )
        } else if request.contains("/verify?") {
            (r#"{"value":true}"#, "200 OK")
        } else {
            (r#"{}"#, "404 Not Found")
        }
    });
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
        client.wrap_key(&path, "RSA_OAEP", &[1; 32]).unwrap().len(),
        32
    );
    assert_eq!(
        client
            .unwrap_key(&path, "RSA-OAEP", &[2; 32])
            .unwrap()
            .len(),
        32
    );
    assert_eq!(client.sign(&path, "RS256", &[3; 32]).unwrap().len(), 32);
    assert!(client.verify(&path, "RS256", &[4; 32], &[5; 32]).unwrap());
    worker.join().unwrap();

    let requests = captured.lock().unwrap();
    assert_eq!(requests.len(), 10);
    assert!(requests[0].starts_with("POST /tenant/oauth2/v2.0/token HTTP/1.1"));
    assert!(requests[1].starts_with("GET /keys/master/v1?api-version=7.4 HTTP/1.1"));
    assert!(requests[3].starts_with("POST /keys/master/v1/wrapkey?api-version=7.4 HTTP/1.1"));
    assert!(requests[5].starts_with("POST /keys/master/v1/unwrapkey?api-version=7.4 HTTP/1.1"));
    assert!(requests[7].starts_with("POST /keys/master/v1/sign?api-version=7.4 HTTP/1.1"));
    assert!(requests[9].starts_with("POST /keys/master/v1/verify?api-version=7.4 HTTP/1.1"));
    assert!(
        requests[1]
            .to_ascii_lowercase()
            .contains("authorization: bearer access-token")
    );
    assert!(
        requests[3]
            .to_ascii_lowercase()
            .contains("accept: application/json")
    );
    assert!(requests[3].contains(r#""alg":"RSA-OAEP""#));
    assert!(requests[7].contains(r#""alg":"RS256""#));
    assert!(requests[9].contains(r#""digest":""#));
}

#[test]
fn preserves_status_and_safe_error_code_without_returning_error_body() {
    let secret = "sensitive-response-value";
    let (authority, _, worker) = endpoint(1, move |_| {
        (
            Box::leak(
                format!(r#"{{"error":{{"code":"Forbidden","message":"{secret}"}}}}"#)
                    .into_boxed_str(),
            ),
            "403 Forbidden",
        )
    });
    let client = AzureKeyVaultHttpClient::new_with_endpoints(
        "client", "secret", "tenant", &authority, false,
    )
    .unwrap();
    let error = client.key_size(&authority, "master", None).unwrap_err();
    worker.join().unwrap();
    assert!(error.contains("HTTP 403"));
    assert!(error.contains("Forbidden"));
    assert!(!error.contains(secret));
}

#[test]
fn rejects_invalid_success_payloads_and_invalid_authority_urls() {
    let (authority, _, worker) = endpoint(2, |request| {
        if request.starts_with("POST /tenant/oauth2/v2.0/token") {
            (r#"{"access_token":"token"}"#, "200 OK")
        } else {
            (r#"{"key":{"kty":"RSA","n":"A"}}"#, "200 OK")
        }
    });
    let client = AzureKeyVaultHttpClient::new_with_endpoints(
        "client", "secret", "tenant", &authority, false,
    )
    .unwrap();
    assert_eq!(
        client.key_size(&authority, "master", None).unwrap_err(),
        "Azure Key Vault returned invalid base64url"
    );
    worker.join().unwrap();
    assert!(
        AzureKeyVaultHttpClient::new_with_endpoints(
            "client",
            "secret",
            "tenant",
            "https://login.example.test/path?secret=value",
            true,
        )
        .is_err()
    );
}
