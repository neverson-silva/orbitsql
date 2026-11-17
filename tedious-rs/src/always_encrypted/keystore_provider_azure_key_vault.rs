use reqwest::Client;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::future::Future;
use std::time::Duration;
use url::Url;

use super::symmetric_key_cache::KeyStoreProvider;

pub const PROVIDER_NAME: &str = "AZURE_KEY_VAULT";
const KEY_VERSION: u8 = 1;
const RSA_OAEP: &str = "RSA-OAEP";
const RS256: &str = "RS256";
const API_VERSION: &str = "7.4";
const MAX_AZURE_RESPONSE_BYTES: u64 = 1_048_576;

pub trait AzureKeyVaultClient: Send + Sync {
    fn allow_non_azure_endpoints(&self) -> bool {
        false
    }

    fn verify_generated_signature(&self) -> bool {
        true
    }

    fn key_size(&self, vault_url: &str, name: &str, version: Option<&str>)
    -> Result<usize, String>;
    fn wrap_key(
        &self,
        key_path: &str,
        algorithm: &str,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, String>;
    fn unwrap_key(
        &self,
        key_path: &str,
        algorithm: &str,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, String>;
    fn sign(&self, key_path: &str, algorithm: &str, digest: &[u8]) -> Result<Vec<u8>, String>;
    fn verify(
        &self,
        key_path: &str,
        algorithm: &str,
        digest: &[u8],
        signature: &[u8],
    ) -> Result<bool, String>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedKeyPath {
    pub vault_url: String,
    pub name: String,
    pub version: Option<String>,
}

pub struct ColumnEncryptionAzureKeyVaultProvider<C> {
    client: C,
}

impl<C> ColumnEncryptionAzureKeyVaultProvider<C>
where
    C: AzureKeyVaultClient,
{
    pub fn new(client: C) -> Self {
        Self { client }
    }

    pub fn parse_path(path: &str) -> Result<ParsedKeyPath, String> {
        Self::parse_path_with_endpoints(path, false)
    }

    fn parse_path_with_endpoints(
        path: &str,
        allow_non_azure_endpoints: bool,
    ) -> Result<ParsedKeyPath, String> {
        if path.trim().is_empty() {
            return Err("Azure Key Vault key path cannot be empty".to_owned());
        }
        let url = Url::parse(path).map_err(|_| "invalid Azure Key Vault key URI".to_owned())?;
        let hostname = url
            .host_str()
            .ok_or_else(|| "Azure Key Vault key URI has no hostname".to_owned())?;
        let loopback = matches!(hostname, "localhost" | "127.0.0.1" | "::1");
        let valid_scheme = url.scheme() == "https"
            || (allow_non_azure_endpoints && url.scheme() == "http" && loopback);
        if !valid_scheme
            || (!hostname.to_ascii_lowercase().ends_with("vault.azure.net")
                && !(allow_non_azure_endpoints && loopback))
        {
            return Err("invalid Azure Key Vault hostname".to_owned());
        }
        if url.query().is_some() || url.fragment().is_some() {
            return Err("Azure Key Vault key URI cannot contain query or fragment".to_owned());
        }
        let segments = url
            .path_segments()
            .ok_or_else(|| "Azure Key Vault key URI has no path".to_owned())?
            .collect::<Vec<_>>();
        if !(segments.len() == 2 || segments.len() == 3)
            || segments[0] != "keys"
            || segments[1].is_empty()
        {
            return Err("Azure Key Vault path must be /keys/name[/version]".to_owned());
        }
        let authority = url
            .host_str()
            .map(|host| match url.port() {
                Some(port) => format!("{host}:{port}"),
                None => host.to_owned(),
            })
            .ok_or_else(|| "Azure Key Vault key URI has no hostname".to_owned())?;
        Ok(ParsedKeyPath {
            vault_url: format!("{}://{authority}", url.scheme()),
            name: segments[1].to_owned(),
            version: segments
                .get(2)
                .filter(|value| !value.is_empty())
                .map(|value| (*value).to_owned()),
        })
    }

    pub fn encrypt_column_encryption_key(
        &self,
        master_key_path: &str,
        encryption_algorithm: &str,
        column_encryption_key: &[u8],
    ) -> Result<Vec<u8>, String> {
        validate_algorithm(encryption_algorithm)?;
        if column_encryption_key.is_empty() {
            return Err("column encryption key cannot be empty".to_owned());
        }
        let parsed = Self::parse_path_with_endpoints(
            master_key_path,
            self.client.allow_non_azure_endpoints(),
        )?;
        let key_size =
            self.client
                .key_size(&parsed.vault_url, &parsed.name, parsed.version.as_deref())?;
        let path_bytes = master_key_path.to_ascii_lowercase().into_bytes();
        if path_bytes.len() > u16::MAX as usize {
            return Err("master key path exceeds u16".to_owned());
        }
        let ciphertext = self
            .client
            .wrap_key(master_key_path, RSA_OAEP, column_encryption_key)?;
        if ciphertext.len() != key_size {
            return Err("wrapped CEK length does not match the Azure RSA key size".to_owned());
        }
        let mut body = vec![KEY_VERSION];
        body.extend((path_bytes.len() as u16).to_le_bytes());
        body.extend((ciphertext.len() as u16).to_le_bytes());
        body.extend(path_bytes);
        body.extend(ciphertext);
        let signature = self
            .client
            .sign(master_key_path, RS256, &Sha256::digest(&body))?;
        if signature.len() != key_size {
            return Err("CEK signature length does not match the Azure RSA key size".to_owned());
        }
        if self.client.verify_generated_signature()
            && !self
                .client
                .verify(master_key_path, RS256, &Sha256::digest(&body), &signature)?
        {
            return Err(
                "Invalid signature of the encrypted column encryption key computed.".to_owned(),
            );
        }
        let mut result = body;
        result.extend(signature);
        Ok(result)
    }
}

impl<C> KeyStoreProvider for ColumnEncryptionAzureKeyVaultProvider<C>
where
    C: AzureKeyVaultClient,
{
    fn decrypt_column_encryption_key(
        &self,
        key_path: &str,
        algorithm_name: &str,
        encrypted_key: &[u8],
    ) -> Result<Vec<u8>, String> {
        validate_algorithm(algorithm_name)?;
        if encrypted_key.is_empty() {
            return Err(
                "Internal error. Empty encrypted column encryption key specified.".to_owned(),
            );
        }
        if encrypted_key[0] != KEY_VERSION {
            return Err(format!(
                "Specified encrypted column encryption key contains an invalid encryption algorithm version {:02x}. Expected version is {:02x}.",
                encrypted_key[0], KEY_VERSION
            ));
        }
        if encrypted_key.len() < 5 {
            return Err("encrypted CEK has an invalid version or length".to_owned());
        }
        let parsed =
            Self::parse_path_with_endpoints(key_path, self.client.allow_non_azure_endpoints())?;
        let key_size =
            self.client
                .key_size(&parsed.vault_url, &parsed.name, parsed.version.as_deref())?;
        let path_length = usize::from(u16::from_le_bytes([encrypted_key[1], encrypted_key[2]]));
        let ciphertext_length =
            usize::from(u16::from_le_bytes([encrypted_key[3], encrypted_key[4]]));
        let body_length = 5usize
            .checked_add(path_length)
            .and_then(|value| value.checked_add(ciphertext_length))
            .ok_or_else(|| "encrypted CEK length overflow".to_owned())?;
        if path_length > encrypted_key.len().saturating_sub(5)
            || encrypted_key.len() < body_length
            || encrypted_key.len() - body_length != key_size
            || ciphertext_length != key_size
        {
            return Err("encrypted CEK RSA lengths are inconsistent".to_owned());
        }
        let body = &encrypted_key[..body_length];
        let signature = &encrypted_key[body_length..];
        if !self
            .client
            .verify(key_path, RS256, &Sha256::digest(body), signature)?
        {
            return Err("encrypted CEK signature is invalid".to_owned());
        }
        self.client.unwrap_key(
            key_path,
            RSA_OAEP,
            &encrypted_key[5 + path_length..body_length],
        )
    }
}

fn validate_algorithm(algorithm: &str) -> Result<(), String> {
    let normalized = if algorithm.eq_ignore_ascii_case("RSA_OAEP") {
        RSA_OAEP
    } else {
        algorithm.trim()
    };
    if normalized.eq_ignore_ascii_case(RSA_OAEP) {
        Ok(())
    } else {
        Err(format!(
            "Invalid key encryption algorithm specified: {algorithm}. Expected value: {RSA_OAEP}."
        ))
    }
}

#[derive(Clone)]
pub struct AzureKeyVaultHttpClient {
    http: Client,
    client_id: String,
    client_secret: String,
    tenant_id: String,
    authority_url: String,
    allow_non_azure_endpoints: bool,
}

impl AzureKeyVaultHttpClient {
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        tenant_id: impl Into<String>,
    ) -> Result<Self, String> {
        Self::new_with_endpoints(
            client_id,
            client_secret,
            tenant_id,
            "https://login.microsoftonline.com",
            true,
        )
    }

    pub fn new_with_endpoints(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        tenant_id: impl Into<String>,
        authority_url: impl Into<String>,
        https_only: bool,
    ) -> Result<Self, String> {
        let client_id = client_id.into();
        let client_secret = client_secret.into();
        let tenant_id = tenant_id.into();
        let authority_url = authority_url.into().trim_end_matches('/').to_owned();
        if client_id.is_empty() || client_secret.is_empty() || tenant_id.is_empty() {
            return Err("Azure credentials cannot be empty".to_owned());
        }
        let authority =
            Url::parse(&authority_url).map_err(|_| "Azure authority URL is invalid".to_owned())?;
        if https_only && authority.scheme() != "https" {
            return Err("Azure authority URL must use HTTPS".to_owned());
        }
        if authority.host_str().is_none()
            || authority.query().is_some()
            || authority.fragment().is_some()
        {
            return Err("Azure authority URL is invalid".to_owned());
        }
        let mut client_builder = Client::builder()
            .https_only(https_only)
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(60));
        if !https_only {
            client_builder = client_builder.no_proxy();
        }
        let http = client_builder
            .build()
            .map_err(|_| "unable to create Azure HTTP client".to_owned())?;
        Ok(Self {
            http,
            client_id,
            client_secret,
            tenant_id,
            authority_url,
            allow_non_azure_endpoints: !https_only,
        })
    }

    fn run<F, T>(future: F) -> Result<T, String>
    where
        F: Future<Output = Result<T, String>> + Send + 'static,
        T: Send + 'static,
    {
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| "unable to create Azure HTTP runtime".to_owned())
                .and_then(|runtime| runtime.block_on(future))
        })
        .join()
        .map_err(|_| "Azure HTTP worker failed".to_owned())?
    }

    fn token(&self) -> Result<String, String> {
        let http = self.http.clone();
        let client_id = self.client_id.clone();
        let client_secret = self.client_secret.clone();
        let tenant_id = self.tenant_id.clone();
        let authority_url = self.authority_url.clone();
        Self::run(async move {
            let response = http
                .post(format!("{authority_url}/{tenant_id}/oauth2/v2.0/token"))
                .header(reqwest::header::ACCEPT, "application/json")
                .form(&[
                    ("client_id", client_id),
                    ("client_secret", client_secret),
                    ("scope", "https://vault.azure.net/.default".to_owned()),
                    ("grant_type", "client_credentials".to_owned()),
                ])
                .send()
                .await
                .map_err(|_| "Azure token request failed".to_owned())?;
            let body = response_json(response, "token").await?;
            body.get("access_token")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| "Azure token response did not contain an access token".to_owned())
        })
    }

    fn key_url(&self, path: &str, operation: Option<&str>) -> Result<String, String> {
        let parsed = ColumnEncryptionAzureKeyVaultProvider::<Self>::parse_path_with_endpoints(
            path,
            self.allow_non_azure_endpoints,
        )?;
        let mut url = Url::parse(&parsed.vault_url)
            .map_err(|_| "invalid Azure Key Vault base URL".to_owned())?;
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| "invalid Azure Key Vault base URL".to_owned())?;
            segments.push("keys").push(&parsed.name);
            if let Some(version) = parsed.version.as_deref() {
                segments.push(version);
            }
            if let Some(operation) = operation {
                segments.push(operation);
            }
        }
        url.query_pairs_mut()
            .append_pair("api-version", API_VERSION);
        Ok(url.into())
    }

    fn operation(&self, path: &str, operation: &str, body: Value) -> Result<Value, String> {
        let url = self.key_url(path, Some(operation))?;
        let token = self.token()?;
        let http = self.http.clone();
        Self::run(async move {
            let response = http
                .post(url)
                .bearer_auth(token)
                .header(reqwest::header::ACCEPT, "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|_| "Azure Key Vault request failed".to_owned())?;
            response_json(response, "Key Vault").await
        })
    }
}

