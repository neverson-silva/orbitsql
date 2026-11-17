use crate::connection::{TokenFuture, TokenProvider};
use reqwest::Client;
use serde::Deserialize;

const SQL_SERVER_SCOPE: &str = "https://database.windows.net/.default";

#[derive(Clone)]
pub struct AzureClientSecretTokenProvider {
    tenant_id: String,
    client_id: String,
    client_secret: String,
    client: Client,
    authority: String,
}

#[derive(Clone)]
pub struct AzureUsernamePasswordTokenProvider {
    tenant_id: String,
    client_id: String,
    username: String,
    password: String,
    client: Client,
    authority: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManagedIdentityEndpoint {
    VirtualMachine,
    AppService,
}

#[derive(Clone)]
pub struct AzureManagedIdentityTokenProvider {
    client_id: Option<String>,
    endpoint: ManagedIdentityEndpoint,
    client: Client,
    endpoint_url: String,
    identity_header: Option<String>,
}

pub trait TokenCredential: Send + Sync {
    fn get_token(&self, scope: &str) -> TokenFuture<'_>;
}

pub struct TokenCredentialAdapter<C> {
    credential: C,
    scope: String,
}

pub struct AzureDefaultTokenProvider {
    authority: String,
    client_id: Option<String>,
}

pub struct AzureVisualStudioCodeTokenProvider {
    authority: String,
    auth_record_path: String,
    token_cache_path: Option<String>,
    access_token: Option<String>,
    client: Client,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AzureCredentialSelection {
    All,
    Production,
    Development,
    Environment,
    WorkloadIdentity,
    ManagedIdentity,
    VisualStudioCode,
    AzureCli,
    AzurePowerShell,
    AzureDeveloperCli,
}

pub fn parse_azure_credential_selection(
    value: Option<&str>,
) -> Result<AzureCredentialSelection, crate::errors::ConnectionError> {
    let Some(value) = value else {
        return Ok(AzureCredentialSelection::All);
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "" => Ok(AzureCredentialSelection::All),
        "prod" => Ok(AzureCredentialSelection::Production),
        "dev" => Ok(AzureCredentialSelection::Development),
        "environmentcredential" => Ok(AzureCredentialSelection::Environment),
        "workloadidentitycredential" => Ok(AzureCredentialSelection::WorkloadIdentity),
        "managedidentitycredential" => Ok(AzureCredentialSelection::ManagedIdentity),
        "visualstudiocodecredential" => Ok(AzureCredentialSelection::VisualStudioCode),
        "azureclicredential" => Ok(AzureCredentialSelection::AzureCli),
        "azurepowershellcredential" => Ok(AzureCredentialSelection::AzurePowerShell),
        "azuredeveloperclicredential" => Ok(AzureCredentialSelection::AzureDeveloperCli),
        _ => Err(crate::errors::ConnectionError::new(
            "invalid AZURE_TOKEN_CREDENTIALS value",
            Some("EAUTH".to_owned()),
        )),
    }
}

#[derive(Clone)]
pub struct AzureWorkloadIdentityTokenProvider {
    tenant_id: String,
    client_id: String,
    token_file: String,
    client: Client,
    authority: String,
}

#[derive(Clone)]
pub struct AzureCliTokenProvider {
    tenant_id: Option<String>,
    subscription: Option<String>,
    command: String,
    timeout: Option<std::time::Duration>,
}

#[derive(Clone)]
pub struct AzurePowerShellTokenProvider {
    tenant_id: Option<String>,
    command: String,
    timeout: Option<std::time::Duration>,
}

#[derive(Clone)]
pub struct AzureDeveloperCliTokenProvider {
    tenant_id: Option<String>,
    command: String,
    timeout: Option<std::time::Duration>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    #[serde(rename = "expires_in")]
    _expires_in: Option<u64>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct CliTokenResponse {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
}

#[derive(Deserialize)]
struct PowerShellTokenResponse {
    #[serde(rename = "Token")]
    token: Option<String>,
}

#[derive(Deserialize)]
struct DeveloperCliTokenResponse {
    token: Option<String>,
}

impl AzureClientSecretTokenProvider {
    pub fn new(
        tenant_id: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            client: Client::new(),
            authority: "https://login.microsoftonline.com".to_owned(),
        }
    }

