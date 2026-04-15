use chrono::Utc;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use tracing::debug;

use crate::models::{ServerStats, ServerStatus, WsEvent};
use crate::state::AppState;

/// Interval between stat collection cycles, in seconds.
const INTERVAL_SECS: u64 = 2;

/// Spawn a background task that periodically collects resource-usage stats
/// for the running server process and broadcasts them over the event hub.
pub fn start_stats_collector(state: AppState) {
    tokio::spawn(async move {
        let mut sys = System::new();

        // Initialise network tracking
        let mut nets = sysinfo::Networks::new_with_refreshed_list();
        // Discard first sample — counts are cumulative since boot and not per-sec yet
        nets.refresh();

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(INTERVAL_SECS)).await;

            let server_info = state.get_server_info().await;
            if server_info.status != ServerStatus::Running {
                // Clear stale stats when the server is not running.
                if state.get_server_stats().await.is_some() {
                    state.set_server_stats(None).await;
                }
                // Keep refreshing networks so the baseline stays current.
                nets.refresh();
                continue;
            }

            let pid = match server_info.pid {
                Some(p) => Pid::from_u32(p),
                None => {
                    nets.refresh();
                    continue;
                }
            };

            // ── Process CPU & memory ──────────────────────────────────────────
            sys.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[pid]),

                ProcessRefreshKind::new().with_cpu().with_memory(),
            );

            let (cpu_raw, memory_bytes) = sys
                .process(pid)
                .map(|p| (p.cpu_usage(), p.memory()))
                .unwrap_or((0.0, 0));

            // Normalise CPU to 0-100 % across all logical cores.
            sys.refresh_cpu_all();
            let num_cpus = sys.cpus().len().max(1) as f32;
            let cpu_percent = (cpu_raw / num_cpus).clamp(0.0, 100.0);

            // ── System total memory ───────────────────────────────────────────
            sys.refresh_memory();
            let memory_total_bytes = sys.total_memory();

            // ── Network (system-wide, per-second average) ─────────────────────
            nets.refresh();
            let net_rx: u64 = nets.iter().map(|(_, n)| n.received()).sum();
            let net_tx: u64 = nets.iter().map(|(_, n)| n.transmitted()).sum();
            // Each `received()`/`transmitted()` value is bytes since the last
            // refresh, which happens at our INTERVAL_SECS cadence, so divide
            // to get a per-second rate.
            let net_rx_bytes_per_sec = net_rx / INTERVAL_SECS;
            let net_tx_bytes_per_sec = net_tx / INTERVAL_SECS;

            // ── Disk folder size ──────────────────────────────────────────────
            let binary_dir = crate::config::AppConfig::binary_dir();
            let disk_used_bytes = tokio::task::spawn_blocking(move || {
                binary_dir.map(|d| folder_size_bytes(&d)).unwrap_or(0)
            })
            .await
            .unwrap_or(0);

            // ── Publish ───────────────────────────────────────────────────────
            let stats = ServerStats {
                cpu_percent,
                memory_bytes,
                memory_total_bytes,
                disk_used_bytes,
                net_rx_bytes_per_sec,
                net_tx_bytes_per_sec,
                collected_at: Utc::now(),
            };

            debug!(
                cpu = %format!("{:.1}%", stats.cpu_percent),
                mem_mb = stats.memory_bytes / 1_048_576,
                "stats collected"
            );

            state
                .event_hub
                .publish(WsEvent::StatsUpdated(stats.clone()));
            state.set_server_stats(Some(stats)).await;
        }
    });
}

/// Recursively sum the size of every file under `path`.
fn folder_size_bytes(path: &std::path::Path) -> u64 {
    if !path.is_dir() {
        return 0;
    }
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            } else if p.is_dir() {
                total += folder_size_bytes(&p);
            }
        }
    }
    total
}