impl AzureKeyVaultClient for AzureKeyVaultHttpClient {
    fn allow_non_azure_endpoints(&self) -> bool {
        self.allow_non_azure_endpoints
    }

    fn verify_generated_signature(&self) -> bool {
        true
    }

    fn key_size(
        &self,
        vault_url: &str,
        name: &str,
        version: Option<&str>,
    ) -> Result<usize, String> {
        let path = format!(
            "{vault_url}/keys/{name}{}",
            version.map(|value| format!("/{value}")).unwrap_or_default()
        );
        let url = self.key_url(&path, None)?;
        let token = self.token()?;
        let http = self.http.clone();
        Self::run(async move {
            let response = http
                .get(url)
                .bearer_auth(token)
                .header(reqwest::header::ACCEPT, "application/json")
                .send()
                .await
                .map_err(|_| "Azure Key Vault getKey request failed".to_owned())?;
            let value = response_json(response, "Key Vault key").await?;
            let key = value
                .get("key")
                .ok_or_else(|| "Azure Key Vault response did not contain a key".to_owned())?;
            if key
                .get("kty")
                .and_then(Value::as_str)
                .is_none_or(|value| !value.eq_ignore_ascii_case("RSA"))
            {
                return Err("Azure Key Vault key is not RSA".to_owned());
            }
            let modulus = decode_base64_url(
                key.get("n")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "Azure Key Vault RSA key has no modulus".to_owned())?,
            )?;
            if modulus.is_empty() {
                return Err("Azure Key Vault RSA key has an empty modulus".to_owned());
            }
            Ok(modulus.len())
        })
    }
    fn wrap_key(
        &self,
        key_path: &str,
        algorithm: &str,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, String> {
        validate_algorithm(algorithm)?;
        self.operation(
            key_path,
            "wrapkey",
            json!({"alg": RSA_OAEP, "value": encode_base64_url(plaintext)}),
        )
        .and_then(|value| {
            decode_base64_url(
                value
                    .get("value")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "Azure wrapKey response did not contain a value".to_owned())?,
            )
        })
    }
    fn unwrap_key(
        &self,
        key_path: &str,
        algorithm: &str,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, String> {
        validate_algorithm(algorithm)?;
        self.operation(
            key_path,
            "unwrapkey",
            json!({"alg": RSA_OAEP, "value": encode_base64_url(ciphertext)}),
        )
        .and_then(|value| {
            decode_base64_url(
                value
                    .get("value")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "Azure unwrapKey response did not contain a value".to_owned())?,
            )
        })
    }
    fn sign(&self, key_path: &str, algorithm: &str, digest: &[u8]) -> Result<Vec<u8>, String> {
        if algorithm != RS256 {
            return Err("Azure Key Vault requires RS256".to_owned());
        }
        self.operation(
            key_path,
            "sign",
            json!({"alg": RS256, "value": encode_base64_url(digest)}),
        )
        .and_then(|value| {
            decode_base64_url(
                value
                    .get("value")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "Azure sign response did not contain a value".to_owned())?,
            )
        })
    }
    fn verify(
        &self,
        key_path: &str,
        algorithm: &str,
        digest: &[u8],
        signature: &[u8],
    ) -> Result<bool, String> {
        if algorithm != RS256 {
            return Err("Azure Key Vault requires RS256".to_owned());
        }
        self.operation(key_path, "verify", json!({"alg": RS256, "digest": encode_base64_url(digest), "value": encode_base64_url(signature)})).and_then(|value| value.get("value").and_then(Value::as_bool).ok_or_else(|| "Azure verify response did not contain a value".to_owned()))
    }
}

