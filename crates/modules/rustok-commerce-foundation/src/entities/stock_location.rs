use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "stock_locations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub code: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub province: Option<String>,
    pub postal_code: Option<String>,
    pub country_code: Option<String>,
    pub phone: Option<String>,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub deleted_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::inventory_level::Entity")]
    InventoryLevels,
    #[sea_orm(has_many = "super::reservation_item::Entity")]
    ReservationItems,
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
