use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Failed to decrypt: {0}")]
    DecryptionFailed(String),

    #[error("Unsupported encryption version: {0}")]
    UnsupportedVersion(String),

    #[cfg(target_os = "macos")]
    #[error("Keychain error: {0}")]
    KeychainError(String),

    #[cfg(target_os = "linux")]
    #[error("Secret service error: {0}")]
    SecretServiceError(String),

    #[cfg(target_os = "windows")]
    #[error("DPAPI error: {0}")]
    DpapiError(String),
}

pub type Result<T> = std::result::Result<T, CryptoError>;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

pub fn decrypt_cookie_value(encrypted_value: &[u8]) -> Result<String> {
    if encrypted_value.is_empty() {
        return Ok(String::new());
    }

    #[cfg(target_os = "macos")]
    return macos::decrypt(encrypted_value);

    #[cfg(target_os = "linux")]
    return linux::decrypt(encrypted_value);

    #[cfg(target_os = "windows")]
    return windows::decrypt(encrypted_value);

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    return Err(CryptoError::UnsupportedVersion(
        "Unsupported OS".to_string(),
    ));
}