fn encode_base64_url(value: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = String::new();
    for chunk in value.chunks(3) {
        let a = chunk[0];
        result.push(TABLE[(a >> 2) as usize] as char);
        let b = chunk.get(1).copied().unwrap_or(0);
        result.push(TABLE[((a & 3) << 4 | b >> 4) as usize] as char);
        if chunk.len() > 1 {
            let c = chunk.get(2).copied().unwrap_or(0);
            result.push(TABLE[((b & 15) << 2 | c >> 6) as usize] as char);
        }
        if chunk.len() > 2 {
            result.push(TABLE[(chunk[2] & 63) as usize] as char);
        }
    }
    result
}

fn decode_base64_url(value: &str) -> Result<Vec<u8>, String> {
    if value.len() % 4 == 1 {
        return Err("Azure Key Vault returned invalid base64url".to_owned());
    }
    let mut result = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u8;
    for byte in value.bytes() {
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return Err("Azure Key Vault returned invalid base64url".to_owned()),
        };
        buffer = (buffer << 6) | u32::from(digit);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            result.push((buffer >> bits) as u8);
        }
    }
    if bits != 0 && (buffer & ((1u32 << bits) - 1)) != 0 {
        return Err("Azure Key Vault returned invalid base64url".to_owned());
    }
    Ok(result)
}

