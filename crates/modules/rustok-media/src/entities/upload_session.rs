use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "media_upload_sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub staging_key: String,
    pub original_name: String,
    pub expected_mime_type: String,
    pub expected_size: Option<i64>,
    pub state: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub expires_at: DateTimeWithTimeZone,
    pub completed_at: Option<DateTimeWithTimeZone>,
    pub staging_deleted_at: Option<DateTimeWithTimeZone>,
    pub last_error: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
