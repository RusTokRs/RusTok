use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::StorefrontProductList;
use crate::entities::product_variant;
use rustok_commerce_foundation::dto::ProductResponse;

const MAX_PUBLISHED_PRODUCTS_PER_PAGE: u64 = 48;

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
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());
        self.get_product_with_locale_fallback(
            tenant_id,
            request.product_id,
            locale,
            request.fallback_locale.as_deref(),
        )
        .await
        .map_err(product_error_to_port_error)
    }

    async fn read_variant_product_projection(
        &self,
        context: PortContext,
        request: VariantProductProjectionRequest,
    ) -> Result<ProductResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let variant = product_variant::Entity::find_by_id(request.variant_id)
            .filter(product_variant::Column::TenantId.eq(tenant_id))
            .one(self.database())
            .await
            .map_err(|error| {
                PortError::unavailable(
                    "product.database_unavailable",
                    format!("product storage unavailable: {error}"),
                )
            })?
            .ok_or_else(|| {
                PortError::new(
                    rustok_api::PortErrorKind::NotFound,
                    "product.variant_not_found",
                    format!("variant {} not found", request.variant_id),
                    false,
                )
            })?;
        let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());

        self.get_product_with_locale_fallback(
            tenant_id,
            variant.product_id,
            locale,
            request.fallback_locale.as_deref(),
        )
        .await
        .map_err(product_error_to_port_error)
    }

    async fn list_published_products(
        &self,
        context: PortContext,
        request: PublishedProductsRequest,
    ) -> Result<StorefrontProductList, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        validate_published_products_request(&request)?;
        let tenant_id = parse_port_tenant_id(&context)?;
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
        .map_err(product_error_to_port_error)
    }
}

fn validate_published_products_request(
    request: &PublishedProductsRequest,
) -> Result<(), PortError> {
    if request.page == 0 {
        return Err(PortError::validation(
            "product.page_invalid",
            "published products page must be greater than zero",
        ));
    }
    if !(1..=MAX_PUBLISHED_PRODUCTS_PER_PAGE).contains(&request.per_page) {
        return Err(PortError::validation(
            "product.per_page_invalid",
            format!(
                "published products per_page must be between 1 and {MAX_PUBLISHED_PRODUCTS_PER_PAGE}"
            ),
        ));
    }
    Ok(())
}

fn parse_port_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "product.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for product ports",
        )
    })
}

fn product_error_to_port_error(
    error: rustok_commerce_foundation::error::CommerceError,
) -> PortError {
    use rustok_commerce_foundation::error::CommerceError;

    match error {
        CommerceError::Database(error) => PortError::unavailable(
            "product.database_unavailable",
            format!("product storage unavailable: {error}"),
        ),
        CommerceError::ProductNotFound(id) => PortError::new(
            rustok_api::PortErrorKind::NotFound,
            "product.product_not_found",
            format!("product {id} not found"),
            false,
        ),
        CommerceError::DuplicateHandle { handle, locale } => PortError::new(
            rustok_api::PortErrorKind::Conflict,
            "product.duplicate_handle",
            format!("duplicate handle `{handle}` for locale `{locale}`"),
            false,
        ),
        CommerceError::Validation(message) => PortError::validation("product.validation", message),
        other => PortError::new(
            rustok_api::PortErrorKind::InvariantViolation,
            "product.invariant_violation",
            format!("product operation failed: {other}"),
            false,
        ),
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
        let error = parse_port_tenant_id(&PortContext::new(
            "tenant-slug",
            PortActor::service("product-contract-test"),
            "ru",
            "corr-product-b",
        ))
        .expect_err("product port tenant_id must be a UUID");

        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "product.tenant_id_invalid");
        assert!(!error.retryable);

        assert_eq!(
            parse_port_tenant_id(&base_context()).expect("nil UUID is a valid UUID"),
            Uuid::nil()
        );
    }

    #[test]
    fn published_products_request_enforces_bounded_pagination() {
        let mut request = published_request();
        request.page = 0;

        let error = validate_published_products_request(&request)
            .expect_err("page zero must be rejected before storage access");

        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "product.page_invalid");

        request.page = 1;
        request.per_page = MAX_PUBLISHED_PRODUCTS_PER_PAGE + 1;

        let error = validate_published_products_request(&request)
            .expect_err("oversized page size must be rejected before storage access");

        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "product.per_page_invalid");

        request.per_page = MAX_PUBLISHED_PRODUCTS_PER_PAGE;
        assert!(validate_published_products_request(&request).is_ok());
    }

    #[test]
    fn commerce_errors_map_to_typed_product_port_errors() {
        let not_found = product_error_to_port_error(CommerceError::ProductNotFound(Uuid::nil()));
        assert_eq!(not_found.kind, PortErrorKind::NotFound);
        assert_eq!(not_found.code, "product.product_not_found");
        assert!(!not_found.retryable);

        let validation = product_error_to_port_error(CommerceError::Validation("bad".to_string()));
        assert_eq!(validation.kind, PortErrorKind::Validation);
        assert_eq!(validation.code, "product.validation");
        assert!(!validation.retryable);

        let duplicate = product_error_to_port_error(CommerceError::DuplicateHandle {
            handle: "sku-a".to_string(),
            locale: "ru".to_string(),
        });
        assert_eq!(duplicate.kind, PortErrorKind::Conflict);
        assert_eq!(duplicate.code, "product.duplicate_handle");
        assert!(!duplicate.retryable);
    }
}
