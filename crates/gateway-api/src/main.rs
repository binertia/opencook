use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    gateway_observability::init_tracing();

    let cmd = gateway_api::cli::Command::from_args();

    match cmd {
        gateway_api::cli::Command::Help => {
            println!("{}", gateway_api::cli::Command::help_text());
            Ok(())
        }
        gateway_api::cli::Command::Config => {
            gateway_api::config_wizard::run().await
        }
        gateway_api::cli::Command::Profile => {
            let config = gateway_api::state::AppConfig::load();
            println!("AI Gateway Profile");
            println!("==================");
            println!("Active profile: {}", config.profile.display_name());
            println!("  Description: {}", config.profile.description());
            println!("  Strategy:    {}", config.profile.default_strategy());
            println!("  Savings:     {}", config.profile.estimated_savings());
            println!("\nRun `gateway config` to change your profile.");
            Ok(())
        }
        gateway_api::cli::Command::Dashboard => {
            let config = gateway_api::state::AppConfig::load();
            gateway_api::dashboard::run(config.profile.display_name().to_string()).await
        }
        gateway_api::cli::Command::Serve => {
            run_server().await
        }
    }
}

async fn run_server() -> anyhow::Result<()> {
    // Initialize Prometheus metrics exporter
    let _metrics_handle = gateway_observability::metrics::init_metrics();
    tracing::info!("Prometheus metrics initialized at /metrics");

    let state = gateway_api::state::AppState::from_env().await?;

    // Spawn health check background worker
    let health_worker = gateway_api::health_worker::HealthWorker::new(
        state.db_pool.clone(),
        state.redis.clone(),
        30, // interval: 30 seconds
        10, // timeout: 10 seconds per provider
    )
    .with_circuit_breaker(state.circuit_breaker.clone());
    let _health_shutdown = health_worker.spawn();

    let app = gateway_api::router::build_router(state);

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("gateway-api listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
