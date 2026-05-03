//! WebSocket channel for streaming build events.
//!
//! `GET /ws/builds` — upgrades to a WebSocket connection and streams
//! `BuildEvent` payloads as newline-delimited JSON until the build hub
//! is dropped or the client disconnects.
//!
//! **Authentication**: Bearer token in the `Authorization` header
//! (standard JWT — same as REST endpoints).

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use loco_rs::app::AppContext;
use serde::Serialize;
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

use crate::services::build_event_hub::build_event_hub_from_context;
use crate::services::build_service::BuildEvent;

// ── Wire-format message ───────────────────────────────────────────────────────

/// JSON message sent over the WebSocket for every build event.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)]
enum WsBuildMessage {
    BuildRequested {
        build_id: Uuid,
        requested_by: String,
    },
    BuildStarted {
        build_id: Uuid,
        stage: String,
        progress: i32,
    },
    BuildProgress {
        build_id: Uuid,
        stage: String,
        progress: i32,
    },
    BuildCompleted {
        build_id: Uuid,
        release_id: Option<String>,
    },
    BuildCancelled {
        build_id: Uuid,
        stage: String,
        progress: i32,
    },
    BuildFailed {
        build_id: Uuid,
        stage: String,
        progress: i32,
        error: String,
    },
}

impl From<BuildEvent> for WsBuildMessage {
    fn from(ev: BuildEvent) -> Self {
        match ev {
            BuildEvent::BuildRequested {
                build_id,
                requested_by,
            } => Self::BuildRequested {
                build_id,
                requested_by,
            },
            BuildEvent::BuildStarted {
                build_id,
                stage,
                progress,
            } => Self::BuildStarted {
                build_id,
                stage: format!("{stage:?}").to_lowercase(),
                progress,
            },
            BuildEvent::BuildProgress {
                build_id,
                stage,
                progress,
            } => Self::BuildProgress {
                build_id,
                stage: format!("{stage:?}").to_lowercase(),
                progress,
            },
            BuildEvent::BuildCompleted {
                build_id,
                release_id,
            } => Self::BuildCompleted {
                build_id,
                release_id,
            },
            BuildEvent::BuildCancelled {
                build_id,
                stage,
                progress,
            } => Self::BuildCancelled {
                build_id,
                stage: format!("{stage:?}").to_lowercase(),
                progress,
            },
            BuildEvent::BuildFailed {
                build_id,
                stage,
                progress,
                error,
            } => Self::BuildFailed {
                build_id,
                stage: format!("{stage:?}").to_lowercase(),
                progress,
                error,
            },
        }
    }
}

// ── Handler ───────────────────────────────────────────────────────────────────

/// Upgrade an HTTP request to a WebSocket connection that streams build events.
pub async fn ws_builds(ws: WebSocketUpgrade, State(ctx): State<AppContext>) -> impl IntoResponse {
    let hub = build_event_hub_from_context(&ctx);
    ws.on_upgrade(move |socket| handle_socket(socket, hub))
}

async fn handle_socket(
    mut socket: WebSocket,
    hub: std::sync::Arc<crate::services::build_event_hub::BuildEventHub>,
) {
    let mut rx = hub.subscribe();

    loop {
        tokio::select! {
            // Forward build events to the client
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        let msg: WsBuildMessage = event.into();
                        match serde_json::to_string(&msg) {
                            Ok(json) => {
                                if socket.send(Message::Text(json.into())).await.is_err() {
                                    break; // client disconnected
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "Failed to serialise build event");
                            }
                        }
                    }
                    Err(RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "WebSocket build channel lagged");
                    }
                    Err(RecvError::Closed) => break,
                }
            }

            // Detect client-side close / ping-pong
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(payload))) => {
                        let _ = socket.send(Message::Pong(payload)).await;
                    }
                    _ => {}
                }
            }
        }
    }
}

// ── Route registration ────────────────────────────────────────────────────────

pub fn routes() -> loco_rs::controller::Routes {
    use axum::routing::get;
    loco_rs::controller::Routes::new()
        .prefix("ws")
        .add("/builds", get(ws_builds))
}
