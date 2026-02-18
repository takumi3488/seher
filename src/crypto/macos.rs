use crate::crypto::{CryptoError, Result};
use aes::cipher::{BlockDecryptMut, KeyIvInit};
use security_framework::os::macos::keychain::SecKeychain;

type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

const SALT: &[u8] = b"saltysalt";
const IV: &[u8] = b"                "; // 16 spaces
const KEY_LENGTH: usize = 16;
const ITERATIONS: u32 = 1003;

pub fn decrypt(encrypted_value: &[u8]) -> Result<String> {
    if encrypted_value.len() < 3 {
        return Ok(String::new());
    }

    let version = &encrypted_value[..3];
    match version {
        b"v10" | b"v11" => {
            let encrypted = &encrypted_value[3..];
            let key = get_encryption_key()?;
            decrypt_aes_cbc(&key, encrypted)
        }
        _ => String::from_utf8(encrypted_value.to_vec())
            .map_err(|e| CryptoError::DecryptionFailed(e.to_string())),
    }
}

fn get_encryption_key() -> Result<Vec<u8>> {
    let keychain = SecKeychain::default()
        .map_err(|e| CryptoError::KeychainError(format!("Failed to access keychain: {}", e)))?;

    let (password_data, _item) = keychain
        .find_generic_password("Chrome Safe Storage", "Chrome")
        .map_err(|e| {
            CryptoError::KeychainError(format!("Failed to find Chrome Safe Storage: {}", e))
        })?;

    let password = password_data.as_ref();

    let mut key = vec![0u8; KEY_LENGTH];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password, SALT, ITERATIONS, &mut key);

    Ok(key)
}

fn decrypt_aes_cbc(key: &[u8], encrypted: &[u8]) -> Result<String> {
    let cipher = Aes128CbcDec::new(key.into(), IV.into());

    let mut buffer = encrypted.to_vec();
    let decrypted = cipher
        .decrypt_padded_mut::<aes::cipher::block_padding::Pkcs7>(&mut buffer)
        .map_err(|e| {
            CryptoError::DecryptionFailed(format!("AES-CBC decryption failed: {:?}", e))
        })?;

    String::from_utf8(decrypted.to_vec())
        .map_err(|e| CryptoError::DecryptionFailed(format!("UTF-8 conversion failed: {}", e)))
}
