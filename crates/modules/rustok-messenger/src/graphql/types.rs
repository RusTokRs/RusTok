use async_graphql::{InputObject, SimpleObject};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Key metrics and runtime topology for the Messenger module.
#[derive(Debug, Clone, SimpleObject)]
pub struct MessengerOverviewGql {
    pub active_conversations_count: usize,
    pub stream_topic: String,
    pub consumer_group: String,
    pub partitions_count: usize,
    pub registered_devices_count: usize,
    pub matrix_server_name: String,
    pub federated_rooms_count: usize,
}

/// Representation of a channel in the administrator control plane.
#[derive(Debug, Clone, SimpleObject)]
pub struct MessengerChannelListItemGql {
    pub id: Uuid,
    pub conversation_type: String,
    pub title: Option<String>,
    pub topic: Option<String>,
    pub member_count: usize,
    pub created_at: DateTime<Utc>,
}

/// Audit view-model for registered E2EE devices and their OTK pool depth.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "MessengerE2eeDeviceAuditGql")]
pub struct MessengerE2eeDeviceAuditGql {
    pub device_id: String,
    pub user_id: Uuid,
    #[graphql(name = "hasCurve25519")]
    pub has_curve25519: bool,
    #[graphql(name = "hasEd25519")]
    pub has_ed25519: bool,
    pub otk_count: usize,
}

/// Input payload for creating a new channel from the admin UI.
#[derive(Debug, Clone, InputObject)]
pub struct CreateMessengerChannelGqlInput {
    pub conversation_type: String,
    pub title: String,
    pub topic: Option<String>,
    pub locale: Option<String>,
}

/// Input payload for uploading public device keys and one-time prekeys.
#[derive(Debug, Clone, InputObject)]
pub struct UploadDeviceKeysGqlInput {
    pub device_id: String,
    pub identity_key: String,
    pub curve25519_key: String,
    pub signature: String,
    pub one_time_prekeys: Vec<OneTimePrekeyGqlInput>,
}

/// Individual one-time prekey in upload payload.
#[derive(Debug, Clone, InputObject)]
pub struct OneTimePrekeyGqlInput {
    pub key_id: String,
    pub key_material: String,
    pub signature: Option<String>,
}

/// Registered device identity output.
#[derive(Debug, Clone, SimpleObject)]
pub struct DeviceKeyGql {
    pub device_id: String,
    pub user_id: Uuid,
    pub identity_key: String,
    pub curve25519_key: String,
    pub signature: String,
    pub created_at: DateTime<Utc>,
}

/// One-time prekey output.
#[derive(Debug, Clone, SimpleObject)]
pub struct OneTimePrekeyGql {
    pub key_id: String,
    pub device_id: String,
    pub key_material: String,
    pub signature: Option<String>,
    pub consumed: bool,
}

/// Prekey bundle output for establishing an Olm Double Ratchet session.
#[derive(Debug, Clone, SimpleObject)]
pub struct PrekeyBundleGql {
    pub user_id: Uuid,
    pub device_id: String,
    pub identity_key: String,
    pub curve25519_key: String,
    pub one_time_prekey: Option<OneTimePrekeyGql>,
}