    pub fn with_authority(mut self, authority: impl Into<String>) -> Self {
        self.authority = authority.into().trim_end_matches('/').to_owned();
        self
    }

    async fn request_token_with_scope(
        &self,
        scope: &str,
    ) -> Result<String, crate::errors::ConnectionError> {
        request_token(
            &self.client,
            &format!("{}/{}/oauth2/v2.0/token", self.authority, self.tenant_id),
            [
                ("grant_type", "client_credentials"),
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("scope", scope),
            ],
        )
        .await
    }

    async fn request_token(&self) -> Result<String, crate::errors::ConnectionError> {
        self.request_token_with_scope(SQL_SERVER_SCOPE).await
    }
}

impl TokenProvider for AzureClientSecretTokenProvider {
    fn get_token(&self) -> TokenFuture<'_> {
        Box::pin(self.request_token())
    }

    fn get_token_for_scope(&self, scope: &str) -> TokenFuture<'_> {
        let scope = scope.to_owned();
        Box::pin(async move { self.request_token_with_scope(&scope).await })
    }
}

impl AzureUsernamePasswordTokenProvider {
    pub fn new(
        tenant_id: impl Into<String>,
        client_id: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            client_id: client_id.into(),
            username: username.into(),
            password: password.into(),
            client: Client::new(),
            authority: "https://login.microsoftonline.com".to_owned(),
        }
    }

    pub fn with_authority(mut self, authority: impl Into<String>) -> Self {
        self.authority = authority.into().trim_end_matches('/').to_owned();
        self
    }

    async fn request_token_with_scope(
        &self,
        scope: &str,
    ) -> Result<String, crate::errors::ConnectionError> {
        request_token(
            &self.client,
            &format!("{}/{}/oauth2/v2.0/token", self.authority, self.tenant_id),
            [
                ("grant_type", "password"),
                ("client_id", self.client_id.as_str()),
                ("username", self.username.as_str()),
                ("password", self.password.as_str()),
                ("scope", scope),
            ],
        )
        .await
    }

    async fn request_token(&self) -> Result<String, crate::errors::ConnectionError> {
        self.request_token_with_scope(SQL_SERVER_SCOPE).await
    }
}

impl TokenProvider for AzureUsernamePasswordTokenProvider {
    fn get_token(&self) -> TokenFuture<'_> {
        Box::pin(self.request_token())
    }

    fn get_token_for_scope(&self, scope: &str) -> TokenFuture<'_> {
        let scope = scope.to_owned();
        Box::pin(async move { self.request_token_with_scope(&scope).await })
    }
}

impl AzureManagedIdentityTokenProvider {
    pub fn virtual_machine(client_id: Option<impl Into<String>>) -> Self {
        Self {
            client_id: client_id.map(Into::into),
            endpoint: ManagedIdentityEndpoint::VirtualMachine,
            client: Client::new(),
            endpoint_url: "http://169.254.169.254/metadata/identity/oauth2/token".to_owned(),
            identity_header: None,
        }
    }

    pub fn app_service(
        endpoint_url: impl Into<String>,
        identity_header: impl Into<String>,
        client_id: Option<impl Into<String>>,
    ) -> Self {
        Self {
            client_id: client_id.map(Into::into),
            endpoint: ManagedIdentityEndpoint::AppService,
            client: Client::new(),
            endpoint_url: endpoint_url.into(),
            identity_header: Some(identity_header.into()),
        }
    }

    pub fn with_endpoint(mut self, endpoint_url: impl Into<String>) -> Self {
        self.endpoint_url = endpoint_url.into();
        self
    }

