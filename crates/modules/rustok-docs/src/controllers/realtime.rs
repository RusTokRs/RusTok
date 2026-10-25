use axum::{
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use rustok_api::{AuthContext, Permission, TenantContext, has_any_effective_permission};
use rustok_web::{HttpError, HttpResult};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tokio::sync::broadcast;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{dto::DocUpdateItemDto, services::DocumentService};

/// Wire messages sent by WebSocket clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Client requests document state synchronization.
    SyncRequest { client_seq_id: Option<i64> },
    /// Client pushes new CRDT deltas generated locally.
    PushDeltas {
        sender_peer_id: u64,
        deltas: Vec<Vec<u8>>,
    },
    /// Ephemeral awareness state (cursor position, selection, user info).
    Awareness {
        sender_peer_id: u64,
        cursor: serde_json::Value,
    },
    /// Heartbeat ping.
    Ping,
}

/// Wire messages sent by the server to connected peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Document synchronization payload (snapshot + updates).
    SyncResponse {
        page_id: Uuid,
        latest_seq_id: i64,
        snapshot: Option<Vec<u8>>,
        updates: Vec<DocUpdateItemDto>,
    },
    /// Broadcasted binary deltas originating from another peer.
    Deltas {
        sender_peer_id: u64,
        deltas: Vec<Vec<u8>>,
    },
    /// Broadcasted ephemeral cursor/presence update.
    Awareness {
        sender_peer_id: u64,
        cursor: serde_json::Value,
    },
    /// Acknowledgment of persisted deltas with new latest sequence number.
    AckDeltas { latest_seq_id: i64 },
    /// Heartbeat pong.
    Pong,
    /// Protocol or permission error.
    Error { code: String, message: String },
}

/// In-memory pub-sub room manager grouping collaborative sessions by (tenant_id, page_id).
#[derive(Default)]
pub struct DocsRoomHub {
    rooms: RwLock<HashMap<(Uuid, Uuid), broadcast::Sender<ServerMessage>>>,
}

impl DocsRoomHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Obtains or initializes the broadcast channel for a collaborative document room.
    pub fn get_or_create_room(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> broadcast::Sender<ServerMessage> {
        let key = (tenant_id, page_id);
        {
            let read_guard = self.rooms.read().unwrap();
            if let Some(sender) = read_guard.get(&key) {
                return sender.clone();
            }
        }
        let mut write_guard = self.rooms.write().unwrap();
        write_guard
            .entry(key)
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(256);
                tx
            })
            .clone()
    }

    /// Broadcasts a server message to all active peers subscribed to the room.
    pub fn broadcast(&self, tenant_id: Uuid, page_id: Uuid, msg: ServerMessage) {
        let key = (tenant_id, page_id);
        let read_guard = self.rooms.read().unwrap();
        if let Some(sender) = read_guard.get(&key) {
            let _ = sender.send(msg);
        }
    }
}

/// Upgrades an incoming HTTP request to a collaborative document WebSocket session.
pub async fn ws_handler(
    State(runtime): State<super::DocsHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(page_id): Path<Uuid>,
    ws: WebSocketUpgrade,
) -> HttpResult<Response> {
    // 1. Ensure docs module is enabled for tenant
    super::ensure_docs_module_enabled(&runtime.db, tenant.id).await?;

    // 2. Enforce read permission before upgrade
    if !has_any_effective_permission(&auth.permissions, &[Permission::PAGES_READ]) {
        return Err(HttpError::forbidden(
            "permission_denied",
            "Permission denied: pages:read required for collaborative session",
        ));
    }

    let db = runtime.db.clone();
    let hub = runtime.room_hub.clone();
    let tenant_id = tenant.id;

    Ok(ws.on_upgrade(move |socket| handle_socket(socket, db, hub, tenant_id, page_id, auth)))
}

