use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "product_option_values")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub option_id: Uuid,
    pub position: i32,
    pub metadata: Json,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::product_option::Entity",
        from = "Column::OptionId",
        to = "super::product_option::Column::Id"
    )]
    Option,
}

impl Related<super::product_option::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Option.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