    async fn request_token_with_scope(
        &self,
        scope: &str,
    ) -> Result<String, crate::errors::ConnectionError> {
        let resource = scope_resource(scope);
        let mut request = self.client.get(&self.endpoint_url).query(&[
            ("api-version", "2018-02-01"),
            ("resource", resource.as_str()),
        ]);
        if let Some(client_id) = &self.client_id {
            request = request.query(&[("client_id", client_id)]);
        }
        match self.endpoint {
            ManagedIdentityEndpoint::VirtualMachine => {
                request = request.header("Metadata", "true");
            }
            ManagedIdentityEndpoint::AppService => {
                if let Some(header) = &self.identity_header {
                    request = request.header("X-IDENTITY-HEADER", header);
                }
            }
        }
        let response = request.send().await.map_err(|_| {
            crate::errors::ConnectionError::new(
                "Managed identity token request failed",
                Some("EAUTH".to_owned()),
            )
        })?;
        let status = response.status();
        let body = response.json::<TokenResponse>().await.map_err(|_| {
            crate::errors::ConnectionError::new(
                "Managed identity token response was invalid",
                Some("EAUTH".to_owned()),
            )
        })?;
        if !status.is_success() {
            return Err(crate::errors::ConnectionError::new(
                body.error
                    .unwrap_or_else(|| "Managed identity token request was rejected".to_owned()),
                Some("EAUTH".to_owned()),
            ));
        }
        body.access_token
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                crate::errors::ConnectionError::new(
                    "Managed identity response did not contain an access token",
                    Some("EAUTH".to_owned()),
                )
            })
    }

    async fn request_token(&self) -> Result<String, crate::errors::ConnectionError> {
        self.request_token_with_scope(SQL_SERVER_SCOPE).await
    }
}

impl TokenProvider for AzureManagedIdentityTokenProvider {
    fn get_token(&self) -> TokenFuture<'_> {
        Box::pin(self.request_token())
    }

    fn get_token_for_scope(&self, scope: &str) -> TokenFuture<'_> {
        let scope = scope.to_owned();
        Box::pin(async move { self.request_token_with_scope(&scope).await })
    }
}

impl AzureWorkloadIdentityTokenProvider {
    pub fn new(
        tenant_id: impl Into<String>,
        client_id: impl Into<String>,
        token_file: impl Into<String>,
    ) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            client_id: client_id.into(),
            token_file: token_file.into(),
            client: Client::new(),
            authority: "https://login.microsoftonline.com".to_owned(),
        }
    }

    pub fn with_authority(mut self, authority: impl Into<String>) -> Self {
        self.authority = authority.into().trim_end_matches('/').to_owned();
        self
    }

    async fn request_token_with_scope(
        &self,
        scope: &str,
    ) -> Result<String, crate::errors::ConnectionError> {
        let assertion = tokio::fs::read_to_string(&self.token_file)
            .await
            .map_err(|_| {
                crate::errors::ConnectionError::new(
                    "Workload identity token file could not be read",
                    Some("EAUTH".to_owned()),
                )
            })?;
        request_token(
            &self.client,
            &format!("{}/{}/oauth2/v2.0/token", self.authority, self.tenant_id),
            [
                ("grant_type", "client_credentials"),
                ("client_id", self.client_id.as_str()),
                (
                    "client_assertion_type",
                    "urn:ietf:params:oauth:client-assertion-type:jwt-bearer",
                ),
                ("client_assertion", assertion.trim()),
                ("scope", scope),
            ],
        )
        .await
    }

    async fn request_token(&self) -> Result<String, crate::errors::ConnectionError> {
        self.request_token_with_scope(SQL_SERVER_SCOPE).await
    }
}

impl TokenProvider for AzureWorkloadIdentityTokenProvider {
    fn get_token(&self) -> TokenFuture<'_> {
        Box::pin(self.request_token())
    }

    fn get_token_for_scope(&self, scope: &str) -> TokenFuture<'_> {
        let scope = scope.to_owned();
        Box::pin(async move { self.request_token_with_scope(&scope).await })
    }
}

