//! PII redaction for log output and error responses.
//!
//! Redacts sensitive data like API keys, email addresses, phone numbers,
//! credit cards, and SSNs from strings before they are written to logs
//! or sent to clients.

use once_cell::sync::Lazy;

// ── Pre-compiled regexes ─────────────────────────────────────────────────────

static API_KEY_RE: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"sk_gw_[A-Za-z0-9]{38,}").unwrap());

static EMAIL_RE: Lazy<regex::Regex> = Lazy::new(|| {
    regex::Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap()
});

static PHONE_RE: Lazy<regex::Regex> = Lazy::new(|| {
    // US/international phone patterns: +1-xxx-xxx-xxxx, (xxx) xxx-xxxx, xxx-xxx-xxxx, xxxxxxxxxx
    regex::Regex::new(r"(\+?1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}").unwrap()
});

static CREDIT_CARD_RE: Lazy<regex::Regex> = Lazy::new(|| {
    // 13–19 digit sequences with optional spaces or hyphens
    regex::Regex::new(r"\b(?:\d[ -]*?){13,19}\b").unwrap()
});

static SSN_RE: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"\b\d{3}[-\s]?\d{2}[-\s]?\d{4}\b").unwrap());

static BEARER_RE: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"Bearer\s+[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+").unwrap());

// ── Redaction level ──────────────────────────────────────────────────────────

/// How aggressively to redact PII.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactionLevel {
    /// No redaction.
    None,
    /// Redact API keys and bearer tokens only.
    KeysOnly,
    /// Redact all known PII patterns (default).
    Full,
}

impl RedactionLevel {
    /// Parse from an environment variable string.
    pub fn from_env() -> Self {
        match std::env::var("GATEWAY_PII_REDACTION_LEVEL").as_deref() {
            Ok("none") => RedactionLevel::None,
            Ok("keys_only") => RedactionLevel::KeysOnly,
            Ok("full") => RedactionLevel::Full,
            _ => RedactionLevel::Full,
        }
    }
}

// ── Individual redactors ─────────────────────────────────────────────────────

/// Redact gateway API keys (`sk_gw_...`).
pub fn redact_api_keys(input: &str) -> String {
    API_KEY_RE.replace_all(input, "[REDACTED:api_key]").into_owned()
}

/// Redact email addresses.
pub fn redact_emails(input: &str) -> String {
    EMAIL_RE.replace_all(input, "[REDACTED:email]").into_owned()
}

/// Redact phone numbers.
pub fn redact_phone_numbers(input: &str) -> String {
    PHONE_RE.replace_all(input, "[REDACTED:phone]").into_owned()
}

/// Redact credit card numbers (with Luhn validation).
pub fn redact_credit_cards(input: &str) -> String {
    CREDIT_CARD_RE
        .replace_all(input, |caps: &regex::Captures<'_>| {
            let matched = caps.get(0).map(|m| m.as_str()).unwrap_or("");
            let digits: String = matched.chars().filter(|c| c.is_ascii_digit()).collect();
            if luhn_check(&digits) {
                "[REDACTED:credit_card]".to_string()
            } else {
                matched.to_string()
            }
        })
        .into_owned()
}

/// Redact SSN-like patterns.
pub fn redact_ssn(input: &str) -> String {
    SSN_RE.replace_all(input, "[REDACTED:ssn]").into_owned()
}

/// Redact bearer tokens.
pub fn redact_bearer_tokens(input: &str) -> String {
    BEARER_RE.replace_all(input, "[REDACTED:bearer_token]").into_owned()
}

// ── Combined redaction ───────────────────────────────────────────────────────

/// Redact sensitive patterns from a string according to the given level.
///
/// * `None` — returns the input unchanged.
/// * `KeysOnly` — redacts API keys and bearer tokens only.
/// * `Full` — redacts all known PII patterns.
pub fn redact(input: &str) -> String {
    redact_with_level(input, RedactionLevel::from_env())
}

