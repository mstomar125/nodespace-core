//! Pro-tier sync commands invoked from the Svelte frontend.
//!
//! All commands no-op (return early with `Ok`) when the Tauri app is
//! running in community mode — i.e. there is no `ProClient` in
//! managed state. That keeps the frontend's invoke calls
//! side-effect-free when probing for sync UI.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter, Manager};

use crate::services::pro_client::pb::WatchSyncStatusRequest;
use crate::services::{ProClient, ProTier};

/// Flag tracking whether the status-stream task is already running.
/// Module-level so repeated calls to `pro_subscribe_sync_status` from
/// the frontend (e.g. across hot-reloads) don't pile up tasks.
static STREAM_SPAWNED: AtomicBool = AtomicBool::new(false);

/// Snapshot of the most recent tier-detection result. Returned to
/// the frontend on demand so the UI doesn't have to wait for the
/// `pro:tier-detected` Tauri event when re-mounting.
#[tauri::command]
pub async fn pro_tier(app: AppHandle) -> Result<ProTier, String> {
    match app.try_state::<ProClient>() {
        Some(pro) => Ok(pro.tier().await),
        None => Ok(ProTier::Community),
    }
}

/// Start a long-lived `WatchSyncStatus` subscription on the daemon
/// and forward each event to the frontend as a Tauri event named
/// `sync:status`.
///
/// Idempotent: only the first call spawns the task. Subsequent calls
/// return immediately.
#[tauri::command]
pub async fn pro_subscribe_sync_status(app: AppHandle) -> Result<(), String> {
    let Some(pro) = app.try_state::<ProClient>() else {
        // Community mode — nothing to subscribe to.
        return Ok(());
    };
    if STREAM_SPAWNED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let pro: ProClient = (*pro).clone();
    let mut client = pro.client().await;
    let app_handle = app.clone();

    tokio::spawn(async move {
        let stream = match client.watch_sync_status(WatchSyncStatusRequest {}).await {
            Ok(resp) => resp.into_inner(),
            Err(e) => {
                tracing::warn!(error = %e, "sync-status subscribe failed");
                STREAM_SPAWNED.store(false, Ordering::SeqCst);
                return;
            }
        };

        use tokio_stream::StreamExt;
        let mut stream = stream;
        while let Some(item) = stream.next().await {
            match item {
                Ok(evt) => {
                    let payload = serde_json::json!({
                        "state": evt.state,
                        "detail": evt.detail,
                    });
                    if let Err(e) = app_handle.emit("sync:status", payload) {
                        tracing::warn!(error = %e, "failed to emit sync:status");
                        break;
                    }
                }
                Err(status) => {
                    tracing::warn!(error = %status, "sync-status stream item error");
                    break;
                }
            }
        }
        STREAM_SPAWNED.store(false, Ordering::SeqCst);
        tracing::info!("sync-status stream ended");
        // Keep the ProClient alive for the task's lifetime — no
        // explicit drop needed; the move semantics handle it.
        let _ = pro;
    });

    Ok(())
}
