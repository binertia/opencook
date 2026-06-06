use std::net::SocketAddr;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    gateway_observability::init_tracing();

    let cmd = gateway_api::cli::Command::from_args();

    match cmd {
        gateway_api::cli::Command::Help => {
            println!("{}", gateway_api::cli::Command::help_text());
            Ok(())
        }
        gateway_api::cli::Command::Config => gateway_api::config_wizard::run().await,
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
        gateway_api::cli::Command::Serve => run_server().await,
    }
}

async fn run_server() -> anyhow::Result<()> {
    // Initialize Prometheus metrics exporter
    let _metrics_handle = gateway_observability::metrics::init_metrics();
    tracing::info!("Prometheus metrics initialized at /metrics");

    let state = gateway_api::state::AppState::from_env().await?;
    let config = state.config.clone();

    // Shutdown state shared across the application
    let shutdown = gateway_api::shutdown::ShutdownState::new();

    // Config reload channel
    let (reload_tx, mut reload_rx) = tokio::sync::mpsc::channel::<()>(4);

    // Spawn signal handlers (SIGTERM, SIGINT, SIGHUP, SIGUSR1)
    gateway_api::shutdown::spawn_signal_handler(shutdown.clone(), reload_tx);

    // Spawn config reload listener
    let reload_config = config.clone();
    tokio::spawn(async move {
        while let Some(()) = reload_rx.recv().await {
            gateway_api::config_reload::handle_reload(reload_config.clone()).await;
        }
    });

    // Spawn health check background worker
    let health_worker = gateway_api::health_worker::HealthWorker::new(
        state.db_pool.clone(),
        state.redis.clone(),
        30, // interval: 30 seconds
        10, // timeout: 10 seconds per provider
        state.config.master_key,
    )
    .with_circuit_breaker(state.circuit_breaker.clone());
    let _health_shutdown = health_worker.spawn();

    let app = gateway_api::router::build_router(state.clone());

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    // TLS mode
    if let (Some(cert), Some(key)) = (&state.config.tls_cert, &state.config.tls_key) {
        let tls_config = gateway_api::tls::TlsConfig::from_env(cert, key);
        let rustls_config = tls_config.to_server_config()?;
        tracing::info!("gateway-api listening with TLS on https://{}", addr);

        let tls_config = axum_server::tls_rustls::RustlsConfig::from_config(rustls_config);
        let handle = axum_server::Handle::new();
        let server_handle = handle.clone();

        let server = axum_server::bind_rustls(addr, tls_config)
            .handle(server_handle)
            .serve(app.into_make_service());

        tokio::select! {
            result = server => {
                if let Err(e) = result {
                    tracing::error!(error = %e, "TLS server error");
                }
            }
            _ = shutdown.notified() => {
                tracing::info!("Shutdown signal received, initiating graceful shutdown");
                handle.graceful_shutdown(Some(Duration::from_secs(30)));
                // Server future will now complete when connections drain or timeout
            }
        }
    } else {
        tracing::info!("gateway-api listening on http://{}", addr);
        let listener = tokio::net::TcpListener::bind(addr).await?;

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown.notified())
            .await?;
    }

    // Drain in-flight requests (up to 30s)
    gateway_api::shutdown::wait_for_shutdown(shutdown, Duration::from_secs(30)).await;

    tracing::info!("gateway-api exited cleanly");
    Ok(())
}
