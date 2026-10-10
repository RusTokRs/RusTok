use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Tracks which subscribers belong to which segments.
/// A subscriber without any explicit segment subscriptions receives
/// all campaigns targeted at "all subscribers" (segment_id = NULL).
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "newsletter_subscriptions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub subscriber_id: Uuid,
    pub segment_id: Uuid,
    pub subscribed_at: DateTimeWithTimeZone,
    pub unsubscribed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
