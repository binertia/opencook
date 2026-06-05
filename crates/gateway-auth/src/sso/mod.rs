//! SSO module — SAML 2.0 and OIDC integration.

pub mod oidc;
pub mod saml;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// SSO provider type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SsoProviderType {
    Saml,
    Oidc,
}

/// SSO configuration for an organization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoConfig {
    pub id: Uuid,
    pub org_id: Uuid,
    pub provider_type: SsoProviderType,
    pub metadata_url: Option<String>,
    pub entity_id: Option<String>,
    pub certificate: Option<String>,
    pub sso_url: Option<String>,
    pub client_id: Option<String>,
    pub client_secret_enc: Option<String>,
    pub idp_issuer: Option<String>,
    pub role_attribute: String,
    pub enabled: bool,
}

/// Result of an SSO authentication attempt.
#[derive(Debug, Clone)]
pub struct SsoAuthResult {
    pub email: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub provider_type: SsoProviderType,
}

/// Errors that can occur during SSO operations.
#[derive(Debug, thiserror::Error)]
pub enum SsoError {
    #[error("SAML error: {0}")]
    Saml(String),
    #[error("OIDC error: {0}")]
    Oidc(String),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}
