use async_graphql::{ErrorExtensions, Result};
use rustok_api::{PortContext, PortError, PortErrorKind, RequestContext};
use rustok_cart::{CartStorefrontPort, CartStorefrontRepriceRequest};
use uuid::Uuid;

const STOREFRONT_REPRICE_GRAPHQL_BOUNDARY: &str = "commerce_graphql_storefront_reprice";

#[derive(Clone, Copy, Debug)]
enum RepriceFailureSource {
    Pricing,
    Cart,
}

impl RepriceFailureSource {
    const fn owner(self) -> &'static str {
        match self {
            Self::Pricing => "rustok_pricing",
            Self::Cart => "rustok_cart",
        }
    }

    const fn operation(self) -> &'static str {
        match self {
            Self::Pricing => "resolve_product_price",
            Self::Cart => "reprice_storefront_line_items",
        }
    }
}

struct RepriceFailure {
    source: RepriceFailureSource,
    error: PortError,
}

impl RepriceFailure {
    fn pricing(error: PortError) -> Self {
        Self {
            source: RepriceFailureSource::Pricing,
            error,
        }
    }

    fn cart(error: PortError) -> Self {
        Self {
            source: RepriceFailureSource::Cart,
            error,
        }
    }
}

fn public_graphql_error(retryable: bool) -> async_graphql::Error {
    async_graphql::Error::new("Cart pricing could not be refreshed").extend_with(|_, extensions| {
        extensions.set("code", "CART_REPRICE_FAILED");
        extensions.set("retryable", retryable);
    })
}

/// Context preparation still uses the established price-refresh public envelope. The
/// selected contextual Pricing decorator retains owner diagnostics; no owner text is exposed.
pub(crate) fn cart_context_preparation_error(error: PortError) -> async_graphql::Error {
    public_graphql_error(error.retryable)
}

#[allow(clippy::too_many_arguments)]
fn reprice_graphql_error(
    failure: RepriceFailure,
    context: &PortContext,
    cart_id: Uuid,
    line_item_id: Option<Uuid>,
    variant_id: Option<Uuid>,
    product_id: Option<Uuid>,
    requested_quantity: Option<i32>,
    planned_update_count: usize,
    cart_line_item_count: usize,
    currency_code_length: usize,
    request_channel_slug_length: Option<usize>,
) -> async_graphql::Error {
    let context_channel_length = context
        .channel
        .as_deref()
        .map(str::chars)
        .map(Iterator::count);
    let context_locale_length = context.locale.chars().count();
    let technical = matches!(
        &failure.error.kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
    );

    if technical {
        tracing::error!(
            owner = failure.source.owner(),
            owner_operation = failure.source.operation(),
            consumer_operation = "reprice_storefront_cart_line_items",
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor_kind = ?context.actor.kind,
            actor_id = %context.actor.id,
            context_channel_length = ?context_channel_length,
            context_locale_length,
            causation_id_present = context.causation_id.is_some(),
            traceparent_present = context.traceparent.is_some(),
            idempotency_key_present = context.idempotency_key.is_some(),
            deadline_ms = ?context.deadline_ms,
            cart_id = %cart_id,
            line_item_id = ?line_item_id,
            variant_id = ?variant_id,
            product_id = ?product_id,
            requested_quantity = ?requested_quantity,
            planned_update_count,
            cart_line_item_count,
            currency_code_length,
            request_channel_slug_length = ?request_channel_slug_length,
            owner_code = %failure.error.code,
            owner_kind = ?failure.error.kind,
            owner_retryable = failure.error.retryable,
            public_code = "CART_REPRICE_FAILED",
            public_retryable = failure.error.retryable,
            boundary = STOREFRONT_REPRICE_GRAPHQL_BOUNDARY,
            "commerce GraphQL storefront reprice owner call failed"
        );
    } else {
        tracing::warn!(
            owner = failure.source.owner(),
            owner_operation = failure.source.operation(),
            consumer_operation = "reprice_storefront_cart_line_items",
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor_kind = ?context.actor.kind,
            actor_id = %context.actor.id,
            context_channel_length = ?context_channel_length,
            context_locale_length,
            causation_id_present = context.causation_id.is_some(),
            traceparent_present = context.traceparent.is_some(),
            idempotency_key_present = context.idempotency_key.is_some(),
            deadline_ms = ?context.deadline_ms,
            cart_id = %cart_id,
            line_item_id = ?line_item_id,
            variant_id = ?variant_id,
            product_id = ?product_id,
            requested_quantity = ?requested_quantity,
            planned_update_count,
            cart_line_item_count,
            currency_code_length,
            request_channel_slug_length = ?request_channel_slug_length,
            owner_code = %failure.error.code,
            owner_kind = ?failure.error.kind,
            owner_retryable = failure.error.retryable,
            public_code = "CART_REPRICE_FAILED",
            public_retryable = failure.error.retryable,
            boundary = STOREFRONT_REPRICE_GRAPHQL_BOUNDARY,
            "commerce GraphQL storefront reprice owner call was rejected"
        );
    }

    public_graphql_error(failure.error.retryable)
}

