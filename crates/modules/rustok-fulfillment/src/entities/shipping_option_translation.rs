use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "shipping_option_translations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub shipping_option_id: Uuid,
    pub locale: String,
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::shipping_option::Entity",
        from = "Column::ShippingOptionId",
        to = "super::shipping_option::Column::Id"
    )]
    ShippingOption,
}

impl Related<super::shipping_option::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ShippingOption.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
