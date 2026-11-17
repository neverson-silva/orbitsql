use std::sync::Arc;

use tedious_rs::azure_auth::AzureVisualStudioCodeTokenProvider;
use tedious_rs::connection::TokenProvider;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

async fn server(body: &'static str) -> (String, Arc<Mutex<Vec<u8>>>) {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let request = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&request);
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buffer = vec![0; 8192];
        let length = socket.read(&mut buffer).await.unwrap();
        buffer.truncate(length);
        captured.lock().await.extend(buffer);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    });
    (format!("http://{address}"), request)
}

#[tokio::test]
async fn reads_a_vscode_access_token_without_falling_back() {
    let provider =
        AzureVisualStudioCodeTokenProvider::new("unused").with_access_token("vscode-token");
    assert_eq!(provider.get_token().await.unwrap(), "vscode-token");
}

#[tokio::test]
async fn reads_msal_cache_access_token_for_the_requested_scope() {
    let root = std::env::temp_dir().join(format!("tedious-rs-vscode-{}", std::process::id()));
    let record = root.with_extension("record.json");
    let cache = root.with_extension("cache.json");
    tokio::fs::write(
        &record,
        r#"{"authority":"https://login.microsoftonline.com/common","homeAccountId":"account","clientId":"client","tenantId":"tenant","username":"user","version":"1.0"}"#,
    )
    .await
    .unwrap();
    tokio::fs::write(
        &cache,
        r#"{"AccessToken":{"entry":{"credential_type":"AccessToken","home_account_id":"account","client_id":"client","target":"https://database.windows.net/user_impersonation","secret":"cached-token"}}}"#,
    )
    .await
    .unwrap();
    let provider = AzureVisualStudioCodeTokenProvider::new(record.to_str().unwrap())
        .with_token_cache_path(cache.to_str().unwrap());
    assert_eq!(
        provider
            .get_token_for_scope("https://database.windows.net/.default")
            .await
            .unwrap(),
        "cached-token"
    );
    let _ = tokio::fs::remove_file(record).await;
    let _ = tokio::fs::remove_file(cache).await;
}

#[tokio::test]
async fn refreshes_from_msal_cache_and_preserves_scope_and_client_id() {
    let root =
        std::env::temp_dir().join(format!("tedious-rs-vscode-refresh-{}", std::process::id()));
    let record = root.with_extension("record.json");
    let cache = root.with_extension("cache.json");
    tokio::fs::write(
        &record,
        r#"{"authority":"https://login.microsoftonline.com/common","homeAccountId":"account","clientId":"client","tenantId":"tenant","username":"user","version":"1.0"}"#,
    )
    .await
    .unwrap();
    tokio::fs::write(
        &cache,
        r#"{"RefreshToken":{"entry":{"credential_type":"RefreshToken","home_account_id":"account","secret":"refresh-token"}}}"#,
    )
    .await
    .unwrap();
    let (authority, request) =
        server(r#"{"access_token":"refreshed-token","expires_in":3600}"#).await;
    let provider = AzureVisualStudioCodeTokenProvider::new(record.to_str().unwrap())
        .with_token_cache_path(cache.to_str().unwrap())
        .with_authority(authority);
    assert_eq!(provider.get_token().await.unwrap(), "refreshed-token");
    let request = String::from_utf8(request.lock().await.clone()).unwrap();
    assert!(request.contains("grant_type=refresh_token"));
    assert!(request.contains("client_id=client"));
    assert!(request.contains("refresh_token=refresh-token"));
    assert!(request.contains("scope=https%3A%2F%2Fdatabase.windows.net%2F.default"));
    let _ = tokio::fs::remove_file(record).await;
    let _ = tokio::fs::remove_file(cache).await;
}

#[tokio::test]
async fn reports_node_vscode_unavailable_error_without_secrets() {
    let provider = AzureVisualStudioCodeTokenProvider::new("missing-record");
    let error = provider.get_token().await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EAUTH"));
    assert!(
        error
            .message
            .contains("Visual Studio Code Authentication is not available")
    );
    assert!(!error.message.contains("missing-record"));
}

#[tokio::test]
async fn rejects_adfs_records_before_network_authentication() {
    let path = std::env::temp_dir().join(format!("tedious-rs-vscode-adfs-{}", std::process::id()));
    tokio::fs::write(
        &path,
        r#"{"authority":"https://login.microsoftonline.com/adfs","homeAccountId":"account","clientId":"client","tenantId":"adfs","username":"user","version":"1.0"}"#,
    )
    .await
    .unwrap();
    let provider = AzureVisualStudioCodeTokenProvider::new(path.to_str().unwrap());
    let error = provider.get_token().await.unwrap_err();
    assert!(
        error
            .message
            .contains("does not support authentication with ADFS tenants")
    );
    let _ = tokio::fs::remove_file(path).await;
}
