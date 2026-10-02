use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::dto::ProductResponse;
use crate::entities::product_variant;
use crate::{AdminProductList, StorefrontProductList};

use super::diagnostics::{
    parse_port_tenant_id, product_error_to_port_error, product_storage_error,
    product_variant_not_found,
};
use super::types::{
    AdminProductsRequest, FilteredPublishedProductsRequest, LegacyAdminProductsRequest,
    LegacyStorefrontProductList, LegacyStorefrontProductsRequest, ProductProjectionRequest,
    PublishedProductsRequest, StorefrontProductProjectionRequest,
    StorefrontProductProjectionSubject, StorefrontVariantProductProjectionRequest,
    VariantProductProjectionRequest, validate_admin_products_request,
    validate_legacy_admin_products_request, validate_legacy_storefront_products_request,
    validate_published_products_request,
};

const READ_PRODUCT_PROJECTION_OPERATION: &str = "read_product_projection";
const READ_VARIANT_PRODUCT_PROJECTION_OPERATION: &str = "read_variant_product_projection";
const READ_STOREFRONT_PRODUCT_PROJECTION_OPERATION: &str = "read_storefront_product_projection";
const LIST_PUBLISHED_PRODUCTS_OPERATION: &str = "list_published_products";
const LIST_FILTERED_PUBLISHED_PRODUCTS_OPERATION: &str = "list_filtered_published_products";
const LIST_LEGACY_STOREFRONT_PRODUCTS_OPERATION: &str = "list_legacy_storefront_products";
const LIST_ADMIN_PRODUCTS_OPERATION: &str = "list_admin_products";
const LIST_LEGACY_ADMIN_PRODUCTS_OPERATION: &str = "list_legacy_admin_products";

// ── Trait ────────────────────────────────────────────────────────────

/// Transport-neutral owner boundary for product catalog read projections.
#[async_trait]
pub trait ProductCatalogReadPort: Send + Sync {
    async fn read_product_projection(
        &self,
        context: PortContext,
        request: ProductProjectionRequest,
    ) -> Result<ProductResponse, PortError>;

    /// Resolve the owner product projection for a variant-first consumer input.
    ///
    /// Checkout consumers may receive a cart line with a variant id before a
    /// product id is materialized. The product owner resolves that association
    /// so consumers do not query product entities directly.
    async fn read_variant_product_projection(
        &self,
        context: PortContext,
        request: VariantProductProjectionRequest,
    ) -> Result<ProductResponse, PortError>;

    async fn list_published_products(
        &self,
        context: PortContext,
        request: PublishedProductsRequest,
    ) -> Result<StorefrontProductList, PortError>;

    /// Optional filtered storefront-list capability for consumers that need the full owner
    /// search/category/sort/attribute-filter contract. Existing adapters remain compatible.
    async fn list_filtered_published_products(
        &self,
        _context: PortContext,
        _request: FilteredPublishedProductsRequest,
    ) -> Result<StorefrontProductList, PortError> {
        Err(PortError::unavailable(
            "product.filtered_published_list_unavailable",
            "filtered product listing is unavailable",
        ))
    }

    /// Optional published storefront detail projection for legacy consumers that need the
    /// owner to apply lifecycle/channel visibility, locale narrowing, and public inventory.
    /// Resolve a published storefront product by variant id, keeping variant-to-product
    /// association and storefront visibility inside the Product owner.
    async fn read_storefront_variant_product_projection(
        &self,
        _context: PortContext,
        _request: StorefrontVariantProductProjectionRequest,
    ) -> Result<Option<ProductResponse>, PortError> {
        Err(PortError::unavailable(
            "product.storefront_variant_detail_unavailable",
            "storefront variant product detail is unavailable",
        ))
    }

    async fn read_storefront_product_projection(
        &self,
        _context: PortContext,
        _request: StorefrontProductProjectionRequest,
    ) -> Result<Option<ProductResponse>, PortError> {
        Err(PortError::unavailable(
            "product.storefront_detail_unavailable",
            "storefront product detail is unavailable",
        ))
    }

