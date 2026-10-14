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
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tokio::sync::broadcast;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    dto::{CursorOutput, FastSyncRequest, FastSyncResponse, MessageOutput, SendMessageInput, UpdateCursorInput},
    services::MessengerService,
};

/// Wire messages sent by WebSocket clients to the messenger.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessengerClientMessage {
    /// Heartbeat ping.
    Ping,
    /// Fast-sync request to catch up with message deltas after reconnect.
    FastSync { since_seq: i64 },
    /// Dispatch a new message in this conversation.
    SendMessage {
        content: String,
        blocks_json: Option<serde_json::Value>,
        reply_to_id: Option<i64>,
        client_msg_id: Option<String>,
    },
    /// Advance read cursor up to a specific message ID.
    ReadCursor { last_read_message_id: i64 },
    /// Ephemeral typing indicator.
    Typing { is_typing: bool },
}

/// Wire messages sent by the server to connected peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessengerServerMessage {
    /// Heartbeat pong.
    Pong,
    /// Response to fast-sync request containing delta messages and latest seq.
    SyncResponse(FastSyncResponse),
    /// Broadcast of a newly posted message.
    NewMessage(MessageOutput),
    /// Broadcast of member read cursor advancement.
    CursorAdvanced(CursorOutput),
    /// Ephemeral typing notification from a room participant.
    TypingIndicator {
        user_id: Uuid,
        is_typing: bool,
    },
    /// Acknowledgment of message persistence.
    Ack {
        client_msg_id: Option<String>,
        message_id: i64,
    },
    /// Protocol or domain error.
    Error {
        code: String,
        message: String,
    },
}

/// In-memory pub-sub room manager grouping real-time sessions by (tenant_id, conversation_id).
#[derive(Default)]
pub struct MessengerRoomHub {
    rooms: RwLock<HashMap<(Uuid, Uuid), broadcast::Sender<MessengerServerMessage>>>,
}

impl MessengerRoomHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Obtains or initializes the broadcast channel for a conversation room.
    pub fn get_or_create_room(
        &self,
        tenant_id: Uuid,
        conversation_id: Uuid,
    ) -> broadcast::Sender<MessengerServerMessage> {
        let key = (tenant_id, conversation_id);
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
                let (tx, _) = broadcast::channel(512);
                tx
            })
            .clone()
    }

    /// Broadcasts a server message to all active peers subscribed to the conversation room.
    pub fn broadcast(
        &self,
        tenant_id: Uuid,
        conversation_id: Uuid,
        msg: MessengerServerMessage,
    ) {
        let key = (tenant_id, conversation_id);
        let read_guard = self.rooms.read().unwrap();
        if let Some(sender) = read_guard.get(&key) {
            let _ = sender.send(msg);
        }
    }
}

/// Upgrades an incoming HTTP request to a real-time conversation WebSocket session.
pub async fn ws_handler(
    State(runtime): State<super::MessengerHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(conversation_id): Path<Uuid>,
    ws: WebSocketUpgrade,
) -> HttpResult<Response> {
    // 1. Ensure messenger module is enabled for tenant
    super::ensure_messenger_module_enabled(&runtime.db, tenant.id).await?;

    // 2. Enforce read permission before upgrade
    if !has_any_effective_permission(&auth.permissions, &[Permission::MESSAGES_READ]) {
        return Err(HttpError::forbidden(
            "PERMISSION_DENIED",
            "Permission denied: messages:read required for real-time messaging",
        ));
    }

    let tenant_id = tenant.id;
    let actor_id = auth.user_id;

    // 3. Ensure actor is an active member of the conversation
    if let Err(err) = runtime.service.ensure_membership(conversation_id, actor_id).await {
        return Err(HttpError::forbidden(
            "NOT_A_MEMBER",
            format!("Actor is not an active member of conversation: {err}"),
        ));
    }

    let hub = runtime.room_hub.clone();
    let service = runtime.service.clone();

    Ok(ws.on_upgrade(move |socket| {
        handle_socket(socket, service, hub, tenant_id, conversation_id, auth)
    }))
}

