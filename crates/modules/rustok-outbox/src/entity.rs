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

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
