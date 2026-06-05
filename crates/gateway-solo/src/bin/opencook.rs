#[tokio::main]
async fn main() -> anyhow::Result<()> {
    gateway_observability::init_tracing();

    let cmd = gateway_solo::parse_cli();

    match cmd {
        gateway_solo::CliCommand::Config => gateway_solo::run_config_wizard().await,
        gateway_solo::CliCommand::Profile => {
            let config = gateway_solo::state::AppConfig::load();
            let bin = gateway_solo::bin_name();
            println!("OpenCook");
            println!("========");
            println!("Active profile: {}", config.profile.display_name());
            println!("  Description: {}", config.profile.description());
            println!("  Strategy:    {}", config.profile.default_strategy());
            println!("\nRun `{} config` to change your profile.", bin);
            Ok(())
        }
        gateway_solo::CliCommand::Serve => gateway_solo::run_server().await,
    }
}
