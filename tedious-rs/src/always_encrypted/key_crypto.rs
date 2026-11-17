use std::time::Duration;

use super::aead_aes_256_cbc_hmac_algorithm::{
    AeadAes256CbcHmac256Algorithm, EncryptionType, algorithm_name,
};
use super::aead_aes_256_cbc_hmac_encryption_key::AeadAes256CbcHmac256EncryptionKey;
use super::symmetric_key_cache::{KeyStoreProvider, get_key};
use super::types::{CryptoMetadata, SqlServerEncryptionType};
use crate::value_codec::{EncodedParameter, Value, encode_varbinary};

pub fn validate_and_get_encryption_algorithm_name(
    cipher_algorithm_id: u8,
    _cipher_algorithm_name: Option<&str>,
) -> Result<&'static str, String> {
    if cipher_algorithm_id != 2 {
        return Err("Custom cipher algorithm not supported.".to_owned());
    }
    Ok(algorithm_name())
}

pub fn decrypt_symmetric_key(
    metadata: &mut CryptoMetadata,
    server_name: &str,
    provider: &dyn KeyStoreProvider,
    cache_ttl: Duration,
) -> Result<(), String> {
    let entry = metadata
        .cek_entry
        .as_ref()
        .ok_or_else(|| "md.EncryptionInfo should not be null in DecryptSymmetricKey.".to_owned())?;
    let mut symmetric_key = None;
    let mut encryption_key_info_chosen = None;
    let mut last_error = None;
    for key_info in &entry.column_encryption_key_values {
        match get_key(server_name, key_info, provider, cache_ttl) {
            Ok(key) => {
                symmetric_key = Some(key);
                encryption_key_info_chosen = Some(key_info.clone());
                break;
            }
            Err(error) => last_error = Some(error),
        }
    }
    let symmetric_key = symmetric_key.ok_or_else(|| {
        last_error.unwrap_or_else(|| {
            "Exception while decryption of encrypted column encryption key.".to_owned()
        })
    })?;
    let algorithm = validate_and_get_encryption_algorithm_name(
        metadata.cipher_algorithm_id,
        metadata.cipher_algorithm_name.as_deref(),
    )?;
    let encryption_type = match metadata.encryption_type {
        SqlServerEncryptionType::Deterministic => EncryptionType::Deterministic,
        _ => EncryptionType::Randomized,
    };
    metadata.cipher_algorithm = Some(AeadAes256CbcHmac256Algorithm::new(
        AeadAes256CbcHmac256EncryptionKey::new(symmetric_key.root_key().to_vec(), algorithm)?,
        encryption_type,
    ));
    metadata.encryption_key_info = encryption_key_info_chosen;
    Ok(())
}

pub fn encrypt_with_key(
    plaintext: &[u8],
    metadata: &mut CryptoMetadata,
    server_name: &str,
    provider: &dyn KeyStoreProvider,
    cache_ttl: Duration,
) -> Result<Vec<u8>, String> {
    if server_name.is_empty() {
        return Err("Server name should not be null in EncryptWithKey".to_owned());
    }
    if metadata.cipher_algorithm.is_none() {
        decrypt_symmetric_key(metadata, server_name, provider, cache_ttl)?;
    }
    metadata
        .cipher_algorithm
        .as_ref()
        .ok_or_else(|| "Cipher Algorithm should not be null in EncryptWithKey".to_owned())?
        .encrypt_data(plaintext)
}

pub fn decrypt_with_key(
    ciphertext: &[u8],
    metadata: &CryptoMetadata,
    server_name: &str,
) -> Result<Vec<u8>, String> {
    if server_name.is_empty() {
        return Err("Server name should not be null in DecryptWithKey".to_owned());
    }
    metadata
        .cipher_algorithm
        .as_ref()
        .ok_or_else(|| "Cipher Algorithm should not be null in DecryptWithKey".to_owned())?
        .decrypt_data(ciphertext)
}

pub fn encrypt_encoded_parameter(
    parameter: &EncodedParameter,
    metadata: &mut CryptoMetadata,
    server_name: &str,
    provider: &dyn KeyStoreProvider,
    cache_ttl: Duration,
) -> Result<EncodedParameter, String> {
    if is_null_parameter(parameter) {
        return encode_varbinary(&Value::Null, 8001).map_err(|error| error.0);
    }
    let plaintext = unwrap_plp_if_present(parameter)?;
    let ciphertext = encrypt_with_key(&plaintext, metadata, server_name, provider, cache_ttl)?;
    encode_varbinary(&Value::Binary(ciphertext), 8001).map_err(|error| error.0)
}

fn is_null_parameter(parameter: &EncodedParameter) -> bool {
    parameter.data.is_empty()
        && (parameter.length.is_empty()
            || parameter.length == [0]
            || parameter.length == [0xFF, 0xFF]
            || parameter.length == [0xFF; 8]
            || parameter.length == [0xFF; 4])
}

fn unwrap_plp_if_present(parameter: &EncodedParameter) -> Result<Vec<u8>, String> {
    if parameter.length != [0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF] {
        return Ok(parameter.data.clone());
    }
    let mut offset = 0;
    let mut plaintext = Vec::new();
    while offset + 4 <= parameter.data.len() {
        let chunk_length = u32::from_le_bytes(
            parameter.data[offset..offset + 4]
                .try_into()
                .map_err(|_| "invalid PLP chunk length".to_owned())?,
        ) as usize;
        offset += 4;
        if chunk_length == 0 {
            if offset != parameter.data.len() {
                return Err("trailing bytes after PLP terminator".to_owned());
            }
            return Ok(plaintext);
        }
        let end = offset
            .checked_add(chunk_length)
            .ok_or_else(|| "PLP chunk length overflow".to_owned())?;
        if end > parameter.data.len() {
            return Err("PLP chunk exceeds parameter data".to_owned());
        }
        plaintext.extend_from_slice(&parameter.data[offset..end]);
        offset = end;
    }
    Err("PLP parameter is missing its terminator".to_owned())
}
