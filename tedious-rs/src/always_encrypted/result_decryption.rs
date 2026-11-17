use std::time::Duration;

use super::key_crypto::{decrypt_symmetric_key, decrypt_with_key};
use super::symmetric_key_cache::KeyStoreProvider;
use super::types::{
    CryptoMetadata, CryptoMetadataWithBaseTypeInfo, DescribeParameterEncryptionResultSet1,
    DescribeParameterEncryptionResultSet2, EncryptedResultColumnMetadata,
    ParameterEncryptionMetadata, ResultSetCryptoMetadata, SqlServerEncryptionType,
};
use crate::always_encrypted::get_parameter_encryption_metadata::{
    CekMetadataRow, ParameterEncryptionMetadataRow,
};
use crate::value_codec::Value;
use crate::value_parser::read_value;

pub fn parse_encrypted_result_column_metadata(
    buf: &[u8],
    offset: usize,
) -> Result<(EncryptedResultColumnMetadata, usize), String> {
    let end = offset
        .checked_add(5)
        .ok_or_else(|| "encrypted result metadata offset overflow".to_owned())?;
    if end > buf.len() {
        return Err("encrypted result metadata is truncated".to_owned());
    }
    let ordinal = u16::from_le_bytes([buf[offset], buf[offset + 1]]) as usize;
    let encryption_type = encryption_type_from_byte(buf[offset + 3])?;
    let metadata = EncryptedResultColumnMetadata::new(
        ordinal,
        buf[offset + 2],
        encryption_type,
        vec![buf[offset + 4]],
    )?;
    Ok((metadata, end))
}

pub fn serialize_encrypted_result_column_metadata(
    metadata: &EncryptedResultColumnMetadata,
) -> Result<Vec<u8>, String> {
    let ordinal = u16::try_from(metadata.ordinal)
        .map_err(|_| "encrypted result metadata ordinal exceeds u16".to_owned())?;
    if metadata.normalization_rule_version.len() != 1 {
        return Err("normalization rule version must contain one byte".to_owned());
    }
    let mut result = Vec::with_capacity(5);
    result.extend(ordinal.to_le_bytes());
    result.push(metadata.cipher_algorithm_id);
    result.push(encryption_type_to_byte(metadata.encryption_type));
    result.push(metadata.normalization_rule_version[0]);
    Ok(result)
}

pub fn parse_cek_metadata_row(values: &[Value]) -> Result<CekMetadataRow, String> {
    if values.len() < 9 {
        return Err("CEK metadata row has fewer than nine columns".to_owned());
    }
    Ok(CekMetadataRow {
        key_ordinal: value_usize(
            &values[DescribeParameterEncryptionResultSet1::KeyOrdinal as usize],
        )?,
        db_id: value_u32(&values[DescribeParameterEncryptionResultSet1::DbId as usize])?,
        key_id: value_u32(&values[DescribeParameterEncryptionResultSet1::KeyId as usize])?,
        key_version: value_u32(
            &values[DescribeParameterEncryptionResultSet1::KeyVersion as usize],
        )?,
        md_version: value_binary(
            &values[DescribeParameterEncryptionResultSet1::KeyMdVersion as usize],
        )?,
        encrypted_key: value_binary(
            &values[DescribeParameterEncryptionResultSet1::EncryptedKey as usize],
        )?,
        provider_name: value_text(
            &values[DescribeParameterEncryptionResultSet1::ProviderName as usize],
        )?,
        key_path: value_text(&values[DescribeParameterEncryptionResultSet1::KeyPath as usize])?,
        algorithm_name: value_text(
            &values[DescribeParameterEncryptionResultSet1::KeyEncryptionAlgorithm as usize],
        )?,
    })
}

pub fn parse_parameter_encryption_metadata_row(
    values: &[Value],
) -> Result<ParameterEncryptionMetadataRow, String> {
    if values.len() < 6 {
        return Err("parameter encryption metadata row has fewer than six columns".to_owned());
    }
    let parameter_ordinal =
        value_usize(&values[DescribeParameterEncryptionResultSet2::ParameterOrdinal as usize])?
            .checked_sub(1)
            .ok_or_else(|| "metadata parameter ordinal must be one-based".to_owned())?;
    Ok(ParameterEncryptionMetadataRow {
        parameter_ordinal,
        parameter_name: value_text(
            &values[DescribeParameterEncryptionResultSet2::ParameterName as usize],
        )?,
        cipher_algorithm_id: value_u8(
            &values[DescribeParameterEncryptionResultSet2::ColumnEncryptionAlgorithm as usize],
        )?,
        encryption_type: value_u8(
            &values[DescribeParameterEncryptionResultSet2::ColumnEncryptionType as usize],
        )?,
        cek_ordinal: value_usize(
            &values[DescribeParameterEncryptionResultSet2::ColumnEncryptionKeyOrdinal as usize],
        )?,
        normalization_rule_version: vec![value_u8(
            &values[DescribeParameterEncryptionResultSet2::NormalizationRuleVersion as usize],
        )?],
    })
}

