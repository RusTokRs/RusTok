use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_listing_events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub listing_id: Uuid,
    pub actor_id: Uuid,
    pub event_kind: String,
    pub locale: String,
    pub note: Option<String>,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::listing::Entity",
        from = "(Column::TenantId, Column::ListingId)",
        to = "(super::listing::Column::TenantId, super::listing::Column::Id)",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Listing,
}

impl Related<super::listing::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Listing.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
