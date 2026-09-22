use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "moderation_application_operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub decision_id: Uuid,
    pub tenant_id: Uuid,
    pub case_id: Uuid,
    pub decision_hash: String,
    pub subject_module: String,
    pub subject_kind: String,
    pub subject_id: Uuid,
    pub subject_revision: i64,
    pub status: String,
    pub attempt_count: i32,
    pub next_attempt_at: DateTimeWithTimeZone,
    pub lease_token: Option<Uuid>,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<DateTimeWithTimeZone>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub applied_revision: Option<i64>,
    pub applied_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
