use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Durable acknowledgement for unpublish/archive/delete lifecycle commands.
///
/// No page foreign key is intentional: delete receipts must survive the page row
/// so a delayed retry can be acknowledged without attempting the deletion again.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "page_lifecycle_operation_receipts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub page_id: Uuid,
    pub owner_id: Option<Uuid>,
    pub operation: String,
    pub actor_id: Option<Uuid>,
    pub idempotency_key: String,
    pub request_hash: String,
    pub response_json: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
