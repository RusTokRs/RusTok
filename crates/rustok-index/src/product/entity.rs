use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "index_products")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub product_id: Uuid,
    pub locale: String,
    pub status: String,
    pub is_published: bool,
    pub title: String,
    pub subtitle: Option<String>,
    pub handle: String,
    pub description: Option<String>,
    pub category_id: Option<Uuid>,
    pub category_name: Option<String>,
    pub category_path: Option<String>,
    pub tags: Json,
    pub brand: Option<String>,
    pub currency: Option<String>,
    pub price_min: Option<i64>,
    pub price_max: Option<i64>,
    pub compare_at_price_min: Option<i64>,
    pub compare_at_price_max: Option<i64>,
    pub on_sale: bool,
    pub in_stock: bool,
    pub total_inventory: i32,
    pub variant_count: i32,
    pub options: Json,
    pub thumbnail_url: Option<String>,
    pub images: Json,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
    pub attributes: Json,
    pub sales_count: i32,
    pub view_count: i32,
    pub rating: Option<Decimal>,
    pub review_count: i32,
    pub published_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub indexed_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
