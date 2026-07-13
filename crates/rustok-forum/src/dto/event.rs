use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct ForumDomainEventQuery {
    pub after_sequence: Option<i64>,
    pub aggregate_type: Option<String>,
    pub aggregate_id: Option<Uuid>,
    pub event_type: Option<String>,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumDomainEventResponse {
    pub sequence_no: i64,
    pub event_id: Uuid,
    pub tenant_id: Uuid,
    pub aggregate_type: String,
    pub aggregate_id: Uuid,
    pub event_type: String,
    pub schema_version: i16,
    pub actor_id: Option<Uuid>,
    pub payload: Value,
    pub created_at: String,
}
