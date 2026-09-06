use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "moderation_cases")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub scope_kind: String,
    pub scope_id: Option<Uuid>,
    pub subject_module: String,
    pub subject_kind: String,
    pub subject_id: Uuid,
    pub subject_revision: i64,
    pub queue_key: String,
    pub policy_id: Option<Uuid>,
    pub policy_version: i32,
    pub priority: String,
    pub status: String,
    pub assigned_moderator_id: Option<Uuid>,
    pub revision: i64,
    pub metadata: Json,
    pub deduplication_key: String,
    pub active_deduplication_key: Option<String>,
    pub opened_at: DateTimeWithTimeZone,
    pub started_at: Option<DateTimeWithTimeZone>,
    pub decided_at: Option<DateTimeWithTimeZone>,
    pub closed_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
