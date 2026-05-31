//! Pre-configured routing profiles — pick a preset, get optimal routing.
//!
//! Instead of manually configuring strategies, weights, timeouts, and fallbacks,
//! users pick a high-level profile that matches their priorities:
//!
//! | Profile       | Priority        | Strategy   | Primary → Fallback     |
//! |---------------|-----------------|------------|------------------------|
//! | privacy-first | Privacy         | Cascade    | Local → Cloud          |
//! | balanced      | Cost/Quality    | Classifier | Simple→Local, Complex→Cloud |
//! | speed         | Low Latency     | Single     | Fastest Cloud          |
//! | frugal        | Minimum Cost    | Cascade    | Local (aggressive) → Cloud |
//! | quality       | Best Output     | Single     | Best Cloud Model       |
//! | offline       | Air-gapped      | Single     | Local Only             |

use serde::{Deserialize, Serialize};
use std::fmt;

/// A pre-configured routing profile that auto-configures the gateway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingProfile {
    /// Privacy-first: route to local LLM, fallback to cloud on failure.
    /// Never sends data externally unless local is down.
    PrivacyFirst,

    /// Balanced: classifier routes simple queries locally, complex ones to cloud.
    /// Best cost/quality tradeoff (~70% savings vs cloud-only).
    Balanced,

    /// Speed: always use the fastest available provider (typically cloud).
    /// No local inference latency. Best for real-time apps.
    Speed,

    /// Frugal: aggressively use local LLM, only fall back on hard failure.
    /// Maximum cost savings (~90% vs cloud-only).
    Frugal,

    /// Quality: always use the highest-quality cloud model.
    /// Never compromise on output quality.
    Quality,

    /// Offline: local LLM only. Zero external network calls.
    /// For air-gapped or secure environments.
    Offline,

    /// Custom: user-defined rules. Profile system does not override.
    Custom,
}

