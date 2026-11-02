use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortContext, PortError, PortErrorKind, normalize_locale_tag};
use rustok_cart::{
    AtomicCartCheckoutPricingResolver, CartCheckoutLineItemPricingUpdate, CartCheckoutPricingPlan,
    CartPricingAdjustmentUpdate, CartResponse, PrepareCartCheckoutSnapshotRequest,
};
use rustok_outbox::TransactionalEventBus;
use rustok_pricing::{
    PricingReadPort, ResolveProductPriceRequest, ResolvedProductPriceSnapshot,
    in_process_pricing_read_port,
};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

/// Request-scoped storefront pricing resolver used by the durable checkout
/// binding after the checkout operation lease has been acquired.
#[derive(Clone)]
pub(crate) struct StorefrontCheckoutPricingResolver {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    product_catalog_read_runtime: rustok_product::ProductCatalogReadRuntime,
    context: PortContext,
}

impl StorefrontCheckoutPricingResolver {
    pub(crate) fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        product_catalog_read_runtime: rustok_product::ProductCatalogReadRuntime,
        context: PortContext,
    ) -> Self {
        Self {
            db,
            event_bus,
            product_catalog_read_runtime,
            context,
        }
    }
}

#[async_trait]
impl AtomicCartCheckoutPricingResolver for StorefrontCheckoutPricingResolver {
    async fn resolve_checkout_pricing(
        &self,
        tenant_id: Uuid,
        cart: &CartResponse,
        request: &PrepareCartCheckoutSnapshotRequest,
    ) -> Result<CartCheckoutPricingPlan, PortError> {
        if Uuid::parse_str(&self.context.tenant_id).ok() != Some(tenant_id)
            || tenant_id.is_nil()
            || cart.tenant_id != tenant_id
            || cart.id != request.cart_id
        {
            return Err(PortError::validation(
                "commerce.checkout_cart_invalid",
                "Checkout must retain the original tenant and Cart identity",
            ));
        }
        crate::services::checkout_catalog::variant_demands(cart).map_err(|_| {
            PortError::validation(
                "commerce.checkout_cart_invalid",
                "Cart lines or combined SKU quantities are invalid or exceed checkout bounds; inspect the cart before retrying",
            )
        })?;
        let pricing_read_port = in_process_pricing_read_port(
            self.db.clone(),
            self.event_bus.clone(),
            self.product_catalog_read_runtime.clone(),
        );
        let effective_region_id = cart.region_id.or(request.input.region_id);
        let cart_channel_slug = normalize_channel_slug(cart.channel_slug.as_deref());
        let currency_code = cart.currency_code.trim().to_ascii_uppercase();
        // Match Cart's saved-first context policy. Quotes are observed before Cart's
        // write lock; Cart verifies this plan against the locked snapshot before applying it.
        let mut pricing_cart = cart.clone();
        pricing_cart.region_id = effective_region_id;
        pricing_cart.locale_code = cart
            .locale_code
            .clone()
            .or_else(|| request.input.locale_code.clone());
        let mut line_items = Vec::new();

        for line_item in &cart.line_items {
            let Some(variant_id) = line_item.variant_id else {
                continue;
            };
            let resolved_price = read_storefront_cart_line_quote(
                pricing_read_port.as_ref(),
                self.context.clone(),
                &pricing_cart,
                line_item.id,
                line_item.quantity,
            )
            .await
            .map_err(checkout_resolution_error)?;

            line_items.push(checkout_line_item_pricing_update(
                line_item.id,
                variant_id,
                line_item.quantity,
                &resolved_price,
            ));
        }

        Ok(CartCheckoutPricingPlan {
            currency_code,
            effective_region_id,
            cart_channel_id: cart.channel_id,
            cart_channel_slug,
            line_items,
        })
    }
}

/// Read one authoritative Cart line quote through the already-selected Pricing owner.
///
/// Saved Cart selectors replace page context, including an explicitly global Cart. Missing
/// Cart locale inherits the admitted caller locale, never a fabricated language. Caller actor,
/// claims, roles, correlation, trace, causation, key and deadline are otherwise unchanged.
/// This observes prices; it neither authorizes Cart access nor reserves money or stock.
pub async fn read_storefront_cart_line_quote(
    pricing: &dyn PricingReadPort,
    context: PortContext,
    cart: &CartResponse,
    line_item_id: Uuid,
    quantity: i32,
) -> Result<ResolvedProductPriceSnapshot, PortError> {
    let (context, request) = build_cart_quote_input(context, cart, line_item_id, quantity)?;
    let quote = pricing
        .resolve_product_price(context, request.clone())
        .await?;
    if !cart_quote_matches(&request, &quote) {
        return Err(PortError::invariant_violation(
            "pricing.checkout_quote_mismatch",
            "Cart pricing returned an incompatible SKU or pricing context",
        ));
    }
    Ok(quote)
}

