use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "product_option_value_translations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub value_id: Uuid,
    pub locale: String,
    pub value: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::product_option_value::Entity",
        from = "Column::ValueId",
        to = "super::product_option_value::Column::Id"
    )]
    Value,
}

impl Related<super::product_option_value::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Value.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
