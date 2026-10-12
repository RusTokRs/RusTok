use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_ledger_receipts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
    pub idempotency_key: String,
    pub command_kind: String,
    pub request_hash: String,
    pub status: String,
    pub response_json: Option<Json>,
    pub created_at: DateTimeWithTimeZone,
    pub completed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