impl AzureCliTokenProvider {
    pub fn new() -> Self {
        Self {
            tenant_id: None,
            subscription: None,
            command: "az".to_owned(),
            timeout: None,
        }
    }

    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    pub fn with_subscription(mut self, subscription: impl Into<String>) -> Self {
        self.subscription = Some(subscription.into());
        self
    }

    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = command.into();
        self
    }

    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    async fn request_token_with_scope(
        &self,
        scope: &str,
    ) -> Result<String, crate::errors::ConnectionError> {
        let resource = scope_resource(scope);
        let mut command = tokio::process::Command::new(&self.command);
        command.args([
            "account",
            "get-access-token",
            "--output",
            "json",
            "--resource",
            &resource,
        ]);
        if let Some(tenant_id) = &self.tenant_id {
            command.args(["--tenant", tenant_id]);
        }
        if let Some(subscription) = &self.subscription {
            command.args(["--subscription", subscription]);
        }
        let output = if let Some(timeout) = self.timeout {
            tokio::time::timeout(timeout, command.output())
                .await
                .map_err(|_| {
                    crate::errors::ConnectionError::new(
                        "Azure CLI token request timed out",
                        Some("EAUTH".to_owned()),
                    )
                })?
        } else {
            command.output().await
        }
        .map_err(|_| {
            crate::errors::ConnectionError::new(
                "Azure CLI token request failed",
                Some("EAUTH".to_owned()),
            )
        })?;
        if !output.status.success() {
            return Err(crate::errors::ConnectionError::new(
                "Azure CLI rejected the token request",
                Some("EAUTH".to_owned()),
            ));
        }
        let response =
            serde_json::from_slice::<CliTokenResponse>(&output.stdout).map_err(|_| {
                crate::errors::ConnectionError::new(
                    "Azure CLI token response was invalid",
                    Some("EAUTH".to_owned()),
                )
            })?;
        response
            .access_token
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                crate::errors::ConnectionError::new(
                    "Azure CLI response did not contain an access token",
                    Some("EAUTH".to_owned()),
                )
            })
    }

    async fn request_token(&self) -> Result<String, crate::errors::ConnectionError> {
        self.request_token_with_scope(SQL_SERVER_SCOPE).await
    }
}

impl Default for AzureCliTokenProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenProvider for AzureCliTokenProvider {
    fn get_token(&self) -> TokenFuture<'_> {
        Box::pin(self.request_token())
    }

    fn get_token_for_scope(&self, scope: &str) -> TokenFuture<'_> {
        let scope = scope.to_owned();
        Box::pin(async move { self.request_token_with_scope(&scope).await })
    }
}

impl AzurePowerShellTokenProvider {
    pub fn new() -> Self {
        Self {
            tenant_id: None,
            command: "pwsh".to_owned(),
            timeout: None,
        }
    }

    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = command.into();
        self
    }

    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    async fn request_token_with_scope(
        &self,
        scope: &str,
    ) -> Result<String, crate::errors::ConnectionError> {
        let resource = scope_resource(scope);
        let tenant = self.tenant_id.as_deref().unwrap_or_default();
        let script = format!(
            "$params = @{{ ResourceUrl = '{resource}' }}; if ('{tenant}') {{ $params['TenantId'] = '{tenant}' }}; $token = Get-AzAccessToken @params; @{{ Token = $token.Token; ExpiresOn = $token.ExpiresOn }} | ConvertTo-Json -Compress"
        );
        let mut command = tokio::process::Command::new(&self.command);
        command.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
        let output = run_command(command, self.timeout, "Azure PowerShell").await?;
        let response =
            serde_json::from_slice::<PowerShellTokenResponse>(&output).map_err(|_| {
                crate::errors::ConnectionError::new(
                    "Azure PowerShell token response was invalid",
                    Some("EAUTH".to_owned()),
                )
            })?;
        response
            .token
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                crate::errors::ConnectionError::new(
                    "Azure PowerShell response did not contain an access token",
                    Some("EAUTH".to_owned()),
                )
            })
    }

    async fn request_token(&self) -> Result<String, crate::errors::ConnectionError> {
        self.request_token_with_scope(SQL_SERVER_SCOPE).await
    }
}

impl Default for AzurePowerShellTokenProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenProvider for AzurePowerShellTokenProvider {
    fn get_token(&self) -> TokenFuture<'_> {
        Box::pin(self.request_token())
    }

    fn get_token_for_scope(&self, scope: &str) -> TokenFuture<'_> {
        let scope = scope.to_owned();
        Box::pin(async move { self.request_token_with_scope(&scope).await })
    }
}

impl AzureDeveloperCliTokenProvider {
    pub fn new() -> Self {
        Self {
            tenant_id: None,
            command: "azd".to_owned(),
            timeout: None,
        }
    }

    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = command.into();
        self
    }

    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    async fn request_token_with_scope(
        &self,
        scope: &str,
    ) -> Result<String, crate::errors::ConnectionError> {
        let mut command = tokio::process::Command::new(&self.command);
        command.args([
            "auth",
            "token",
            "--output",
            "json",
            "--no-prompt",
            "--scope",
            scope,
        ]);
        if let Some(tenant_id) = &self.tenant_id {
            command.args(["--tenant-id", tenant_id]);
        }
        let output = run_command(command, self.timeout, "Azure Developer CLI").await?;
        let response =
            serde_json::from_slice::<DeveloperCliTokenResponse>(&output).map_err(|_| {
                crate::errors::ConnectionError::new(
                    "Azure Developer CLI token response was invalid",
                    Some("EAUTH".to_owned()),
                )
            })?;
        response
            .token
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                crate::errors::ConnectionError::new(
                    "Azure Developer CLI response did not contain an access token",
                    Some("EAUTH".to_owned()),
                )
            })
    }

    async fn request_token(&self) -> Result<String, crate::errors::ConnectionError> {
        self.request_token_with_scope(SQL_SERVER_SCOPE).await
    }
}

impl Default for AzureDeveloperCliTokenProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenProvider for AzureDeveloperCliTokenProvider {
    fn get_token(&self) -> TokenFuture<'_> {
        Box::pin(self.request_token())
    }

    fn get_token_for_scope(&self, scope: &str) -> TokenFuture<'_> {
        let scope = scope.to_owned();
        Box::pin(async move { self.request_token_with_scope(&scope).await })
    }
}

async fn run_command(
    mut command: tokio::process::Command,
    timeout: Option<std::time::Duration>,
    name: &str,
) -> Result<Vec<u8>, crate::errors::ConnectionError> {
    let output = if let Some(timeout) = timeout {
        tokio::time::timeout(timeout, command.output())
            .await
            .map_err(|_| {
                crate::errors::ConnectionError::new(
                    format!("{name} token request timed out"),
                    Some("EAUTH".to_owned()),
                )
            })?
    } else {
        command.output().await
    }
    .map_err(|_| {
        crate::errors::ConnectionError::new(
            format!("{name} token request failed"),
            Some("EAUTH".to_owned()),
        )
    })?;
    if !output.status.success() {
        return Err(crate::errors::ConnectionError::new(
            format!("{name} rejected the token request"),
            Some("EAUTH".to_owned()),
        ));
    }
    Ok(output.stdout)
}

fn scope_resource(scope: &str) -> String {
    let resource = scope.strip_suffix("/.default").unwrap_or(scope);
    format!("{}/", resource.trim_end_matches('/'))
}

impl<C> TokenCredentialAdapter<C> {
    pub fn new(credential: C) -> Self {
        Self {
            credential,
            scope: SQL_SERVER_SCOPE.to_owned(),
        }
    }

    pub fn with_scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = scope.into();
        self
    }
}

impl<C> TokenProvider for TokenCredentialAdapter<C>
where
    C: TokenCredential,
{
    fn get_token(&self) -> TokenFuture<'_> {
        self.credential.get_token(&self.scope)
    }

    fn get_token_for_scope(&self, scope: &str) -> TokenFuture<'_> {
        self.credential.get_token(scope)
    }
}

impl AzureVisualStudioCodeTokenProvider {
    pub fn new(auth_record_path: impl Into<String>) -> Self {
        Self {
            authority: "https://login.microsoftonline.com".to_owned(),
            auth_record_path: auth_record_path.into(),
            token_cache_path: None,
            access_token: None,
            client: Client::new(),
        }
    }

    pub fn from_environment() -> Self {
        let auth_record_path = std::env::var("AZURE_VSCODE_AUTH_RECORD_PATH")
            .or_else(|_| std::env::var("VSCODE_AUTH_RECORD_PATH"))
            .unwrap_or_default();
        let token_cache_path = std::env::var("AZURE_VSCODE_TOKEN_CACHE")
            .or_else(|_| std::env::var("VSCODE_TOKEN_CACHE_PATH"))
            .ok();
        Self {
            token_cache_path,
            access_token: std::env::var("AZURE_VSCODE_ACCESS_TOKEN").ok(),
            ..Self::new(auth_record_path)
        }
    }

    pub fn with_authority(mut self, authority: impl Into<String>) -> Self {
        self.authority = authority.into().trim_end_matches('/').to_owned();
        self
    }

    pub fn with_token_cache_path(mut self, path: impl Into<String>) -> Self {
        self.token_cache_path = Some(path.into());
        self
    }

