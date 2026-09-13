use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "brand_products")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub brand_id: Uuid,
    pub product_id: Uuid,
    pub is_primary: bool,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::brand::Entity",
        from = "Column::BrandId",
        to = "super::brand::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Brand,
}

impl Related<super::brand::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Brand.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
