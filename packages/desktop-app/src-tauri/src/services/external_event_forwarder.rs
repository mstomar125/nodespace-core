//! External-daemon-mode replacement for `DomainEventForwarder`.
//!
//! When the desktop app dials a remote `nodespaced` / `nodespaced-pro`
//! (NODESPACED_ADDR set), the in-process `NodeService` broadcast does
//! not exist, so `DomainEventForwarder` can't run. Instead, this
//! module subscribes to the daemon's `NodeService.WatchNodes`
//! server-streaming RPC and re-emits each event as the same Tauri
//! event names the in-process forwarder uses — so the Svelte
//! frontend doesn't care which mode it's running in.
//!
//! Out of scope for now: relationship events. `WatchNodes` returns
//! node events only; the parent-child tree will reconcile on the
//! next outliner refresh. The Pro demo needs cross-window node
//! visibility, which this delivers; nested ordering correctness is a
//! follow-up (the daemon would need a sibling stream for relationship
//! events).

use nodespace_daemon::nodespace::node_event::Event as NodeEventKind;
use nodespace_daemon::nodespace::WatchRequest;
use nodespace_daemon::NodeServiceClient;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio_stream::StreamExt;
use tonic::transport::Channel;

/// Mirror of the payload the in-process `DomainEventForwarder` emits
/// for `node:*` events (id + optional type). The frontend treats
/// this as a "something changed for this id" signal and fetches
/// detail via `GetNode` if it needs to re-render.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeIdPayload {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    node_type: Option<String>,
}

/// Mirror of the in-process `RelationshipEvent` shape (camelCase via
/// the core struct's serde rename) that the frontend's
/// `tauri-sync-listener.ts` parses for `relationship:created` /
/// `relationship:updated`. Properties is parsed back from the proto's
/// JSON-encoded string so the frontend gets a real object (the
/// `has_child` listener reads `properties.order`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RelationshipPayloadOut {
    id: String,
    from_id: String,
    to_id: String,
    relationship_type: String,
    properties: serde_json::Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RelationshipDeletedOut {
    id: String,
    from_id: String,
    to_id: String,
    relationship_type: String,
}

/// Spawn the subscription task. Returns immediately; the task owns
/// its own gRPC client and keeps running until the daemon closes the
/// stream or the app shuts down.
pub fn spawn(app: AppHandle, channel: Channel) {
    tauri::async_runtime::spawn(async move {
        let mut client = NodeServiceClient::new(channel);

        // Empty filter = stream all events (the daemon currently
        // ignores filter fields anyway — issue #1114 Non-Goal).
        let stream = match client.watch_nodes(WatchRequest::default()).await {
            Ok(resp) => resp.into_inner(),
            Err(e) => {
                tracing::warn!(error = %e, "WatchNodes subscribe failed; frontend will not auto-refresh");
                return;
            }
        };

        tracing::info!("WatchNodes stream subscribed; forwarding to Tauri events");
        let mut stream = stream;
        while let Some(item) = stream.next().await {
            match item {
                Ok(node_event) => {
                    let Some(kind) = node_event.event else {
                        tracing::debug!("WatchNodes received empty event variant; skipping");
                        continue;
                    };
                    match kind {
                        NodeEventKind::Created(data) => {
                            tracing::info!(node_id = %data.id, "forward node:created");
                            let payload = NodeIdPayload {
                                id: data.id,
                                node_type: Some(data.node_type),
                            };
                            if let Err(e) = app.emit("node:created", &payload) {
                                tracing::warn!(error = %e, "failed to emit node:created");
                            }
                        }
                        NodeEventKind::Updated(data) => {
                            tracing::info!(node_id = %data.id, "forward node:updated");
                            let payload = NodeIdPayload {
                                id: data.id,
                                node_type: None,
                            };
                            if let Err(e) = app.emit("node:updated", &payload) {
                                tracing::warn!(error = %e, "failed to emit node:updated");
                            }
                        }
                        NodeEventKind::Deleted(d) => {
                            tracing::info!(node_id = %d.node_id, "forward node:deleted");
                            let payload = NodeIdPayload {
                                id: d.node_id,
                                node_type: Some(d.node_type),
                            };
                            if let Err(e) = app.emit("node:deleted", &payload) {
                                tracing::warn!(error = %e, "failed to emit node:deleted");
                            }
                        }
                        NodeEventKind::RelationshipCreated(r) => {
                            tracing::info!(
                                rel_id = %r.id,
                                rel_type = %r.relationship_type,
                                "forward relationship:created"
                            );
                            let props = serde_json::from_str(&r.properties)
                                .unwrap_or(serde_json::Value::Object(Default::default()));
                            let payload = RelationshipPayloadOut {
                                id: r.id,
                                from_id: r.from_id,
                                to_id: r.to_id,
                                relationship_type: r.relationship_type,
                                properties: props,
                            };
                            if let Err(e) = app.emit("relationship:created", &payload) {
                                tracing::warn!(error = %e, "failed to emit relationship:created");
                            }
                        }
                        NodeEventKind::RelationshipUpdated(r) => {
                            tracing::info!(
                                rel_id = %r.id,
                                rel_type = %r.relationship_type,
                                "forward relationship:updated"
                            );
                            let props = serde_json::from_str(&r.properties)
                                .unwrap_or(serde_json::Value::Object(Default::default()));
                            let payload = RelationshipPayloadOut {
                                id: r.id,
                                from_id: r.from_id,
                                to_id: r.to_id,
                                relationship_type: r.relationship_type,
                                properties: props,
                            };
                            if let Err(e) = app.emit("relationship:updated", &payload) {
                                tracing::warn!(error = %e, "failed to emit relationship:updated");
                            }
                        }
                        NodeEventKind::RelationshipDeleted(r) => {
                            tracing::info!(rel_id = %r.id, "forward relationship:deleted");
                            let payload = RelationshipDeletedOut {
                                id: r.id,
                                from_id: r.from_id,
                                to_id: r.to_id,
                                relationship_type: r.relationship_type,
                            };
                            if let Err(e) = app.emit("relationship:deleted", &payload) {
                                tracing::warn!(error = %e, "failed to emit relationship:deleted");
                            }
                        }
                    }
                }
                Err(status) => {
                    tracing::warn!(error = %status, "WatchNodes stream errored; stopping forwarder");
                    break;
                }
            }
        }
        tracing::info!("WatchNodes stream ended");
    });
}