    pub fn with_access_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = Some(token.into());
        self
    }

    async fn request_token_with_scope(
        &self,
        scope: &str,
    ) -> Result<String, crate::errors::ConnectionError> {
        if let Some(token) = self
            .access_token
            .as_deref()
            .filter(|v| !v.trim().is_empty())
        {
            return Ok(token.to_owned());
        }
        let auth_record_path = if self.auth_record_path.is_empty() {
            return Err(vscode_unavailable());
        } else {
            self.auth_record_path.as_str()
        };
        let record = read_json_file(
            auth_record_path,
            "Cannot load authentication record in Visual Studio Code.",
        )
        .await
        .map_err(|_| vscode_unavailable())?;
        let tenant_id = json_string(&record, "tenantId").unwrap_or_else(|| "common".to_owned());
        if tenant_id.eq_ignore_ascii_case("adfs") {
            return Err(crate::errors::ConnectionError::new(
                "The VisualStudioCodeCredential does not support authentication with ADFS tenants.",
                Some("EAUTH".to_owned()),
            ));
        }
        let client_id = json_string(&record, "clientId")
            .unwrap_or_else(|| "aebc6443-996d-45c2-90f0-388ff96faa56".to_owned());
        if let Some(token_path) = self.token_cache_path.as_deref() {
            let cache = read_json_file(
                token_path,
                "Cannot load authentication record in Visual Studio Code.",
            )
            .await
            .map_err(|_| vscode_unavailable())?;
            if let Some(token) = find_cached_access_token(&cache, &record, scope) {
                return Ok(token);
            }
            if let Some(refresh_token) = find_cached_refresh_token(&cache, &record) {
                return self
                    .refresh_token(&tenant_id, &client_id, &refresh_token, scope)
                    .await;
            }
        }
        Err(vscode_unavailable())
    }

    async fn refresh_token(
        &self,
        tenant_id: &str,
        client_id: &str,
        refresh_token: &str,
        scope: &str,
    ) -> Result<String, crate::errors::ConnectionError> {
        request_token(
            &self.client,
            &format!("{}/{}/oauth2/v2.0/token", self.authority, tenant_id),
            [
                ("grant_type", "refresh_token"),
                ("client_id", client_id),
                ("refresh_token", refresh_token),
                ("scope", scope),
            ],
        )
        .await
    }

    async fn request_token(&self) -> Result<String, crate::errors::ConnectionError> {
        self.request_token_with_scope(SQL_SERVER_SCOPE).await
    }
}

impl TokenProvider for AzureVisualStudioCodeTokenProvider {
    fn get_token(&self) -> TokenFuture<'_> {
        Box::pin(self.request_token())
    }

    fn get_token_for_scope(&self, scope: &str) -> TokenFuture<'_> {
        let scope = scope.to_owned();
        Box::pin(async move { self.request_token_with_scope(&scope).await })
    }
}

async fn read_json_file(
    path: &str,
    message: &str,
) -> Result<serde_json::Value, crate::errors::ConnectionError> {
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|_| crate::errors::ConnectionError::new(message, Some("EAUTH".to_owned())))?;
    serde_json::from_str(&content)
        .map_err(|_| crate::errors::ConnectionError::new(message, Some("EAUTH".to_owned())))
}

fn vscode_unavailable() -> crate::errors::ConnectionError {
    crate::errors::ConnectionError::new(
        "Visual Studio Code Authentication is not available. Ensure you have have Azure Resources Extension installed in VS Code, signed into Azure via VS Code, installed the @azure/identity-vscode package, and properly configured the extension.",
        Some("EAUTH".to_owned()),
    )
}

fn json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

fn find_cached_access_token(
    cache: &serde_json::Value,
    record: &serde_json::Value,
    scope: &str,
) -> Option<String> {
    let client_id = json_string(record, "clientId");
    let home_account_id = json_string(record, "homeAccountId");
    let target = scope.trim_end_matches("/.default");
    find_cache_entry(cache, "AccessToken", |entry| {
        let token = json_string(entry, "secret")
            .or_else(|| json_string(entry, "accessToken"))
            .or_else(|| json_string(entry, "access_token"));
        let entry_target = json_string(entry, "target").unwrap_or_default();
        let account_matches = home_account_id
            .as_deref()
            .map(|id| json_string(entry, "home_account_id").as_deref() == Some(id))
            .unwrap_or(true);
        let client_matches = client_id
            .as_deref()
            .map(|id| json_string(entry, "client_id").as_deref() == Some(id))
            .unwrap_or(true);
        token.filter(|value| {
            !value.trim().is_empty()
                && account_matches
                && client_matches
                && (entry_target.is_empty()
                    || entry_target
                        .split_whitespace()
                        .any(|v| v == scope || v == target || v.starts_with(&format!("{target}/"))))
        })
    })
}

