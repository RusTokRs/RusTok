use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "product_variant_axis_values")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub axis_id: Uuid,
    pub option_id: Uuid,
    pub position: i32,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::product_variant_axis::Entity",
        from = "Column::AxisId",
        to = "super::product_variant_axis::Column::Id"
    )]
    Axis,
}

impl Related<super::product_variant_axis::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Axis.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
