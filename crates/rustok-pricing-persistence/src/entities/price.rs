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
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
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
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
