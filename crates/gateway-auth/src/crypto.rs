//! AES-256-GCM encryption for sensitive data at rest.
//!
//! Used to encrypt provider API keys before storing in PostgreSQL.
//! Master key is loaded from `GATEWAY_MASTER_KEY` env var (32 bytes, hex-encoded).

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};

/// AES-256-GCM nonce size: 96 bits (12 bytes).
const NONCE_SIZE: usize = 12;

/// Encrypt plaintext with AES-256-GCM.
/// Returns ciphertext with random nonce prepended: `[nonce (12 bytes) | ciphertext]`.
pub fn encrypt(plaintext: &str, master_key: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(master_key)
        .map_err(|_| CryptoError::InvalidKeyLength)?;

    let mut nonce_bytes = [0u8; NONCE_SIZE];
    aes_gcm::aead::rand_core::RngCore::try_fill_bytes(&mut OsRng, &mut nonce_bytes)
        .map_err(|_| CryptoError::EncryptFailed)?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| CryptoError::EncryptFailed)?;

    // Prepend nonce to ciphertext
    let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    result.extend_from_slice(&nonce_bytes[..NONCE_SIZE]);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

/// Decrypt ciphertext with AES-256-GCM.
/// Expects format: `[nonce (12 bytes) | ciphertext]`.
pub fn decrypt(ciphertext: &[u8], master_key: &[u8]) -> Result<String, CryptoError> {
    if ciphertext.len() < NONCE_SIZE {
        return Err(CryptoError::InvalidCiphertext);
    }

    let cipher = Aes256Gcm::new_from_slice(master_key)
        .map_err(|_| CryptoError::InvalidKeyLength)?;

    let nonce = Nonce::from_slice(&ciphertext[..NONCE_SIZE]);
    let encrypted = &ciphertext[NONCE_SIZE..];

    let plaintext = cipher
        .decrypt(nonce, encrypted)
        .map_err(|_| CryptoError::DecryptFailed)?;

    String::from_utf8(plaintext).map_err(|_| CryptoError::InvalidUtf8)
}

/// Parse a hex-encoded master key string into a 32-byte array.
pub fn parse_master_key(hex_str: &str) -> Result<[u8; 32], CryptoError> {
    let bytes = hex::decode(hex_str.trim()).map_err(|_| CryptoError::InvalidHex)?;
    if bytes.len() != 32 {
        return Err(CryptoError::InvalidKeyLength);
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

/// Crypto error type.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CryptoError {
    #[error("Invalid key length: expected 32 bytes")]
    InvalidKeyLength,
    #[error("Invalid hex encoding")]
    InvalidHex,
    #[error("Encryption failed")]
    EncryptFailed,
    #[error("Decryption failed — tampered ciphertext or wrong key")]
    DecryptFailed,
    #[error("Invalid ciphertext: too short")]
    InvalidCiphertext,
    #[error("Decrypted data is not valid UTF-8")]
    InvalidUtf8,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        [0u8; 32]
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = test_key();
        let plaintext = "sk-openai-secret-key-12345";

        let ciphertext = encrypt(plaintext, &key).unwrap();
        assert!(ciphertext.len() > NONCE_SIZE);

        let decrypted = decrypt(&ciphertext, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_different_plaintexts_produce_different_ciphertexts() {
        let key = test_key();
        let ct1 = encrypt("hello", &key).unwrap();
        let ct2 = encrypt("hello", &key).unwrap();
        // Random nonce means same plaintext → different ciphertext
        assert_ne!(ct1, ct2);
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let key = test_key();
        let mut ciphertext = encrypt("secret", &key).unwrap();
        // Tamper with a byte in the ciphertext portion
        let idx = NONCE_SIZE + 2;
        ciphertext[idx] ^= 0xFF;

        assert!(matches!(
            decrypt(&ciphertext, &key),
            Err(CryptoError::DecryptFailed)
        ));
    }

    #[test]
    fn test_wrong_key_fails() {
        let key = test_key();
        let ciphertext = encrypt("secret", &key).unwrap();

        let wrong_key = [1u8; 32];
        assert!(matches!(
            decrypt(&ciphertext, &wrong_key),
            Err(CryptoError::DecryptFailed)
        ));
    }

    #[test]
    fn test_parse_master_key_valid() {
        let hex = "a".repeat(64);
        let key = parse_master_key(&hex).unwrap();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn test_parse_master_key_invalid_length() {
        let hex = "a".repeat(32); // 16 bytes, not 32
        assert!(matches!(
            parse_master_key(&hex),
            Err(CryptoError::InvalidKeyLength)
        ));
    }

    #[test]
    fn test_parse_master_key_invalid_hex() {
        assert!(matches!(
            parse_master_key("not-hex!!!"),
            Err(CryptoError::InvalidHex)
        ));
    }

    #[test]
    fn test_empty_plaintext() {
        let key = test_key();
        let ciphertext = encrypt("", &key).unwrap();
        let decrypted = decrypt(&ciphertext, &key).unwrap();
        assert_eq!(decrypted, "");
    }
}
