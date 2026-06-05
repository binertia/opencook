//! AES-256-GCM encryption for sensitive data at rest.
//!
//! Used to encrypt provider API keys before storing in PostgreSQL.
//! Master key is loaded from `GATEWAY_MASTER_KEY` env var (32 bytes, hex-encoded).

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;

/// HMAC-SHA256 type alias.
type HmacSha256 = Hmac<Sha256>;

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

/// Encrypt with the primary key of an `ActiveKeyPair`.
pub fn encrypt_with_keys(plaintext: &str, keys: &crate::key_rotation::ActiveKeyPair<[u8; 32]>) -> Result<Vec<u8>, CryptoError> {
    encrypt(plaintext, &keys.primary)
}

/// Decrypt by trying the primary key first, then the secondary (grace period).
pub fn decrypt_with_keys(
    ciphertext: &[u8],
    keys: &crate::key_rotation::ActiveKeyPair<[u8; 32]>,
) -> Result<String, CryptoError> {
    match decrypt(ciphertext, &keys.primary) {
        Ok(plaintext) => Ok(plaintext),
        Err(CryptoError::DecryptFailed) => {
            if let Some(ref secondary) = keys.secondary {
                decrypt(ciphertext, secondary)
            } else {
                Err(CryptoError::DecryptFailed)
            }
        }
        Err(e) => Err(e),
    }
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

/// Generate an HMAC-SHA256 signature for a webhook payload.
///
/// Returns a lowercase hex-encoded signature string.
/// The signature format is compatible with Stripe-style webhook signatures.
pub fn hmac_sha256_hex(secret: &str, payload: &[u8]) -> Result<String, CryptoError> {
    let mut mac = <HmacSha256 as hmac::Mac>::new_from_slice(secret.as_bytes())
        .map_err(|_| CryptoError::InvalidKeyLength)?;
    mac.update(payload);
    let result = mac.finalize();
    let bytes = result.into_bytes();
    Ok(hex::encode(bytes))
}

/// Verify an HMAC-SHA256 signature for a webhook payload.
///
/// `expected_signature` should be the lowercase hex-encoded signature.
pub fn verify_hmac_sha256(secret: &str, payload: &[u8], expected_signature: &str) -> Result<bool, CryptoError> {
    let computed = hmac_sha256_hex(secret, payload)?;
    use subtle::ConstantTimeEq;
    Ok(computed.as_bytes().ct_eq(expected_signature.as_bytes()).into())
}

/// Generate a random webhook signing secret (32 bytes, hex-encoded).
pub fn generate_webhook_secret() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
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

    #[test]
    fn test_hmac_sha256_hex() {
        let secret = "whsec_test_secret";
        let payload = b"{\"event\":\"request.completed\"}";
        let sig = hmac_sha256_hex(secret, payload).unwrap();
        assert_eq!(sig.len(), 64); // 32 bytes = 64 hex chars
        assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_hmac_signature_is_deterministic() {
        let secret = "whsec_test_secret";
        let payload = b"test payload";
        let sig1 = hmac_sha256_hex(secret, payload).unwrap();
        let sig2 = hmac_sha256_hex(secret, payload).unwrap();
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_verify_hmac_sha256_valid() {
        let secret = "whsec_test_secret";
        let payload = b"test payload";
        let sig = hmac_sha256_hex(secret, payload).unwrap();
        assert!(verify_hmac_sha256(secret, payload, &sig).unwrap());
    }

    #[test]
    fn test_verify_hmac_sha256_invalid() {
        let secret = "whsec_test_secret";
        let payload = b"test payload";
        assert!(!verify_hmac_sha256(secret, payload, "invalid_sig").unwrap());
    }

    #[test]
    fn test_generate_webhook_secret() {
        let secret1 = generate_webhook_secret();
        let secret2 = generate_webhook_secret();
        assert_eq!(secret1.len(), 64); // 32 bytes hex-encoded
        assert_ne!(secret1, secret2); // Should be random
    }

    #[test]
    fn test_hmac_empty_payload() {
        let secret = "whsec_test";
        let sig = hmac_sha256_hex(secret, b"").unwrap();
        assert_eq!(sig.len(), 64);
        assert!(verify_hmac_sha256(secret, b"", &sig).unwrap());
    }

    #[test]
    fn test_hmac_unicode_payload() {
        let secret = "whsec_日本語";
        let payload = "イベント: 完了 🎉".as_bytes();
        let sig = hmac_sha256_hex(secret, payload).unwrap();
        assert!(verify_hmac_sha256(secret, payload, &sig).unwrap());
    }

    #[test]
    fn test_hmac_large_payload() {
        let secret = "whsec_test";
        let payload = vec![b'x'; 1_000_000];
        let sig = hmac_sha256_hex(secret, &payload).unwrap();
        assert!(verify_hmac_sha256(secret, &payload, &sig).unwrap());
    }

    #[test]
    fn test_hmac_different_secrets_produce_different_sigs() {
        let payload = b"same payload";
        let sig1 = hmac_sha256_hex("secret_a", payload).unwrap();
        let sig2 = hmac_sha256_hex("secret_b", payload).unwrap();
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_hmac_different_payloads_produce_different_sigs() {
        let secret = "whsec_test";
        let sig1 = hmac_sha256_hex(secret, b"payload_a").unwrap();
        let sig2 = hmac_sha256_hex(secret, b"payload_b").unwrap();
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_verify_hmac_case_sensitive() {
        let secret = "whsec_test";
        let payload = b"test";
        let sig = hmac_sha256_hex(secret, payload).unwrap();
        let upper_sig = sig.to_uppercase();
        assert!(!verify_hmac_sha256(secret, payload, &upper_sig).unwrap());
    }

    #[test]
    fn test_verify_hmac_single_bit_flip() {
        let secret = "whsec_test";
        let payload = b"test";
        let sig = hmac_sha256_hex(secret, payload).unwrap();
        let mut flipped = sig.clone();
        let last = flipped.pop().unwrap();
        let flipped_last = if last == 'a' { 'b' } else { 'a' };
        flipped.push(flipped_last);
        assert!(!verify_hmac_sha256(secret, payload, &flipped).unwrap());
    }
}
