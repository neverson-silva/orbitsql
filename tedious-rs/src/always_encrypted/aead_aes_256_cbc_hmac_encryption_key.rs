use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::symmetric_key::SymmetricKey;

pub const KEY_SIZE_BITS: usize = 256;
pub const ALGORITHM_NAME: &str = "AEAD_AES_256_CBC_HMAC_SHA256";

pub fn derive_key(root_key: &[u8], salt: &str) -> Result<Vec<u8>, String> {
    let mut hmac =
        Hmac::<Sha256>::new_from_slice(root_key).map_err(|_| "invalid HMAC root key".to_owned())?;
    let salt = salt
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    hmac.update(&salt);
    Ok(hmac.finalize().into_bytes().to_vec())
}

pub fn generate_key_salt(key_type: &str, algorithm_name: &str, key_size: usize) -> String {
    format!(
        "Microsoft SQL Server cell {key_type} key with encryption algorithm:{algorithm_name} and key length:{key_size}"
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AeadAes256CbcHmac256EncryptionKey {
    root: SymmetricKey,
    encryption: SymmetricKey,
    mac: SymmetricKey,
    iv: SymmetricKey,
}

impl AeadAes256CbcHmac256EncryptionKey {
    pub fn new(root_key: Vec<u8>, algorithm_name: &str) -> Result<Self, String> {
        if root_key.is_empty() {
            return Err("Empty column encryption key specified.".to_owned());
        }
        if root_key.len() != KEY_SIZE_BITS / 8 {
            return Err(format!(
                "The column encryption key has been successfully decrypted but it's length: {} does not match the length: {} for algorithm \"{}\". Verify the encrypted value of the column encryption key in the database.",
                root_key.len(),
                KEY_SIZE_BITS / 8,
                algorithm_name
            ));
        }
        let encryption = SymmetricKey::new(derive_key(
            &root_key,
            &generate_key_salt("encryption", algorithm_name, KEY_SIZE_BITS),
        )?)?;
        let mac = SymmetricKey::new(derive_key(
            &root_key,
            &generate_key_salt("MAC", algorithm_name, KEY_SIZE_BITS),
        )?)?;
        let iv = SymmetricKey::new(derive_key(
            &root_key,
            &generate_key_salt("IV", algorithm_name, KEY_SIZE_BITS),
        )?)?;
        Ok(Self {
            root: SymmetricKey::new(root_key)?,
            encryption,
            mac,
            iv,
        })
    }

    pub fn get_encryption_key(&self) -> &[u8] {
        self.encryption.root_key()
    }

    pub fn get_mac_key(&self) -> &[u8] {
        self.mac.root_key()
    }

    pub fn get_iv_key(&self) -> &[u8] {
        self.iv.root_key()
    }

    pub fn root_key(&self) -> &[u8] {
        self.root.root_key()
    }
}
