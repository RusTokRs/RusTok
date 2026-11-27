use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum SysEventStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "dispatched")]
    Dispatched,
    #[sea_orm(string_value = "failed")]
    Failed,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "sys_events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// Denormalised tenant scope copied from the envelope at write time.
    ///
    /// The payload remains the canonical envelope; this column is a query
    /// projection so tenant-scoped operator reads (admin counters, DLQ)
    /// never extract identity from JSON. Platform-scoped events carry
    /// `None`. Rows written before the column existed are backfilled by
    /// the platform migration; until then readers may fall back to the
    /// payload. New writers must always populate it.
    pub tenant_id: Option<Uuid>,
    /// Event type string for fast filtering (e.g., "node.created")
    pub event_type: String,
    /// Schema version for this event type
    pub schema_version: i16,
    pub payload: Json,
    pub status: SysEventStatus,
    pub retry_count: i32,
    pub next_attempt_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub claimed_by: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub dispatched_at: Option<DateTime<Utc>>,
}

impl Model {
    /// Checks the canonical envelope identity against the durable row.
    /// Null projections are allowed for historical and platform-scoped rows.
    pub fn matches_envelope_identity(&self, id: Uuid, tenant_id: Uuid) -> bool {
        !id.is_nil()
            && self.id == id
            && self.tenant_id.is_none_or(|stored| !stored.is_nil() && stored == tenant_id)
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
