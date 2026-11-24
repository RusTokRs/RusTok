use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Immutable request identity and the acknowledgement of a committed document
/// mutation. Response payloads may expire; the key/hash tombstone never does.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "page_document_save_receipts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub page_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub idempotency_key: String,
    pub request_hash: String,
    pub result_revision: String,
    pub response_json: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::page::Entity",
        from = "Column::PageId",
        to = "super::page::Column::Id"
    )]
    Page,
}

impl Related<super::page::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Page.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
