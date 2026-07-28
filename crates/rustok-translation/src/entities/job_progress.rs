use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "translation_job_progress")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub job_id: Uuid,
    pub source_digest: String,
    pub total_items: i64,
    pub assigned_items: i64,
    pub terminal_items: i64,
    pub missing_items: i64,
    pub draft_items: i64,
    pub in_review_items: i64,
    pub approved_items: i64,
    pub applying_items: i64,
    pub applied_items: i64,
    pub stale_items: i64,
    pub conflict_items: i64,
    pub blocked_items: i64,
    pub excluded_items: i64,
    pub cancelled_items: i64,
    pub required_units: i64,
    pub optional_units: i64,
    pub applied_required_units: i64,
    pub applied_optional_units: i64,
    pub approved_required_units: i64,
    pub approved_optional_units: i64,
    pub complete_resources: i64,
    pub source_characters: i64,
    pub translated_characters: i64,
    pub revision: i64,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
