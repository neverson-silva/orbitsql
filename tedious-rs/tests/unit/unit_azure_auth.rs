use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use tedious_rs::azure_auth::{
    AzureCliTokenProvider, AzureClientSecretTokenProvider, AzureCredentialSelection,
    AzureDeveloperCliTokenProvider, AzureManagedIdentityTokenProvider,
    AzurePowerShellTokenProvider, AzureUsernamePasswordTokenProvider,
    AzureWorkloadIdentityTokenProvider, TokenCredential, TokenCredentialAdapter,
    parse_azure_credential_selection,
};
use tedious_rs::connection::TokenFuture;
use tedious_rs::connection::TokenProvider;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

static TEMP_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_temp_path(prefix: &str) -> std::path::PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = TEMP_PATH_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{timestamp}-{counter}",
        std::process::id()
    ))
}

#[cfg(unix)]
async fn make_executable(path: &std::path::Path) {
    let mut permissions = tokio::fs::metadata(path).await.unwrap().permissions();
    permissions.set_mode(0o700);
    tokio::fs::set_permissions(path, permissions).await.unwrap();
}

#[test]
fn azure_default_credential_selection_matches_node_values() {
    assert_eq!(
        parse_azure_credential_selection(None).unwrap(),
        AzureCredentialSelection::All
    );
    assert_eq!(
        parse_azure_credential_selection(Some("prod")).unwrap(),
        AzureCredentialSelection::Production
    );
    assert_eq!(
        parse_azure_credential_selection(Some("DEV")).unwrap(),
        AzureCredentialSelection::Development
    );
    assert_eq!(
        parse_azure_credential_selection(Some("azureclicredential")).unwrap(),
        AzureCredentialSelection::AzureCli
    );
    assert_eq!(
        parse_azure_credential_selection(Some("managedidentitycredential")).unwrap(),
        AzureCredentialSelection::ManagedIdentity
    );
}

#[test]
fn azure_default_credential_selection_rejects_unknown_values() {
    let error = parse_azure_credential_selection(Some("unknowncredential")).unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EAUTH"));
    assert!(!error.message.contains("unknowncredential"));
}

