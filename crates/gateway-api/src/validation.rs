//! Input validation and sanitization helpers.
//!
//! Provides reusable validators for common gateway inputs and basic
//! sanitization to reduce injection attack surface.

use regex::Regex;
use std::sync::OnceLock;
use validator::ValidationError;

fn model_id_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9_./:-]{1,128}$").unwrap())
}

fn safe_string_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[\x20-\x7E]{0,4096}$").unwrap())
}

fn provider_kind_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(?i)(openai|anthropic|gemini|ollama|qwen|kimi|tencent|groq|mistral|cohere|azure|custom)$")
            .unwrap()
    })
}

/// Validate a model identifier.
pub fn validate_model_id(model_id: &str) -> Result<(), ValidationError> {
    if model_id_re().is_match(model_id) {
        Ok(())
    } else {
        let mut err = ValidationError::new("invalid_model_id");
        err.message = Some(format!("'{}' is not a valid model identifier", model_id).into());
        Err(err)
    }
}

/// Validate a provider kind string.
pub fn validate_provider_kind(kind: &str) -> Result<(), ValidationError> {
    if provider_kind_re().is_match(kind) {
        Ok(())
    } else {
        let mut err = ValidationError::new("invalid_provider_kind");
        err.message = Some(format!("'{}' is not a supported provider kind", kind).into());
        Err(err)
    }
}

/// Validate a CIDR block or single IP address (IPv4 or IPv6).
pub fn validate_cidr(cidr: &str) -> Result<(), ValidationError> {
    if cidr.parse::<std::net::IpAddr>().is_ok() {
        return Ok(());
    }
    if cidr.contains('/')
        && cidr.parse::<std::net::IpAddr>().is_err() {
            // Try parsing as CIDR (e.g., 192.168.0.0/24)
            let parts: Vec<&str> = cidr.split('/').collect();
            if parts.len() == 2
                && parts[0].parse::<std::net::IpAddr>().is_ok()
                    && parts[1].parse::<u8>().map(|m| m <= 128).unwrap_or(false) {
                        return Ok(());
                    }
        }
    let mut err = ValidationError::new("invalid_cidr");
    err.message = Some(format!("'{}' is not a valid IP or CIDR", cidr).into());
    Err(err)
}

/// Basic input sanitizer: trims whitespace and strips null bytes.
pub fn sanitize_input(s: &str) -> String {
    s.trim().replace(['\0', '\x1b'], "")
}

/// Sanitize a string that will be used as a display name or description.
/// Strips HTML-like tags to reduce XSS surface.
pub fn sanitize_display_text(s: &str) -> String {
    let s = sanitize_input(s);
    // Very permissive: just remove < and > characters.
    s.replace('<', "&lt;").replace('>', "&gt;")
}

/// Check whether a string contains suspicious SQL injection patterns.
/// This is a defense-in-depth check; all queries must still be parameterized.
pub fn contains_sql_injection(input: &str) -> bool {
    let lower = input.to_lowercase();
    let patterns = [
        "; drop ", "; delete ", "; insert ", "; update ", "; select ",
        "' or ", "' and ", "'--", "/*", "*/", "; --", "union select",
        "exec(", "execute(", "xp_", "sp_",
    ];
    patterns.iter().any(|p| lower.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_model_id_accepts_common_models() {
        assert!(validate_model_id("gpt-4o").is_ok());
        assert!(validate_model_id("claude-3-5-sonnet").is_ok());
        assert!(validate_model_id("accounts/fireworks/models/llama-v3p1-405b").is_ok());
    }

    #[test]
    fn test_validate_model_id_rejects_empty() {
        assert!(validate_model_id("").is_err());
    }

    #[test]
    fn test_validate_model_id_rejects_too_long() {
        let long = "a".repeat(129);
        assert!(validate_model_id(&long).is_err());
    }

    #[test]
    fn test_validate_provider_kind() {
        assert!(validate_provider_kind("openai").is_ok());
        assert!(validate_provider_kind("Anthropic").is_ok());
        assert!(validate_provider_kind("unknown").is_err());
    }

    #[test]
    fn test_validate_cidr_ipv4() {
        assert!(validate_cidr("192.168.1.1").is_ok());
        assert!(validate_cidr("10.0.0.0/8").is_ok());
    }

    #[test]
    fn test_validate_cidr_ipv6() {
        assert!(validate_cidr("::1").is_ok());
        assert!(validate_cidr("2001:db8::/32").is_ok());
    }

    #[test]
    fn test_validate_cidr_rejects_invalid() {
        assert!(validate_cidr("not-an-ip").is_err());
        assert!(validate_cidr("10.0.0.0/999").is_err());
    }

    #[test]
    fn test_sanitize_input_strips_null_and_escape() {
        assert_eq!(sanitize_input("hello\0world\x1b"), "helloworld");
    }

    #[test]
    fn test_sanitize_display_text_escapes_html() {
        assert_eq!(sanitize_display_text("<script>alert(1)</script>"), "&lt;script&gt;alert(1)&lt;/script&gt;");
    }

    #[test]
    fn test_detects_sql_injection_patterns() {
        assert!(contains_sql_injection("'; DROP TABLE users; --"));
        assert!(contains_sql_injection("1' OR '1'='1"));
        assert!(!contains_sql_injection("Hello, world!"));
    }
}
