//! Interactive configuration wizard.
//!
//! Run with `gateway config` to set up your gateway via prompts.

use gateway_core::profiles::{ProfileConfig, ProviderConfig, RoutingProfile};
use std::io::{self, Write};

pub async fn run() -> anyhow::Result<()> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           AI Gateway — Setup Wizard                          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Let's configure your gateway in 3 easy steps.\n");

    // Step 1: Pick a routing profile
    let profile = pick_profile()?;
    println!();

    // Step 2: Configure providers
    let local = if profile.requires_local() {
        println!("--- Local Provider Setup ---");
        Some(configure_provider(true)?)
    } else {
        None
    };

    let cloud = if profile.requires_cloud() {
        println!("\n--- Cloud Provider Setup ---");
        Some(configure_provider(false)?)
    } else {
        None
    };

    // Step 3: Build and validate config
    let config = ProfileConfig::from_profile(profile, local, cloud);
    if let Err(e) = config.validate() {
        println!("\n⚠ Validation error: {}", e);
        return Ok(());
    }

    // Step 4: Show summary and write file
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("  Configuration Summary");
    println!("═══════════════════════════════════════════════════════════════");
    println!("  Profile:         {}", config.profile.display_name());
    println!("  Strategy:        {}", config.profile.default_strategy());
    println!("  Primary Timeout: {}ms", config.timeout_ms);
    println!("  Fallback Timeout:{}ms", config.fallback_timeout_ms);
    println!("  Circuit Breaker: {}", if config.enable_circuit_breaker { "enabled" } else { "disabled" });
    if let Some(ref p) = config.local_provider {
        println!("  Local Provider:  {} / {}", p.kind, p.model);
    }
    if let Some(ref p) = config.cloud_provider {
        println!("  Cloud Provider:  {} / {}", p.kind, p.model);
    }
    println!("  Est. Savings:    {}", config.profile.estimated_savings());
    println!("═══════════════════════════════════════════════════════════════");

    let write_file = prompt_yn("\nWrite this configuration to gateway.toml?")?;
    if write_file {
        let toml = generate_toml(&config);
        std::fs::write("gateway.toml", toml)?;
        println!("✓ Configuration saved to gateway.toml");
        println!("\nStart the gateway with:  gateway serve");
    } else {
        println!("Configuration not saved. You can run the wizard again anytime.");
    }

    Ok(())
}

fn pick_profile() -> anyhow::Result<RoutingProfile> {
    println!("Step 1/3: Choose your routing priority\n");
    let presets = RoutingProfile::PRESETS;
    for (i, p) in presets.iter().enumerate() {
        println!(
            "  {}. {:15} — {} (save {})",
            i + 1,
            p.display_name(),
            p.description(),
            p.estimated_savings()
        );
    }
    println!();

    let choice = loop {
        let input = prompt("Enter choice (1-6): ")?;
        match input.trim().parse::<usize>() {
            Ok(n) if n >= 1 && n <= presets.len() => break presets[n - 1],
            _ => println!("Invalid choice. Please enter 1-6."),
        }
    };

    let setup = choice.recommended_setup();
    println!(
        "\n✓ You selected: {} — {}",
        choice.display_name(),
        choice.description()
    );
    if let Some(m) = setup.local_model {
        println!("  Recommended local model: {}", m);
    }
    if let Some(m) = setup.cloud_model {
        println!("  Recommended cloud model: {}", m);
    }
    println!("  {}", setup.notes);

    Ok(choice)
}

fn configure_provider(is_local: bool) -> anyhow::Result<ProviderConfig> {
    let kinds = if is_local {
        vec!["ollama"]
    } else {
        vec!["openai", "anthropic", "gemini"]
    };

    println!("Available providers: {}", kinds.join(", "));

    let kind = loop {
        let input = prompt("Provider kind: ")?;
        let normalized = input.trim().to_lowercase();
        if kinds.contains(&normalized.as_str()) {
            break normalized;
        }
        println!("Invalid. Choose from: {}", kinds.join(", "));
    };

    let default_model = if is_local {
        "llama3.2"
    } else if kind == "openai" {
        "gpt-4o-mini"
    } else if kind == "anthropic" {
        "claude-3-5-sonnet-20241022"
    } else {
        "gemini-1.5-flash"
    };

    let model = prompt_default("Model", default_model)?;

    let api_key = if is_local {
        None
    } else {
        let key = prompt("API key (leave empty to set via env): ")?;
        let key = key.trim();
        if key.is_empty() {
            None
        } else {
            Some(key.to_string())
        }
    };

    let base_url = if is_local {
        Some(prompt_default("Base URL", "http://localhost:11434")?)
    } else {
        let url = prompt("Custom base URL (leave empty for default): ")?;
        let url = url.trim();
        if url.is_empty() {
            None
        } else {
            Some(url.to_string())
        }
    };

    Ok(ProviderConfig {
        kind,
        model,
        api_key,
        base_url,
    })
}

fn generate_toml(config: &ProfileConfig) -> String {
    let mut out = String::new();
    out.push_str("# AI Gateway Configuration\n");
    out.push_str("# Generated by `gateway config` wizard\n\n");

    out.push_str("[gateway]\n");
    out.push_str(&format!("profile = \"{}\"\n", serde_json::to_string(&config.profile).unwrap().trim_matches('"')));
    out.push_str(&format!("timeout_ms = {}\n", config.timeout_ms));
    out.push_str(&format!("fallback_timeout_ms = {}\n", config.fallback_timeout_ms));
    out.push_str(&format!("circuit_breaker = {}\n\n", config.enable_circuit_breaker));

    if let Some(ref p) = config.local_provider {
        out.push_str("[local_provider]\n");
        out.push_str(&format!("kind = \"{}\"\n", p.kind));
        out.push_str(&format!("model = \"{}\"\n", p.model));
        if let Some(ref url) = p.base_url {
            out.push_str(&format!("base_url = \"{}\"\n", url));
        }
        out.push('\n');
    }

    if let Some(ref p) = config.cloud_provider {
        out.push_str("[cloud_provider]\n");
        out.push_str(&format!("kind = \"{}\"\n", p.kind));
        out.push_str(&format!("model = \"{}\"\n", p.model));
        if let Some(ref key) = p.api_key {
            out.push_str(&format!("api_key = \"{}\"\n", key));
        }
        if let Some(ref url) = p.base_url {
            out.push_str(&format!("base_url = \"{}\"\n", url));
        }
        out.push('\n');
    }

    out.push_str("# Database (required for TEAM mode)\n");
    out.push_str("# database_url = \"postgres://gateway:gateway@localhost:5432/gateway\"\n\n");
    out.push_str("# Cache (required for TEAM mode)\n");
    out.push_str("# redis_url = \"redis://localhost:6379\"\n");

    out
}

fn prompt(msg: &str) -> anyhow::Result<String> {
    print!("{}", msg);
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf.trim().to_string())
}

fn prompt_default(msg: &str, default: &str) -> anyhow::Result<String> {
    let input = prompt(&format!("{} [{}]: ", msg, default))?;
    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input)
    }
}

fn prompt_yn(msg: &str) -> anyhow::Result<bool> {
    loop {
        let input = prompt(&format!("{} [Y/n]: ", msg))?;
        match input.trim().to_lowercase().as_str() {
            "" | "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("Please enter 'y' or 'n'."),
        }
    }
}