async fn token_server(body: &'static str) -> (String, Arc<Mutex<Vec<u8>>>) {
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
async fn client_secret_provider_posts_oauth_form_and_returns_token() {
    let (authority, request) = token_server(r#"{"access_token":"client-token"}"#).await;
    let provider =
        AzureClientSecretTokenProvider::new("tenant", "client", "secret").with_authority(authority);
    assert_eq!(provider.get_token().await.unwrap(), "client-token");
    let request = String::from_utf8(request.lock().await.clone()).unwrap();
    assert!(request.contains("/tenant/oauth2/v2.0/token"));
    assert!(request.contains("grant_type=client_credentials"));
    assert!(request.contains("client_id=client"));
    assert!(request.contains("scope=https%3A%2F%2Fdatabase.windows.net%2F.default"));
}

#[tokio::test]
async fn username_password_provider_posts_oauth_form_and_returns_token() {
    let (authority, request) = token_server(r#"{"access_token":"password-token"}"#).await;
    let provider = AzureUsernamePasswordTokenProvider::new(
        "tenant",
        "client",
        "user@example.test",
        "password",
    )
    .with_authority(authority);
    assert_eq!(provider.get_token().await.unwrap(), "password-token");
    let request = String::from_utf8(request.lock().await.clone()).unwrap();
    assert!(request.contains("grant_type=password"));
    assert!(request.contains("username=user%40example.test"));
}

#[tokio::test]
async fn provider_rejects_empty_access_token_without_exposing_secret() {
    let (authority, _) = token_server(r#"{"access_token":"   "}"#).await;
    let provider = AzureClientSecretTokenProvider::new("tenant", "client", "secret-value")
        .with_authority(authority);
    let error = provider.get_token().await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EAUTH"));
    assert!(!error.message.contains("secret-value"));
}

#[tokio::test]
async fn managed_identity_provider_uses_metadata_header_and_resource_query() {
    let (authority, request) = token_server(r#"{"access_token":"managed-token"}"#).await;
    let provider =
        AzureManagedIdentityTokenProvider::virtual_machine(Some("client")).with_endpoint(authority);
    assert_eq!(provider.get_token().await.unwrap(), "managed-token");
    let request = String::from_utf8(request.lock().await.clone())
        .unwrap()
        .to_ascii_lowercase();
    assert!(request.contains("metadata: true"));
    assert!(request.contains("api-version=2018-02-01"));
    assert!(request.contains("resource="));
    assert!(request.contains("database.windows.net"));
    assert!(request.contains("client_id=client"));
}

#[tokio::test]
async fn app_service_provider_uses_identity_header() {
    let (authority, request) = token_server(r#"{"access_token":"app-token"}"#).await;
    let provider =
        AzureManagedIdentityTokenProvider::app_service(authority, "header", None::<String>);
    assert_eq!(provider.get_token().await.unwrap(), "app-token");
    let request = String::from_utf8(request.lock().await.clone())
        .unwrap()
        .to_ascii_lowercase();
    assert!(request.contains("x-identity-header: header"));
    assert!(!request.contains("metadata: true"));
}

struct FakeCredential;

impl TokenCredential for FakeCredential {
    fn get_token(&self, scope: &str) -> TokenFuture<'_> {
        let value = scope.to_owned();
        Box::pin(async move { Ok(value) })
    }
}

#[tokio::test]
async fn token_credential_adapter_passes_the_sql_server_scope() {
    let provider = TokenCredentialAdapter::new(FakeCredential);
    assert_eq!(
        provider.get_token().await.unwrap(),
        "https://database.windows.net/.default"
    );
}

#[tokio::test]
async fn token_credential_adapter_passes_the_server_announced_scope() {
    let provider = TokenCredentialAdapter::new(FakeCredential);
    assert_eq!(
        provider
            .get_token_for_scope("https://server.example/.default")
            .await
            .unwrap(),
        "https://server.example/.default"
    );
}

#[tokio::test]
async fn workload_identity_provider_reads_assertion_and_posts_it() {
    let path = unique_temp_path("tedious-rs-workload");
    tokio::fs::write(&path, "signed-assertion").await.unwrap();
    let (authority, request) = token_server(r#"{"access_token":"workload-token"}"#).await;
    let provider =
        AzureWorkloadIdentityTokenProvider::new("tenant", "client", path.to_str().unwrap())
            .with_authority(authority);
    assert_eq!(provider.get_token().await.unwrap(), "workload-token");
    let request = String::from_utf8(request.lock().await.clone()).unwrap();
    assert!(request.contains("client_assertion=signed-assertion"));
    assert!(request.contains("client_assertion_type="));
    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn azure_cli_provider_parses_access_token_from_command_output() {
    let path = unique_temp_path("tedious-rs-az");
    tokio::fs::write(
        &path,
        "#!/bin/sh\nprintf '%s' '{\"accessToken\":\"cli-token\"}'\n",
    )
    .await
    .unwrap();
    make_executable(&path).await;
    let provider = AzureCliTokenProvider::new()
        .with_command(path.to_str().unwrap())
        .with_tenant_id("tenant")
        .with_subscription("subscription")
        .with_timeout(std::time::Duration::from_secs(2));
    assert_eq!(provider.get_token().await.unwrap(), "cli-token");
    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn developer_cli_provider_uses_node_command_shape_and_parses_token() {
    let path = unique_temp_path("tedious-rs-azd");
    tokio::fs::write(
        &path,
        "#!/bin/sh\nprintf '%s' '{\"token\":\"azd-token\",\"expiresOn\":\"2030-01-01T00:00:00Z\"}'\n",
    )
    .await
    .unwrap();
    make_executable(&path).await;
    let provider = AzureDeveloperCliTokenProvider::new()
        .with_command(path.to_str().unwrap())
        .with_tenant_id("tenant")
        .with_timeout(std::time::Duration::from_secs(2));
    assert_eq!(provider.get_token().await.unwrap(), "azd-token");
    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn powershell_provider_parses_token_from_json_output() {
    let path = unique_temp_path("tedious-rs-pwsh");
    tokio::fs::write(
        &path,
        "#!/bin/sh\nprintf '%s' '{\"Token\":\"powershell-token\",\"ExpiresOn\":\"2030-01-01T00:00:00Z\"}'\n",
    )
    .await
    .unwrap();
    make_executable(&path).await;
    let provider = AzurePowerShellTokenProvider::new()
        .with_command(path.to_str().unwrap())
        .with_tenant_id("tenant")
        .with_timeout(std::time::Duration::from_secs(2));
    assert_eq!(provider.get_token().await.unwrap(), "powershell-token");
    let _ = tokio::fs::remove_file(path).await;
}
