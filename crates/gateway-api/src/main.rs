use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    gateway_observability::init_tracing();

    let state = gateway_api::state::AppState::from_env().await?;
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
