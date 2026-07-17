use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_listing_terms")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub listing_id: Uuid,
    pub version: i32,
    pub pricing_reference: Option<String>,
    pub inventory_reference: Option<String>,
    pub fulfillment_profile_slug: Option<String>,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::listing::Entity",
        from = "Column::ListingId",
        to = "super::listing::Column::Id"
    )]
    Listing,
}

impl Related<super::listing::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Listing.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
