use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "product_bundle_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub bundle_id: Uuid,
    pub product_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub quantity: i32,
    pub is_optional: bool,
    pub discount_rate: Option<Decimal>,
    pub position: i32,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::bundle::Entity",
        from = "Column::BundleId",
        to = "super::bundle::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Bundle,
}

impl Related<super::bundle::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Bundle.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