fn encryption_type_from_byte(value: u8) -> Result<SqlServerEncryptionType, String> {
    match value {
        0 => Ok(SqlServerEncryptionType::PlainText),
        1 => Ok(SqlServerEncryptionType::Deterministic),
        2 => Ok(SqlServerEncryptionType::Randomized),
        _ => Err(format!("unknown SQL Server encryption type {value}")),
    }
}

fn encryption_type_to_byte(value: SqlServerEncryptionType) -> u8 {
    value as u8
}

fn value_usize(value: &Value) -> Result<usize, String> {
    match value {
        Value::Integer(number) if *number >= 0 => {
            usize::try_from(*number).map_err(|_| "metadata integer exceeds usize".to_owned())
        }
        Value::BigInteger(number) => number
            .parse::<u128>()
            .ok()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| "metadata integer exceeds usize".to_owned()),
        _ => Err("metadata value is not a non-negative integer".to_owned()),
    }
}

fn value_u32(value: &Value) -> Result<u32, String> {
    u32::try_from(value_usize(value)?).map_err(|_| "metadata integer exceeds u32".to_owned())
}

fn value_u8(value: &Value) -> Result<u8, String> {
    u8::try_from(value_u32(value)?).map_err(|_| "metadata integer exceeds u8".to_owned())
}

fn value_text(value: &Value) -> Result<String, String> {
    match value {
        Value::Text(value) => Ok(value.clone()),
        _ => Err("metadata value is not text".to_owned()),
    }
}

fn value_binary(value: &Value) -> Result<Vec<u8>, String> {
    match value {
        Value::Binary(value) => Ok(value.clone()),
        _ => Err("metadata value is not binary".to_owned()),
    }
}

pub fn associate_base_type_info(
    crypto_metadata: CryptoMetadata,
    base_type_info: crate::metadata_parser::Metadata,
) -> CryptoMetadataWithBaseTypeInfo {
    CryptoMetadataWithBaseTypeInfo::new(crypto_metadata, base_type_info)
}

pub fn associate_result_set_metadata(
    columns: Vec<Option<(CryptoMetadata, crate::metadata_parser::Metadata)>>,
) -> ResultSetCryptoMetadata {
    ResultSetCryptoMetadata::new(
        columns
            .into_iter()
            .map(|column| {
                column.map(|(crypto_metadata, base_type_info)| {
                    associate_base_type_info(crypto_metadata, base_type_info)
                })
            })
            .collect(),
    )
}

pub fn associate_return_value_metadata(
    result_metadata: &mut ResultSetCryptoMetadata,
    ordinal: usize,
    name: impl Into<String>,
    crypto_metadata: CryptoMetadata,
    base_type_info: crate::metadata_parser::Metadata,
) {
    result_metadata.add_return_value(
        ordinal,
        name,
        associate_base_type_info(crypto_metadata, base_type_info),
    );
}

pub fn decrypt_result_value(
    ciphertext: &[u8],
    metadata: &CryptoMetadataWithBaseTypeInfo,
    server_name: &str,
) -> Result<Value, String> {
    let plaintext = decrypt_with_key(ciphertext, &metadata.crypto_metadata, server_name)?;
    decode_result_value(&plaintext, metadata.base_type_info())
}

pub fn decrypt_result_value_with_provider(
    ciphertext: &[u8],
    metadata: &mut CryptoMetadataWithBaseTypeInfo,
    server_name: &str,
    provider: &dyn KeyStoreProvider,
    cache_ttl: Duration,
) -> Result<Value, String> {
    if metadata.crypto_metadata.cipher_algorithm.is_none() {
        decrypt_symmetric_key(
            metadata.crypto_metadata_mut(),
            server_name,
            provider,
            cache_ttl,
        )?;
    }
    decrypt_result_value(ciphertext, metadata, server_name)
}

pub fn decrypt_result_column(
    value: &Value,
    metadata: &CryptoMetadataWithBaseTypeInfo,
    server_name: &str,
) -> Result<Value, String> {
    match value {
        Value::Null => Ok(Value::Null),
        Value::Binary(ciphertext) => decrypt_result_value(ciphertext, metadata, server_name),
        _ => Err("encrypted result value must be binary or null".to_owned()),
    }
}

pub fn decrypt_result_column_with_provider(
    value: &Value,
    metadata: &mut CryptoMetadataWithBaseTypeInfo,
    server_name: &str,
    provider: &dyn KeyStoreProvider,
    cache_ttl: Duration,
) -> Result<Value, String> {
    match value {
        Value::Null => Ok(Value::Null),
        Value::Binary(ciphertext) => decrypt_result_value_with_provider(
            ciphertext,
            metadata,
            server_name,
            provider,
            cache_ttl,
        ),
        _ => Err("encrypted result value must be binary or null".to_owned()),
    }
}