/// Prepare one immutable owner context command without changing Cart state. The admitted
/// caller must already have obtained the original Cart through its guarded owner read.
/// Batches retain expected-parent proof and per-line tiers; no single-price/local fallback
/// is attempted when the selected provider cannot prepare the complete target context.
pub async fn prepare_storefront_cart_context(
    pricing: &dyn PricingReadPort,
    context: PortContext,
    cart: &CartResponse,
    input: rustok_cart::UpdateCartContextInput,
) -> Result<rustok_cart::CartContextUpdateRequest, PortError> {
    use rustok_cart::{
        CartContextPricing, CartContextPricingPlan, CartLineItemPricingUpdate,
        CartPricingContextGuard, CartPricingScope, CartStatus,
    };
    use rustok_pricing::{
        MAX_PRICE_RESOLUTION_VARIANTS, PriceResolutionContext, ResolveProductPricesRequest,
    };
    use std::collections::BTreeMap;
    context.require_write_semantics()?;
    if cart.lifecycle_status().map_err(|_| {
        PortError::invariant_violation(
            "commerce.cart_lifecycle_invalid",
            "Cart lifecycle is invalid",
        )
    })? != CartStatus::Active
    {
        return Err(PortError::conflict(
            "commerce.cart_context_locked",
            "Cart context is locked; recover the original checkout instead of changing prices",
        ));
    }
    let pricing_context = CartPricingContextGuard::capture(cart)?;
    let target = cart.project_context(&input).map_err(|_| {
        PortError::validation(
            "commerce.cart_context_invalid",
            "Cart context is invalid; inspect its country, locale and shipping selections",
        )
    })?;
    let read_context = cart_pricing_context(context.clone(), &target)?;
    // Grouping by expected parent (not just quantity) retains Product-parent proof before
    // Pricing reads tenant-less rows. Repeated same-SKU lines stay separate Cart identities.
    let mut groups: BTreeMap<(Option<Uuid>, i32), BTreeMap<Uuid, Vec<Uuid>>> = BTreeMap::new();
    for line in &cart.line_items {
        let variant_id = line.variant_id.filter(|id| !id.is_nil()).ok_or_else(|| PortError::validation(
            "commerce.cart_pricing_variant_required", "Context repricing requires every original SKU; remove or reconcile unavailable lines first"))?;
        groups
            .entry((line.product_id, line.quantity))
            .or_default()
            .entry(variant_id)
            .or_default()
            .push(line.id);
    }
    let budget = std::time::Duration::from_millis(context.deadline_ms.unwrap_or(2000).min(30_000));
    let updates = tokio::time::timeout(budget, async {
        let mut updates = Vec::with_capacity(cart.line_items.len());
        for ((parent, quantity), variants) in groups {
            let ids: Vec<_> = variants.keys().copied().collect();
            for chunk in ids.chunks(MAX_PRICE_RESOLUTION_VARIANTS) {
                let request = ResolveProductPricesRequest {
                    product_id: parent,
                    variant_ids: chunk.to_vec(),
                    pricing: PriceResolutionContext {
                        currency_code: target.currency_code.clone(),
                        region_id: target.region_id,
                        price_list_id: None,
                        channel_id: target.channel_id,
                        channel_slug: target.channel_slug.clone(),
                        quantity: Some(quantity),
                    },
                };
                let batch = pricing
                    .resolve_product_prices(read_context.clone(), request)
                    .await?;
                if batch.items.len() != chunk.len() {
                    return Err(PortError::invariant_violation(
                        "commerce.cart_context_quote_invalid",
                        "Pricing omitted or duplicated context quote items",
                    ));
                }
                for (expected, item) in chunk.iter().zip(batch.items) {
                    if item.variant_id != *expected
                        || parent.is_some_and(|id| item.product_id != id)
                    {
                        return Err(PortError::invariant_violation(
                            "commerce.cart_context_quote_invalid",
                            "Pricing returned another SKU or parent while preparing context",
                        ));
                    }
                    let price = item.price.ok_or_else(|| {
                        PortError::not_found(
                            "pricing.price_not_found",
                            "A price is required for every Cart line in the target context",
                        )
                    })?;
                    let quote = ResolvedProductPriceSnapshot {
                        product_id: item.product_id,
                        variant_id: item.variant_id,
                        source: price.source,
                        context: batch.context.clone(),
                        resolved_at: batch.resolved_at,
                        currency_code: price.currency_code,
                        amount: price.amount,
                        compare_at_amount: price.compare_at_amount,
                        discount_percent: price.discount_percent,
                        on_sale: price.on_sale,
                        region_id: price.region_id,
                        min_quantity: price.min_quantity,
                        max_quantity: price.max_quantity,
                        price_list_id: price.price_list_id,
                        channel_id: price.channel_id,
                        channel_slug: price.channel_slug,
                    };
                    let expected_request = ResolveProductPriceRequest {
                        product_id: parent,
                        variant_id: *expected,
                        currency_code: target.currency_code.clone(),
                        region_id: target.region_id,
                        channel_id: target.channel_id,
                        channel_slug: target.channel_slug.clone(),
                        price_list_id: None,
                        quantity: Some(quantity),
                    };
                    if !cart_quote_matches(&expected_request, &quote) {
                        return Err(PortError::invariant_violation(
                            "commerce.cart_context_quote_invalid",
                            "Pricing returned incompatible target context",
                        ));
                    }
                    let (unit_price, adjustment) = cart_line_pricing_snapshot(quantity, &quote);
                    let line_ids = variants.get(expected).ok_or_else(|| {
                        PortError::invariant_violation(
                            "commerce.cart_context_quote_invalid",
                            "Cart context quote lost its original line identities",
                        )
                    })?;
                    for line_id in line_ids {
                        updates.push(CartLineItemPricingUpdate {
                            line_item_id: *line_id,
                            quantity,
                            unit_price,
                            pricing_adjustment: adjustment.clone(),
                        });
                    }
                }
            }
        }
        Ok::<_, PortError>(updates)
    })
    .await
    .map_err(|_| {
        PortError::timeout(
            "commerce.cart_context_pricing_timeout",
            "Cart context pricing exceeded its inherited wait budget; inspect before retrying",
        )
    })??;
    Ok(rustok_cart::CartContextUpdateRequest {
        cart_id: cart.id,
        input,
        pricing_context,
        pricing: CartContextPricing::Reprice(CartContextPricingPlan {
            scope: CartPricingScope::capture(&target),
            updates,
        }),
    })
}

