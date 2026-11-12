use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::ProductStatus;

pub const MAX_PUBLISHED_PRODUCTS_PER_PAGE: u64 = 48;
pub const MAX_ADMIN_PRODUCTS_PER_PAGE: u64 = 100;
pub const MAX_VARIANT_IDENTITY_BATCH: usize = 256;
pub const MAX_ATTRIBUTE_FILTERS: usize = 8;
pub const MAX_STOREFRONT_PRODUCT_SEARCH_BYTES: usize = 1022;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductProjectionRequest {
    pub product_id: Uuid,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VariantProductProjectionRequest {
    pub variant_id: Uuid,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublishedProductsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilteredPublishedProductsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
    pub query: StorefrontProductListQuery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorefrontCatalogFacetsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
    pub query: StorefrontProductListQuery,
    pub facet_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminCatalogFacetsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub query: AdminProductListQuery,
    pub facet_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorefrontProductProjectionSubject {
    ProductId { product_id: Uuid },
    Handle { handle: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorefrontVariantProductProjectionRequest {
    pub variant_id: Uuid,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorefrontProductProjectionRequest {
    pub subject: StorefrontProductProjectionSubject,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyStorefrontProductsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub search: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyAdminProductsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub search: Option<String>,
    pub status: Option<ProductStatus>,
    pub vendor: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminProductsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub query: AdminProductListQuery,
    pub raw_status: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub empty_missing_title: bool,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductVariantIdentitiesRequest {
    pub product_id: Option<Uuid>,
    pub variant_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductVariantIdentityListRequest {
    pub product_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductVariantIdentitySnapshot {
    pub tenant_id: Uuid,
    pub product_id: Uuid,
    pub variant_id: Uuid,
}

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
    pub status: ProductStatus,
    pub title: String,
    pub handle: String,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub tags: Vec<String>,
    pub primary_image: Option<StorefrontProductListImage>,
    pub price_from: Option<StorefrontProductListPrice>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorefrontProductListImage {
    pub media_id: Uuid,
    pub url: String,
    pub alt_text: Option<String>,
    pub position: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorefrontProductListPrice {
    pub currency_code: String,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
    pub on_sale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyStorefrontProductList {
    pub items: Vec<LegacyStorefrontProductListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegacyStorefrontProductListItem {
    pub id: Uuid,
    pub status: ProductStatus,
    pub title: String,
    pub handle: String,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: String,
    pub tags: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminProductList {
    pub items: Vec<AdminProductListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminProductListItem {
    pub id: Uuid,
    pub status: ProductStatus,
    pub title: String,
    pub handle: String,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub primary_category_id: Option<Uuid>,
    pub tags: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorefrontCatalogFacetValue {
    pub value: String,
    pub label: String,
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorefrontCatalogFacet {
    pub code: String,
    pub label: String,
    pub value_type: String,
    pub is_localized: bool,
    pub is_enumerable: bool,
    pub is_truncated: bool,
    pub values: Vec<StorefrontCatalogFacetValue>,
    pub total_products: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorefrontProductSortBy {
    #[default]
    PublishedAt,
    CreatedAt,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorefrontProductSortDirection {
    Asc,
    #[default]
    Desc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductAttributeFilter {
    pub code: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorefrontProductListQuery {
    pub search: Option<String>,
    pub category_id: Option<Uuid>,
    pub sort_by: StorefrontProductSortBy,
    pub sort_direction: StorefrontProductSortDirection,
    pub attribute_filters: Vec<ProductAttributeFilter>,
    pub currency_code: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

impl Default for StorefrontProductListQuery {
    fn default() -> Self {
        Self {
            search: None,
            category_id: None,
            sort_by: StorefrontProductSortBy::default(),
            sort_direction: StorefrontProductSortDirection::default(),
            attribute_filters: Vec::new(),
            currency_code: None,
            page: 1,
            per_page: 12,
        }
    }
}

impl StorefrontProductListQuery {
    pub fn with_pagination(mut self, page: u64, per_page: u64) -> Self {
        self.page = page;
        self.per_page = per_page;
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminProductListQuery {
    pub search: Option<String>,
    pub status: Option<ProductStatus>,
    pub category_id: Option<Uuid>,
    pub sort_by: StorefrontProductSortBy,
    pub sort_direction: StorefrontProductSortDirection,
    pub attribute_filters: Vec<ProductAttributeFilter>,
}
