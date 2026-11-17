use hmac::{Hmac, Mac};
use sha2::Sha256;
use tedious_rs::always_encrypted::{
    AeadAes256CbcHmac256Algorithm, AeadAes256CbcHmac256EncryptionKey, EncryptionType,
};

const ALGORITHM_NAME: &str = "AEAD_AES_256_CBC_HMAC_SHA256";

fn algorithm() -> AeadAes256CbcHmac256Algorithm {
    let key = AeadAes256CbcHmac256EncryptionKey::new(vec![0x11; 32], ALGORITHM_NAME).unwrap();
    AeadAes256CbcHmac256Algorithm::new(key, EncryptionType::Deterministic)
}

fn authentication_tag(
    key: &AeadAes256CbcHmac256EncryptionKey,
    iv: &[u8],
    encrypted: &[u8],
) -> [u8; 32] {
    let mut hmac = Hmac::<Sha256>::new_from_slice(key.get_mac_key()).unwrap();
    hmac.update(&[1]);
    hmac.update(iv);
    hmac.update(encrypted);
    hmac.update(&[1]);
    hmac.finalize().into_bytes().into()
}

fn ciphertext_with_encrypted(key: &AeadAes256CbcHmac256EncryptionKey, encrypted: &[u8]) -> Vec<u8> {
    let iv = [0x22; 16];
    let tag = authentication_tag(key, &iv, encrypted);
    let mut ciphertext = vec![1];
    ciphertext.extend(tag);
    ciphertext.extend(iv);
    ciphertext.extend(encrypted);
    ciphertext
}

#[test]
fn decrypt_size_error_matches_node_message() {
    let error = algorithm().decrypt_data(&[1; 64]).err().unwrap();
    assert_eq!(
        error,
        "Specified ciphertext has an invalid size of 64 bytes, which is below the minimum 65 bytes required for decryption."
    );
}

#[test]
fn decrypt_version_error_matches_node_message() {
    let mut ciphertext = algorithm().encrypt_data(b"value").unwrap();
    ciphertext[0] = 2;
    assert_eq!(
        algorithm().decrypt_data(&ciphertext).err().unwrap(),
        "The specified ciphertext's encryption algorithm version 02 does not match the expected encryption algorithm version 1."
    );
}

#[test]
fn decrypt_authentication_error_matches_node_message() {
    let mut ciphertext = algorithm().encrypt_data(b"value").unwrap();
    ciphertext[1] ^= 1;
    assert_eq!(
        algorithm().decrypt_data(&ciphertext).err().unwrap(),
        "Specified ciphertext has an invalid authentication tag."
    );
}

#[test]
fn decrypt_padding_error_matches_node_message() {
    let key = AeadAes256CbcHmac256EncryptionKey::new(vec![0x11; 32], ALGORITHM_NAME).unwrap();
    let ciphertext = ciphertext_with_encrypted(&key, &[0; 16]);
    assert_eq!(
        algorithm().decrypt_data(&ciphertext).err().unwrap(),
        "Internal error while decryption: error:1C800064:Provider routines::bad decrypt"
    );
}

#[test]
fn decrypt_block_length_error_matches_node_message() {
    let key = AeadAes256CbcHmac256EncryptionKey::new(vec![0x11; 32], ALGORITHM_NAME).unwrap();
    let ciphertext = ciphertext_with_encrypted(&key, &[0; 17]);
    assert_eq!(
        algorithm().decrypt_data(&ciphertext).err().unwrap(),
        "Internal error while decryption: error:1C80006B:Provider routines::wrong final block length"
    );
}

#[test]
fn key_length_error_matches_node_message() {
    assert_eq!(
        AeadAes256CbcHmac256EncryptionKey::new(vec![0; 31], ALGORITHM_NAME).unwrap_err(),
        "The column encryption key has been successfully decrypted but it's length: 31 does not match the length: 32 for algorithm \"AEAD_AES_256_CBC_HMAC_SHA256\". Verify the encrypted value of the column encryption key in the database."
    );
}

#[test]
fn empty_key_error_matches_node_message() {
    assert_eq!(
        AeadAes256CbcHmac256EncryptionKey::new(Vec::new(), ALGORITHM_NAME).unwrap_err(),
        "Empty column encryption key specified."
    );
}