    /// Optional compatibility projection for the mounted legacy storefront GraphQL list.
    /// Existing adapters remain source-compatible and fail closed until they explicitly
    /// implement the exact legacy list semantics.
    async fn list_legacy_storefront_products(
        &self,
        _context: PortContext,
        _request: LegacyStorefrontProductsRequest,
    ) -> Result<LegacyStorefrontProductList, PortError> {
        Err(PortError::unavailable(
            "product.legacy_storefront_list_unavailable",
            "legacy product storefront listing is unavailable",
        ))
    }

    /// Optional compatibility projection for the mounted legacy admin REST list.
    /// Existing adapters remain source-compatible and fail closed until they explicitly
    /// implement the exact legacy list semantics.
    async fn list_admin_products(
        &self,
        _context: PortContext,
        _request: AdminProductsRequest,
    ) -> Result<AdminProductList, PortError> {
        Err(PortError::unavailable(
            "product.admin_list_unavailable",
            "product admin listing is unavailable",
        ))
    }

    /// Optional compatibility projection for the mounted legacy admin GraphQL list.
    /// Existing adapters remain source-compatible and fail closed until they explicitly
    /// implement the exact legacy list semantics.
    async fn list_legacy_admin_products(
        &self,
        _context: PortContext,
        _request: LegacyAdminProductsRequest,
    ) -> Result<AdminProductList, PortError> {
        Err(PortError::unavailable(
            "product.legacy_admin_list_unavailable",
            "legacy product admin listing is unavailable",
        ))
    }
}

// ── CatalogService adapter ──────────────────────────────────────────

