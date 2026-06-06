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
    if cidr.contains('/') && cidr.parse::<std::net::IpAddr>().is_err() {
        // Try parsing as CIDR (e.g., 192.168.0.0/24)
        let parts: Vec<&str> = cidr.split('/').collect();
        if parts.len() == 2
            && parts[0].parse::<std::net::IpAddr>().is_ok()
            && parts[1].parse::<u8>().map(|m| m <= 128).unwrap_or(false)
        {
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
        "; drop ",
        "; delete ",
        "; insert ",
        "; update ",
        "; select ",
        "' or ",
        "' and ",
        "'--",
        "/*",
        "*/",
        "; --",
        "union select",
        "exec(",
        "execute(",
        "xp_",
        "sp_",
    ];
    patterns.iter().any(|p| lower.contains(p))
}

/// Validate that a URL does not point to private/internal addresses.
/// Blocks loopback, link-local, multicast, and RFC 1918 / RFC 4193 ranges.
/// Also blocks well-known internal hostnames like localhost.
pub fn validate_url_not_internal(url: &str) -> Result<(), ValidationError> {
    let parsed = match url.parse::<reqwest::Url>() {
        Ok(u) => u,
        Err(_) => {
            let mut err = ValidationError::new("invalid_url");
            err.message = Some("Invalid URL format".into());
            return Err(err);
        }
    };

    // Only allow http and https schemes
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        let mut err = ValidationError::new("invalid_url_scheme");
        err.message = Some("URL scheme must be http or https".into());
        return Err(err);
    }

    let host = match parsed.host_str() {
        Some(h) => h,
        None => {
            let mut err = ValidationError::new("invalid_url_host");
            err.message = Some("URL must have a host".into());
            return Err(err);
        }
    };

    // Check if host is an IP address (strip brackets for IPv6)
    let host_for_ip = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    if let Ok(ip) = host_for_ip.parse::<std::net::IpAddr>() {
        if is_internal_ip(ip) {
            let mut err = ValidationError::new("url_internal_ip");
            err.message = Some("URL points to a private or internal IP address".into());
            return Err(err);
        }
        return Ok(());
    }

    // Block well-known internal hostnames (case-insensitive)
    let lower_host = host.to_lowercase();
    let blocked_hostnames = [
        "localhost",
        "localhost.localdomain",
        "ip6-localhost",
        "ip6-loopback",
    ];
    if blocked_hostnames.iter().any(|&h| lower_host == h) {
        let mut err = ValidationError::new("url_internal_host");
        err.message = Some("URL points to an internal hostname".into());
        return Err(err);
    }

    Ok(())
}

fn is_internal_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_broadcast()
                || v4.is_documentation()
                // 0.0.0.0/8
                || v4.octets()[0] == 0
                // 169.254.0.0/16 (link-local, already covered)
                // 127.0.0.0/8 (loopback, already covered)
                // 198.18.0.0/15 (benchmark)
                || (v4.octets()[0] == 198 && v4.octets()[1] == 18)
                || (v4.octets()[0] == 198 && v4.octets()[1] == 19)
                // 192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24 (documentation)
                // 192.88.99.0/24 (6to4 relay anycast)
                || (v4.octets()[0] == 192 && v4.octets()[1] == 88 && v4.octets()[2] == 99)
                // 240.0.0.0/4 (reserved)
                || v4.octets()[0] >= 240
        }
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_multicast()
                || v6.is_unspecified()
                // Unique local addresses (fc00::/7)
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                // Link-local (fe80::/10, already covered by is_loopback for ::1)
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                // IPv4-mapped IPv6 addresses that are internal
                || v6.to_ipv4_mapped().is_some_and(|v4| is_internal_ip(std::net::IpAddr::V4(v4)))
        }
    }
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
        assert_eq!(
            sanitize_display_text("<script>alert(1)</script>"),
            "&lt;script&gt;alert(1)&lt;/script&gt;"
        );
    }

    #[test]
    fn test_detects_sql_injection_patterns() {
        assert!(contains_sql_injection("'; DROP TABLE users; --"));
        assert!(contains_sql_injection("1' OR '1'='1"));
        assert!(!contains_sql_injection("Hello, world!"));
    }

    #[test]
    fn test_validate_url_not_internal_blocks_private_ips() {
        assert!(validate_url_not_internal("http://127.0.0.1/webhook").is_err());
        assert!(validate_url_not_internal("http://10.0.0.1/webhook").is_err());
        assert!(validate_url_not_internal("http://192.168.1.1/webhook").is_err());
        assert!(validate_url_not_internal("http://172.16.0.1/webhook").is_err());
        assert!(validate_url_not_internal("http://169.254.169.254/latest/meta-data/").is_err());
        assert!(validate_url_not_internal("http://0.0.0.0/").is_err());
        assert!(validate_url_not_internal("http://[::1]/webhook").is_err());
        assert!(validate_url_not_internal("http://[fc00::1]/webhook").is_err());
    }

    #[test]
    fn test_validate_url_not_internal_blocks_localhost() {
        assert!(validate_url_not_internal("http://localhost/webhook").is_err());
        assert!(validate_url_not_internal("http://localhost:8080/webhook").is_err());
        assert!(validate_url_not_internal("http://LOCALHOST/webhook").is_err());
    }

    #[test]
    fn test_validate_url_not_internal_allows_public_urls() {
        assert!(validate_url_not_internal("https://hooks.example.com/webhook").is_ok());
        assert!(validate_url_not_internal("https://example.com:8443/webhook").is_ok());
        assert!(validate_url_not_internal("http://1.2.3.4/webhook").is_ok());
    }

    #[test]
    fn test_validate_url_not_internal_blocks_non_http_schemes() {
        assert!(validate_url_not_internal("ftp://example.com/file").is_err());
        assert!(validate_url_not_internal("file:///etc/passwd").is_err());
        assert!(validate_url_not_internal("gopher://example.com").is_err());
    }

    #[test]
    fn test_validate_url_not_internal_blocks_ipv6_mapped_ipv4() {
        assert!(validate_url_not_internal("http://[::ffff:127.0.0.1]/webhook").is_err());
        assert!(validate_url_not_internal("http://[::ffff:192.168.1.1]/webhook").is_err());
    }
}
