//! Scheduled-restart service.
//!
//! # Overview
//!
//! The scheduler runs as a permanent background tokio task.  Every 30 seconds
//! it checks whether the current local time matches the configured restart
//! time-of-day window.  When a match is found (and the restart has not already
//! fired today) it starts a warning countdown, broadcasting a
//! `schedule_countdown` WebSocket event once per second.
//!
//! # Countdown cancellation
//!
//! `POST /api/schedule/cancel` sets `AppState::schedule_cancel` to `true`.
//! The countdown loop checks this flag each iteration and aborts early.
//!
//! # State machine
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │  Scheduler loop (30 s tick)                              │
//! │                                                          │
//! │  enabled && time_matches && !already_fired_today         │
//! │                    │                                     │
//! │                    ▼                                     │
//! │  Countdown (1 s ticks, broadcasts events)                │
//! │        │                  │                              │
//! │  cancelled?           reached zero?                      │
//! │        │                  │                              │
//! │    abort ◄────────── trigger restart                     │
//! └─────────────────────────────────────────────────────────┘
//! ```

use std::sync::atomic::Ordering;

use chrono::{Local, Timelike as _};
use tokio::time::Duration;
use tracing::{error, info};

use crate::models::WsEvent;
use crate::services::server_service;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Spawn the scheduler background task.
///
/// Should be called once at startup.  The task runs forever and picks up
/// schedule-config changes made at runtime.
pub fn start_scheduler(state: AppState) {
    tokio::spawn(async move {
        run_scheduler(state).await;
    });
}

/// Request cancellation of the active countdown.
///
/// Has no effect if no countdown is currently running.
pub async fn cancel_countdown(state: &AppState) {
    if state.get_schedule_state().await.countdown_active {
        state.schedule_cancel.store(true, Ordering::Relaxed);
        info!("Scheduled-restart countdown cancellation requested");
    }
}

// ---------------------------------------------------------------------------
// Scheduler loop
// ---------------------------------------------------------------------------

async fn run_scheduler(state: AppState) {
    loop {
        // Check every 30 s to keep CPU usage near zero.
        tokio::time::sleep(Duration::from_secs(30)).await;

        let sched = state.get_schedule_state().await;

        if !sched.config.enabled {
            continue;
        }

        // Don't start a second countdown while one is already running.
        if sched.countdown_active {
            continue;
        }

        let now = Local::now();
        let current_hour = now.hour() as u8;
        let current_minute = now.minute() as u8;
        let today = now.format("%Y-%m-%d").to_string();

        // Fire if the current HH:MM matches the configured target AND we have
        // not already restarted today.
        if current_hour == sched.config.restart_hour
            && current_minute == sched.config.restart_minute
            && sched.last_restart_date.as_deref() != Some(&today)
        {
            info!(
                hour = current_hour,
                minute = current_minute,
                "Scheduled restart window reached — starting warning countdown"
            );
            run_countdown(&state, today).await;
        }
    }
}

// ---------------------------------------------------------------------------
// Countdown
// ---------------------------------------------------------------------------

async fn run_countdown(state: &AppState, today: String) {
    let warning_seconds = state.get_schedule_state().await.config.warning_seconds;

    // Ensure the cancel flag is clear before we start.
    state.schedule_cancel.store(false, Ordering::Relaxed);

    state
        .set_countdown_active(true, Some(warning_seconds))
        .await;
    state.event_hub.publish(WsEvent::ScheduleCountdown {
        seconds_remaining: warning_seconds,
        cancelled: false,
    });

    // Tick down one second at a time.
    for remaining in (0..warning_seconds).rev() {
        tokio::time::sleep(Duration::from_secs(1)).await;

        if state.schedule_cancel.load(Ordering::Relaxed) {
            info!("Scheduled-restart countdown cancelled by operator");
            state.schedule_cancel.store(false, Ordering::Relaxed);
            state.set_countdown_active(false, None).await;
            state.event_hub.publish(WsEvent::ScheduleCountdown {
                seconds_remaining: 0,
                cancelled: true,
            });
            state.event_hub.publish(WsEvent::Notification {
                level: "info".to_string(),
                message: "Scheduled restart cancelled".to_string(),
            });
            return;
        }

        state.set_countdown_active(true, Some(remaining)).await;
        state.event_hub.publish(WsEvent::ScheduleCountdown {
            seconds_remaining: remaining,
            cancelled: false,
        });
    }

    // Countdown complete — fire the restart.
    state.set_countdown_active(false, None).await;
    state.set_last_restart_date(Some(today)).await;

    info!("Scheduled-restart countdown complete — triggering restart");
    state.event_hub.publish(WsEvent::Notification {
        level: "info".to_string(),
        message: "Performing scheduled server restart".to_string(),
    });

    if let Err(e) = server_service::restart(state).await {
        error!("Scheduled restart failed: {e}");
        state.event_hub.publish(WsEvent::Notification {
            level: "error".to_string(),
            message: format!("Scheduled restart failed: {e}"),
        });
    }
}