#[async_trait]
impl ProductCatalogReadPort for crate::CatalogService {
    async fn read_product_projection(
        &self,
        context: PortContext,
        request: ProductProjectionRequest,
    ) -> Result<ProductResponse, PortError> {
        let owner_operation = READ_PRODUCT_PROJECTION_OPERATION;
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());
        self.get_product_with_locale_fallback(
            tenant_id,
            request.product_id,
            locale,
            request.fallback_locale.as_deref(),
        )
        .await
        .map_err(|error| product_error_to_port_error(&context, owner_operation, error))
    }

    async fn read_variant_product_projection(
        &self,
        context: PortContext,
        request: VariantProductProjectionRequest,
    ) -> Result<ProductResponse, PortError> {
        let owner_operation = READ_VARIANT_PRODUCT_PROJECTION_OPERATION;
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let variant = product_variant::Entity::find_by_id(request.variant_id)
            .filter(product_variant::Column::TenantId.eq(tenant_id))
            .one(self.database())
            .await
            .map_err(|error| product_storage_error(&context, owner_operation, error))?
            .ok_or_else(|| {
                product_variant_not_found(&context, owner_operation, request.variant_id)
            })?;
        let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());

        self.get_product_with_locale_fallback(
            tenant_id,
            variant.product_id,
            locale,
            request.fallback_locale.as_deref(),
        )
        .await
        .map_err(|error| product_error_to_port_error(&context, owner_operation, error))
    }

    async fn list_published_products(
        &self,
        context: PortContext,
        request: PublishedProductsRequest,
    ) -> Result<StorefrontProductList, PortError> {
        let owner_operation = LIST_PUBLISHED_PRODUCTS_OPERATION;
        context.require_policy(PortCallPolicy::read())?;
        validate_published_products_request(&context, owner_operation, &request)?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());
        self.list_published_products_with_locale_fallback(
            tenant_id,
            locale,
            request.fallback_locale.as_deref(),
            request.public_channel_slug.as_deref(),
            request.page,
            request.per_page,
        )
        .await
        .map_err(|error| product_error_to_port_error(&context, owner_operation, error))
    }

    async fn list_filtered_published_products(
        &self,
        context: PortContext,
        request: FilteredPublishedProductsRequest,
    ) -> Result<StorefrontProductList, PortError> {
        let owner_operation = LIST_FILTERED_PUBLISHED_PRODUCTS_OPERATION;
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());
        crate::CatalogService::list_published_products_with_query(
            self,
            tenant_id,
            locale,
            request.fallback_locale.as_deref(),
            request.public_channel_slug.as_deref(),
            request.query,
        )
        .await
        .map_err(|error| product_error_to_port_error(&context, owner_operation, error))
    }

    async fn read_storefront_product_projection(
        &self,
        context: PortContext,
        request: StorefrontProductProjectionRequest,
    ) -> Result<Option<ProductResponse>, PortError> {
        let owner_operation = READ_STOREFRONT_PRODUCT_PROJECTION_OPERATION;
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let StorefrontProductProjectionRequest {
            subject,
            locale,
            fallback_locale,
            public_channel_slug,
        } = request;
        let locale = locale.as_deref().unwrap_or(context.locale.as_str());
        let result = match subject {
            StorefrontProductProjectionSubject::ProductId { product_id } => {
                self.get_published_product_by_id_with_locale_fallback(
                    tenant_id,
                    product_id,
                    locale,
                    fallback_locale.as_deref(),
                    public_channel_slug.as_deref(),
                )
                .await
            }
            StorefrontProductProjectionSubject::Handle { handle } => {
                self.get_published_product_by_handle_with_locale_fallback(
                    tenant_id,
                    handle.as_str(),
                    locale,
                    fallback_locale.as_deref(),
                    public_channel_slug.as_deref(),
                )
                .await
            }
        };
        result.map_err(|error| product_error_to_port_error(&context, owner_operation, error))
    }

    async fn read_storefront_variant_product_projection(
        &self,
        context: PortContext,
        request: StorefrontVariantProductProjectionRequest,
    ) -> Result<Option<ProductResponse>, PortError> {
        let owner_operation = "read_storefront_variant_product_projection";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let variant = product_variant::Entity::find_by_id(request.variant_id)
            .filter(product_variant::Column::TenantId.eq(tenant_id))
            .one(self.database())
            .await
            .map_err(|error| product_storage_error(&context, owner_operation, error))?;

        let Some(variant) = variant else {
            return Ok(None);
        };

        let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());
        let product = self
            .get_published_product_by_id_with_locale_fallback(
                tenant_id,
                variant.product_id,
                locale,
                request.fallback_locale.as_deref(),
                request.public_channel_slug.as_deref(),
            )
            .await
            .map_err(|error| product_error_to_port_error(&context, owner_operation, error))?;

        Ok(product.and_then(|product| {
            product
                .variants
                .iter()
                .any(|item| item.id == request.variant_id)
                .then_some(product)
        }))
    }

    async fn list_legacy_storefront_products(
        &self,
        context: PortContext,
        request: LegacyStorefrontProductsRequest,
    ) -> Result<LegacyStorefrontProductList, PortError> {
        let owner_operation = LIST_LEGACY_STOREFRONT_PRODUCTS_OPERATION;
        context.require_policy(PortCallPolicy::read())?;
        validate_legacy_storefront_products_request(&context, owner_operation, &request)?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let LegacyStorefrontProductsRequest {
            locale,
            fallback_locale,
            public_channel_slug,
            vendor,
            product_type,
            search,
            page,
            per_page,
        } = request;
        let locale = locale.as_deref().unwrap_or(context.locale.as_str());
        self.list_legacy_storefront_products_with_locale_fallback(
            tenant_id,
            locale,
            fallback_locale.as_deref(),
            public_channel_slug.as_deref(),
            vendor.as_deref(),
            product_type.as_deref(),
            search.as_deref(),
            page,
            per_page,
        )
        .await
        .map_err(|error| product_error_to_port_error(&context, owner_operation, error))
    }

    async fn list_admin_products(
        &self,
        context: PortContext,
        request: AdminProductsRequest,
    ) -> Result<AdminProductList, PortError> {
        let owner_operation = LIST_ADMIN_PRODUCTS_OPERATION;
        context.require_policy(PortCallPolicy::read())?;
        validate_admin_products_request(&context, owner_operation, &request)?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let AdminProductsRequest {
            locale,
            fallback_locale,
            query,
            raw_status,
            vendor,
            product_type,
            empty_missing_title,
            page,
            per_page,
        } = request;
        let locale = locale.as_deref().unwrap_or(context.locale.as_str());
        let page = page.max(1);
        self.list_admin_products_with_compatibility_query(
            tenant_id,
            locale,
            fallback_locale.as_deref(),
            query,
            page,
            per_page,
            raw_status.as_deref(),
            vendor.as_deref(),
            product_type.as_deref(),
            empty_missing_title,
            empty_missing_title,
        )
        .await
        .map_err(|error| product_error_to_port_error(&context, owner_operation, error))
    }

    async fn list_legacy_admin_products(
        &self,
        context: PortContext,
        request: LegacyAdminProductsRequest,
    ) -> Result<AdminProductList, PortError> {
        let owner_operation = LIST_LEGACY_ADMIN_PRODUCTS_OPERATION;
        context.require_policy(PortCallPolicy::read())?;
        validate_legacy_admin_products_request(&context, owner_operation, &request)?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let LegacyAdminProductsRequest {
            locale,
            fallback_locale,
            search,
            status,
            vendor,
            page,
            per_page,
        } = request;
        let page = page.max(1);
        let locale = locale.as_deref().unwrap_or(context.locale.as_str());
        self.list_admin_products_with_compatibility_query(
            tenant_id,
            locale,
            fallback_locale.as_deref(),
            crate::AdminProductListQuery {
                search,
                status,
                category_id: None,
                sort_by: crate::StorefrontProductSortBy::CreatedAt,
                sort_direction: crate::StorefrontProductSortDirection::Desc,
                attribute_filters: Vec::new(),
            },
            page,
            per_page,
            None,
            vendor.as_deref(),
            None,
            false,
            true,
        )
        .await
        .map_err(|error| product_error_to_port_error(&context, owner_operation, error))
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::error::CommerceError;
    use rustok_api::{PortActor, PortErrorKind};

    use super::super::types::MAX_PUBLISHED_PRODUCTS_PER_PAGE;
    use super::*;
    use uuid::Uuid;

    fn base_context() -> PortContext {
        PortContext::new(
            Uuid::nil().to_string(),
            PortActor::service("product-contract-test"),
            "ru",
            "corr-product-a",
        )
    }

    fn published_request() -> PublishedProductsRequest {
        PublishedProductsRequest {
            locale: None,
            fallback_locale: Some("en".to_string()),
            public_channel_slug: Some("web".to_string()),
            page: 1,
            per_page: 24,
        }
    }

    #[test]
    fn product_read_ports_require_deadline_policy() {
        let error = base_context()
            .require_policy(PortCallPolicy::read())
            .expect_err("product read ports require deadline semantics");

        assert_eq!(error.kind, PortErrorKind::Timeout);
        assert_eq!(error.code, "port.deadline_required");
        assert!(error.retryable);

        assert!(
            base_context()
                .with_deadline(Duration::from_secs(3))
                .require_policy(PortCallPolicy::read())
                .is_ok()
        );
    }

    #[test]
    fn product_port_tenant_scope_requires_uuid_context() {
        let context = PortContext::new(
            "tenant-slug",
            PortActor::service("product-contract-test"),
            "ru",
            "corr-product-b",
        );
        let error = parse_port_tenant_id(&context, READ_PRODUCT_PROJECTION_OPERATION)
            .expect_err("product tenant ids must be uuid-backed");

        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "product.tenant_id_invalid");
        assert_eq!(error.message, "product request context is invalid");
    }

    #[test]
    fn published_products_request_enforces_bounded_pagination() {
        let context = base_context().with_deadline(Duration::from_secs(3));
        let mut request = published_request();

        request.page = 0;
        let page_error = validate_published_products_request(
            &context,
            LIST_PUBLISHED_PRODUCTS_OPERATION,
            &request,
        )
        .expect_err("page 0 must be rejected");
        assert_eq!(page_error.kind, PortErrorKind::Validation);
        assert_eq!(page_error.code, "product.page_invalid");
        assert_eq!(page_error.message, "published products page is invalid");

        request.page = 1;
        request.per_page = 0;
        let zero_per_page = validate_published_products_request(
            &context,
            LIST_PUBLISHED_PRODUCTS_OPERATION,
            &request,
        )
        .expect_err("per_page 0 must be rejected");
        assert_eq!(zero_per_page.kind, PortErrorKind::Validation);
        assert_eq!(zero_per_page.code, "product.per_page_invalid");
        assert_eq!(
            zero_per_page.message,
            "published products page size is invalid"
        );

        request.per_page = MAX_PUBLISHED_PRODUCTS_PER_PAGE + 1;
        let large_per_page = validate_published_products_request(
            &context,
            LIST_PUBLISHED_PRODUCTS_OPERATION,
            &request,
        )
        .expect_err("oversized per_page must be rejected");
        assert_eq!(large_per_page.kind, PortErrorKind::Validation);
        assert_eq!(large_per_page.code, "product.per_page_invalid");
        assert_eq!(
            large_per_page.message,
            "published products page size is invalid"
        );

        request.per_page = MAX_PUBLISHED_PRODUCTS_PER_PAGE;
        assert!(
            validate_published_products_request(
                &context,
                LIST_PUBLISHED_PRODUCTS_OPERATION,
                &request,
            )
            .is_ok()
        );
    }

    #[test]
    fn product_database_error_maps_to_unavailable() {
        let context = base_context().with_deadline(Duration::from_secs(3));
        let error = product_storage_error(
            &context,
            READ_PRODUCT_PROJECTION_OPERATION,
            sea_orm::DbErr::Custom("connection pool exhausted".to_string()),
        );

        assert_eq!(error.kind, PortErrorKind::Unavailable);
        assert_eq!(error.code, "product.database_unavailable");
        assert_eq!(
            error.message,
            "the requested capability is temporarily unavailable"
        );
        assert!(error.retryable);
    }

    #[test]
    fn commerce_errors_map_to_typed_product_port_errors() {
        let context = base_context().with_deadline(Duration::from_secs(3));

        let not_found = product_error_to_port_error(
            &context,
            READ_PRODUCT_PROJECTION_OPERATION,
            CommerceError::ProductNotFound(Uuid::nil()),
        );
        assert_eq!(not_found.kind, PortErrorKind::NotFound);
        assert_eq!(not_found.code, "product.product_not_found");
        assert_eq!(not_found.message, "product was not found");
        assert!(!not_found.retryable);

        let duplicate = product_error_to_port_error(
            &context,
            READ_PRODUCT_PROJECTION_OPERATION,
            CommerceError::DuplicateHandle {
                handle: "sample-product".to_string(),
                locale: "ru".to_string(),
            },
        );
        assert_eq!(duplicate.kind, PortErrorKind::Conflict);
        assert_eq!(duplicate.code, "product.duplicate_handle");
        assert_eq!(
            duplicate.message,
            "product handle conflicts with an existing product"
        );
        assert!(!duplicate.retryable);

        let variant_not_found = product_error_to_port_error(
            &context,
            READ_PRODUCT_PROJECTION_OPERATION,
            CommerceError::VariantNotFound(Uuid::nil()),
        );
        assert_eq!(variant_not_found.kind, PortErrorKind::NotFound);
        assert_eq!(variant_not_found.code, "product.variant_not_found");
        assert_eq!(variant_not_found.message, "product variant was not found");
        assert!(!variant_not_found.retryable);

        let cannot_delete_only = product_error_to_port_error(
            &context,
            READ_PRODUCT_PROJECTION_OPERATION,
            CommerceError::CannotDeleteOnlyVariant,
        );
        assert_eq!(cannot_delete_only.kind, PortErrorKind::Conflict);
        assert_eq!(
            cannot_delete_only.code,
            "product.cannot_delete_only_variant"
        );
        assert_eq!(
            cannot_delete_only.message,
            "cannot delete the only variant of a product"
        );
        assert!(!cannot_delete_only.retryable);

        let image_not_found = product_error_to_port_error(
            &context,
            READ_PRODUCT_PROJECTION_OPERATION,
            CommerceError::ImageNotFound(Uuid::nil()),
        );
        assert_eq!(image_not_found.kind, PortErrorKind::NotFound);
        assert_eq!(image_not_found.code, "product.image_not_found");
        assert_eq!(image_not_found.message, "product image was not found");
        assert!(!image_not_found.retryable);
    }
}
