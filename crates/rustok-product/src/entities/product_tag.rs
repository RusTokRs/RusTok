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
        belongs_to = "rustok_commerce_foundation::entities::product::Entity",
        from = "Column::ProductId",
        to = "rustok_commerce_foundation::entities::product::Column::Id"
    )]
    Product,
    #[sea_orm(
        belongs_to = "rustok_taxonomy::entities::taxonomy_term::Entity",
        from = "Column::TermId",
        to = "rustok_taxonomy::entities::taxonomy_term::Column::Id"
    )]
    Term,
}

impl Related<rustok_commerce_foundation::entities::product::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Product.def()
    }
}

impl Related<rustok_taxonomy::entities::taxonomy_term::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Term.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
