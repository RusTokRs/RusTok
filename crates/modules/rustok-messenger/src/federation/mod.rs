use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    dto::{MessageOutput, SendMessageInput},
    error::MessengerResult,
    services::MessengerService,
};

/// Mapping between a native RusToK conversation and an external Matrix room.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MatrixRoomBridge {
    pub conversation_id: Uuid,
    pub matrix_room_id: String,
    pub matrix_room_alias: Option<String>,
    pub is_federated: bool,
    pub created_at: DateTime<Utc>,
}

/// Canonical Matrix event representation for external client-server and server-server transport.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MatrixEventBridge {
    pub event_id: String,
    pub room_id: String,
    pub sender: String,
    pub event_type: String,
    pub origin_server_ts: i64,
    pub content: serde_json::Value,
}

impl MatrixEventBridge {
    /// Projects a native RusToK message into an RFC-compliant Matrix `m.room.message` event.
    pub fn from_message(msg: &MessageOutput, matrix_room_id: &str, server_name: &str) -> Self {
        let event_id = format!("${}:{server_name}", msg.id);
        let sender = format!("@{}:{server_name}", msg.sender_id);
        let origin_server_ts = msg.created_at.timestamp_millis();

        let content = if msg.is_encrypted {
            serde_json::json!({
                "algorithm": msg.encryption_algo.as_deref().unwrap_or("m.megolm.v1.aes-sha2"),
                "ciphertext": msg.content,
            })
        } else {
            serde_json::json!({
                "msgtype": "m.text",
                "body": msg.content,
                "rustok_blocks": msg.blocks_json,
            })
        };

        let event_type = if msg.is_encrypted {
            "m.room.encrypted".to_string()
        } else {
            "m.room.message".to_string()
        };

        Self {
            event_id,
            room_id: matrix_room_id.to_string(),
            sender,
            event_type,
            origin_server_ts,
            content,
        }
    }
}

/// Inbound payload received from an external Matrix homeserver or Element client bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMatrixMessagePayload {
    pub matrix_room_id: String,
    pub matrix_event_id: String,
    pub matrix_sender: String,
    pub body: String,
    pub blocks_json: Option<serde_json::Value>,
    pub is_encrypted: bool,
    pub encryption_algo: Option<String>,
}

/// Cross-boundary port seam for Matrix federation gateways (e.g. `rustok-matrix-gateway`).
pub struct MatrixGatewayAdapter<'a> {
    service: &'a MessengerService,
    server_name: String,
}

impl<'a> MatrixGatewayAdapter<'a> {
    pub fn new(service: &'a MessengerService, server_name: impl Into<String>) -> Self {
        Self {
            service,
            server_name: server_name.into(),
        }
    }

    /// Server domain name used for Matrix federation ID generation (e.g. `@user:domain.com`).
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// Ingests an inbound Matrix event received from federation into the sovereign conversation.
    pub async fn ingest_inbound_matrix_event(
        &self,
        tenant_id: Uuid,
        conversation_id: Uuid,
        actor_id: Uuid,
        payload: InboundMatrixMessagePayload,
    ) -> MessengerResult<MessageOutput> {
        self.service
            .send_message(
                tenant_id,
                actor_id,
                SendMessageInput {
                    conversation_id,
                    message_type: None,
                    content: payload.body,
                    blocks_json: payload.blocks_json,
                    reply_to_id: None,
                    is_encrypted: Some(payload.is_encrypted),
                    encryption_algo: payload.encryption_algo,
                },
            )
            .await
    }

    /// Exports an outbound RusToK message into an external Matrix event suitable for federation.
    pub fn export_outbound_matrix_event(
        &self,
        msg: &MessageOutput,
        matrix_room_id: &str,
    ) -> MatrixEventBridge {
        MatrixEventBridge::from_message(msg, matrix_room_id, &self.server_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_event_projection_plaintext_and_encrypted() {
        let tenant_id = Uuid::now_v7();
        let conv_id = Uuid::now_v7();
        let sender_id = Uuid::now_v7();
        let now = Utc::now();

        let plain_msg = MessageOutput {
            id: 100200300,
            tenant_id,
            conversation_id: conv_id,
            sender_id,
            message_type: crate::domain::MessageType::Text,
            content: "Hello Element / Matrix!".into(),
            blocks_json: None,
            is_encrypted: false,
            encryption_algo: None,
            reply_to_id: None,
            created_at: now,
            is_deleted: false,
        };

        let matrix_event =
            MatrixEventBridge::from_message(&plain_msg, "!room123:rustok.org", "rustok.org");

        assert_eq!(matrix_event.event_id, "$100200300:rustok.org");
        assert_eq!(matrix_event.sender, format!("@{sender_id}:rustok.org"));
        assert_eq!(matrix_event.event_type, "m.room.message");
        assert_eq!(matrix_event.content["body"], "Hello Element / Matrix!");

        // Encrypted message projection
        let mut enc_msg = plain_msg.clone();
        enc_msg.is_encrypted = true;
        enc_msg.encryption_algo = Some("m.megolm.v1.aes-sha2".into());
        enc_msg.content = "ciphertext_blob_base64".into();

        let enc_event =
            MatrixEventBridge::from_message(&enc_msg, "!room123:rustok.org", "rustok.org");

        assert_eq!(enc_event.event_type, "m.room.encrypted");
        assert_eq!(enc_event.content["ciphertext"], "ciphertext_blob_base64");
        assert_eq!(enc_event.content["algorithm"], "m.megolm.v1.aes-sha2");
    }
}