/// Reuse saved Cart scope and saved-or-admitted locale for all owner reads in one observation.
pub(crate) fn cart_pricing_context(
    mut context: PortContext,
    cart: &CartResponse,
) -> Result<PortContext, PortError> {
    context.require_read_semantics()?;
    if cart.id.is_nil()
        || cart.tenant_id.is_nil()
        || Uuid::parse_str(&context.tenant_id).ok() != Some(cart.tenant_id)
    {
        return Err(PortError::validation(
            "commerce.cart_pricing_request_invalid",
            "Cart pricing requires the original tenant and Cart",
        ));
    }
    if cart.channel_id.is_some_and(|id| id.is_nil())
        || cart
            .channel_slug
            .as_deref()
            .is_some_and(|slug| slug.trim().is_empty())
    {
        return Err(PortError::validation(
            "commerce.cart_pricing_channel_invalid",
            "Saved Cart channel is invalid; restore the Cart context before repricing",
        ));
    }
    let locale = cart.locale_code.as_deref().unwrap_or(&context.locale);
    context.locale = normalize_locale_tag(locale).ok_or_else(|| {
        PortError::validation(
            "commerce.cart_pricing_locale_invalid",
            "Cart pricing requires a valid saved or admitted caller locale",
        )
    })?;
    // Clear page scope even for an ID-only or global Cart. Only Channel may complete
    // a saved selector; borrowing the caller's slug would create another scope.
    context.channel = cart.channel_slug.clone();
    Ok(context)
}

