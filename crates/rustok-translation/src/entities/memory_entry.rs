use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "translation_memory_entries")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub source_locale: String,
    pub target_locale: String,
    pub owner_slug: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub subresource_id: Option<String>,
    pub field_key: String,
    pub source_text: String,
    pub target_text: String,
    pub source_key: String,
    pub source_hash: String,
    pub target_hash: String,
    pub context_fingerprint: String,
    pub segmentation_version: String,
    pub origin: String,
    pub quality_state: String,
    pub reviewer_actor_kind: String,
    pub reviewer_actor_id: String,
    pub proposal_id: Uuid,
    pub apply_receipt_id: Uuid,
    pub retention_policy: String,
    pub retain_until: Option<DateTimeWithTimeZone>,
    pub tombstoned_at: Option<DateTimeWithTimeZone>,
    pub revision: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