async fn response_json(response: reqwest::Response, operation: &str) -> Result<Value, String> {
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_AZURE_RESPONSE_BYTES)
    {
        return Err(format!("Azure {operation} response was too large"));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| format!("Azure {operation} response could not be read"))?;
    if bytes.len() as u64 > MAX_AZURE_RESPONSE_BYTES {
        return Err(format!("Azure {operation} response was too large"));
    }
    let body = String::from_utf8(bytes.to_vec())
        .map_err(|_| format!("Azure {operation} response was invalid"))?;
    if status.as_u16() != 200 {
        let code = serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|value| {
                value
                    .get("error")
                    .and_then(|error| error.get("code"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .map(|value| sanitize_error_code(&value));
        return match code {
            Some(code) => Err(format!(
                "Azure {operation} request returned HTTP {} ({code})",
                status.as_u16()
            )),
            None => Err(format!(
                "Azure {operation} request returned HTTP {}",
                status.as_u16()
            )),
        };
    }
    serde_json::from_str(&body).map_err(|_| format!("Azure {operation} response was invalid"))
}

fn sanitize_error_code(value: &str) -> String {
    let code: String = value
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
        .take(128)
        .collect();
    if code.is_empty() {
        "unknown".to_owned()
    } else {
        code
    }
}
