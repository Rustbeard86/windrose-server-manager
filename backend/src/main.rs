mod api;
mod config;
mod embedded;
mod models;
mod pid;
mod process;
mod services;
mod state;

use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::config::AppConfig;
use crate::embedded::EmbeddedAssetsService;
use crate::models::{ServerInfo, ServerStatus};
use crate::services::{log_service, schedule_service};
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "windrose_server_manager=info,tower_http=info".parse().unwrap()),
        )
        .init();

    let app_config = AppConfig::load();
    let socket_addr: SocketAddr = app_config
        .socket_addr()
        .parse()
        .expect("Invalid bind address");

    // Clean up any leftover updater artefacts from a previous self-update.
    services::update_service::cleanup_updater_artefacts();

    let log_file_path = app_config.log_file_path.clone();
    let app_state = AppState::new(app_config);

    // If a PID file exists from a previous session, re-adopt the server state
    // so the dashboard reflects the still-running process.
    if let Some(pid) = pid::read() {
        info!(pid, "Found server PID file — re-adopting running server");
        app_state
            .set_server_info(ServerInfo {
                status: ServerStatus::Running,
                pid: Some(pid),
                uptime_seconds: None,
                started_at: None,
            })
            .await;
    }

    // Load persisted player-event history (if history_file_path is configured).
    app_state.load_history().await;

    // Start log-tailing background task if a log file path is configured.
    if let Some(log_path) = log_file_path {
        info!(path = %log_path.display(), "Log tail enabled");
        log_service::start_log_tail(app_state.clone(), log_path);
    } else {
        info!("No log_file_path configured — log tailing disabled");
    }

    // Start the scheduled-restart background task.
    schedule_service::start_scheduler(app_state.clone());

    // Start the server-stats background task.
    services::stats_service::start_stats_collector(app_state.clone());

    // Build the router.
    let api_router = api::build_router(app_state.clone());
    let app = api_router
        .fallback_service(EmbeddedAssetsService)
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(socket_addr).await?;

    {
        const W: usize = 50;
        let pad = |s: String| {
            let extra = W.saturating_sub(s.len());
            format!("║{}{}║", s, " ".repeat(extra))
        };
        info!("╔{}╗", "═".repeat(W));
        info!("{}", pad(format!("   Windrose Server Manager v{}", env!("CARGO_PKG_VERSION"))));
        info!("╠{}╣", "═".repeat(W));
        info!("{}", pad(format!("  Listening on http://{}", socket_addr)));
        info!("{}", pad(format!("  API:       http://{}/api/health", socket_addr)));
        info!("{}", pad(format!("  WebSocket: ws://{}/ws", socket_addr)));
        info!("{}", pad("  Press Ctrl+C to stop (server keeps running)".to_string()));
        info!("╚{}╝", "═".repeat(W));
    }

    // Serve until Ctrl+C or the apply-update shutdown signal.
    let shutdown = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
        if let Some(pid) = pid::read() {
            info!(pid, "Manager shutting down — game server process will keep running");
        }
        info!("Shutdown signal received");
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;

    Ok(())
}
