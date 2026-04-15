mod api;
mod config;
mod models;
mod process;
mod services;
mod state;

use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::config::AppConfig;
use crate::services::log_service;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialise structured logging. The `RUST_LOG` environment variable
    // controls verbosity; defaults to `info` when not set.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "windrose_server_manager=info,tower_http=info".parse().unwrap()),
        )
        .init();

    let app_config = AppConfig::default();
    let socket_addr: SocketAddr = app_config
        .socket_addr()
        .parse()
        .expect("Invalid bind address");

    let static_dir = app_config.static_dir.clone();
    let log_file_path = app_config.log_file_path.clone();
    let app_state = AppState::new(app_config);

    // Start log-tailing background task if a log file path is configured.
    if let Some(log_path) = log_file_path {
        info!(path = %log_path.display(), "Log tail enabled");
        log_service::start_log_tail(app_state.clone(), log_path);
    } else {
        info!("No log_file_path configured — log tailing disabled");
    }

    // Build the API router.
    let api_router = api::build_router(app_state.clone());

    // Compose: API routes + static file serving from ./static (with index.html
    // fallback for SPA routing).
    let app = api_router
        .fallback_service(
            ServeDir::new(&static_dir)
                .append_index_html_on_directories(true),
        )
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(socket_addr).await?;

    // Width of the version column in the startup banner box.
    const VERSION_COLUMN_WIDTH: usize = 36;
    info!(
        "╔══════════════════════════════════════════════════╗"
    );
    info!(
        "║   Windrose Server Manager v{}{}║",
        env!("CARGO_PKG_VERSION"),
        " ".repeat(VERSION_COLUMN_WIDTH.saturating_sub(env!("CARGO_PKG_VERSION").len()))
    );
    info!(
        "╠══════════════════════════════════════════════════╣"
    );
    info!("║  Listening on http://{}                  ║", socket_addr);
    info!("║  API:       http://{}/api/health        ║", socket_addr);
    info!("║  WebSocket: ws://{}/ws               ║", socket_addr);
    info!("║  Press Ctrl+C to stop                            ║");
    info!(
        "╚══════════════════════════════════════════════════╝"
    );

    axum::serve(listener, app).await?;

    Ok(())
}
