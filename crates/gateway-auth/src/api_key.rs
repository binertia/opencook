//! API key generation, hashing, and validation.

use rand::RngCore;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Generate a new API key.
/// Format: `sk_gw_{base58_random_32}{base58_checksum_6}` = 44 chars total.
/// Returns (full_key, key_hash, key_prefix).
pub fn generate_api_key() -> (String, String, String) {
    let mut random_bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut random_bytes);

    let random_b58 = bs58::encode(&random_bytes).into_string();
    let random_part = &random_b58[..32.min(random_b58.len())];

    // Compute checksum on the base58 random part (not raw bytes) so truncation is consistent
    let checksum_raw = crc32c::crc32c(random_part.as_bytes());
    let checksum_b58 = bs58::encode(&checksum_raw.to_le_bytes()).into_string();
    let checksum_slice = &checksum_b58[..6.min(checksum_b58.len())];
    // Pad with '1' (base58 zero) to ensure exactly 6 chars
    let checksum_part = format!("{}{}", "1".repeat(6 - checksum_slice.len()), checksum_slice);
    let full_key = format!("sk_gw_{random_part}{checksum_part}");
    let key_hash = sha256_hex(&full_key);
    let key_prefix = full_key.chars().take(8).collect();

    (full_key, key_hash, key_prefix)
}

/// Validate API key format and checksum.
pub fn validate_key_format(key: &str) -> bool {
    if !key.starts_with("sk_gw_") {
        return false;
    }
    let payload = &key[6..];
    if payload.len() != 38 {
        return false;
    }
    let random_part = &payload[..32];
    let checksum_part = &payload[32..38];

    // Compute checksum on the base58 random part string (same as generation)
    let checksum_raw = crc32c::crc32c(random_part.as_bytes());
    let checksum_b58 = bs58::encode(&checksum_raw.to_le_bytes()).into_string();
    let checksum_slice = &checksum_b58[..6.min(checksum_b58.len())];
    let expected_checksum = format!("{}{}", "1".repeat(6 - checksum_slice.len()), checksum_slice);

    checksum_part == expected_checksum
}

/// Compute SHA-256 hex digest.
pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Verify a full API key against a stored hash using constant-time comparison.
pub fn verify_key_hash(key: &str, expected_hash: &str) -> bool {
    let computed = sha256_hex(key);
    computed.as_bytes().ct_eq(expected_hash.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_format() {
        let (full_key, _hash, _prefix) = generate_api_key();
        assert!(full_key.starts_with("sk_gw_"));
        assert_eq!(full_key.len(), 44);
        assert!(validate_key_format(&full_key));
    }

    #[test]
    fn test_invalid_key_format() {
        assert!(!validate_key_format("invalid"));
        assert!(!validate_key_format("sk_gw_123"));
    }

    #[test]
    fn test_hash_verification() {
        let (full_key, hash, _prefix) = generate_api_key();
        assert!(verify_key_hash(&full_key, &hash));
        assert!(!verify_key_hash("wrong_key", &hash));
    }

    #[test]
    fn test_prefix_length() {
        let (_key, _hash, prefix) = generate_api_key();
        assert_eq!(prefix.len(), 8);
    }
}
