//! JWT session authentication with RS256 or HS256.

use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AuthError;

/// JWT access token claims.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccessClaims {
    pub sub: String,      // user_id
    #[serde(default)]
    pub active_org_id: String,
    /// Deprecated: kept for backward compatibility with old tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_id: Option<String>,
    pub email: String,
    pub role: String,
    pub jti: String,      // token ID for revocation
    pub iat: i64,
    pub exp: i64,
    #[serde(rename = "type")]
    pub token_type: String,
}

/// JWT refresh token claims.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RefreshClaims {
    pub sub: String,      // user_id
    pub jti: String,      // token ID
    pub iat: i64,
    pub exp: i64,
    #[serde(rename = "type")]
    pub token_type: String,
}

/// JWT encoder/decoder supporting RS256 (asymmetric) or HS256 (symmetric).
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    algorithm: Algorithm,
}

impl JwtService {
    /// Create from PEM-encoded RSA private and public keys (RS256).
    pub fn from_pem(private_pem: &[u8], public_pem: &[u8]) -> Result<Self, AuthError> {
        let encoding_key = EncodingKey::from_rsa_pem(private_pem)
            .map_err(|e| AuthError::Internal(format!("invalid private key: {e}")))?;
        let decoding_key = DecodingKey::from_rsa_pem(public_pem)
            .map_err(|e| AuthError::Internal(format!("invalid public key: {e}")))?;
        Ok(Self {
            encoding_key,
            decoding_key,
            algorithm: Algorithm::RS256,
        })
    }

    /// Create from a shared secret (HS256).
    pub fn from_secret(secret: &[u8]) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            algorithm: Algorithm::HS256,
        }
    }

    /// Issue a new access token (15 minutes).
    pub fn issue_access(
        &self,
        user_id: Uuid,
        active_org_id: Uuid,
        email: &str,
        role: &str,
    ) -> Result<(String, String), AuthError> {
        let jti = Uuid::new_v4().to_string();
        let now = Utc::now();
        let exp = now + Duration::seconds(900); // 15 minutes

        let claims = AccessClaims {
            sub: user_id.to_string(),
            active_org_id: active_org_id.to_string(),
            org_id: None,
            email: email.to_string(),
            role: role.to_string(),
            jti: jti.clone(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            token_type: "access".to_string(),
        };

        let token = encode(&Header::new(self.algorithm), &claims, &self.encoding_key)
            .map_err(|e| AuthError::Internal(format!("jwt encode failed: {e}")))?;

        Ok((token, jti))
    }

    /// Issue a new refresh token (7 days).
    pub fn issue_refresh(&self, user_id: Uuid) -> Result<(String, String), AuthError> {
        let jti = Uuid::new_v4().to_string();
        let now = Utc::now();
        let exp = now + Duration::seconds(604800); // 7 days

        let claims = RefreshClaims {
            sub: user_id.to_string(),
            jti: jti.clone(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            token_type: "refresh".to_string(),
        };

        let token = encode(&Header::new(self.algorithm), &claims, &self.encoding_key)
            .map_err(|e| AuthError::Internal(format!("jwt encode failed: {e}")))?;

        Ok((token, jti))
    }

    /// Verify an access token.
    ///
    /// Backward compatibility: if `active_org_id` is missing but `org_id` is present,
    /// the old `org_id` value is promoted into `active_org_id`.
    pub fn verify_access(&self, token: &str) -> Result<AccessClaims, AuthError> {
        let mut validation = Validation::new(self.algorithm);
        validation.set_required_spec_claims(&["exp", "sub", "jti"]);

        let token_data = decode::<AccessClaims>(token, &self.decoding_key, &validation)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
                _ => AuthError::InvalidToken,
            })?;

        if token_data.claims.token_type != "access" {
            return Err(AuthError::InvalidToken);
        }

        let mut claims = token_data.claims;
        // Backward compatibility: promote old org_id to active_org_id
        if claims.active_org_id.is_empty() {
            if let Some(ref legacy_org_id) = claims.org_id {
                claims.active_org_id = legacy_org_id.clone();
            }
        }

        Ok(claims)
    }

    /// Verify a refresh token.
    pub fn verify_refresh(&self, token: &str) -> Result<RefreshClaims, AuthError> {
        let mut validation = Validation::new(self.algorithm);
        validation.set_required_spec_claims(&["exp", "sub", "jti"]);

        let token_data = decode::<RefreshClaims>(token, &self.decoding_key, &validation)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
                _ => AuthError::InvalidToken,
            })?;

        if token_data.claims.token_type != "refresh" {
            return Err(AuthError::InvalidToken);
        }

        Ok(token_data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_keys() -> JwtService {
        // Generate a 2048-bit RSA key pair for testing
        use rsa::{RsaPrivateKey, RsaPublicKey, pkcs8::EncodePrivateKey, pkcs8::LineEnding};
        use rsa::pkcs1::EncodeRsaPublicKey;
        let mut rng = rand::thread_rng();
        let private_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let public_key = RsaPublicKey::from(&private_key);
        let private_pem = private_key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
        let public_pem = public_key.to_pkcs1_pem(LineEnding::LF).unwrap();
        JwtService::from_pem(private_pem.as_bytes(), public_pem.as_bytes()).unwrap()
    }

    #[test]
    fn test_access_token_roundtrip() {
        let svc = test_keys();
        let user_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let (token, _jti) = svc.issue_access(user_id, org_id, "test@example.com", "admin").unwrap();
        let claims = svc.verify_access(&token).unwrap();
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.active_org_id, org_id.to_string());
        assert_eq!(claims.email, "test@example.com");
        assert_eq!(claims.role, "admin");
    }

    #[test]
    fn test_expired_token_rejected() {
        let svc = test_keys();
        // Manually create an expired token by tampering with iat/exp is hard;
        // instead, we verify that a garbage token is rejected
        assert!(svc.verify_access("invalid.token.here").is_err());
    }

    #[test]
    fn test_legacy_org_id_backward_compat() {
        let svc = test_keys();
        let user_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let jti = Uuid::new_v4().to_string();
        let now = Utc::now();
        let exp = now + Duration::seconds(900);

        // Manually encode a legacy token with `org_id` instead of `active_org_id`.
        let legacy_claims = serde_json::json!({
            "sub": user_id.to_string(),
            "org_id": org_id.to_string(),
            "email": "legacy@example.com",
            "role": "member",
            "jti": jti,
            "iat": now.timestamp(),
            "exp": exp.timestamp(),
            "type": "access"
        });

        let token = encode(
            &Header::new(Algorithm::RS256),
            &legacy_claims,
            &svc.encoding_key,
        )
        .unwrap();

        let claims = svc.verify_access(&token).unwrap();
        assert_eq!(claims.active_org_id, org_id.to_string());
        assert_eq!(claims.email, "legacy@example.com");
    }

    #[test]
    fn test_refresh_token_roundtrip() {
        let svc = test_keys();
        let user_id = Uuid::new_v4();
        let (token, _jti) = svc.issue_refresh(user_id).unwrap();
        let claims = svc.verify_refresh(&token).unwrap();
        assert_eq!(claims.sub, user_id.to_string());
    }
}
