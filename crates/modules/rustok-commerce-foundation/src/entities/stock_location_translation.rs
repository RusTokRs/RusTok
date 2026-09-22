use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "stock_location_translations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub stock_location_id: Uuid,
    pub locale: String,
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::stock_location::Entity",
        from = "Column::StockLocationId",
        to = "super::stock_location::Column::Id"
    )]
    StockLocation,
}

impl Related<super::stock_location::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StockLocation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
