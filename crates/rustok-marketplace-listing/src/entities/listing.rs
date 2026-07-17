use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_listings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub seller_id: Uuid,
    pub master_product_id: Uuid,
    pub master_variant_id: Uuid,
    pub seller_sku: String,
    pub market_slug: String,
    pub channel_slug: String,
    pub status: String,
    pub approval_status: String,
    pub approval_note: Option<String>,
    pub suspension_reason: Option<String>,
    pub current_terms_version: i32,
    pub metadata: Json,
    pub published_at: Option<DateTimeWithTimeZone>,
    pub approved_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::listing_terms::Entity")]
    Terms,
    #[sea_orm(has_many = "super::listing_event::Entity")]
    Events,
}

impl Related<super::listing_terms::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Terms.def()
    }
}

impl Related<super::listing_event::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Events.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