async fn handle_socket(
    socket: WebSocket,
    service: MessengerService,
    hub: Arc<MessengerRoomHub>,
    tenant_id: Uuid,
    conversation_id: Uuid,
    auth: AuthContext,
) {
    let (mut sender, mut receiver) = socket.split();
    let room_sender = hub.get_or_create_room(tenant_id, conversation_id);
    let mut room_receiver = room_sender.subscribe();
    let actor_id = auth.user_id;

    info!(
        tenant_id = %tenant_id,
        conversation_id = %conversation_id,
        actor_id = %actor_id,
        "Peer joined messenger room"
    );

    // Forward broadcast messages from room to client WebSocket
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
    let service_for_recv = service.clone();
    let permissions = auth.permissions.clone();

    // Process incoming client frames
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(message)) = receiver.next().await {
            match message {
                Message::Text(text) => {
                    let client_msg: Result<MessengerClientMessage, _> = serde_json::from_str(&text);
                    match client_msg {
                        Ok(MessengerClientMessage::Ping) => {
                            let _ = room_sender_for_recv.send(MessengerServerMessage::Pong);
                        }
                        Ok(MessengerClientMessage::FastSync { since_seq }) => {
                            match service_for_recv
                                .fast_sync(
                                    tenant_id,
                                    actor_id,
                                    FastSyncRequest {
                                        conversation_id,
                                        since_seq,
                                        limit: Some(100),
                                    },
                                )
                                .await
                            {
                                Ok(sync_res) => {
                                    let _ = room_sender_for_recv
                                        .send(MessengerServerMessage::SyncResponse(sync_res));
                                }
                                Err(err) => {
                                    let _ = room_sender_for_recv.send(MessengerServerMessage::Error {
                                        code: "SYNC_FAILED".to_string(),
                                        message: err.to_string(),
                                    });
                                }
                            }
                        }
                        Ok(MessengerClientMessage::SendMessage {
                            content,
                            blocks_json,
                            reply_to_id,
                            client_msg_id,
                        }) => {
                            if !has_any_effective_permission(&permissions, &[Permission::MESSAGES_CREATE]) {
                                let _ = room_sender_for_recv.send(MessengerServerMessage::Error {
                                    code: "FORBIDDEN".to_string(),
                                    message: "Permission denied: messages:create required".to_string(),
                                });
                                continue;
                            }

                            match service_for_recv
                                .send_message(
                                    tenant_id,
                                    actor_id,
                                    SendMessageInput {
                                        conversation_id,
                                        message_type: None,
                                        content,
                                        blocks_json,
                                        reply_to_id,
                                        is_encrypted: None,
                                        encryption_algo: None,
                                    },
                                )
                                .await
                            {
                                Ok(output) => {
                                    // Acknowledge back to sender
                                    let _ = room_sender_for_recv.send(MessengerServerMessage::Ack {
                                        client_msg_id,
                                        message_id: output.id,
                                    });
                                    // Broadcast new message to all peers in room
                                    let _ = room_sender_for_recv
                                        .send(MessengerServerMessage::NewMessage(output));
                                }
                                Err(err) => {
                                    let _ = room_sender_for_recv.send(MessengerServerMessage::Error {
                                        code: "SEND_FAILED".to_string(),
                                        message: err.to_string(),
                                    });
                                }
                            }
                        }
                        Ok(MessengerClientMessage::ReadCursor {
                            last_read_message_id,
                        }) => {
                            match service_for_recv
                                .update_cursor(
                                    tenant_id,
                                    actor_id,
                                    UpdateCursorInput {
                                        conversation_id,
                                        last_read_message_id,
                                    },
                                )
                                .await
                            {
                                Ok(cursor_output) => {
                                    let _ = room_sender_for_recv.send(
                                        MessengerServerMessage::CursorAdvanced(cursor_output),
                                    );
                                }
                                Err(err) => {
                                    let _ = room_sender_for_recv.send(MessengerServerMessage::Error {
                                        code: "CURSOR_UPDATE_FAILED".to_string(),
                                        message: err.to_string(),
                                    });
                                }
                            }
                        }
                        Ok(MessengerClientMessage::Typing { is_typing }) => {
                            let _ = room_sender_for_recv.send(
                                MessengerServerMessage::TypingIndicator {
                                    user_id: actor_id,
                                    is_typing,
                                },
                            );
                        }
                        Err(parse_err) => {
                            warn!("Invalid messenger client message payload: {parse_err}");
                            let _ = room_sender_for_recv.send(MessengerServerMessage::Error {
                                code: "INVALID_PAYLOAD".to_string(),
                                message: parse_err.to_string(),
                            });
                        }
                    }
                }
                Message::Ping(_) => {
                    let _ = room_sender_for_recv.send(MessengerServerMessage::Pong);
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Terminate when either socket stream completes
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    info!(
        tenant_id = %tenant_id,
        conversation_id = %conversation_id,
        actor_id = %actor_id,
        "Peer disconnected from messenger room"
    );
}
