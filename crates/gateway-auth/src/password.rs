//! Argon2id password hashing and validation.

use crate::error::AuthError;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2, Params,
};

/// Argon2id hasher with OWASP-recommended parameters.
pub struct PasswordHasherService {
    argon2: Argon2<'static>,
}

impl Default for PasswordHasherService {
    fn default() -> Self {
        Self::new()
    }
}

impl PasswordHasherService {
    /// Create a new hasher with secure parameters.
    /// time_cost=3, memory_cost=65536 (64MB), parallelism=4
    pub fn new() -> Self {
        let params = Params::new(65536, 3, 4, None).expect("valid argon2 params");
        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
        Self { argon2 }
    }

    /// Hash a plaintext password. Returns the PHC string.
    pub fn hash_password(&self, password: &str) -> Result<String, AuthError> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = self
            .argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| AuthError::Internal(format!("argon2 hash failed: {e}")))?;
        Ok(hash.to_string())
    }

    /// Verify a plaintext password against a stored PHC hash.
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<(), AuthError> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| AuthError::Internal(format!("invalid hash format: {e}")))?;
        self.argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| AuthError::InvalidCredentials)
    }

    /// Check if a password needs rehashing (e.g., parameters changed).
    pub fn needs_rehash(&self, hash: &str) -> Result<bool, AuthError> {
        let _parsed_hash = PasswordHash::new(hash)
            .map_err(|e| AuthError::Internal(format!("invalid hash format: {e}")))?;
        // Simple heuristic: if the hash string doesn't contain our expected params, rehash
        Ok(!hash.contains("m=65536") || !hash.contains("t=3") || !hash.contains("p=4"))
    }
}

/// Validate password strength.
/// Enforces: min 12 chars, max 128, at least 1 uppercase, 1 lowercase, 1 digit, 1 special char.
pub fn validate_password_strength(password: &str) -> Result<(), AuthError> {
    let len = password.len();
    if len < 12 {
        return Err(AuthError::InvalidPasswordFormat(
            "Password must be at least 12 characters".into(),
        ));
    }
    if len > 128 {
        return Err(AuthError::InvalidPasswordFormat(
            "Password must be at most 128 characters".into(),
        ));
    }
    let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
    let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password.chars().any(|c| !c.is_alphanumeric());

    if !has_upper {
        return Err(AuthError::InvalidPasswordFormat(
            "Password must contain at least one uppercase letter".into(),
        ));
    }
    if !has_lower {
        return Err(AuthError::InvalidPasswordFormat(
            "Password must contain at least one lowercase letter".into(),
        ));
    }
    if !has_digit {
        return Err(AuthError::InvalidPasswordFormat(
            "Password must contain at least one digit".into(),
        ));
    }
    if !has_special {
        return Err(AuthError::InvalidPasswordFormat(
            "Password must contain at least one special character".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_verify_roundtrip() {
        let hasher = PasswordHasherService::new();
        let password = "MyStr0ng!Pass";
        let hash = hasher.hash_password(password).unwrap();
        assert!(hasher.verify_password(password, &hash).is_ok());
    }

    #[test]
    fn test_verify_wrong_password_fails() {
        let hasher = PasswordHasherService::new();
        let hash = hasher.hash_password("correct_password!123").unwrap();
        assert!(hasher.verify_password("wrong_password!456", &hash).is_err());
    }

    #[test]
    fn test_password_too_short() {
        assert!(validate_password_strength("short1!").is_err());
    }

    #[test]
    fn test_password_missing_uppercase() {
        assert!(validate_password_strength("lowercase1!").is_err());
    }

    #[test]
    fn test_password_missing_digit() {
        assert!(validate_password_strength("NoDigitsHere!").is_err());
    }

    #[test]
    fn test_password_missing_special() {
        assert!(validate_password_strength("NoSpecial123").is_err());
    }

    #[test]
    fn test_valid_password() {
        assert!(validate_password_strength("ValidP@ssw0rd!").is_ok());
    }
}