async fn handle_socket(
    socket: WebSocket,
    db: DatabaseConnection,
    hub: Arc<DocsRoomHub>,
    tenant_id: Uuid,
    page_id: Uuid,
    auth: AuthContext,
) {
    let (mut sender, mut receiver) = socket.split();
    let room_sender = hub.get_or_create_room(tenant_id, page_id);
    let mut room_receiver = room_sender.subscribe();

    info!(tenant_id = %tenant_id, page_id = %page_id, "Peer joined collaborative session");

    // Spawn task to forward broadcast messages from the room to the client WebSocket
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = room_receiver.recv().await {
            let Ok(json) = serde_json::to_string(&msg) else {
                continue;
            };
            if sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    let room_sender_for_recv = room_sender.clone();
    let db_for_recv = db.clone();
    let auth_for_recv = auth.clone();

    // Process incoming client frames
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(message)) = receiver.next().await {
            match message {
                Message::Text(text) => {
                    let client_msg: Result<ClientMessage, _> = serde_json::from_str(&text);
                    match client_msg {
                        Ok(ClientMessage::SyncRequest { client_seq_id }) => {
                            match DocumentService::get_sync_payload(
                                &db_for_recv,
                                tenant_id,
                                page_id,
                                client_seq_id,
                            )
                            .await
                            {
                                Ok(sync_dto) => {
                                    let resp = ServerMessage::SyncResponse {
                                        page_id,
                                        latest_seq_id: sync_dto.latest_seq_id,
                                        snapshot: sync_dto.snapshot,
                                        updates: sync_dto.updates,
                                    };
                                    let _ = room_sender_for_recv.send(resp);
                                }
                                Err(err) => {
                                    let _ = room_sender_for_recv.send(ServerMessage::Error {
                                        code: "sync_failed".to_string(),
                                        message: err.to_string(),
                                    });
                                }
                            }
                        }
                        Ok(ClientMessage::PushDeltas {
                            sender_peer_id,
                            deltas,
                        }) => {
                            // Check write permissions
                            if !has_any_effective_permission(
                                &auth_for_recv.permissions,
                                &[Permission::PAGES_UPDATE],
                            ) {
                                let _ = room_sender_for_recv.send(ServerMessage::Error {
                                    code: "forbidden".to_string(),
                                    message: "pages:update required to push deltas".to_string(),
                                });
                                continue;
                            }

                            let delta_dto = crate::dto::ApplyDeltaDto { deltas: deltas.clone() };
                            if let Err(err) = delta_dto.validate() {
                                let _ = room_sender_for_recv.send(ServerMessage::Error {
                                    code: "validation_error".to_string(),
                                    message: err.to_string(),
                                });
                                continue;
                            }

                            // 1. Broadcast deltas immediately to connected peers (sub-millisecond latency)
                            let _ = room_sender_for_recv.send(ServerMessage::Deltas {
                                sender_peer_id,
                                deltas: deltas.clone(),
                            });

                            // 2. Persist deltas to PostgreSQL monotonically
                            match DocumentService::append_updates(
                                &db_for_recv,
                                tenant_id,
                                page_id,
                                deltas,
                            )
                            .await
                            {
                                Ok(latest_seq_id) => {
                                    let _ = room_sender_for_recv
                                        .send(ServerMessage::AckDeltas { latest_seq_id });
                                }
                                Err(err) => {
                                    error!(error = %err, page_id = %page_id, "Failed to persist document deltas");
                                    let _ = room_sender_for_recv.send(ServerMessage::Error {
                                        code: "persist_failed".to_string(),
                                        message: err.to_string(),
                                    });
                                }
                            }
                        }
                        Ok(ClientMessage::Awareness {
                            sender_peer_id,
                            cursor,
                        }) => {
                            let _ = room_sender_for_recv.send(ServerMessage::Awareness {
                                sender_peer_id,
                                cursor,
                            });
                        }
                        Ok(ClientMessage::Ping) => {
                            let _ = room_sender_for_recv.send(ServerMessage::Pong);
                        }
                        Err(err) => {
                            warn!(error = %err, "Malformed WebSocket client frame");
                        }
                    }
                }
                Message::Close(_) => {
                    break;
                }
                _ => {}
            }
        }
    });

    // When either task completes, abort the other
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    info!(tenant_id = %tenant_id, page_id = %page_id, "Peer disconnected from collaborative session");
}