fn find_cached_refresh_token(
    cache: &serde_json::Value,
    record: &serde_json::Value,
) -> Option<String> {
    let home_account_id = json_string(record, "homeAccountId");
    find_cache_entry(cache, "RefreshToken", |entry| {
        let account_matches = home_account_id
            .as_deref()
            .map(|id| json_string(entry, "home_account_id").as_deref() == Some(id))
            .unwrap_or(true);
        json_string(entry, "secret").filter(|value| !value.trim().is_empty() && account_matches)
    })
}

fn find_cache_entry<F>(value: &serde_json::Value, kind: &str, predicate: F) -> Option<String>
where
    F: Fn(&serde_json::Value) -> Option<String> + Copy,
{
    match value {
        serde_json::Value::Object(map) => {
            if map
                .get("credential_type")
                .and_then(serde_json::Value::as_str)
                == Some(kind)
                && let Some(token) = predicate(value)
            {
                return Some(token);
            }
            map.values()
                .find_map(|child| find_cache_entry(child, kind, predicate))
        }
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(|child| find_cache_entry(child, kind, predicate)),
        _ => None,
    }
}

impl AzureDefaultTokenProvider {
    pub fn new() -> Self {
        Self {
            authority: "https://login.microsoftonline.com".to_owned(),
            client_id: None,
        }
    }

    pub fn with_authority(mut self, authority: impl Into<String>) -> Self {
        self.authority = authority.into().trim_end_matches('/').to_owned();
        self
    }

