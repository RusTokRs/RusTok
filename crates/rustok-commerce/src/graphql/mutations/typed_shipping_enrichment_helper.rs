use async_graphql::{ErrorExtensions, Result};
use rustok_api::{PortContext, PortError, PortErrorKind, RequestContext};
use rustok_fulfillment::ListShippingOptionProjectionsRequest;
use uuid::Uuid;

use crate::{
    dto::CartResponse,
    storefront_channel::normalize_public_channel_slug,
    storefront_shipping::enrich_cart_delivery_groups_from_options,
};

const STOREFRONT_SHIPPING_ENRICHMENT_GRAPHQL_BOUNDARY: &str =
    "commerce_graphql_storefront_shipping_enrichment";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShippingEnrichmentFailureKind {
    Validation,
    NotFound,
    Conflict,
    Forbidden,
    StorageUnavailable,
    Invariant,
}

#[derive(Debug)]
struct ShippingEnrichmentFailure {
    kind: ShippingEnrichmentFailureKind,
    internal_code: String,
    internal_kind: &'static str,
    internal_retryable: bool,
    owner_error: PortError,
}

impl ShippingEnrichmentFailure {
    fn from_owner(error: PortError) -> Self {
        let (kind, internal_kind) = match &error.kind {
            PortErrorKind::Validation => (
                ShippingEnrichmentFailureKind::Validation,
                "validation",
            ),
            PortErrorKind::NotFound => (
                ShippingEnrichmentFailureKind::NotFound,
                "not_found",
            ),
            PortErrorKind::Conflict => (
                ShippingEnrichmentFailureKind::Conflict,
                "conflict",
            ),
            PortErrorKind::Forbidden => (
                ShippingEnrichmentFailureKind::Forbidden,
                "forbidden",
            ),
            PortErrorKind::Unavailable | PortErrorKind::Timeout => (
                ShippingEnrichmentFailureKind::StorageUnavailable,
                "unavailable",
            ),
            PortErrorKind::InvariantViolation => (
                ShippingEnrichmentFailureKind::Invariant,
                "invariant",
            ),
        };

        Self {
            kind,
            internal_code: error.code.clone(),
            internal_kind,
            internal_retryable: error.retryable,
            owner_error: error,
        }
    }

    fn technical_owner_error(&self) -> Option<&PortError> {
        if matches!(
            self.kind,
            ShippingEnrichmentFailureKind::StorageUnavailable
                | ShippingEnrichmentFailureKind::Invariant
        ) {
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
    context: &PortContext,
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

    if matches!(
        failure.kind,
        ShippingEnrichmentFailureKind::StorageUnavailable
            | ShippingEnrichmentFailureKind::Invariant
    ) {
        tracing::error!(
            error = ?technical_owner_error,
            owner = "rustok_fulfillment",
            owner_operation = "list_shipping_option_projections",
            internal_code = %failure.internal_code,
            internal_kind = failure.internal_kind,
            internal_retryable = failure.internal_retryable,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            context_channel_length = context.channel.as_deref().map(str::len),
            context_locale_length = context.locale.len(),
            deadline_ms = ?context.deadline_ms,
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
            owner_operation = "list_shipping_option_projections",
            internal_code = %failure.internal_code,
            internal_kind = failure.internal_kind,
            internal_retryable = failure.internal_retryable,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            context_channel_length = context.channel.as_deref().map(str::len),
            context_locale_length = context.locale.len(),
            deadline_ms = ?context.deadline_ms,
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
    let owner_context = super::shipping_option_read_context::storefront_shipping_option_read_context(
        tenant_id,
        cart.id,
        request_context.locale.as_str(),
        public_channel_slug.as_deref(),
        "list-options",
    );
    let shipping_option_read_port =
        super::shipping_option_read_context::storefront_shipping_option_read_port(db.clone());

    let options = shipping_option_read_port
        .list_shipping_option_projections(
            owner_context.clone(),
            ListShippingOptionProjectionsRequest {
                requested_locale: Some(request_context.locale.clone()),
                tenant_default_locale: Some(tenant_default_locale.to_string()),
            },
        )
        .await
        .map_err(|error| {
            shipping_enrichment_graphql_error(
                ShippingEnrichmentFailure::from_owner(error),
                &owner_context,
                cart_id,
                line_item_count,
                delivery_group_count,
                currency_code_length,
                public_channel_slug.as_deref(),
                Some(request_context.locale.as_str()),
                Some(tenant_default_locale),
            )
        })?;

    Ok(enrich_cart_delivery_groups_from_options(
        cart,
        options,
        public_channel_slug.as_deref(),
    ))
}
