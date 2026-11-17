use super::types::EncryptionKeyInfo;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CekEntry {
    pub ordinal: usize,
    pub database_id: u32,
    pub cek_id: u32,
    pub cek_version: u32,
    pub cek_md_version: Vec<u8>,
    pub column_encryption_key_values: Vec<EncryptionKeyInfo>,
}

impl CekEntry {
    pub fn new(ordinal: usize) -> Self {
        Self {
            ordinal,
            database_id: 0,
            cek_id: 0,
            cek_version: 0,
            cek_md_version: Vec::new(),
            column_encryption_key_values: Vec::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add(
        &mut self,
        encrypted_key: Vec<u8>,
        db_id: u32,
        key_id: u32,
        key_version: u32,
        md_version: Vec<u8>,
        key_path: String,
        key_store_name: String,
        algorithm_name: String,
    ) -> Result<(), String> {
        if encrypted_key.is_empty() {
            return Err("Empty encrypted column encryption key specified.".to_owned());
        }
        if key_store_name.is_empty() {
            return Err("Column encryption key store provider cannot be empty.".to_owned());
        }
        if key_path.is_empty() {
            return Err("Column encryption key path cannot be empty.".to_owned());
        }
        if algorithm_name.is_empty() {
            return Err("Column encryption key encryption algorithm cannot be empty.".to_owned());
        }
        let value = EncryptionKeyInfo {
            encrypted_key,
            db_id,
            key_id,
            key_version,
            md_version,
            key_path,
            key_store_name,
            algorithm_name,
        };
        self.column_encryption_key_values.push(value.clone());
        if self.database_id == 0 {
            self.database_id = value.db_id;
            self.cek_id = value.key_id;
            self.cek_version = value.key_version;
            self.cek_md_version = value.md_version.clone();
        } else if self.database_id != value.db_id
            || self.cek_id != value.key_id
            || self.cek_version != value.key_version
            || self.cek_md_version.len() != value.md_version.len()
        {
            return Err("Invalid databaseId, cekId, cekVersion or cekMdVersion.".to_owned());
        }
        Ok(())
    }
}
