use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Durable acknowledgement for one page creation request.
///
/// This table deliberately has no foreign key to `pages`: the receipt remains
/// available after a page is later deleted, so a delayed retry cannot recreate it.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "page_create_operation_receipts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub idempotency_key: String,
    pub request_hash: String,
    pub page_id: Uuid,
    pub response_json: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
