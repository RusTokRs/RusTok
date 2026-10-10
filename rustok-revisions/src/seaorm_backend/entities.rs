//! SeaORM entity definitions.

use sea_orm::entity::prelude::*;
use serde_json::Value;

/// Revision entity.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "content_revisions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub content_id: Uuid,
    pub content_type: String,
    pub locale: String,
    pub revision_number: i64,
    pub parent_revision_id: Option<Uuid>,
    pub event: String,
    pub content: Value,
    pub user_id: Uuid,
    pub source: String,
    pub summary: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub custom_metadata: Value,
    pub created_at: DateTimeUtc,
    pub version_name: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
