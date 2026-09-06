use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "moderation_reports")]
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
    pub reporter_kind: String,
    pub reporter_id: Option<Uuid>,
    pub reason_code: String,
    pub description_reference: Option<String>,
    pub status: String,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
