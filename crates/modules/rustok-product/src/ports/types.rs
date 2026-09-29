use rustok_api::{PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AdminProductListQuery, StorefrontProductListQuery, StorefrontProductSortBy,
    StorefrontProductSortDirection,
};

pub const MAX_PUBLISHED_PRODUCTS_PER_PAGE: u64 = 48;
pub const MAX_ADMIN_PRODUCTS_PER_PAGE: u64 = 100;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilteredPublishedProductsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
    pub query: StorefrontProductListQuery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorefrontProductProjectionSubject {
    ProductId { product_id: Uuid },
    Handle { handle: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorefrontVariantProductProjectionRequest {
    pub variant_id: Uuid,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorefrontProductProjectionRequest {
    pub subject: StorefrontProductProjectionSubject,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyStorefrontProductList {
    pub items: Vec<LegacyStorefrontProductListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyStorefrontProductListItem {
    pub id: Uuid,
    pub status: crate::entities::product::ProductStatus,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminProductsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub query: AdminProductListQuery,
    /// Compatibility-only exact filters for mounted legacy REST. Owner-native callers leave
    /// these unset and retain the typed query semantics above.
    pub raw_status: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    /// Preserve the mounted legacy REST projection: empty missing titles and normalized
    /// shipping-profile metadata fallback. Owner-native callers leave this disabled.
    pub empty_missing_title: bool,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyAdminProductsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub search: Option<String>,
    pub status: Option<crate::entities::product::ProductStatus>,
    pub vendor: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

// ── Pagination validators ───────────────────────────────────────────

pub(super) fn validate_published_products_request(
    context: &PortContext,
    owner_operation: &'static str,
    request: &PublishedProductsRequest,
) -> Result<(), PortError> {
    if request.page == 0 {
        tracing::warn!(
            page = request.page,
            per_page = request.per_page,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            operation = owner_operation,
            code = "product.page_invalid",
            "published product page validation failed"
        );
        return Err(PortError::validation(
            "product.page_invalid",
            "published products page is invalid",
        ));
    }
    if !(1..=MAX_PUBLISHED_PRODUCTS_PER_PAGE).contains(&request.per_page) {
        tracing::warn!(
            page = request.page,
            per_page = request.per_page,
            max_per_page = MAX_PUBLISHED_PRODUCTS_PER_PAGE,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            operation = owner_operation,
            code = "product.per_page_invalid",
            "published product page-size validation failed"
        );
        return Err(PortError::validation(
            "product.per_page_invalid",
            "published products page size is invalid",
        ));
    }
    Ok(())
}

pub(super) fn validate_legacy_storefront_products_request(
    context: &PortContext,
    owner_operation: &'static str,
    request: &LegacyStorefrontProductsRequest,
) -> Result<(), PortError> {
    if request.page == 0 {
        tracing::warn!(
            page = request.page,
            per_page = request.per_page,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            operation = owner_operation,
            code = "product.page_invalid",
            "legacy storefront product page validation failed"
        );
        return Err(PortError::validation(
            "product.page_invalid",
            "published products page is invalid",
        ));
    }
    if !(1..=MAX_PUBLISHED_PRODUCTS_PER_PAGE).contains(&request.per_page) {
        tracing::warn!(
            page = request.page,
            per_page = request.per_page,
            max_per_page = MAX_PUBLISHED_PRODUCTS_PER_PAGE,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            operation = owner_operation,
            code = "product.per_page_invalid",
            "legacy storefront product page-size validation failed"
        );
        return Err(PortError::validation(
            "product.per_page_invalid",
            "published products page size is invalid",
        ));
    }
    Ok(())
}

pub(super) fn validate_admin_products_request(
    context: &PortContext,
    owner_operation: &'static str,
    request: &AdminProductsRequest,
) -> Result<(), PortError> {
    if request.page == 0 {
        tracing::warn!(
            page = request.page,
            per_page = request.per_page,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            operation = owner_operation,
            code = "product.page_invalid",
            "admin product page validation failed"
        );
        return Err(PortError::validation(
            "product.page_invalid",
            "admin products page is invalid",
        ));
    }
    if !(1..=MAX_ADMIN_PRODUCTS_PER_PAGE).contains(&request.per_page) {
        tracing::warn!(
            page = request.page,
            per_page = request.per_page,
            max_per_page = MAX_ADMIN_PRODUCTS_PER_PAGE,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            operation = owner_operation,
            code = "product.per_page_invalid",
            "admin product page-size validation failed"
        );
        return Err(PortError::validation(
            "product.per_page_invalid",
            "admin products page size is invalid",
        ));
    }
    Ok(())
}

pub(super) fn validate_legacy_admin_products_request(
    context: &PortContext,
    owner_operation: &'static str,
    request: &LegacyAdminProductsRequest,
) -> Result<(), PortError> {
    if request.page == 0 {
        tracing::warn!(
            page = request.page,
            per_page = request.per_page,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            operation = owner_operation,
            code = "product.page_invalid",
            "legacy admin product page validation failed"
        );
        return Err(PortError::validation(
            "product.page_invalid",
            "admin products page is invalid",
        ));
    }
    if !(1..=MAX_ADMIN_PRODUCTS_PER_PAGE).contains(&request.per_page) {
        tracing::warn!(
            page = request.page,
            per_page = request.per_page,
            max_per_page = MAX_ADMIN_PRODUCTS_PER_PAGE,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            operation = owner_operation,
            code = "product.per_page_invalid",
            "legacy admin product page-size validation failed"
        );
        return Err(PortError::validation(
            "product.per_page_invalid",
            "admin products page size is invalid",
        ));
    }
    Ok(())
}
