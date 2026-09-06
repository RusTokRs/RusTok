use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "fulfillment_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub fulfillment_id: Uuid,
    pub order_line_item_id: Uuid,
    pub quantity: i32,
    pub shipped_quantity: i32,
    pub delivered_quantity: i32,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::fulfillment::Entity",
        from = "Column::FulfillmentId",
        to = "super::fulfillment::Column::Id"
    )]
    Fulfillment,
}

impl Related<super::fulfillment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Fulfillment.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
