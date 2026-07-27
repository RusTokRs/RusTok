#![allow(hidden_glob_reexports)]

pub(crate) use super::safe_order_helpers_impl::*;

#[allow(clippy::too_many_arguments)]
pub(crate) async fn resolve_storefront_line_item_input(
    db: &sea_orm::DatabaseConnection,
    tenant_id: uuid::Uuid,
    pricing_read_port: &dyn rustok_pricing::PricingReadPort,
    pricing_port_context: rustok_api::PortContext,
    pricing_context: &rustok_pricing::PriceResolutionContext,
    locale: &str,
    default_locale: &str,
    public_channel_slug: Option<&str>,
    input: super::super::types::AddStorefrontCartLineItemInput,
) -> async_graphql::Result<super::legacy_helpers::ResolvedStorefrontLineItemInput> {
    super::typed_line_item_helpers::resolve_storefront_line_item_input(
        db,
        tenant_id,
        pricing_read_port,
        pricing_port_context,
        pricing_context,
        locale,
        default_locale,
        public_channel_slug,
        input,
    )
    .await
}

pub(crate) async fn validate_storefront_line_item_quantity(
    db: &sea_orm::DatabaseConnection,
    tenant_id: uuid::Uuid,
    variant_id: uuid::Uuid,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
) -> async_graphql::Result<()> {
    super::typed_line_item_helpers::validate_storefront_line_item_quantity(
        db,
        tenant_id,
        variant_id,
        requested_quantity,
        public_channel_slug,
    )
    .await
}
