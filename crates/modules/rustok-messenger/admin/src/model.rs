use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessengerOverviewPayload {
    pub active_conversations_count: usize,
    pub stream_topic: String,
    pub consumer_group: String,
    pub partitions_count: usize,
    pub registered_devices_count: usize,
    pub matrix_server_name: String,
    pub federated_rooms_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelListItemPayload {
    pub id: Uuid,
    pub conversation_type: String,
    pub title: Option<String>,
    pub topic: Option<String>,
    pub member_count: usize,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateChannelPayload {
    pub conversation_type: String,
    pub title: String,
    pub topic: Option<String>,
    pub locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct E2eeDeviceAuditPayload {
    pub device_id: String,
    pub user_id: Uuid,
    pub has_curve25519: bool,
    pub has_ed25519: bool,
    pub otk_count: usize,
}
