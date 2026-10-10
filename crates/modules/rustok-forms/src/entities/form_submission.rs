use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "form_submissions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub form_id: String,
    pub locale: String,
    pub page_id: Option<Uuid>,
    pub payload: Json,
    pub state: String,
    pub ip_hash: String,
    pub user_agent: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub handled_at: Option<DateTimeWithTimeZone>,
    pub handled_by: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