fn build_cart_quote_input(
    mut context: PortContext,
    cart: &CartResponse,
    line_item_id: Uuid,
    quantity: i32,
) -> Result<(PortContext, ResolveProductPriceRequest), PortError> {
    context.require_read_semantics()?;
    if cart.id.is_nil()
        || cart.tenant_id.is_nil()
        || Uuid::parse_str(&context.tenant_id).ok() != Some(cart.tenant_id)
        || line_item_id.is_nil()
        || quantity < 1
    {
        return Err(PortError::validation(
            "commerce.cart_pricing_request_invalid",
            "Cart pricing requires the original tenant, Cart, line and a positive quantity",
        ));
    }
    let mut matches = cart
        .line_items
        .iter()
        .filter(|line| line.id == line_item_id);
    let line = matches.next().ok_or_else(|| {
        PortError::not_found(
            "commerce.cart_pricing_line_not_found",
            "The selected Cart line is no longer available",
        )
    })?;
    if matches.next().is_some()
        || line.cart_id != cart.id
        || line.quantity < 1
        || line.product_id.is_some_and(|id| id.is_nil())
        || !line.currency_code.eq_ignore_ascii_case(&cart.currency_code)
    {
        return Err(PortError::invariant_violation(
            "commerce.cart_pricing_line_invalid",
            "Saved Cart line identity, quantity or currency is inconsistent",
        ));
    }
    let variant_id = line.variant_id.filter(|id| !id.is_nil()).ok_or_else(|| {
        PortError::validation(
            "commerce.cart_pricing_variant_required",
            "Cart pricing requires the original SKU identity",
        )
    })?;
    context = cart_pricing_context(context, cart)?;
    let request = ResolveProductPriceRequest {
        product_id: line.product_id,
        variant_id,
        region_id: cart.region_id,
        channel_id: cart.channel_id,
        channel_slug: cart.channel_slug.clone(),
        price_list_id: None,
        quantity: Some(quantity),
        currency_code: cart.currency_code.trim().to_ascii_uppercase(),
    };
    Ok((context, request))
}

/// Correlate an authoritative owner quote with the exact Cart request. Completing one saved
/// channel selector is allowed; borrowing a page/default channel or accepting a list preview is
/// not. Row/source scope may be global while the normalized request context is channel-scoped.
pub(crate) fn cart_quote_matches(
    request: &ResolveProductPriceRequest,
    quote: &ResolvedProductPriceSnapshot,
) -> bool {
    // Reject malformed/overflowing provider money before projecting an adjustment. Both
    // effective and compare-at totals must use the canonical currency's fixed-point bounds.
    let Ok(exponent) = rustok_core::money::currency_exponent(&request.currency_code) else {
        return false;
    };
    let quantity = Decimal::from(request.quantity.unwrap_or(1));
    for amount in [
        quote.amount,
        quote.compare_at_amount.unwrap_or(quote.amount),
    ] {
        if amount < Decimal::ZERO
            || rustok_core::money::to_fixed_point_units(amount, exponent).is_err()
            || amount.checked_mul(quantity).is_none_or(|total| {
                rustok_core::money::to_fixed_point_units(total, exponent).is_err()
            })
        {
            return false;
        }
    }
    let context = &quote.context;
    if request.variant_id.is_nil()
        || request.quantity.is_some_and(|quantity| quantity < 1)
        || quote.product_id.is_nil()
        || quote.variant_id != request.variant_id
        || request.product_id.is_some_and(|id| id != quote.product_id)
        || !quote
            .currency_code
            .eq_ignore_ascii_case(&request.currency_code)
        || !context
            .currency_code
            .eq_ignore_ascii_case(&request.currency_code)
        || context.quantity != Some(request.quantity.unwrap_or(1))
        || context.region_id != request.region_id
        || request.price_list_id.is_some()
        || context.price_list_id.is_some()
        || context.channel_id.is_some() != context.channel_slug.is_some()
        || context.channel_id.is_some_and(|id| id.is_nil())
        || context
            .channel_slug
            .as_deref()
            .is_some_and(|slug| slug.trim().is_empty())
    {
        return false;
    }
    if request
        .channel_id
        .is_some_and(|id| context.channel_id != Some(id))
    {
        return false;
    }
    if let Some(expected) = request.channel_slug.as_deref()
        && !context.channel_slug.as_deref().is_some_and(|actual| {
            !expected.trim().is_empty() && actual.eq_ignore_ascii_case(expected.trim())
        })
    {
        return false;
    }
    if request.channel_id.is_none() && request.channel_slug.is_none() {
        return context.channel_id.is_none() && context.channel_slug.is_none();
    }
    true
}

