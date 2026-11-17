use std::fmt;

use rand::Rng;

use crate::ntlm::{NtlmChallenge, NtlmError, create_ntlm_response};

#[derive(Clone, PartialEq, Eq)]
pub struct NtlmResponseOptions {
    pub domain: String,
    pub user_name: String,
    pub password: String,
    pub challenge: NtlmChallenge,
    pub client_nonce: Option<[u8; 8]>,
    pub unix_millis: Option<i64>,
}

impl fmt::Debug for NtlmResponseOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NtlmResponseOptions")
            .field("domain", &self.domain)
            .field("user_name", &self.user_name)
            .field("password", &"[REDACTED]")
            .field("challenge", &"[REDACTED]")
            .field(
                "client_nonce",
                &self.client_nonce.as_ref().map(|_| "[REDACTED]"),
            )
            .field("unix_millis", &self.unix_millis)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct NtlmResponsePayload {
    pub data: Vec<u8>,
}

impl fmt::Debug for NtlmResponsePayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NtlmResponsePayload")
            .field(
                "data",
                &format_args!("[REDACTED; {} bytes]", self.data.len()),
            )
            .finish()
    }
}

impl NtlmResponsePayload {
    pub fn new(options: NtlmResponseOptions) -> Result<Self, NtlmError> {
        let client_nonce = options.client_nonce.unwrap_or_else(random_nonce);
        let unix_millis = options.unix_millis.unwrap_or_else(current_unix_millis);
        let data = create_ntlm_response(
            &options.domain,
            &options.user_name,
            &options.password,
            &options.challenge.nonce,
            &options.challenge.target,
            &client_nonce,
            unix_millis,
        )?;
        Ok(Self { data })
    }

    pub fn to_string(&self, indent: &str) -> String {
        format!("{indent}NTLM Auth")
    }
}

fn random_nonce() -> [u8; 8] {
    rand::rng().random()
}

fn current_unix_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}