/// Redact with an explicit level (useful for testing and per-request overrides).
pub fn redact_with_level(input: &str, level: RedactionLevel) -> String {
    match level {
        RedactionLevel::None => input.to_string(),
        RedactionLevel::KeysOnly => {
            let out = redact_bearer_tokens(input);
            redact_api_keys(&out)
        }
        RedactionLevel::Full => {
            let mut out = input.to_string();
            out = redact_bearer_tokens(&out);
            out = redact_api_keys(&out);
            out = redact_credit_cards(&out);
            out = redact_emails(&out);
            out = redact_phone_numbers(&out);
            out = redact_ssn(&out);
            out
        }
    }
}

/// Redact a JSON value recursively.
pub fn redact_json(value: &serde_json::Value) -> serde_json::Value {
    redact_json_with_level(value, RedactionLevel::from_env())
}

/// Redact a JSON value recursively with an explicit level.
pub fn redact_json_with_level(value: &serde_json::Value, level: RedactionLevel) -> serde_json::Value {
    if level == RedactionLevel::None {
        return value.clone();
    }

    match value {
        serde_json::Value::String(s) => serde_json::Value::String(redact_with_level(s, level)),
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(|v| redact_json_with_level(v, level)).collect())
        }
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                let key_lower = k.to_lowercase();
                let is_sensitive_key = key_lower.contains("api_key")
                    || key_lower.contains("apikey")
                    || key_lower.contains("secret")
                    || key_lower.contains("token")
                    || key_lower.contains("password")
                    || key_lower.contains("authorization")
                    || key_lower.contains("credit_card")
                    || key_lower.contains("ssn")
                    || key_lower.contains("phone");

                if level == RedactionLevel::Full && is_sensitive_key {
                    out.insert(k.clone(), serde_json::Value::String("[REDACTED]".to_string()));
                } else {
                    out.insert(k.clone(), redact_json_with_level(v, level));
                }
            }
            serde_json::Value::Object(out)
        }
        other => other.clone(),
    }
}

