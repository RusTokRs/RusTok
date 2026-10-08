//! Retired canonical slugs of blog posts.
//!
//! Blog post slugs are global canonical identifiers (not locale-aware), so the
//! history is keyed by `(tenant_id, slug)` and does not carry a locale. A row is
//! written when a post changes its slug and removed as soon as the same slug is
//! claimed again by any post in the tenant.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "blog_post_slug_history")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub slug: String,
    pub post_id: Uuid,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
