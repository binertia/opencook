//! Key rotation support — active/previous key pairs for zero-downtime rotation.
//!
//! # Encryption rotation
//! `GATEWAY_MASTER_KEY` supports comma-separated keys: `new_key,old_key`
//! - First key: active (for encryption)
//! - Second key: previous (for decryption of old data)
//!
//! # JWT rotation
//! `GATEWAY_JWT_PRIVATE_KEY` and `GATEWAY_JWT_PUBLIC_KEY` support comma-separated PEMs.
//! - First PEM: active (for signing)
//! - Second PEM: previous (for verifying old tokens)

use crate::crypto::CryptoError;
use crate::error::AuthError;

/// A pair of keys: primary (active) and optional secondary (previous/grace).
///
/// Primary is always used for signing/encryption.
/// Secondary is used for verification/decryption during the grace period.
#[derive(Debug, Clone)]
pub struct ActiveKeyPair<T> {
    pub primary: T,
    pub secondary: Option<T>,
}

impl<T> ActiveKeyPair<T> {
    /// Create a new key pair with only a primary key.
    pub fn new(primary: T) -> Self {
        Self {
            primary,
            secondary: None,
        }
    }

    /// Create a new key pair with primary and secondary keys.
    pub fn with_secondary(primary: T, secondary: T) -> Self {
        Self {
            primary,
            secondary: Some(secondary),
        }
    }

    /// Try an operation with primary key first, then secondary on failure.
    pub fn try_both<F, R, E>(&self, mut op: F) -> Result<R, E>
    where
        F: FnMut(&T) -> Result<R, E>,
    {
        op(&self.primary).or_else(|e| {
            if let Some(ref secondary) = self.secondary {
                op(secondary)
            } else {
                Err(e)
            }
        })
    }

    /// Try an operation with primary key first, then secondary on failure.
    /// This async version takes a closure returning a future.
    /// Requires `T: Clone` so keys can be passed by value into the async closure.
    pub async fn try_both_async<F, Fut, R, E>(&self, mut op: F) -> Result<R, E>
    where
        T: Clone,
        F: FnMut(T) -> Fut,
        Fut: std::future::Future<Output = Result<R, E>>,
    {
        match op(self.primary.clone()).await {
            Ok(r) => Ok(r),
            Err(_) => {
                if let Some(ref secondary) = self.secondary {
                    op(secondary.clone()).await
                } else {
                    op(self.primary.clone()).await
                }
            }
        }
    }
}

/// Parse a comma-separated list of hex-encoded master keys.
/// Returns an `ActiveKeyPair<[u8; 32]>` with the first key as primary.
pub fn parse_master_key_pair(hex_str: &str) -> Result<ActiveKeyPair<[u8; 32]>, CryptoError> {
    let parts: Vec<&str> = hex_str.split(',').map(|s| s.trim()).collect();
    let primary = crate::crypto::parse_master_key(parts[0])?;

    let secondary = if parts.len() > 1 {
        Some(crate::crypto::parse_master_key(parts[1])?)
    } else {
        None
    };

    Ok(ActiveKeyPair { primary, secondary })
}

/// Parse comma-separated PEM-encoded key pairs.
/// Returns an `ActiveKeyPair<Vec<u8>>` with the first PEM as primary.
pub fn parse_pem_pair(pem_str: &str) -> Result<ActiveKeyPair<Vec<u8>>, AuthError> {
    let parts: Vec<&str> = pem_str.split("-----END").collect();

    // If there's only one PEM, return it as primary
    if parts.len() <= 2 || pem_str.matches("-----BEGIN").count() <= 1 {
        Ok(ActiveKeyPair::new(pem_str.as_bytes().to_vec()))
    } else {
        // Split on the boundary between two PEM blocks
        let boundary = pem_str.find("-----BEGIN").unwrap_or(0);
        let second_begin = pem_str[boundary + 1..]
            .find("-----BEGIN")
            .map(|i| boundary + 1 + i);

        if let Some(second_pos) = second_begin {
            let first_pem = pem_str[..second_pos].trim().as_bytes().to_vec();
            let second_pem = pem_str[second_pos..].trim().as_bytes().to_vec();
            Ok(ActiveKeyPair::with_secondary(first_pem, second_pem))
        } else {
            Ok(ActiveKeyPair::new(pem_str.as_bytes().to_vec()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_master_key() {
        let hex = "a".repeat(64);
        let pair = parse_master_key_pair(&hex).unwrap();
        assert!(pair.secondary.is_none());
        assert_eq!(pair.primary.len(), 32);
    }

    #[test]
    fn test_master_key_pair() {
        let hex1 = "00".repeat(32);
        let hex2 = "ff".repeat(32);
        let pair = parse_master_key_pair(&format!("{}, {}", hex1, hex2)).unwrap();
        assert!(pair.secondary.is_some());
        assert_eq!(pair.primary, [0u8; 32]);
        assert_eq!(pair.secondary.unwrap(), [255u8; 32]);
    }

    #[test]
    fn test_try_both_primary_succeeds() {
        let pair = ActiveKeyPair::new(42);
        let result = pair.try_both(|k| {
            if *k == 42 {
                Ok("primary")
            } else {
                Err("wrong")
            }
        });
        assert_eq!(result.unwrap(), "primary");
    }

    #[test]
    fn test_try_both_fallback_to_secondary() {
        let pair = ActiveKeyPair::with_secondary(0, 42);
        let result = pair.try_both(|k| {
            if *k == 42 {
                Ok("secondary")
            } else {
                Err("wrong")
            }
        });
        assert_eq!(result.unwrap(), "secondary");
    }

    #[test]
    fn test_try_both_no_secondary_fails() {
        let pair = ActiveKeyPair::<i32>::new(0);
        let result = pair.try_both(|k| {
            if *k == 42 {
                Ok("success")
            } else {
                Err("failure")
            }
        });
        assert_eq!(result.unwrap_err(), "failure");
    }

    #[tokio::test]
    async fn test_try_both_async_fallback() {
        let pair = ActiveKeyPair::with_secondary(0, 42);
        let result = pair
            .try_both_async(|k| async move {
                if k == 42 {
                    Ok("secondary")
                } else {
                    Err("wrong")
                }
            })
            .await;
        assert_eq!(result.unwrap(), "secondary");
    }
}