pub fn decrypt_result_set_with_provider(
    values: &mut [Value],
    metadata: &mut ResultSetCryptoMetadata,
    server_name: &str,
    provider: &dyn KeyStoreProvider,
    cache_ttl: Duration,
) -> Result<(), String> {
    if values.len() != metadata.columns().len() {
        return Err("result values and encryption metadata differ in length".to_owned());
    }
    for (ordinal, value) in values.iter_mut().enumerate() {
        let Some(column_metadata) = metadata.column_mut(ordinal) else {
            continue;
        };
        *value = decrypt_result_column_with_provider(
            value,
            column_metadata,
            server_name,
            provider,
            cache_ttl,
        )?;
    }
    Ok(())
}

pub fn build_parameter_encryption_metadata_with_force(
    parameters: Vec<CryptoMetadata>,
    force_encrypt: Vec<bool>,
) -> Result<ParameterEncryptionMetadata, String> {
    ParameterEncryptionMetadata::new(parameters, force_encrypt)
}

fn decode_result_value(
    plaintext: &[u8],
    base_type_info: &crate::metadata_parser::Metadata,
) -> Result<Value, String> {
    let wire_value = result_wire_value(plaintext, base_type_info)?;
    let parsed = read_value(&wire_value, 0, base_type_info)
        .map_err(|error| format!("unable to decode decrypted result: {error:?}"))?;
    if parsed.offset != wire_value.len() {
        return Err("decrypted result contains trailing bytes".to_owned());
    }
    Ok(parsed.value)
}

fn result_wire_value(
    plaintext: &[u8],
    metadata: &crate::metadata_parser::Metadata,
) -> Result<Vec<u8>, String> {
    let id = metadata.data_type.id;
    let mut wire = Vec::new();
    if matches!(id, 0xA5 | 0xAD | 0xA7 | 0xAF | 0xE7 | 0xEF)
        && metadata.data_length == Some(u32::from(u16::MAX))
    {
        let length = u64::try_from(plaintext.len())
            .map_err(|_| "decrypted result value exceeds eight-byte length".to_owned())?;
        wire.extend(length.to_le_bytes());
        if !plaintext.is_empty() {
            let chunk_length = u32::try_from(plaintext.len())
                .map_err(|_| "decrypted result chunk exceeds four-byte length".to_owned())?;
            wire.extend(chunk_length.to_le_bytes());
            wire.extend(plaintext);
        }
        wire.extend([0, 0, 0, 0]);
        return Ok(wire);
    }
    match id {
        0x24 | 0x26 | 0x6D | 0x6E | 0x68 | 0x6F | 0x29..=0x2B | 0x6A | 0x6C => {
            if plaintext.len() > usize::from(u8::MAX) {
                return Err("decrypted result value exceeds one-byte length".to_owned());
            }
            let length = u8::try_from(plaintext.len())
                .map_err(|_| "decrypted result value exceeds one-byte length".to_owned())?;
            wire.push(length);
        }
        0xA5 | 0xAD | 0xA7 | 0xAF | 0xE7 | 0xEF => {
            if plaintext.len() >= usize::from(u16::MAX) {
                return Err("decrypted result value exceeds two-byte length".to_owned());
            }
            let length = u16::try_from(plaintext.len())
                .map_err(|_| "decrypted result value exceeds two-byte length".to_owned())?;
            wire.extend(length.to_le_bytes());
        }
        0xF0 | 0xF1 => {
            if id == 0xF1 && !plaintext.len().is_multiple_of(2) {
                return Err("decrypted wide result has an odd byte length".to_owned());
            }
            let length = u32::try_from(plaintext.len())
                .map_err(|_| "decrypted result value exceeds four-byte length".to_owned())?;
            wire.extend(
                u64::try_from(plaintext.len())
                    .map_err(|_| "decrypted result value exceeds eight-byte length".to_owned())?
                    .to_le_bytes(),
            );
            wire.extend(plaintext);
            wire.splice(8..8, length.to_le_bytes());
            wire.extend([0, 0, 0, 0]);
            return Ok(wire);
        }
        0x22 | 0x23 | 0x63 => {
            if id == 0x63 && !plaintext.len().is_multiple_of(2) {
                return Err("decrypted ntext result has an odd byte length".to_owned());
            }
            wire.push(16);
            wire.extend([0; 16]);
            wire.extend([0; 8]);
            let length = u32::try_from(plaintext.len())
                .map_err(|_| "decrypted result value exceeds four-byte length".to_owned())?;
            wire.extend(length.to_le_bytes());
            wire.extend(plaintext);
            return Ok(wire);
        }
        _ => {}
    }
    wire.extend(plaintext);
    Ok(wire)
}
