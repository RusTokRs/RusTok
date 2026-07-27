use async_graphql::{ErrorExtensions, Result};
use rustok_api::RequestContext;
use rustok_fulfillment::FulfillmentError;
use uuid::Uuid;

use crate::{
    dto::CartResponse,
    storefront_channel::normalize_public_channel_slug,
    storefront_shipping::enrich_cart_delivery_groups_typed,
};

const STOREFRONT_SHIPPING_ENRICHMENT_GRAPHQL_BOUNDARY: &str =
    "commerce_graphql_storefront_shipping_enrichment";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShippingEnrichmentFailureKind {
    Validation,
    ShippingOptionNotFound,
    FulfillmentNotFound,
    LifecycleConflict,
    StorageUnavailable,
}

#[derive(Debug)]
struct ShippingEnrichmentFailure {
    kind: ShippingEnrichmentFailureKind,
    internal_code: &'static str,
    internal_kind: &'static str,
    internal_retryable: bool,
    owner_error: FulfillmentError,
}

impl ShippingEnrichmentFailure {
    fn from_owner(error: FulfillmentError) -> Self {
        let (kind, internal_code, internal_kind, internal_retryable) = match &error {
            FulfillmentError::Validation(_) => (
                ShippingEnrichmentFailureKind::Validation,
                "fulfillment.validation",
                "validation",
                false,
            ),
            FulfillmentError::ShippingOptionNotFound(_) => (
                ShippingEnrichmentFailureKind::ShippingOptionNotFound,
                "fulfillment.shipping_option_not_found",
                "not_found",
                false,
            ),
            FulfillmentError::FulfillmentNotFound(_) => (
                ShippingEnrichmentFailureKind::FulfillmentNotFound,
                "fulfillment.fulfillment_not_found",
                "not_found",
                false,
            ),
            FulfillmentError::InvalidTransition { .. } => (
                ShippingEnrichmentFailureKind::LifecycleConflict,
                "fulfillment.invalid_transition",
                "conflict",
                false,
            ),
            FulfillmentError::Database(_) => (
                ShippingEnrichmentFailureKind::StorageUnavailable,
                "fulfillment.database_unavailable",
                "unavailable",
                true,
            ),
        };

        Self {
            kind,
            internal_code,
            internal_kind,
            internal_retryable,
            owner_error: error,
        }
    }

    fn technical_owner_error(&self) -> Option<&FulfillmentError> {
        if matches!(self.kind, ShippingEnrichmentFailureKind::StorageUnavailable) {
            Some(&self.owner_error)
        } else {
            None
        }
    }
}

fn public_graphql_error() -> async_graphql::Error {
    async_graphql::Error::new("Cart shipping details are temporarily unavailable").extend_with(
        |_, extensions| {
            extensions.set("code", "CART_ENRICHMENT_UNAVAILABLE");
            extensions.set("retryable", true);
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn shipping_enrichment_graphql_error(
    failure: ShippingEnrichmentFailure,
    tenant_id: Uuid,
    cart_id: Uuid,
    line_item_count: usize,
    delivery_group_count: usize,
    currency_code_length: usize,
    public_channel_slug: Option<&str>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> async_graphql::Error {
    let channel_slug_length = public_channel_slug.map(str::chars).map(Iterator::count);
    let requested_locale_length = requested_locale.map(str::chars).map(Iterator::count);
    let tenant_default_locale_length = tenant_default_locale.map(str::chars).map(Iterator::count);
    let technical_owner_error = failure.technical_owner_error();

    if matches!(failure.kind, ShippingEnrichmentFailureKind::StorageUnavailable) {
        tracing::error!(
            error = ?technical_owner_error,
            owner = "rustok_fulfillment",
            owner_operation = "list_shipping_options",
            internal_code = failure.internal_code,
            internal_kind = failure.internal_kind,
            internal_retryable = failure.internal_retryable,
            tenant_id = %tenant_id,
            cart_id = %cart_id,
            line_item_count,
            delivery_group_count,
            currency_code_length,
            channel_slug_length = ?channel_slug_length,
            requested_locale_length = ?requested_locale_length,
            tenant_default_locale_length = ?tenant_default_locale_length,
            public_code = "CART_ENRICHMENT_UNAVAILABLE",
            public_retryable = true,
            boundary = STOREFRONT_SHIPPING_ENRICHMENT_GRAPHQL_BOUNDARY,
            "commerce GraphQL storefront shipping enrichment dependency failed"
        );
    } else {
        tracing::warn!(
            owner = "rustok_fulfillment",
            owner_operation = "list_shipping_options",
            internal_code = failure.internal_code,
            internal_kind = failure.internal_kind,
            internal_retryable = failure.internal_retryable,
            tenant_id = %tenant_id,
            cart_id = %cart_id,
            line_item_count,
            delivery_group_count,
            currency_code_length,
            channel_slug_length = ?channel_slug_length,
            requested_locale_length = ?requested_locale_length,
            tenant_default_locale_length = ?tenant_default_locale_length,
            public_code = "CART_ENRICHMENT_UNAVAILABLE",
            public_retryable = true,
            boundary = STOREFRONT_SHIPPING_ENRICHMENT_GRAPHQL_BOUNDARY,
            "commerce GraphQL storefront shipping enrichment was rejected"
        );
    }

    public_graphql_error()
}

pub(crate) async fn enrich_storefront_cart(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    request_context: &RequestContext,
    tenant_default_locale: &str,
    cart: CartResponse,
) -> Result<CartResponse> {
    let cart_id = cart.id;
    let line_item_count = cart.line_items.len();
    let delivery_group_count = cart.delivery_groups.len();
    let currency_code_length = cart.currency_code.chars().count();
    let public_channel_slug = normalize_public_channel_slug(cart.channel_slug.as_deref())
        .or_else(|| normalize_public_channel_slug(request_context.channel_slug.as_deref()));

    enrich_cart_delivery_groups_typed(
        db,
        tenant_id,
        cart,
        public_channel_slug.as_deref(),
        Some(request_context.locale.as_str()),
        Some(tenant_default_locale),
    )
    .await
    .map_err(|error| {
        shipping_enrichment_graphql_error(
            ShippingEnrichmentFailure::from_owner(error),
            tenant_id,
            cart_id,
            line_item_count,
            delivery_group_count,
            currency_code_length,
            public_channel_slug.as_deref(),
            Some(request_context.locale.as_str()),
            Some(tenant_default_locale),
        )
    })
}
