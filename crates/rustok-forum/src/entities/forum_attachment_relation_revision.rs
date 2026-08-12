use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Immutable identity header for one Forum attachment projection.
///
/// The header exists even when a content revision has zero attachments, so an
/// empty projection is distinguishable from a projection that was never persisted.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_attachment_relation_revisions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub target_kind: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub target_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub source_revision: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub locale: String,
    pub projection_fingerprint: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
