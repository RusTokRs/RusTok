use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Durable idempotency record for Comments lifecycle events projected by Blog.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "blog_comment_projection_deliveries")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub event_id: Uuid,
    pub tenant_id: Uuid,
    pub comment_id: Uuid,
    pub post_id: Uuid,
    pub delta: i32,
    pub processed_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