fn checkout_resolution_error(error: PortError) -> PortError {
    // The owner decides retryability, including permanent Unavailable/Timeout refusals.
    // Presentation sanitizes diagnostics but must not turn missing capability into a retry loop.
    let public = rustok_pricing::map_pricing_port_error(&error);
    let code = if matches!(
        error.kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout
    ) {
        "pricing.checkout_resolution_unavailable"
    } else {
        "pricing.checkout_resolution_refused"
    };
    PortError::new(error.kind, code, public.message, error.retryable)
}

fn checkout_line_item_pricing_update(
    line_item_id: Uuid,
    variant_id: Uuid,
    quantity: i32,
    resolved_price: &ResolvedProductPriceSnapshot,
) -> CartCheckoutLineItemPricingUpdate {
    let (base_unit_price, pricing_adjustment) =
        cart_line_pricing_snapshot(quantity, resolved_price);

    CartCheckoutLineItemPricingUpdate {
        line_item_id,
        variant_id,
        quantity,
        unit_price: base_unit_price,
        pricing_adjustment,
    }
}

/// The same owner-composed price snapshot is used when adding a line and at checkout.
/// Pricing supplies the resolved money/source; this only projects it into Cart's adjustment DTO.
pub(crate) fn cart_line_pricing_snapshot(
    quantity: i32,
    resolved_price: &ResolvedProductPriceSnapshot,
) -> (Decimal, Option<CartPricingAdjustmentUpdate>) {
    let base_unit_price = resolved_price
        .compare_at_amount
        .filter(|compare_at| *compare_at > resolved_price.amount)
        .unwrap_or(resolved_price.amount);
    let pricing_adjustment = if base_unit_price > resolved_price.amount {
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "price_id".into(),
            Value::from(resolved_price.source.price_id().to_string()),
        );
        metadata.insert(
            "price_source".into(),
            Value::from(resolved_price.source.kind()),
        );
        metadata.insert(
            "quoted_at".into(),
            Value::from(resolved_price.resolved_at.to_rfc3339()),
        );
        metadata.insert("quoted_quantity".into(), Value::from(quantity));
        if let Some(rule) = resolved_price.source.rule() {
            metadata.insert("rule_kind".into(), Value::from(rule.kind.as_str()));
            metadata.insert(
                "rule_percent".into(),
                Value::from(rule.adjustment_percent.normalize().to_string()),
            );
        }
        metadata.insert(
            "kind".to_string(),
            Value::from(if resolved_price.price_list_id.is_some() {
                "price_list"
            } else {
                "sale"
            }),
        );
        metadata.insert(
            "base_amount".to_string(),
            Value::from(base_unit_price.normalize().to_string()),
        );
        metadata.insert(
            "effective_amount".to_string(),
            Value::from(resolved_price.amount.normalize().to_string()),
        );
        if let Some(compare_at_amount) = resolved_price.compare_at_amount {
            metadata.insert(
                "compare_at_amount".to_string(),
                Value::from(compare_at_amount.normalize().to_string()),
            );
        }
        if let Some(discount_percent) = resolved_price.discount_percent {
            metadata.insert(
                "discount_percent".to_string(),
                Value::from(discount_percent.normalize().to_string()),
            );
        }
        if let Some(price_list_id) = resolved_price.price_list_id {
            metadata.insert(
                "price_list_id".to_string(),
                Value::from(price_list_id.to_string()),
            );
        }
        if let Some(channel_id) = resolved_price.channel_id {
            metadata.insert(
                "channel_id".to_string(),
                Value::from(channel_id.to_string()),
            );
        }
        if let Some(channel_slug) = resolved_price.channel_slug.as_deref() {
            metadata.insert("channel_slug".to_string(), Value::from(channel_slug));
        }

        Some(CartPricingAdjustmentUpdate {
            source_id: resolved_price.price_list_id.map(|value| value.to_string()),
            amount: (base_unit_price - resolved_price.amount) * Decimal::from(quantity),
            metadata: Value::Object(metadata),
        })
    } else {
        None
    };

    (base_unit_price, pricing_adjustment)
}

