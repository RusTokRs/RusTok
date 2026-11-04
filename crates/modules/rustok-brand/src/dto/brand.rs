use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BrandTranslationInput {
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BrandTranslationDto {
    pub id: Uuid,
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateBrandInput {
    pub slug: String,
    pub logo_media_id: Option<Uuid>,
    pub banner_media_id: Option<Uuid>,
    pub website_url: Option<String>,
    pub is_active: Option<bool>,
    pub metadata: Option<serde_json::Value>,
    pub translations: Vec<BrandTranslationInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct UpdateBrandInput {
    pub slug: Option<String>,
    pub logo_media_id: Option<Option<Uuid>>,
    pub banner_media_id: Option<Option<Uuid>>,
    pub website_url: Option<Option<String>>,
    pub is_active: Option<bool>,
    pub metadata: Option<serde_json::Value>,
    pub translations: Option<Vec<BrandTranslationInput>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BrandDto {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub logo_media_id: Option<Uuid>,
    pub banner_media_id: Option<Uuid>,
    pub website_url: Option<String>,
    pub is_active: bool,
    pub metadata: serde_json::Value,
    pub translations: Vec<BrandTranslationDto>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct BrandFilter {
    pub search: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BrandListResponse {
    pub items: Vec<BrandDto>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BrandProductDto {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub brand_id: Uuid,
    pub product_id: Uuid,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
}
