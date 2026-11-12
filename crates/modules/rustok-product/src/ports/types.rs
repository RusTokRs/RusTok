use rustok_api::{PortContext, PortError};

pub use rustok_product_ports::{
    MAX_ADMIN_PRODUCTS_PER_PAGE, MAX_PUBLISHED_PRODUCTS_PER_PAGE, AdminCatalogFacetsRequest,
    AdminProductsRequest, FilteredPublishedProductsRequest, LegacyAdminProductsRequest,
    LegacyStorefrontProductList, LegacyStorefrontProductListItem, LegacyStorefrontProductsRequest,
    ProductProjectionRequest, PublishedProductsRequest, StorefrontCatalogFacetsRequest,
    StorefrontProductProjectionRequest, StorefrontProductProjectionSubject,
    StorefrontVariantProductProjectionRequest, VariantProductProjectionRequest,
};

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