fn normalize_channel_slug(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkout_keeps_definitive_quote_refusals_distinct_from_transient_failures() {
        let error = checkout_resolution_error(PortError::conflict(
            "pricing.price_ambiguous",
            "private-price-details",
        ));
        assert_eq!(error.kind, PortErrorKind::Conflict);
        assert!(!error.retryable);
        assert!(!error.message.contains("private-price-details"));
        let error = checkout_resolution_error(PortError::timeout(
            "pricing.resolution_timeout",
            "private-timeout",
        ));
        assert_eq!(error.kind, PortErrorKind::Timeout);
        assert!(error.retryable);
        assert_eq!(error.code, "pricing.checkout_resolution_unavailable");
    }

    #[test]
    fn permanent_owner_refusals_keep_their_retry_bit_and_sanitized_message() {
        for kind in [PortErrorKind::Unavailable, PortErrorKind::Timeout] {
            let error = checkout_resolution_error(PortError::new(
                kind.clone(),
                "pricing.product_identity_capability_unavailable",
                "private-capability-details",
                false,
            ));
            assert_eq!(error.kind, kind);
            assert!(!error.retryable);
            assert!(!error.message.contains("private-capability-details"));
        }
    }

    pub(super) fn cart_fixture() -> (PortContext, CartResponse, Uuid) {
        let now = chrono::Utc::now();
        let tenant_id = Uuid::from_u128(1);
        let cart_id = Uuid::from_u128(2);
        let line_id = Uuid::from_u128(5);
        let cart = CartResponse {
            id: cart_id,
            tenant_id,
            channel_id: Some(Uuid::from_u128(8)),
            channel_slug: Some("web".into()),
            customer_id: None,
            email: None,
            region_id: Some(Uuid::from_u128(7)),
            country_code: None,
            locale_code: Some("ru".into()),
            selected_shipping_option_id: None,
            status: "active".into(),
            currency_code: "USD".into(),
            subtotal_amount: Decimal::ZERO,
            adjustment_total: Decimal::ZERO,
            shipping_total: Decimal::ZERO,
            total_amount: Decimal::ZERO,
            tax_total: Decimal::ZERO,
            metadata: Value::Null,
            created_at: now,
            updated_at: now,
            completed_at: None,
            line_items: vec![rustok_cart::CartLineItemResponse {
                id: line_id,
                cart_id,
                product_id: Some(Uuid::from_u128(3)),
                variant_id: Some(Uuid::from_u128(4)),
                fulfillment_requirement: rustok_cart::CartLineFulfillmentRequirement::Digital,
                shipping_profile_slug: None,
                seller_id: None,
                seller_scope: None,
                sku: Some("saved-sku".into()),
                title: "Saved label".into(),
                quantity: 3,
                unit_price: Decimal::from(999),
                total_price: Decimal::from(2997),
                currency_code: "USD".into(),
                metadata: Value::Null,
                created_at: now,
                updated_at: now,
            }],
            adjustments: vec![],
            tax_lines: vec![],
            delivery_groups: vec![],
        };
        let context = PortContext::new(
            tenant_id.to_string(),
            rustok_api::PortActor::user(Uuid::from_u128(9).to_string()),
            "fr",
            "original-correlation",
        )
        .with_claim("original-claim")
        .with_role("original-role")
        .with_channel("page-channel")
        .with_causation_id("original-causation")
        .with_traceparent("original-trace")
        .with_idempotency_key("original-key")
        .with_deadline(std::time::Duration::from_millis(73));
        (context, cart, line_id)
    }

    #[test]
    fn cart_quote_input_keeps_caller_metadata_and_saved_scope_for_a_quantity_edit() {
        let (context, cart, line_id) = cart_fixture();
        let mut expected_context = context.clone();
        expected_context.channel = cart.channel_slug.clone();
        expected_context.locale = "ru".into();
        let (actual, request) = build_cart_quote_input(context, &cart, line_id, 2).unwrap();
        assert_eq!(actual, expected_context);
        assert_eq!(request.variant_id, cart.line_items[0].variant_id.unwrap());
        assert_eq!(request.product_id, cart.line_items[0].product_id);
        assert_eq!(request.region_id, cart.region_id);
        assert_eq!(request.channel_id, cart.channel_id);
        assert_eq!(request.channel_slug, cart.channel_slug);
        assert_eq!(request.currency_code, "USD");
        assert_eq!(request.quantity, Some(2));
        assert_eq!(request.price_list_id, None);
        assert_eq!(cart.line_items[0].quantity, 3);
        assert_eq!(cart.line_items[0].unit_price, Decimal::from(999));
    }

    #[test]
    fn saved_id_only_and_global_cart_never_borrow_page_slug() {
        let (context, mut cart, line_id) = cart_fixture();
        cart.channel_slug = None;
        let (actual, request) = build_cart_quote_input(context.clone(), &cart, line_id, 3).unwrap();
        assert_eq!(actual.channel, None);
        assert_eq!(request.channel_slug, None);
        assert_eq!(request.channel_id, cart.channel_id);
        cart.channel_id = None;
        let (actual, request) = build_cart_quote_input(context, &cart, line_id, 3).unwrap();
        assert_eq!(actual.channel, None);
        assert_eq!(request.channel_id, None);
        assert_eq!(request.channel_slug, None);
    }

    #[test]
    fn missing_cart_locale_inherits_caller_but_malformed_saved_locale_refuses() {
        let (context, mut cart, line_id) = cart_fixture();
        cart.locale_code = None;
        let (actual, _) = build_cart_quote_input(context.clone(), &cart, line_id, 3).unwrap();
        assert_eq!(actual.locale, "fr");
        for saved in ["", "??"] {
            cart.locale_code = Some(saved.into());
            let error = build_cart_quote_input(context.clone(), &cart, line_id, 3).unwrap_err();
            assert_eq!(error.code, "commerce.cart_pricing_locale_invalid");
        }
    }

    #[test]
    fn cart_quote_input_refuses_wrong_tenant_or_line_binding_before_owner_io() {
        let (mut context, mut cart, line_id) = cart_fixture();
        context.tenant_id = Uuid::from_u128(99).to_string();
        assert!(build_cart_quote_input(context.clone(), &cart, line_id, 3).is_err());
        context.tenant_id = cart.tenant_id.to_string();
        cart.line_items[0].cart_id = Uuid::from_u128(99);
        assert!(build_cart_quote_input(context.clone(), &cart, line_id, 3).is_err());
        cart.line_items[0].cart_id = cart.id;
        cart.line_items.push(cart.line_items[0].clone());
        assert!(build_cart_quote_input(context, &cart, line_id, 3).is_err());
    }

    #[test]
    fn cart_quote_input_rejects_malformed_channel_without_page_fallback() {
        let (context, mut cart, line_id) = cart_fixture();
        cart.channel_slug = Some(" ".into());
        assert!(build_cart_quote_input(context.clone(), &cart, line_id, 3).is_err());
        cart.channel_slug = None;
        cart.channel_id = Some(Uuid::nil());
        assert!(build_cart_quote_input(context, &cart, line_id, 3).is_err());
    }

    #[test]
    fn cart_quote_input_never_reconstructs_a_missing_sku_or_repairs_invalid_saved_lines() {
        let (context, mut cart, line_id) = cart_fixture();
        for quantity in [0, -1] {
            assert!(build_cart_quote_input(context.clone(), &cart, line_id, quantity).is_err());
        }
        cart.line_items[0].quantity = 0;
        assert!(build_cart_quote_input(context.clone(), &cart, line_id, 3).is_err());
        cart.line_items[0].quantity = 3;
        cart.line_items[0].currency_code = "EUR".into();
        assert!(build_cart_quote_input(context.clone(), &cart, line_id, 3).is_err());
        cart.line_items[0].currency_code = "USD".into();
        cart.line_items[0].variant_id = None;
        assert!(build_cart_quote_input(context.clone(), &cart, line_id, 3).is_err());
        cart.line_items[0].variant_id = Some(Uuid::nil());
        assert!(build_cart_quote_input(context, &cart, line_id, 3).is_err());
    }

    fn quote_fixture() -> (ResolveProductPriceRequest, ResolvedProductPriceSnapshot) {
        let request = ResolveProductPriceRequest {
            product_id: Some(Uuid::from_u128(3)),
            variant_id: Uuid::from_u128(4),
            region_id: Some(Uuid::from_u128(7)),
            channel_id: Some(Uuid::from_u128(8)),
            channel_slug: Some("web".to_string()),
            price_list_id: None,
            quantity: Some(3),
            currency_code: "USD".to_string(),
        };
        let quote = ResolvedProductPriceSnapshot {
            source: rustok_pricing::PriceResolutionSource::Base {
                price_id: Uuid::from_u128(6),
            },
            context: rustok_pricing::PriceResolutionContext {
                currency_code: request.currency_code.clone(),
                region_id: request.region_id,
                channel_id: request.channel_id,
                channel_slug: request.channel_slug.clone(),
                price_list_id: None,
                quantity: request.quantity,
            },
            resolved_at: chrono::DateTime::parse_from_rfc3339("2026-10-30T12:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            product_id: request.product_id.unwrap(),
            variant_id: request.variant_id,
            currency_code: "USD".to_string(),
            amount: Decimal::from(90),
            compare_at_amount: Some(Decimal::from(100)),
            discount_percent: Some(Decimal::from(10)),
            on_sale: true,
            region_id: None,
            min_quantity: None,
            max_quantity: None,
            price_list_id: None,
            channel_id: None,
            channel_slug: None,
        };
        (request, quote)
    }

    #[test]
    fn cart_quote_accepts_global_price_row_in_an_exact_normalized_request_context() {
        let (request, quote) = quote_fixture();
        assert!(cart_quote_matches(&request, &quote));
        let (regular, adjustment) = cart_line_pricing_snapshot(3, &quote);
        assert_eq!(regular, Decimal::from(100));
        let adjustment = adjustment.unwrap();
        assert_eq!(adjustment.amount, Decimal::from(30));
        assert_eq!(
            adjustment.metadata["price_id"],
            quote.source.price_id().to_string()
        );
        assert_eq!(adjustment.metadata["quoted_quantity"], 3);
    }

    #[test]
    fn cart_quote_correlation_rejects_wrong_uuid_quantity_currency_region_and_preview() {
        let (request, quote) = quote_fixture();
        let mut invalid = quote.clone();
        invalid.variant_id = Uuid::new_v4();
        assert!(!cart_quote_matches(&request, &invalid));
        invalid = quote.clone();
        invalid.product_id = Uuid::new_v4();
        assert!(!cart_quote_matches(&request, &invalid));
        invalid = quote.clone();
        invalid.context.quantity = Some(2);
        assert!(!cart_quote_matches(&request, &invalid));
        invalid = quote.clone();
        invalid.context.currency_code = "EUR".to_string();
        assert!(!cart_quote_matches(&request, &invalid));
        invalid = quote.clone();
        invalid.currency_code = "EUR".to_string();
        assert!(!cart_quote_matches(&request, &invalid));
        invalid = quote.clone();
        invalid.context.region_id = None;
        assert!(!cart_quote_matches(&request, &invalid));
        invalid = quote.clone();
        invalid.context.price_list_id = Some(Uuid::new_v4());
        assert!(!cart_quote_matches(&request, &invalid));
        let mut preview = request;
        preview.price_list_id = Some(Uuid::new_v4());
        assert!(!cart_quote_matches(&preview, &quote));
    }

    #[test]
    fn cart_quote_channel_completion_never_means_null_as_a_wildcard_or_page_borrowing() {
        let (request, quote) = quote_fixture();
        let mut id_only = request.clone();
        id_only.channel_slug = None;
        assert!(cart_quote_matches(&id_only, &quote));
        let mut slug_only = request.clone();
        slug_only.channel_id = None;
        assert!(cart_quote_matches(&slug_only, &quote));
        let mut invalid = quote.clone();
        invalid.context.channel_slug = Some("mobile".to_string());
        assert!(!cart_quote_matches(&request, &invalid));
        invalid = quote.clone();
        invalid.context.channel_id = Some(Uuid::new_v4());
        assert!(!cart_quote_matches(&request, &invalid));
        invalid = quote.clone();
        invalid.context.channel_slug = None;
        assert!(!cart_quote_matches(&id_only, &invalid));
        let mut global = request;
        global.channel_id = None;
        global.channel_slug = None;
        assert!(!cart_quote_matches(&global, &quote));
        let mut global_quote = quote;
        global_quote.context.channel_id = None;
        global_quote.context.channel_slug = None;
        assert!(cart_quote_matches(&global, &global_quote));
    }
    #[test]
    fn cart_quote_money_is_checked_before_adjustment_projection() {
        let (request, quote) = quote_fixture();
        for invalid_amount in [Decimal::MAX, Decimal::NEGATIVE_ONE, Decimal::new(1234, 3)] {
            let mut invalid = quote.clone();
            invalid.amount = invalid_amount;
            assert!(!cart_quote_matches(&request, &invalid));
            let mut invalid = quote.clone();
            invalid.compare_at_amount = Some(invalid_amount);
            assert!(!cart_quote_matches(&request, &invalid));
        }
        let mut invalid = quote;
        // Individually valid money must also fit the quoted line's fixed-point total.
        invalid.compare_at_amount = Some(Decimal::from(i64::MAX) / Decimal::from(100));
        assert!(!cart_quote_matches(&request, &invalid));
    }
}

#[cfg(test)]
#[path = "storefront_cart_context_pricing_tests.rs"]
mod context_pricing_tests;
