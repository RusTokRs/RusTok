use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "prices")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub variant_id: Uuid,
    pub price_list_id: Option<Uuid>,
    pub currency_code: String,
    pub region_id: Option<Uuid>,
    #[sea_orm(column_name = "amount_decimal")]
    pub amount: Decimal,
    #[sea_orm(column_name = "compare_at_amount_decimal")]
    pub compare_at_amount: Option<Decimal>,
    #[sea_orm(column_name = "amount")]
    pub legacy_amount: Option<i64>,
    #[sea_orm(column_name = "compare_at_amount")]
    pub legacy_compare_at_amount: Option<i64>,
    pub min_quantity: Option<i32>,
    pub max_quantity: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::product_variant::Entity",
        from = "Column::VariantId",
        to = "super::product_variant::Column::Id"
    )]
    Variant,
}

impl Related<super::product_variant::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Variant.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
