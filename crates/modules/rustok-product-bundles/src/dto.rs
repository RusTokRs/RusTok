use chrono::{DateTime, Utc};
use sea_orm::prelude::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BundleTranslationInput {
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BundleTranslationDto {
    pub id: Uuid,
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BundleItemInput {
    pub product_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub quantity: i32,
    pub is_optional: Option<bool>,
    pub discount_rate: Option<Decimal>,
    pub position: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BundleItemDto {
    pub id: Uuid,
    pub bundle_id: Uuid,
    pub product_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub quantity: i32,
    pub is_optional: bool,
    pub discount_rate: Option<Decimal>,
    pub position: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateBundleInput {
    pub bundle_product_id: Option<Uuid>,
    pub slug: String,
    pub bundle_type: Option<String>,
    pub status: Option<String>,
    pub discount_type: Option<String>,
    pub discount_value: Option<Decimal>,
    pub metadata: Option<serde_json::Value>,
    pub translations: Vec<BundleTranslationInput>,
    pub items: Vec<BundleItemInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct UpdateBundleInput {
    pub bundle_product_id: Option<Option<Uuid>>,
    pub slug: Option<String>,
    pub bundle_type: Option<String>,
    pub status: Option<String>,
    pub discount_type: Option<String>,
    pub discount_value: Option<Decimal>,
    pub metadata: Option<serde_json::Value>,
    pub translations: Option<Vec<BundleTranslationInput>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BundleDto {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub bundle_product_id: Option<Uuid>,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub bundle_type: String,
    pub status: String,
    pub discount_type: String,
    pub discount_value: Decimal,
    pub metadata: serde_json::Value,
    pub translations: Vec<BundleTranslationDto>,
    pub items: Vec<BundleItemDto>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct BundleFilter {
    pub search: Option<String>,
    pub status: Option<String>,
    pub bundle_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BundleListResponse {
    pub items: Vec<BundleDto>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}
