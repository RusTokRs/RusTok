use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::StorefrontProductList;
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