impl RoutingProfile {
    /// All available profiles (except Custom) for UI listing.
    pub const PRESETS: &'static [RoutingProfile] = &[
        RoutingProfile::PrivacyFirst,
        RoutingProfile::Balanced,
        RoutingProfile::Speed,
        RoutingProfile::Frugal,
        RoutingProfile::Quality,
        RoutingProfile::Offline,
    ];

    /// Human-readable name.
    pub fn display_name(&self) -> &'static str {
        match self {
            RoutingProfile::PrivacyFirst => "Privacy First",
            RoutingProfile::Balanced => "Balanced",
            RoutingProfile::Speed => "Speed",
            RoutingProfile::Frugal => "Frugal",
            RoutingProfile::Quality => "Quality",
            RoutingProfile::Offline => "Offline",
            RoutingProfile::Custom => "Custom",
        }
    }

    /// One-line description for UI/tooltips.
    pub fn description(&self) -> &'static str {
        match self {
            RoutingProfile::PrivacyFirst => {
                "Local LLM first, cloud fallback. Keeps your data local."
            }
            RoutingProfile::Balanced => {
                "Smart classifier routes simple queries locally, complex ones to cloud. Best savings."
            }
            RoutingProfile::Speed => {
                "Always use the fastest provider. No local inference delay."
            }
            RoutingProfile::Frugal => {
                "Aggressively use local LLM. Maximum cost savings."
            }
            RoutingProfile::Quality => {
                "Always use the best cloud model. Never compromise on quality."
            }
            RoutingProfile::Offline => {
                "Local LLM only. Zero external network calls."
            }
            RoutingProfile::Custom => "User-defined routing rules.",
        }
    }

    /// Estimated cost savings vs cloud-only baseline.
    pub fn estimated_savings(&self) -> &'static str {
        match self {
            RoutingProfile::PrivacyFirst => "~60%",
            RoutingProfile::Balanced => "~70%",
            RoutingProfile::Speed => "0%",
            RoutingProfile::Frugal => "~90%",
            RoutingProfile::Quality => "0%",
            RoutingProfile::Offline => "100%",
            RoutingProfile::Custom => "Varies",
        }
    }

    /// Whether this profile requires a cloud provider.
    pub fn requires_cloud(&self) -> bool {
        !matches!(self, RoutingProfile::Offline)
    }

    /// Whether this profile requires a local provider.
    pub fn requires_local(&self) -> bool {
        matches!(
            self,
            RoutingProfile::PrivacyFirst
                | RoutingProfile::Balanced
                | RoutingProfile::Frugal
                | RoutingProfile::Offline
        )
    }

    /// Default routing strategy for this profile.
    pub fn default_strategy(&self) -> &'static str {
        match self {
            RoutingProfile::PrivacyFirst | RoutingProfile::Frugal => "fallback",
            RoutingProfile::Balanced => "classifier",
            RoutingProfile::Speed | RoutingProfile::Quality | RoutingProfile::Offline => "single",
            RoutingProfile::Custom => "custom",
        }
    }

    /// Default timeout for primary provider (ms).
    pub fn primary_timeout_ms(&self) -> u64 {
        match self {
            // Give local models more time (they're slower)
            RoutingProfile::PrivacyFirst => 45_000,
            // Balanced: local gets moderate timeout
            RoutingProfile::Balanced => 30_000,
            // Frugal: very aggressive — fail fast to try local
            RoutingProfile::Frugal => 20_000,
            // Speed/Quality/Offline: standard
            _ => 30_000,
        }
    }

    /// Default timeout for fallback providers (ms).
    pub fn fallback_timeout_ms(&self) -> u64 {
        match self {
            // Frugal: wait a bit longer on fallback since we're trying to avoid it
            RoutingProfile::Frugal => 60_000,
            _ => 30_000,
        }
    }

    /// Whether to enable circuit breaker for this profile.
    pub fn circuit_breaker_enabled(&self) -> bool {
        // All profiles benefit from circuit breaker
        *self != RoutingProfile::Offline
    }

    /// Recommended provider setup for this profile.
    pub fn recommended_setup(&self) -> ProfileSetup {
        match self {
            RoutingProfile::PrivacyFirst => ProfileSetup {
                local_model: Some("llama3.2"),
                cloud_model: Some("gpt-4o-mini"),
                notes: "Local first. Cloud only when local fails.",
            },
            RoutingProfile::Balanced => ProfileSetup {
                local_model: Some("llama3.2"),
                cloud_model: Some("gpt-4o-mini"),
                notes: "Classifier decides based on query complexity.",
            },
            RoutingProfile::Speed => ProfileSetup {
                local_model: None,
                cloud_model: Some("gpt-4o-mini"),
                notes: "Fastest cloud model. No local inference delay.",
            },
            RoutingProfile::Frugal => ProfileSetup {
                local_model: Some("llama3.2"),
                cloud_model: Some("gpt-4o-mini"),
                notes: "Force local. Cloud only on hard failure.",
            },
            RoutingProfile::Quality => ProfileSetup {
                local_model: None,
                cloud_model: Some("gpt-4o"),
                notes: "Best quality model regardless of cost.",
            },
            RoutingProfile::Offline => ProfileSetup {
                local_model: Some("llama3.2"),
                cloud_model: None,
                notes: "Local only. No external calls.",
            },
            RoutingProfile::Custom => ProfileSetup {
                local_model: None,
                cloud_model: None,
                notes: "Configure your own rules.",
            },
        }
    }
}

impl RoutingProfile {
    /// Return all preset profiles as a Vec.
    pub fn all_profiles() -> Vec<RoutingProfile> {
        Self::PRESETS.to_vec()
    }

    /// Config file key for this profile.
    pub fn config_key(&self) -> &'static str {
        match self {
            RoutingProfile::PrivacyFirst => "privacy-first",
            RoutingProfile::Balanced => "balanced",
            RoutingProfile::Speed => "speed",
            RoutingProfile::Frugal => "frugal",
            RoutingProfile::Quality => "quality",
            RoutingProfile::Offline => "offline",
            RoutingProfile::Custom => "custom",
        }
    }
}

impl Default for RoutingProfile {
    fn default() -> Self {
        RoutingProfile::Balanced
    }
}

impl fmt::Display for RoutingProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Suggested provider configuration for a profile.
#[derive(Debug, Clone)]
pub struct ProfileSetup {
    pub local_model: Option<&'static str>,
    pub cloud_model: Option<&'static str>,
    pub notes: &'static str,
}

