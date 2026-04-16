mod api;
mod config;
mod embedded;
mod models;
mod pid;
mod process;
mod services;
mod state;

use std::net::SocketAddr;
use axum::extract::{Host, State};
use axum::http::{StatusCode, Uri};
use axum::response::Redirect;
use axum::routing::any;
use axum::Router;
use axum_server::Handle;
use axum_server::tls_rustls::RustlsConfig;
use tokio::net::TcpListener;
use tokio::time::{interval, Duration};
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::config::AppConfig;
use crate::embedded::EmbeddedAssetsService;
use crate::models::{ServerInfo, ServerStatus};
use crate::services::{log_service, schedule_service};
use crate::state::AppState;

#[derive(Clone)]
struct RedirectState {
    https_port: u16,
}

async fn redirect_http_to_https(
    Host(host): Host,
    uri: Uri,
    State(state): State<RedirectState>,
) -> Result<Redirect, StatusCode> {
    let host_base = host.split(':').next().unwrap_or(host.as_str());
    let authority = if state.https_port == 443 {
        host_base.to_string()
    } else {
        format!("{host_base}:{}", state.https_port)
    };
    let path_q = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");
    Ok(Redirect::permanent(&format!("https://{authority}{path_q}")))
}

async fn wait_for_shutdown() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for Ctrl+C");
    if let Some(pid) = pid::read() {
        info!(pid, "Manager shutting down — game server process will keep running");
    }
    info!("Shutdown signal received");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_ansi(false)
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

    let _ = services::config_service::load_server_config(&app_state).await;
    let _ = services::config_service::load_world_config(&app_state).await;

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

    // Monitor process liveness, keep uptime fresh, and refresh dashboard config.
    {
        let state = app_state.clone();
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(2));
            loop {
                ticker.tick().await;

                let current = state.get_server_info().await;
                if matches!(current.status, ServerStatus::Running | ServerStatus::Starting | ServerStatus::Stopping) {
                    if let Some(pid) = current.pid {
                        if process::pid_is_running(pid) {
                            if matches!(current.status, ServerStatus::Running | ServerStatus::Starting) {
                                let uptime_seconds = current.started_at.map(|started_at| {
                                    chrono::Utc::now()
                                        .signed_duration_since(started_at)
                                        .num_seconds()
                                        .max(0) as u64
                                });

                                state
                                    .set_server_info(ServerInfo {
                                        status: ServerStatus::Running,
                                        pid: Some(pid),
                                        uptime_seconds,
                                        started_at: current.started_at,
                                    })
                                    .await;
                            }

                            let _ = services::config_service::load_server_config(&state).await;
                        } else {
                            *state.process.lock().await = None;
                            pid::remove();
                            state.clear_players().await;
                            state.set_server_stats(None).await;
                            state
                                .set_server_info(ServerInfo {
                                    status: if current.status == ServerStatus::Stopping {
                                        ServerStatus::Stopped
                                    } else {
                                        ServerStatus::Crashed
                                    },
                                    pid: None,
                                    uptime_seconds: current.uptime_seconds,
                                    started_at: current.started_at,
                                })
                                .await;
                        }
                    }
                }
            }
        });
    }

    // Periodic auth maintenance (currently audit retention cleanup).
    {
        let auth = app_state.auth.clone();
        let retention_days = app_state.config.audit_retention_days;
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(6 * 60 * 60));
            loop {
                ticker.tick().await;
                match auth.cleanup_audit_events(retention_days) {
                    Ok(deleted) if deleted > 0 => {
                        tracing::info!(deleted, retention_days, "Pruned old auth audit events");
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(retention_days, "Failed to prune auth audit events: {e}");
                    }
                }
            }
        });
    }

    // Build the router.
    let api_router = api::build_router(app_state.clone());
    let app = api_router
        .fallback_service(EmbeddedAssetsService)
        .layer(TraceLayer::new_for_http());

    let https_socket_addr: SocketAddr = app_state
        .config
        .tls_socket_addr()
        .parse()
        .expect("Invalid TLS bind address");

    {
        const W: usize = 50;
        let pad = |s: String| {
            let extra = W.saturating_sub(s.len());
            format!("║{}{}║", s, " ".repeat(extra))
        };
        info!("╔{}╗", "═".repeat(W));
        info!("{}", pad(format!("   Windrose Server Manager v{}", env!("CARGO_PKG_VERSION"))));
        info!("╠{}╣", "═".repeat(W));
        if app_state.config.tls_enabled {
            info!("{}", pad(format!("  Listening on https://{}", https_socket_addr)));
            info!("{}", pad(format!("  API:       https://{}/api/health", https_socket_addr)));
            info!("{}", pad(format!("  WebSocket: wss://{}/ws", https_socket_addr)));
            if app_state.config.http_redirect_enabled {
                info!(
                    "{}",
                    pad(format!("  HTTP redirect: http://{}", app_state.config.http_redirect_socket_addr()))
                );
            }
        } else {
            info!("{}", pad(format!("  Listening on http://{}", socket_addr)));
            info!("{}", pad(format!("  API:       http://{}/api/health", socket_addr)));
            info!("{}", pad(format!("  WebSocket: ws://{}/ws", socket_addr)));
        }
        info!("{}", pad("  Press Ctrl+C to stop (server keeps running)".to_string()));
        info!("╚{}╝", "═".repeat(W));
    }

    if app_state.config.tls_enabled {
        let cert_path = app_state
            .config
            .tls_cert_path
            .clone()
            .ok_or_else(|| anyhow::anyhow!("TLS is enabled but tls_cert_path is not configured"))?;
        let key_path = app_state
            .config
            .tls_key_path
            .clone()
            .ok_or_else(|| anyhow::anyhow!("TLS is enabled but tls_key_path is not configured"))?;

        let tls_config = RustlsConfig::from_pem_file(cert_path, key_path).await?;

        if app_state.config.http_redirect_enabled {
            let redirect_addr: SocketAddr = app_state
                .config
                .http_redirect_socket_addr()
                .parse()
                .expect("Invalid HTTP redirect bind address");
            let redirect_state = RedirectState {
                https_port: app_state.config.tls_port,
            };
            tokio::spawn(async move {
                match TcpListener::bind(redirect_addr).await {
                    Ok(listener) => {
                        let redirect_app = Router::new()
                            .fallback(any(redirect_http_to_https))
                            .with_state(redirect_state);
                        if let Err(e) = axum::serve(listener, redirect_app).await {
                            tracing::error!("HTTP redirect server failed: {e}");
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to bind HTTP redirect listener: {e}");
                    }
                }
            });
        }

        let handle = Handle::new();
        let shutdown_handle = handle.clone();
        tokio::spawn(async move {
            wait_for_shutdown().await;
            shutdown_handle.graceful_shutdown(None);
        });

        axum_server::bind_rustls(https_socket_addr, tls_config)
            .handle(handle)
            .serve(app.into_make_service())
            .await?;
    } else {
        let listener = TcpListener::bind(socket_addr).await?;
        axum::serve(listener, app)
            .with_graceful_shutdown(wait_for_shutdown())
            .await?;
    }

    Ok(())
}