pub(crate) async fn reprice_storefront_cart_line_items(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    request_context: &RequestContext,
    event_bus: &rustok_outbox::TransactionalEventBus,
    cart_storefront_port: &dyn CartStorefrontPort,
    cart: crate::dto::CartResponse,
) -> Result<crate::dto::CartResponse> {
    if cart.line_items.is_empty() {
        return Ok(cart);
    }

    let request_channel_slug_length = cart
        .channel_slug
        .as_deref()
        .map(str::chars)
        .map(Iterator::count);
    let currency_code_length = cart.currency_code.chars().count();
    let cart_line_item_count = cart.line_items.len();
    let pricing_context = rustok_cart::CartPricingContextGuard::capture(&cart)
        .map_err(|error| public_graphql_error(error.retryable))?;
    let pricing_read_port = super::cart::contextual_pricing_read_port(
        db.clone(),
        event_bus.clone(),
        crate::graphql_runtime::product_catalog_read_runtime_for_current_graphql_scope(
            db.clone(),
            event_bus.clone(),
        ),
    );
    let mut updates = Vec::new();

    for line_item in &cart.line_items {
        let Some(variant_id) = line_item.variant_id else {
            continue;
        };
        let pricing_port_context = super::cart_safe_helpers::storefront_pricing_port_context(
            tenant_id,
            request_context,
            cart.id,
            line_item.id,
        );
        let resolved_price: rustok_pricing::ResolvedPrice = crate::read_storefront_cart_line_quote(
            pricing_read_port.as_ref(),
            pricing_port_context.clone(),
            &cart,
            line_item.id,
            line_item.quantity,
        )
        .await
        .map_err(|error| {
            reprice_graphql_error(
                RepriceFailure::pricing(error),
                &pricing_port_context,
                cart.id,
                Some(line_item.id),
                Some(variant_id),
                line_item.product_id,
                Some(line_item.quantity),
                updates.len(),
                cart_line_item_count,
                currency_code_length,
                request_channel_slug_length,
            )
        })?
        .into();
        updates.push(super::cart_safe_helpers::storefront_cart_pricing_update(
            line_item.id,
            line_item.quantity,
            &resolved_price,
        ));
    }

    if updates.is_empty() {
        return Ok(cart);
    }

    let cart_port_context = super::cart_safe_helpers::storefront_cart_port_context(
        tenant_id,
        request_context,
        None,
        cart.id,
        "reprice",
        true,
    );
    let planned_update_count = updates.len();
    cart_storefront_port
        .reprice_storefront_line_items(
            cart_port_context.clone(),
            CartStorefrontRepriceRequest {
                cart_id: cart.id,
                updates,
                pricing_context,
            },
        )
        .await
        .map_err(|error| {
            reprice_graphql_error(
                RepriceFailure::cart(error),
                &cart_port_context,
                cart.id,
                None,
                None,
                None,
                None,
                planned_update_count,
                cart_line_item_count,
                currency_code_length,
                request_channel_slug_length,
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_reprice_envelope_keeps_owner_retryability_without_diagnostics() {
        for retryable in [false, true] {
            let public = public_graphql_error(retryable);
            assert_eq!(public.message, "Cart pricing could not be refreshed");
            let extensions = public.extensions.as_ref().unwrap();
            assert_eq!(
                extensions.get("code"),
                Some(&async_graphql::Value::from("CART_REPRICE_FAILED"))
            );
            assert_eq!(
                extensions.get("retryable"),
                Some(&async_graphql::Value::Boolean(retryable))
            );
        }
    }
}
