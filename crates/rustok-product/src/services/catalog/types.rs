use rustok_commerce_foundation::entities;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorefrontProductList {
    pub items: Vec<StorefrontProductListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorefrontProductListItem {
    pub id: Uuid,
    pub status: entities::product::ProductStatus,
    pub title: String,
    pub handle: String,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub tags: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct ProductTagState {
    pub tags: Vec<String>,
}
