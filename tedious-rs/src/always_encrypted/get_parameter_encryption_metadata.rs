use std::collections::HashMap;

use super::cek_entry::CekEntry;
use super::types::{CryptoMetadata, SqlServerEncryptionType};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CekMetadataRow {
    pub key_ordinal: usize,
    pub db_id: u32,
    pub key_id: u32,
    pub key_version: u32,
    pub md_version: Vec<u8>,
    pub encrypted_key: Vec<u8>,
    pub provider_name: String,
    pub key_path: String,
    pub algorithm_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterEncryptionMetadataRow {
    pub parameter_ordinal: usize,
    pub parameter_name: String,
    pub cipher_algorithm_id: u8,
    pub encryption_type: u8,
    pub cek_ordinal: usize,
    pub normalization_rule_version: Vec<u8>,
}

pub fn build_parameter_encryption_metadata(
    cek_rows: &[CekMetadataRow],
    parameter_rows: &[ParameterEncryptionMetadataRow],
    parameter_names: &[String],
    force_encrypt: &[bool],
) -> Result<Vec<CryptoMetadata>, String> {
    if parameter_names.len() != force_encrypt.len() {
        return Err("parameter names and force-encrypt settings differ in length".to_owned());
    }
    let mut entries: HashMap<usize, CekEntry> = HashMap::new();
    for row in cek_rows {
        entries
            .entry(row.key_ordinal)
            .or_insert_with(|| CekEntry::new(row.key_ordinal))
            .add(
                row.encrypted_key.clone(),
                row.db_id,
                row.key_id,
                row.key_version,
                row.md_version.clone(),
                row.key_path.clone(),
                row.provider_name.clone(),
                row.algorithm_name.clone(),
            )?;
    }
    let mut result = (0..parameter_names.len())
        .map(|_| None)
        .collect::<Vec<Option<CryptoMetadata>>>();
    let mut seen = vec![false; parameter_names.len()];
    for row in parameter_rows {
        let _server_parameter_ordinal = row.parameter_ordinal;
        let row_name = row.parameter_name.trim_start_matches('@');
        let index = parameter_names
            .iter()
            .position(|name| name.trim_start_matches('@') == row_name)
            .ok_or_else(|| {
                format!(
                    "metadata references unknown parameter {}",
                    row.parameter_name
                )
            })?;
        if seen[index] {
            return Err(format!(
                "parameter metadata is duplicated for {}",
                row.parameter_name
            ));
        }
        seen[index] = true;
        let encryption_type = match row.encryption_type {
            0 => SqlServerEncryptionType::PlainText,
            1 => SqlServerEncryptionType::Deterministic,
            2 => SqlServerEncryptionType::Randomized,
            value => return Err(format!("unknown SQL Server encryption type {value}")),
        };
        if encryption_type == SqlServerEncryptionType::PlainText {
            if force_encrypt[index] {
                return Err(format!(
                    "parameter {} requires encryption but the server expects plaintext",
                    index + 1
                ));
            }
            result[index] = Some(CryptoMetadata {
                cek_entry: None,
                cipher_algorithm_id: row.cipher_algorithm_id,
                cipher_algorithm_name: None,
                normalization_rule_version: row.normalization_rule_version.clone(),
                encryption_key_info: None,
                ordinal: row.cek_ordinal,
                encryption_type,
                cipher_algorithm: None,
            });
            continue;
        }
        if row.normalization_rule_version.len() != 1 {
            return Err(format!(
                "normalization rule version for parameter {} must contain one byte",
                index + 1
            ));
        }
        let entry = entries
            .get(&row.cek_ordinal)
            .cloned()
            .ok_or_else(|| format!("missing CEK ordinal {}", row.cek_ordinal))?;
        result[index] = Some(CryptoMetadata {
            cek_entry: Some(entry),
            cipher_algorithm_id: row.cipher_algorithm_id,
            cipher_algorithm_name: None,
            normalization_rule_version: row.normalization_rule_version.clone(),
            encryption_key_info: None,
            ordinal: row.cek_ordinal,
            encryption_type,
            cipher_algorithm: None,
        });
    }
    if seen.iter().any(|present| !present) {
        return Err("metadata for some parameters is missing".to_owned());
    }
    result
        .into_iter()
        .map(|metadata| {
            metadata.ok_or_else(|| "metadata for some parameters is missing".to_owned())
        })
        .collect()
}
