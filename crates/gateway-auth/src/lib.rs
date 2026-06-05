//! Gateway Auth — API key validation, JWT sessions, RBAC, tenant isolation.

pub mod api_key;
pub mod crypto;
pub mod email;
pub mod error;
pub mod jwt;
pub mod key_rotation;
pub mod models;
pub mod password;
pub mod rbac;
pub mod tenant;

pub use api_key::{generate_api_key, sha256_hex, validate_key_format, verify_key_hash};
pub use email::{EmailConfig, EmailError, EmailService};
pub use error::AuthError;
pub use jwt::{AccessClaims, JwtService, RefreshClaims};
pub use key_rotation::{parse_master_key_pair, parse_pem_pair, ActiveKeyPair};
pub use models::*;
pub use password::{validate_password_strength, PasswordHasherService};
pub use rbac::{check_permission, permissions_for_role, Permission, Role};
pub use tenant::{require_same_org, tenant_isolation_middleware};
