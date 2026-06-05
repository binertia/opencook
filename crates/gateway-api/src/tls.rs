//! TLS configuration for the gateway API.
//!
//! Loads PEM-encoded certificate and key files and builds a Rustls
//! `ServerConfig` that requires TLS 1.2+ and prefers TLS 1.3.

use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer},
    ServerConfig,
};
use std::{
    fs::File,
    io::{self, BufReader},
    path::Path,
    sync::Arc,
};

/// TLS configuration holder.
#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
}

impl TlsConfig {
    pub fn from_env(cert: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            cert_path: cert.into(),
            key_path: key.into(),
        }
    }

    /// Build a Rustls `ServerConfig` from the certificate and key files.
    pub fn to_server_config(&self) -> Result<Arc<ServerConfig>, TlsError> {
        let certs = load_certs(&self.cert_path)?;
        let key = load_private_key(&self.key_path)?;

        let cfg = ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
            .map_err(|e| TlsError::InvalidCertificate(rustls::Error::General(e.to_string())))?
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(TlsError::InvalidCertificate)?;

        Ok(Arc::new(cfg))
    }
}

/// Errors that can occur while loading TLS materials.
#[derive(Debug, thiserror::Error)]
pub enum TlsError {
    #[error("failed to read TLS file: {0}")]
    Io(#[from] io::Error),
    #[error("no private key found in {0}")]
    MissingKey(String),
    #[error("invalid certificate or key: {0}")]
    InvalidCertificate(rustls::Error),
}

fn load_certs(path: impl AsRef<Path>) -> Result<Vec<CertificateDer<'static>>, TlsError> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| TlsError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;
    Ok(certs)
}

fn load_private_key(path: impl AsRef<Path>) -> Result<PrivateKeyDer<'static>, TlsError> {
    let file = File::open(&path)?;
    let mut reader = BufReader::new(file);

    // Try PKCS#8 first, then RSA traditional.
    if let Some(key) = rustls_pemfile::pkcs8_private_keys(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| TlsError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?
        .into_iter()
        .next()
    {
        return Ok(key.into());
    }

    let file = File::open(&path)?;
    let mut reader = BufReader::new(file);
    if let Some(key) = rustls_pemfile::rsa_private_keys(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| TlsError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?
        .into_iter()
        .next()
    {
        return Ok(key.into());
    }

    Err(TlsError::MissingKey(
        path.as_ref().display().to_string(),
    ))
}
