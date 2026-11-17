use aes::Aes256;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;

use super::aead_aes_256_cbc_hmac_encryption_key::{
    ALGORITHM_NAME, AeadAes256CbcHmac256EncryptionKey,
};

const BLOCK_SIZE_IN_BYTES: usize = 16;
const KEY_SIZE_IN_BYTES: usize = 32;

type Aes256CbcEncryptor = cbc::Encryptor<Aes256>;
type Aes256CbcDecryptor = cbc::Decryptor<Aes256>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncryptionType {
    Deterministic,
    Randomized,
}

pub const ALGORITHM_VERSION: u8 = 1;

pub struct AeadAes256CbcHmac256Algorithm {
    key: AeadAes256CbcHmac256EncryptionKey,
    encryption_type: EncryptionType,
}

impl AeadAes256CbcHmac256Algorithm {
    pub fn new(key: AeadAes256CbcHmac256EncryptionKey, encryption_type: EncryptionType) -> Self {
        Self {
            key,
            encryption_type,
        }
    }

    pub fn encrypt_data(&self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        let iv = match self.encryption_type {
            EncryptionType::Randomized => {
                let mut iv = [0u8; 16];
                rand::rng().fill_bytes(&mut iv);
                iv
            }
            EncryptionType::Deterministic => {
                let mut hmac = Hmac::<Sha256>::new_from_slice(self.key.get_iv_key())
                    .map_err(|_| "invalid IV key".to_owned())?;
                hmac.update(plaintext);
                let digest = hmac.finalize().into_bytes();
                let mut iv = [0u8; 16];
                iv.copy_from_slice(&digest[..16]);
                iv
            }
        };
        let cipher = Aes256CbcEncryptor::new_from_slices(self.key.get_encryption_key(), &iv)
            .map_err(|_| "invalid AES encryption key".to_owned())?;
        let mut buffer = vec![0; plaintext.len() + 16];
        buffer[..plaintext.len()].copy_from_slice(plaintext);
        let ciphertext = cipher
            .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext.len())
            .map_err(|_| "AES encryption failed".to_owned())?;
        let tag = self.authentication_tag(&iv, ciphertext)?;
        let mut output = vec![ALGORITHM_VERSION];
        output.extend(tag);
        output.extend(iv);
        output.extend(ciphertext);
        Ok(output)
    }

    pub fn decrypt_data(&self, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        let minimum_ciphertext_length =
            1 + KEY_SIZE_IN_BYTES + BLOCK_SIZE_IN_BYTES + BLOCK_SIZE_IN_BYTES;
        if ciphertext.len() < minimum_ciphertext_length {
            return Err(format!(
                "Specified ciphertext has an invalid size of {} bytes, which is below the minimum {} bytes required for decryption.",
                ciphertext.len(),
                minimum_ciphertext_length
            ));
        }
        if ciphertext[0] != ALGORITHM_VERSION {
            return Err(format!(
                "The specified ciphertext's encryption algorithm version {:02x} does not match the expected encryption algorithm version {}.",
                ciphertext[0], ALGORITHM_VERSION
            ));
        }
        let tag = &ciphertext[1..33];
        let iv = &ciphertext[33..49];
        let encrypted = &ciphertext[49..];
        let expected = self.authentication_tag(iv, encrypted)?;
        if !constant_time_equal(tag, &expected) {
            return Err("Specified ciphertext has an invalid authentication tag.".to_owned());
        }
        let cipher = Aes256CbcDecryptor::new_from_slices(self.key.get_encryption_key(), iv)
            .map_err(|_| "invalid AES decryption key".to_owned())?;
        let mut buffer = encrypted.to_vec();
        cipher
            .decrypt_padded_mut::<Pkcs7>(&mut buffer)
            .map(|value| value.to_vec())
            .map_err(|_| {
                let message = if encrypted.len().is_multiple_of(BLOCK_SIZE_IN_BYTES) {
                    "error:1C800064:Provider routines::bad decrypt"
                } else {
                    "error:1C80006B:Provider routines::wrong final block length"
                };
                format!("Internal error while decryption: {message}")
            })
    }

    fn authentication_tag(&self, iv: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        let mut hmac = Hmac::<Sha256>::new_from_slice(self.key.get_mac_key())
            .map_err(|_| "invalid MAC key".to_owned())?;
        hmac.update(&[ALGORITHM_VERSION]);
        hmac.update(iv);
        hmac.update(ciphertext);
        hmac.update(&[1]);
        Ok(hmac.finalize().into_bytes().to_vec())
    }
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .fold(0u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}

pub fn algorithm_name() -> &'static str {
    ALGORITHM_NAME
}
