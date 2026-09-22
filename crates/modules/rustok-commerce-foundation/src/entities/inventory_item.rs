use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub variant_id: Uuid,
    pub sku: Option<String>,
    pub requires_shipping: bool,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::product_variant::Entity",
        from = "Column::VariantId",
        to = "super::product_variant::Column::Id"
    )]
    Variant,
    #[sea_orm(has_many = "super::inventory_level::Entity")]
    InventoryLevels,
    #[sea_orm(has_many = "super::reservation_item::Entity")]
    ReservationItems,
}

impl Related<super::product_variant::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Variant.def()
    }
}

impl Related<super::inventory_level::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InventoryLevels.def()
    }
}

impl Related<super::reservation_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ReservationItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