    pub fn with_client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into());
        self
    }

    async fn request_token_with_scope(
        &self,
        scope: &str,
    ) -> Result<String, crate::errors::ConnectionError> {
        let selection = parse_azure_credential_selection(
            std::env::var("AZURE_TOKEN_CREDENTIALS").ok().as_deref(),
        )?;
        let tenant = std::env::var("AZURE_TENANT_ID").ok();
        let client_id = self
            .client_id
            .clone()
            .or_else(|| std::env::var("AZURE_CLIENT_ID").ok());
        let client_secret = std::env::var("AZURE_CLIENT_SECRET").ok();
        let username = std::env::var("AZURE_USERNAME").ok();
        let password = std::env::var("AZURE_PASSWORD").ok();
        let mut unavailable = 0_usize;

        let production = matches!(
            selection,
            AzureCredentialSelection::All | AzureCredentialSelection::Production
        );
        let development = matches!(
            selection,
            AzureCredentialSelection::All | AzureCredentialSelection::Development
        );
        let environment = production || selection == AzureCredentialSelection::Environment;
        let workload = production || selection == AzureCredentialSelection::WorkloadIdentity;
        let managed = production || selection == AzureCredentialSelection::ManagedIdentity;
        let visual_studio_code =
            development || selection == AzureCredentialSelection::VisualStudioCode;
        let cli = development || selection == AzureCredentialSelection::AzureCli;
        let powershell = development || selection == AzureCredentialSelection::AzurePowerShell;
        let developer_cli = development || selection == AzureCredentialSelection::AzureDeveloperCli;

        if environment
            && let (Some(tenant), Some(client_id), Some(client_secret)) =
                (tenant.as_ref(), client_id.as_ref(), client_secret.as_ref())
        {
            match AzureClientSecretTokenProvider::new(tenant, client_id, client_secret)
                .with_authority(self.authority.clone())
                .request_token_with_scope(scope)
                .await
            {
                Ok(token) => return Ok(token),
                Err(_) => unavailable += 1,
            }
        } else if environment
            && let (Some(tenant), Some(client_id), Some(username), Some(password)) = (
                tenant.as_ref(),
                client_id.as_ref(),
                username.as_ref(),
                password.as_ref(),
            )
        {
            match AzureUsernamePasswordTokenProvider::new(tenant, client_id, username, password)
                .with_authority(self.authority.clone())
                .request_token_with_scope(scope)
                .await
            {
                Ok(token) => return Ok(token),
                Err(_) => unavailable += 1,
            }
        }

        if workload
            && let (Some(tenant), Some(client_id), Ok(token_file)) = (
                tenant.as_ref(),
                client_id.as_ref(),
                std::env::var("AZURE_FEDERATED_TOKEN_FILE"),
            )
        {
            match AzureWorkloadIdentityTokenProvider::new(tenant, client_id, token_file)
                .with_authority(self.authority.clone())
                .request_token_with_scope(scope)
                .await
            {
                Ok(token) => return Ok(token),
                Err(_) => unavailable += 1,
            }
        }

        if managed {
            if let Ok(endpoint) = std::env::var("IDENTITY_ENDPOINT") {
                let header = std::env::var("IDENTITY_HEADER").unwrap_or_default();
                match AzureManagedIdentityTokenProvider::app_service(
                    endpoint,
                    header,
                    client_id.clone(),
                )
                .request_token_with_scope(scope)
                .await
                {
                    Ok(token) => return Ok(token),
                    Err(_) => unavailable += 1,
                }
            }

            if let Ok(endpoint) = std::env::var("MSI_ENDPOINT") {
                let header = std::env::var("MSI_SECRET").unwrap_or_default();
                match AzureManagedIdentityTokenProvider::app_service(
                    endpoint,
                    header,
                    client_id.clone(),
                )
                .request_token_with_scope(scope)
                .await
                {
                    Ok(token) => return Ok(token),
                    Err(_) => unavailable += 1,
                }
            }

            match AzureManagedIdentityTokenProvider::virtual_machine(client_id)
                .request_token_with_scope(scope)
                .await
            {
                Ok(token) => return Ok(token),
                Err(_) => unavailable += 1,
            }
        }

        if visual_studio_code {
            match AzureVisualStudioCodeTokenProvider::from_environment()
                .with_authority(self.authority.clone())
                .request_token_with_scope(scope)
                .await
            {
                Ok(token) => return Ok(token),
                Err(_) => unavailable += 1,
            }
        }

        if cli {
            match AzureCliTokenProvider::new()
                .request_token_with_scope(scope)
                .await
            {
                Ok(token) => return Ok(token),
                Err(_) => unavailable += 1,
            }
        }
        if powershell {
            match AzurePowerShellTokenProvider::new()
                .request_token_with_scope(scope)
                .await
            {
                Ok(token) => return Ok(token),
                Err(_) => unavailable += 1,
            }
        }
        if developer_cli {
            match AzureDeveloperCliTokenProvider::new()
                .request_token_with_scope(scope)
                .await
            {
                Ok(token) => return Ok(token),
                Err(_) => unavailable += 1,
            }
        }

        Err(crate::errors::ConnectionError::new(
            format!(
                "Default Azure credential chain unavailable ({unavailable} configured sources failed)"
            ),
            Some("EAUTH".to_owned()),
        ))
    }

    async fn request_token(&self) -> Result<String, crate::errors::ConnectionError> {
        self.request_token_with_scope(SQL_SERVER_SCOPE).await
    }
}

impl Default for AzureDefaultTokenProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenProvider for AzureDefaultTokenProvider {
    fn get_token(&self) -> TokenFuture<'_> {
        Box::pin(self.request_token())
    }

    fn get_token_for_scope(&self, scope: &str) -> TokenFuture<'_> {
        let scope = scope.to_owned();
        Box::pin(async move { self.request_token_with_scope(&scope).await })
    }
}

async fn request_token<const N: usize>(
    client: &Client,
    endpoint: &str,
    form: [(&str, &str); N],
) -> Result<String, crate::errors::ConnectionError> {
    let response = client
        .post(endpoint)
        .form(&form[..])
        .send()
        .await
        .map_err(|_| {
            crate::errors::ConnectionError::new(
                "Azure token request failed",
                Some("EAUTH".to_owned()),
            )
        })?;
    let status = response.status();
    let body = response.json::<TokenResponse>().await.map_err(|_| {
        crate::errors::ConnectionError::new(
            "Azure token response was invalid",
            Some("EAUTH".to_owned()),
        )
    })?;
    if !status.is_success() {
        return Err(crate::errors::ConnectionError::new(
            body.error
                .or(body.error_description)
                .unwrap_or_else(|| "Azure token request was rejected".to_owned()),
            Some("EAUTH".to_owned()),
        ));
    }
    let token = body.access_token.filter(|value| !value.trim().is_empty());
    token.ok_or_else(|| {
        crate::errors::ConnectionError::new(
            "Azure token response did not contain an access token",
            Some("EAUTH".to_owned()),
        )
    })
}
