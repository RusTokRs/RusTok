use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::StorefrontProductList;
use crate::entities::product_variant;
use rustok_commerce_foundation::dto::ProductResponse;

const MAX_PUBLISHED_PRODUCTS_PER_PAGE: u64 = 48;
const READ_PRODUCT_PROJECTION_OPERATION: &str = "read_product_projection";
const READ_VARIANT_PRODUCT_PROJECTION_OPERATION: &str = "read_variant_product_projection";
const LIST_PUBLISHED_PRODUCTS_OPERATION: &str = "list_published_products";

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
}

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

#[async_trait]
impl ProductCatalogReadPort for crate::CatalogService {
    async fn read_product_projection(
        &self,
        context: PortContext,
        request: ProductProjectionRequest,
    ) -> Result<ProductResponse, PortError> {
        let owner_operation = READ_PRODUCT_PROJECTION_OPERATION;
        context
            .require_policy(PortCallPolicy::read())
            .map_err(|error| product_context_error(&context, owner_operation, error))?;
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
        context
            .require_policy(PortCallPolicy::read())
            .map_err(|error| product_context_error(&context, owner_operation, error))?;
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
        context
            .require_policy(PortCallPolicy::read())
            .map_err(|error| product_context_error(&context, owner_operation, error))?;
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
}

fn validate_published_products_request(
    context: &PortContext,
    owner_operation: &'static str,
    request: &PublishedProductsRequest,
) -> Result<(), PortError> {
    if request.page == 0 {
        tracing::warn!(
            page = request.page,
            per_page = request.per_page,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
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
            tenant_id = %context.tenant_id,
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

fn parse_port_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|error| {
        tracing::warn!(
            error = ?error,
            internal_tenant_id = %context.tenant_id,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation = owner_operation,
            code = "product.tenant_id_invalid",
            "product catalog tenant context is invalid"
        );
        PortError::validation(
            "product.tenant_id_invalid",
            "product request context is invalid",
        )
    })
}

fn product_context_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: PortError,
) -> PortError {
    tracing::warn!(
        internal_code = %error.code,
        internal_message = %error.message,
        kind = ?error.kind,
        retryable = error.retryable,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        code = "product.context_invalid",
        "product catalog call context was rejected"
    );

    let PortError {
        kind,
        code,
        retryable,
        ..
    } = error;
    match kind {
        PortErrorKind::Timeout => {
            PortError::timeout(code, "product request context is invalid")
        }
        PortErrorKind::Validation => {
            PortError::validation(code, "product request context is invalid")
        }
        kind => PortError::new(
            kind,
            "product.context_invalid",
            "product request context is invalid",
            retryable,
        ),
    }
}

fn product_storage_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: sea_orm::DbErr,
) -> PortError {
    tracing::error!(
        error = ?error,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        code = "product.database_unavailable",
        "product catalog storage failed"
    );
    PortError::unavailable(
        "product.database_unavailable",
        "product storage is temporarily unavailable",
    )
}

fn product_variant_not_found(
    context: &PortContext,
    owner_operation: &'static str,
    variant_id: Uuid,
) -> PortError {
    tracing::warn!(
        internal_variant_id = %variant_id,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        code = "product.variant_not_found",
        "product variant projection was not found"
    );
    PortError::not_found("product.variant_not_found", "product variant was not found")
}

fn product_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: rustok_commerce_foundation::error::CommerceError,
) -> PortError {
    use rustok_commerce_foundation::error::CommerceError;

    let code = product_error_code(&error);
    tracing::error!(
        error = ?error,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        code,
        "product catalog owner operation failed"
    );

    match error {
        CommerceError::Database(_) => PortError::unavailable(
            "product.database_unavailable",
            "product storage is temporarily unavailable",
        ),
        CommerceError::ProductNotFound(_) => {
            PortError::not_found("product.product_not_found", "product was not found")
        }
        CommerceError::DuplicateHandle { .. } => PortError::conflict(
            "product.duplicate_handle",
            "product handle conflicts with an existing product",
        ),
        CommerceError::Validation(_) => {
            PortError::validation("product.validation", "product request is invalid")
        }
        _ => PortError::invariant_violation(
            "product.invariant_violation",
            "product operation could not be completed safely",
        ),
    }
}

fn product_error_code(
    error: &rustok_commerce_foundation::error::CommerceError,
) -> &'static str {
    use rustok_commerce_foundation::error::CommerceError;

    match error {
        CommerceError::Database(_) => "product.database_unavailable",
        CommerceError::ProductNotFound(_) => "product.product_not_found",
        CommerceError::DuplicateHandle { .. } => "product.duplicate_handle",
        CommerceError::Validation(_) => "product.validation",
        _ => "product.invariant_violation",
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rustok_api::{PortActor, PortErrorKind};
    use rustok_commerce_foundation::error::CommerceError;

    use super::*;

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
            .expect_err("product port tenant_id must be a UUID");

        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "product.tenant_id_invalid");
        assert_eq!(error.message, "product request context is invalid");
        assert!(!error.retryable);

        assert_eq!(
            parse_port_tenant_id(&base_context(), READ_PRODUCT_PROJECTION_OPERATION)
                .expect("nil UUID is a valid UUID"),
            Uuid::nil()
        );
    }

    #[test]
    fn published_products_request_enforces_bounded_pagination() {
        let context = base_context();
        let mut request = published_request();
        request.page = 0;

        let error = validate_published_products_request(
            &context,
            LIST_PUBLISHED_PRODUCTS_OPERATION,
            &request,
        )
        .expect_err("page zero must be rejected before storage access");

        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "product.page_invalid");
        assert_eq!(error.message, "published products page is invalid");

        request.page = 1;
        request.per_page = MAX_PUBLISHED_PRODUCTS_PER_PAGE + 1;

        let error = validate_published_products_request(
            &context,
            LIST_PUBLISHED_PRODUCTS_OPERATION,
            &request,
        )
        .expect_err("oversized page size must be rejected before storage access");

        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "product.per_page_invalid");
        assert_eq!(error.message, "published products page size is invalid");

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
    fn commerce_errors_map_to_typed_product_port_errors() {
        let context = base_context();
        let not_found = product_error_to_port_error(
            &context,
            READ_PRODUCT_PROJECTION_OPERATION,
            CommerceError::ProductNotFound(Uuid::nil()),
        );
        assert_eq!(not_found.kind, PortErrorKind::NotFound);
        assert_eq!(not_found.code, "product.product_not_found");
        assert_eq!(not_found.message, "product was not found");
        assert!(!not_found.retryable);

        let validation = product_error_to_port_error(
            &context,
            READ_PRODUCT_PROJECTION_OPERATION,
            CommerceError::Validation("bad".to_string()),
        );
        assert_eq!(validation.kind, PortErrorKind::Validation);
        assert_eq!(validation.code, "product.validation");
        assert_eq!(validation.message, "product request is invalid");
        assert!(!validation.retryable);

        let duplicate = product_error_to_port_error(
            &context,
            READ_PRODUCT_PROJECTION_OPERATION,
            CommerceError::DuplicateHandle {
                handle: "sku-a".to_string(),
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
    }
}
