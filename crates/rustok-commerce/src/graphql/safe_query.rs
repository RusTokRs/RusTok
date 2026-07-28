mod query_error_boundary;

pub(crate) const MODULE_SLUG: &str = super::MODULE_SLUG;
pub(crate) const PRODUCT_MODULE_SLUG: &str = super::PRODUCT_MODULE_SLUG;

pub(crate) mod types {
    pub(crate) use super::super::types::*;
}

pub(crate) fn map_product_service_error(
    error: rustok_product::CommerceError,
    operation: &'static str,
) -> query_error_boundary::BoundaryError {
    super::map_product_service_error(error, operation).into()
}

pub(crate) fn product_query_tenant(
    ctx: &::async_graphql::Context<'_>,
    requested_tenant_id: uuid::Uuid,
) -> Result<uuid::Uuid, query_error_boundary::BoundaryError> {
    super::product_query_tenant(ctx, requested_tenant_id).map_err(Into::into)
}

pub(crate) fn require_commerce_permission(
    ctx: &::async_graphql::Context<'_>,
    permissions: &[::rustok_api::Permission],
    message: &str,
) -> Result<::rustok_api::AuthContext, query_error_boundary::BoundaryError> {
    super::require_commerce_permission(ctx, permissions, message).map_err(Into::into)
}

pub(crate) async fn require_storefront_channel_enabled(
    ctx: &::async_graphql::Context<'_>,
) -> Result<(), query_error_boundary::BoundaryError> {
    super::require_storefront_channel_enabled(ctx)
        .await
        .map_err(Into::into)
}

mod source;

pub use source::CommerceQuery;
