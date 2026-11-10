use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "product_tags")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub product_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub term_id: Uuid,
    pub tenant_id: Uuid,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::entities::product::Entity",
        from = "Column::ProductId",
        to = "crate::entities::product::Column::Id"
    )]
    Product,
}

impl Related<crate::entities::product::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Product.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
