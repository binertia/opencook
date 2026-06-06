//! OpenID Connect client implementation.

use tracing::debug;

use super::{SsoAuthResult, SsoError, SsoProviderType};

/// OIDC client configuration.
#[derive(Clone)]
pub struct OidcProvider {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub issuer: String,
    pub role_attribute: String,
}

impl OidcProvider {
    pub fn new(
        client_id: String,
        client_secret: String,
        redirect_uri: String,
        authorization_endpoint: String,
        token_endpoint: String,
        userinfo_endpoint: String,
        issuer: String,
        role_attribute: String,
    ) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_uri,
            authorization_endpoint,
            token_endpoint,
            userinfo_endpoint,
            issuer,
            role_attribute,
        }
    }

    /// Generate the authorization URL for the OIDC flow.
    pub fn authorization_url(&self, state: &str, nonce: &str) -> String {
        let url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email%20profile&state={}&nonce={}",
            self.authorization_endpoint,
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(state),
            urlencoding::encode(nonce),
        );
        debug!(url = %url, "Generated OIDC authorization URL");
        url
    }

    /// Exchange authorization code for tokens and fetch user info.
    pub async fn exchange_code(&self, code: &str, _nonce: &str) -> Result<SsoAuthResult, SsoError> {
        let client = reqwest::Client::new();

        // Token exchange
        let token_resp = client
            .post(&self.token_endpoint)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", &self.redirect_uri),
                ("client_id", &self.client_id),
                ("client_secret", &self.client_secret),
            ])
            .send()
            .await
            .map_err(|e| SsoError::Oidc(format!("token request failed: {e}")))?;

        if !token_resp.status().is_success() {
            let status = token_resp.status();
            let text = token_resp.text().await.unwrap_or_default();
            return Err(SsoError::Oidc(format!(
                "token endpoint error {status}: {text}"
            )));
        }

        let token_json: serde_json::Value = token_resp
            .json()
            .await
            .map_err(|e| SsoError::Oidc(format!("token JSON parse failed: {e}")))?;

        let access_token = token_json
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SsoError::Oidc("missing access_token".to_string()))?;

        // Fetch userinfo
        let userinfo_resp = client
            .get(&self.userinfo_endpoint)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| SsoError::Oidc(format!("userinfo request failed: {e}")))?;

        if !userinfo_resp.status().is_success() {
            let status = userinfo_resp.status();
            let text = userinfo_resp.text().await.unwrap_or_default();
            return Err(SsoError::Oidc(format!("userinfo error {status}: {text}")));
        }

        let userinfo: serde_json::Value = userinfo_resp
            .json()
            .await
            .map_err(|e| SsoError::Oidc(format!("userinfo JSON parse failed: {e}")))?;

        let email = userinfo
            .get("email")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SsoError::Oidc("missing email in userinfo".to_string()))?;

        let name = userinfo
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let role = userinfo
            .get(&self.role_attribute)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        debug!(email = %email, role = ?role, "OIDC userinfo received");

        Ok(SsoAuthResult {
            email: email.to_string(),
            name,
            role,
            provider_type: SsoProviderType::Oidc,
        })
    }
}
