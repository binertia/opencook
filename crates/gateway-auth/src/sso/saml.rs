//! SAML 2.0 Service Provider implementation.

use tracing::debug;
use uuid::Uuid;

use super::{SsoAuthResult, SsoError, SsoProviderType};

/// SAML Service Provider state.
#[derive(Clone)]
pub struct SamlProvider {
    pub entity_id: String,
    pub acs_url: String,
    pub idp_sso_url: String,
    pub idp_certificate: Option<String>,
    pub role_attribute: String,
}

impl SamlProvider {
    pub fn new(
        entity_id: String,
        acs_url: String,
        idp_sso_url: String,
        idp_certificate: Option<String>,
        role_attribute: String,
    ) -> Self {
        Self {
            entity_id,
            acs_url,
            idp_sso_url,
            idp_certificate,
            role_attribute,
        }
    }

    /// Generate a SAML AuthnRequest redirect URL.
    pub fn authn_request_url(&self, relay_state: &str) -> Result<String, SsoError> {
        // Build a minimal SAML AuthnRequest XML
        let request_id = format!("id-{}", Uuid::new_v4());
        let instant = chrono::Utc::now().to_rfc3339();

        let authn_request = format!(
            r#"<samlp:AuthnRequest xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol"
                xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion"
                ID="{request_id}"
                Version="2.0"
                IssueInstant="{instant}"
                Destination="{}"
                AssertionConsumerServiceURL="{}">
                <saml:Issuer>{entity_id}</saml:Issuer>
                <samlp:NameIDPolicy Format="urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress" AllowCreate="true"/>
            </samlp:AuthnRequest>"#,
            self.idp_sso_url,
            self.acs_url,
            entity_id = self.entity_id,
        );

        // Deflate + base64 encode the request
        use flate2::write::DeflateEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(authn_request.as_bytes())
            .map_err(|e| SsoError::Saml(format!("deflate failed: {e}")))?;
        let deflated = encoder
            .finish()
            .map_err(|e| SsoError::Saml(format!("deflate finish failed: {e}")))?;
        let encoded = base64::encode(&deflated);

        let url = format!(
            "{}?SAMLRequest={}&RelayState={}",
            self.idp_sso_url,
            urlencoding::encode(&encoded),
            urlencoding::encode(relay_state),
        );

        debug!(url = %url, "Generated SAML AuthnRequest URL");
        Ok(url)
    }

    /// Parse a SAML Response and extract user identity.
    pub fn parse_response(&self, saml_response_b64: &str) -> Result<SsoAuthResult, SsoError> {
        let decoded = base64::decode(saml_response_b64)
            .map_err(|e| SsoError::Saml(format!("base64 decode failed: {e}")))?;
        let xml = String::from_utf8(decoded)
            .map_err(|e| SsoError::Saml(format!("utf8 decode failed: {e}")))?;

        // Parse NameID (email) from the Assertion
        let name_id = extract_name_id(&xml)
            .ok_or_else(|| SsoError::Saml("Missing NameID in SAML response".to_string()))?;

        let role = extract_attribute(&xml, &self.role_attribute);

        debug!(email = %name_id, role = ?role, "Parsed SAML response");

        Ok(SsoAuthResult {
            email: name_id,
            name: None,
            role,
            provider_type: SsoProviderType::Saml,
        })
    }
}

fn extract_name_id(xml: &str) -> Option<String> {
    // Simple regex-like extraction for NameID
    let start = xml.find("<saml:NameID")?;
    let tag_end = xml[start..].find('>')? + start + 1;
    let close = xml[tag_end..].find("</saml:NameID>")? + tag_end;
    let value = xml[tag_end..close].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn extract_attribute(xml: &str, name: &str) -> Option<String> {
    // Look for saml:Attribute with the given Name
    let search = format!(r#"Name="{}""#, name);
    let start = xml.find(&search)?;
    let tag_end = xml[start..].find('>')? + start + 1;
    let close = xml[tag_end..].find("</saml:Attribute>")? + tag_end;
    let inner = &xml[tag_end..close];
    let value_start = inner.find("<saml:AttributeValue")?;
    let value_tag_end = inner[value_start..].find('>')? + value_start + 1;
    let value_close = inner[value_tag_end..].find("</saml:AttributeValue>")? + value_tag_end;
    let value = inner[value_tag_end..value_close].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

// Base64 helpers using base64 crate
mod base64 {
    pub fn encode(input: &[u8]) -> String {
        use base64::{engine::general_purpose::STANDARD, Engine};
        STANDARD.encode(input)
    }
    pub fn decode(input: &str) -> Result<Vec<u8>, base64::DecodeError> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        STANDARD.decode(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_name_id() {
        let xml = r#"<saml:Assertion><saml:Subject><saml:NameID Format="email">user@example.com</saml:NameID></saml:Subject></saml:Assertion>"#;
        assert_eq!(extract_name_id(xml), Some("user@example.com".to_string()));
    }

    #[test]
    fn test_extract_attribute() {
        let xml = r#"<saml:Attribute Name="role"><saml:AttributeValue>admin</saml:AttributeValue></saml:Attribute>"#;
        assert_eq!(extract_attribute(xml, "role"), Some("admin".to_string()));
    }
}