/// Full profile configuration that can be serialized to gateway.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub profile: RoutingProfile,
    pub local_provider: Option<ProviderConfig>,
    pub cloud_provider: Option<ProviderConfig>,
    pub timeout_ms: u64,
    pub fallback_timeout_ms: u64,
    pub enable_circuit_breaker: bool,
}

impl ProfileConfig {
    /// Build a ProfileConfig from a profile + user-provided provider details.
    pub fn from_profile(
        profile: RoutingProfile,
        local: Option<ProviderConfig>,
        cloud: Option<ProviderConfig>,
    ) -> Self {
        Self {
            profile,
            local_provider: local,
            cloud_provider: cloud,
            timeout_ms: profile.primary_timeout_ms(),
            fallback_timeout_ms: profile.fallback_timeout_ms(),
            enable_circuit_breaker: profile.circuit_breaker_enabled(),
        }
    }

    /// Validate that required providers are configured.
    pub fn validate(&self) -> Result<(), ProfileValidationError> {
        if self.profile.requires_local() && self.local_provider.is_none() {
            return Err(ProfileValidationError::MissingLocalProvider);
        }
        if self.profile.requires_cloud() && self.cloud_provider.is_none() {
            return Err(ProfileValidationError::MissingCloudProvider);
        }
        Ok(())
    }
}

/// Provider configuration for a profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub kind: String,        // "openai", "anthropic", "ollama", etc.
    pub model: String,       // e.g. "gpt-4o-mini"
    pub api_key: Option<String>, // None for local/Ollama
    pub base_url: Option<String>, // Custom endpoint
}

/// Validation errors for profile configuration.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ProfileValidationError {
    #[error("This profile requires a local provider (e.g. Ollama). Please configure one.")]
    MissingLocalProvider,
    #[error("This profile requires a cloud provider (e.g. OpenAI). Please configure one.")]
    MissingCloudProvider,
    #[error("Invalid provider kind: {0}")]
    InvalidProviderKind(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_presets() {
        assert_eq!(RoutingProfile::PRESETS.len(), 6);
        assert!(!RoutingProfile::PRESETS.contains(&RoutingProfile::Custom));
    }

    #[test]
    fn test_privacy_first_requires_local() {
        assert!(RoutingProfile::PrivacyFirst.requires_local());
        assert!(RoutingProfile::PrivacyFirst.requires_cloud());
        assert_eq!(RoutingProfile::PrivacyFirst.default_strategy(), "fallback");
    }

    #[test]
    fn test_offline_never_cloud() {
        assert!(!RoutingProfile::Offline.requires_cloud());
        assert!(RoutingProfile::Offline.requires_local());
        assert_eq!(RoutingProfile::Offline.default_strategy(), "single");
    }

    #[test]
    fn test_speed_never_local() {
        assert!(!RoutingProfile::Speed.requires_local());
        assert!(RoutingProfile::Speed.requires_cloud());
    }

    #[test]
    fn test_profile_serde_roundtrip() {
        for profile in RoutingProfile::PRESETS {
            let json = serde_json::to_string(profile).unwrap();
            let parsed: RoutingProfile = serde_json::from_str(&json).unwrap();
            assert_eq!(*profile, parsed);
        }
    }

    #[test]
    fn test_profile_config_validation() {
        let cfg = ProfileConfig::from_profile(
            RoutingProfile::Offline,
            Some(ProviderConfig {
                kind: "ollama".into(),
                model: "llama3.2".into(),
                api_key: None,
                base_url: Some("http://localhost:11434".into()),
            }),
            None,
        );
        assert!(cfg.validate().is_ok());

        let bad = ProfileConfig::from_profile(RoutingProfile::Offline, None, None);
        assert!(matches!(
            bad.validate(),
            Err(ProfileValidationError::MissingLocalProvider)
        ));
    }

    #[test]
    fn test_frugal_most_savings() {
        assert_eq!(RoutingProfile::Frugal.estimated_savings(), "~90%");
        assert_eq!(RoutingProfile::Frugal.primary_timeout_ms(), 20_000);
    }
}
