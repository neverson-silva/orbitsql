use super::aead_aes_256_cbc_hmac_algorithm::AeadAes256CbcHmac256Algorithm;
use super::cek_entry::CekEntry;
use crate::metadata_parser::Metadata;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncryptionKeyInfo {
    pub encrypted_key: Vec<u8>,
    pub db_id: u32,
    pub key_id: u32,
    pub key_version: u32,
    pub md_version: Vec<u8>,
    pub key_path: String,
    pub key_store_name: String,
    pub algorithm_name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SqlServerEncryptionType {
    PlainText = 0,
    Deterministic = 1,
    Randomized = 2,
}

#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescribeParameterEncryptionResultSet1 {
    KeyOrdinal = 0,
    DbId = 1,
    KeyId = 2,
    KeyVersion = 3,
    KeyMdVersion = 4,
    EncryptedKey = 5,
    ProviderName = 6,
    KeyPath = 7,
    KeyEncryptionAlgorithm = 8,
}

#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescribeParameterEncryptionResultSet2 {
    ParameterOrdinal = 0,
    ParameterName = 1,
    ColumnEncryptionAlgorithm = 2,
    ColumnEncryptionType = 3,
    ColumnEncryptionKeyOrdinal = 4,
    NormalizationRuleVersion = 5,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncryptedResultColumnMetadata {
    pub ordinal: usize,
    pub cipher_algorithm_id: u8,
    pub encryption_type: SqlServerEncryptionType,
    pub normalization_rule_version: Vec<u8>,
}

impl EncryptedResultColumnMetadata {
    pub fn new(
        ordinal: usize,
        cipher_algorithm_id: u8,
        encryption_type: SqlServerEncryptionType,
        normalization_rule_version: Vec<u8>,
    ) -> Result<Self, String> {
        if normalization_rule_version.len() != 1 {
            return Err("normalization rule version must contain one byte".to_owned());
        }
        Ok(Self {
            ordinal,
            cipher_algorithm_id,
            encryption_type,
            normalization_rule_version,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StatementColumnEncryptionSetting {
    #[default]
    UseConnectionSetting,
    Enabled,
    ResultSetOnly,
    Disabled,
}

pub struct CryptoMetadata {
    pub cek_entry: Option<CekEntry>,
    pub cipher_algorithm_id: u8,
    pub cipher_algorithm_name: Option<String>,
    pub normalization_rule_version: Vec<u8>,
    pub encryption_key_info: Option<EncryptionKeyInfo>,
    pub ordinal: usize,
    pub encryption_type: SqlServerEncryptionType,
    pub cipher_algorithm: Option<AeadAes256CbcHmac256Algorithm>,
}

impl Clone for CryptoMetadata {
    fn clone(&self) -> Self {
        Self {
            cek_entry: self.cek_entry.clone(),
            cipher_algorithm_id: self.cipher_algorithm_id,
            cipher_algorithm_name: self.cipher_algorithm_name.clone(),
            normalization_rule_version: self.normalization_rule_version.clone(),
            encryption_key_info: self.encryption_key_info.clone(),
            ordinal: self.ordinal,
            encryption_type: self.encryption_type,
            cipher_algorithm: None,
        }
    }
}

pub struct CryptoMetadataWithBaseTypeInfo {
    pub crypto_metadata: CryptoMetadata,
    pub base_type_info: Metadata,
}

#[derive(Default)]
pub struct ResultSetCryptoMetadata {
    columns: Vec<Option<CryptoMetadataWithBaseTypeInfo>>,
    return_values_by_ordinal: HashMap<usize, CryptoMetadataWithBaseTypeInfo>,
    return_value_ordinals_by_name: HashMap<String, usize>,
}

impl ResultSetCryptoMetadata {
    pub fn new(columns: Vec<Option<CryptoMetadataWithBaseTypeInfo>>) -> Self {
        Self {
            columns,
            return_values_by_ordinal: HashMap::new(),
            return_value_ordinals_by_name: HashMap::new(),
        }
    }

    pub fn column(&self, ordinal: usize) -> Option<&CryptoMetadataWithBaseTypeInfo> {
        self.columns.get(ordinal).and_then(Option::as_ref)
    }

    pub fn column_mut(&mut self, ordinal: usize) -> Option<&mut CryptoMetadataWithBaseTypeInfo> {
        self.columns.get_mut(ordinal).and_then(Option::as_mut)
    }

    pub fn columns(&self) -> &[Option<CryptoMetadataWithBaseTypeInfo>] {
        &self.columns
    }

    pub fn add_return_value(
        &mut self,
        ordinal: usize,
        name: impl Into<String>,
        metadata: CryptoMetadataWithBaseTypeInfo,
    ) {
        self.return_values_by_ordinal.insert(ordinal, metadata);
        self.return_value_ordinals_by_name
            .insert(name.into(), ordinal);
    }

    pub fn return_value_by_ordinal(
        &self,
        ordinal: usize,
    ) -> Option<&CryptoMetadataWithBaseTypeInfo> {
        self.return_values_by_ordinal.get(&ordinal)
    }

    pub fn return_value_by_name(&self, name: &str) -> Option<&CryptoMetadataWithBaseTypeInfo> {
        self.return_value_ordinals_by_name
            .get(name)
            .and_then(|ordinal| self.return_value_by_ordinal(*ordinal))
    }
}

impl CryptoMetadataWithBaseTypeInfo {
    pub fn new(crypto_metadata: CryptoMetadata, base_type_info: Metadata) -> Self {
        Self {
            crypto_metadata,
            base_type_info,
        }
    }

    pub fn crypto_metadata_mut(&mut self) -> &mut CryptoMetadata {
        &mut self.crypto_metadata
    }

    pub fn base_type_info(&self) -> &Metadata {
        &self.base_type_info
    }
}

pub struct ParameterEncryptionMetadata {
    pub parameters: Vec<CryptoMetadata>,
    force_encrypt: Vec<bool>,
}

impl ParameterEncryptionMetadata {
    pub fn new(parameters: Vec<CryptoMetadata>, force_encrypt: Vec<bool>) -> Result<Self, String> {
        if parameters.len() != force_encrypt.len() {
            return Err(
                "parameter metadata and force-encrypt settings differ in length".to_owned(),
            );
        }
        Ok(Self {
            parameters,
            force_encrypt,
        })
    }

    pub fn force_encrypt(&self, ordinal: usize) -> Result<bool, String> {
        self.force_encrypt
            .get(ordinal)
            .copied()
            .ok_or_else(|| format!("parameter ordinal {ordinal} is out of range"))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&CryptoMetadata, bool)> {
        self.parameters
            .iter()
            .zip(self.force_encrypt.iter().copied())
    }
}