/// Truncate a string to a max length, adding an ellipsis.
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Luhn check for credit card validation.
fn luhn_check(digits: &str) -> bool {
    if digits.len() < 13 || digits.len() > 19 {
        return false;
    }
    let mut sum = 0;
    let mut double = false;
    for ch in digits.chars().rev() {
        let mut digit = ch.to_digit(10).unwrap_or(0);
        if double {
            digit *= 2;
            if digit > 9 {
                digit -= 9;
            }
        }
        sum += digit;
        double = !double;
    }
    sum % 10 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_api_key() {
        let key = "sk_gw_abcdefghijklmnopqrstuvwxyz1234567890abcd";
        let input = format!("Authorization: {}", key);
        let out = redact_with_level(&input, RedactionLevel::Full);
        assert!(!out.contains(key));
        assert!(out.contains("[REDACTED:api_key]"));
    }

    #[test]
    fn test_redact_api_key_min_length() {
        // API keys must be at least 38 chars after prefix to match
        let short = "sk_gw_abc";
        let out = redact_api_keys(short);
        assert!(out.contains("sk_gw_abc")); // too short
    }

    #[test]
    fn test_redact_email() {
        let input = "Contact admin@example.com for support";
        let out = redact_emails(input);
        assert!(!out.contains("admin@example.com"));
        assert!(out.contains("[REDACTED:email]"));
    }

    #[test]
    fn test_redact_phone() {
        let input = "Call me at 555-123-4567 or (555) 987-6543";
        let out = redact_phone_numbers(input);
        assert!(!out.contains("555-123-4567"));
        assert!(!out.contains("(555) 987-6543"));
        assert!(out.contains("[REDACTED:phone]"));
    }

    #[test]
    fn test_redact_ssn() {
        let input = "SSN: 123-45-6789";
        let out = redact_ssn(input);
        assert!(!out.contains("123-45-6789"));
        assert!(out.contains("[REDACTED:ssn]"));
    }

    #[test]
    fn test_redact_bearer_token() {
        let input = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        let out = redact_bearer_tokens(input);
        assert!(!out.contains("eyJhbGci"));
        assert!(out.contains("[REDACTED:bearer_token]"));
    }

    #[test]
    fn test_redact_credit_card_valid() {
        // Valid Visa test number: 4111 1111 1111 1111
        let input = "Card: 4111 1111 1111 1111";
        let out = redact_credit_cards(input);
        assert!(!out.contains("4111 1111 1111 1111"));
        assert!(out.contains("[REDACTED:credit_card]"));
    }

    #[test]
    fn test_redact_credit_card_invalid() {
        // Invalid number (fails Luhn)
        let input = "Card: 4111 1111 1111 1112";
        let out = redact_credit_cards(input);
        // Should NOT be redacted because it fails Luhn
        assert!(out.contains("4111 1111 1111 1112"));
    }

    #[test]
    fn test_redact_level_none() {
        let input = "Key: sk_gw_abc123xyz Email: test@example.com";
        let out = redact_with_level(input, RedactionLevel::None);
        assert!(out.contains("sk_gw_abc123xyz"));
        assert!(out.contains("test@example.com"));
    }

    #[test]
    fn test_redact_level_keys_only() {
        let key = "sk_gw_abcdefghijklmnopqrstuvwxyz1234567890abcd";
        let input = format!("Key: {} Email: test@example.com", key);
        let out = redact_with_level(&input, RedactionLevel::KeysOnly);
        assert!(!out.contains(key));
        assert!(out.contains("test@example.com"));
    }

    #[test]
    fn test_redact_level_full() {
        let key = "sk_gw_abcdefghijklmnopqrstuvwxyz1234567890abcd";
        let input = format!("Key: {} Email: test@example.com", key);
        let out = redact_with_level(&input, RedactionLevel::Full);
        assert!(!out.contains(key));
        assert!(!out.contains("test@example.com"));
    }

    #[test]
    fn test_redact_json_object() {
        let key = "sk_gw_abcdefghijklmnopqrstuvwxyz1234567890abcd";
        let input = serde_json::json!({
            "message": "Hello admin@example.com",
            "api_key": key,
            "nested": {
                "email": "user@test.org"
            }
        });
        let out = redact_json_with_level(&input, RedactionLevel::Full);
        let out_str = out.to_string();
        assert!(!out_str.contains("admin@example.com"));
        assert!(!out_str.contains(key));
        assert!(!out_str.contains("user@test.org"));
        assert!(out_str.contains("[REDACTED]"));
    }

    #[test]
    fn test_luhn_check() {
        assert!(luhn_check("4111111111111111"));
        assert!(!luhn_check("4111111111111112"));
        assert!(luhn_check("4532015112830366"));
        assert!(!luhn_check("1234567890123456"));
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("this is a long string", 10), "this is a ...");
    }

    #[test]
    fn test_redaction_performance() {
        // Build a 1KB message body with mixed PII
        let mut body = String::new();
        for i in 0..20 {
            body.push_str(&format!(
                "User {} has email user{}@example.com and phone 555-{:03}-{:04}. ",
                i, i, i * 10, i * 1000
            ));
        }
        assert!(body.len() >= 1000, "Body should be at least 1KB");

        let start = std::time::Instant::now();
        let _ = redact_with_level(&body, RedactionLevel::Full);
        let elapsed = start.elapsed();

        // In debug builds regex can be slower; allow up to 250ms.
        // In release builds this is well under 1ms.
        let threshold_ms = if cfg!(debug_assertions) { 250 } else { 1 };
        assert!(
            elapsed.as_millis() < threshold_ms,
            "Redaction took {}ms, expected < {}ms for 1KB body",
            elapsed.as_millis(),
            threshold_ms
        );
    }
}
