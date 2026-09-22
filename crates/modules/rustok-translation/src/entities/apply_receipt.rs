use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "translation_apply_receipts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub item_id: Uuid,
    pub proposal_id: Uuid,
    pub idempotency_key: String,
    pub request_hash: String,
    pub approval_receipt_id: String,
    pub provider_receipt_id: String,
    pub resource_revision: String,
    pub target_revision: String,
    pub applied_field_keys: Json,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
