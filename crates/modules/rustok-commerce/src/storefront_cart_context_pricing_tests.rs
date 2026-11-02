//! Focused selected-owner context-preparation regressions. Source only until run by the maintainer.
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use super::*;
use rustok_cart::{
    CartContextPricing, CartPricingContextGuard, CartPricingScope, UpdateCartContextInput,
};
use rustok_pricing as pricing;

#[derive(Clone, Copy)]
enum Reply {
    Exact,
    Unavailable,
    MissingItem,
    Duplicate,
    Reordered,
    WrongVariant,
    WrongParent,
    WrongCurrency,
    WrongRegion,
    WrongQuantity,
    Preview,
    MissingPrice,
    WrongChannel,
}

struct PricingProbe {
    requests: Mutex<Vec<(PortContext, pricing::ResolveProductPricesRequest)>>,
    singles: AtomicUsize,
    reply: Reply,
    delay: Duration,
}

impl PricingProbe {
    fn new(reply: Reply) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            singles: AtomicUsize::new(0),
            reply,
            delay: Duration::ZERO,
        }
    }
}

#[async_trait::async_trait]
impl PricingReadPort for PricingProbe {
    async fn resolve_product_price(
        &self,
        _: PortContext,
        _: ResolveProductPriceRequest,
    ) -> Result<ResolvedProductPriceSnapshot, PortError> {
        self.singles.fetch_add(1, Ordering::SeqCst);
        Err(PortError::invariant_violation(
            "test.unexpected_single",
            "Context preparation must not fall back to single reads",
        ))
    }

    async fn resolve_product_prices(
        &self,
        context: PortContext,
        request: pricing::ResolveProductPricesRequest,
    ) -> Result<pricing::ResolvedProductPricesSnapshot, PortError> {
        self.requests
            .lock()
            .unwrap()
            .push((context, request.clone()));
        tokio::time::sleep(self.delay).await;
        if matches!(self.reply, Reply::Unavailable) {
            return Err(PortError::new(
                PortErrorKind::Unavailable,
                "pricing.batch_capability_unavailable",
                "private selected-provider detail",
                false,
            ));
        }
        let mut scope = request.pricing.clone();
        if scope.channel_id.is_some() && scope.channel_slug.is_none() {
            scope.channel_slug = Some("web".into());
        }
        let mut items = request
            .variant_ids
            .iter()
            .map(|id| pricing::ProductPriceResolutionSnapshot {
                product_id: request.product_id.unwrap_or(Uuid::from_u128(3)),
                variant_id: *id,
                price: Some(pricing::ResolvedPrice {
                    source: pricing::PriceResolutionSource::Base {
                        price_id: Uuid::from_u128(60),
                    },
                    currency_code: scope.currency_code.clone(),
                    amount: Decimal::from(scope.quantity.unwrap()),
                    compare_at_amount: Some(Decimal::from(100)),
                    discount_percent: None,
                    on_sale: true,
                    region_id: None,
                    min_quantity: None,
                    max_quantity: None,
                    price_list_id: None,
                    channel_id: None,
                    channel_slug: None,
                }),
            })
            .collect::<Vec<_>>();
        match self.reply {
            Reply::MissingItem => {
                items.pop();
            }
            Reply::Duplicate => {
                items[1] = items[0].clone();
            }
            Reply::Reordered => items.reverse(),
            Reply::WrongVariant => items[0].variant_id = Uuid::from_u128(99),
            Reply::WrongParent => items[0].product_id = Uuid::from_u128(99),
            Reply::WrongCurrency => scope.currency_code = "EUR".into(),
            Reply::WrongRegion => scope.region_id = None,
            Reply::WrongQuantity => scope.quantity = Some(999),
            Reply::Preview => scope.price_list_id = Some(Uuid::from_u128(99)),
            Reply::MissingPrice => items[0].price = None,
            Reply::WrongChannel => scope.channel_slug = Some("another".into()),
            Reply::Exact | Reply::Unavailable => {}
        }
        Ok(pricing::ResolvedProductPricesSnapshot {
            context: scope,
            resolved_at: chrono::Utc::now(),
            items,
        })
    }

    async fn list_storefront_product_pricing(
        &self,
        _: PortContext,
        _: pricing::StorefrontProductPricingListRequest,
    ) -> Result<pricing::StorefrontPricingProductList, PortError> {
        Err(PortError::invariant_violation(
            "test.unexpected_read",
            "Unexpected listing",
        ))
    }
    async fn read_price_list_projection(
        &self,
        _: PortContext,
        _: pricing::PriceListProjectionRequest,
    ) -> Result<pricing::PriceListProjectionSnapshot, PortError> {
        Err(PortError::invariant_violation(
            "test.unexpected_read",
            "Unexpected price-list read",
        ))
    }
    async fn list_active_price_list_projections(
        &self,
        _: PortContext,
        _: pricing::ActivePriceListProjectionRequest,
    ) -> Result<Vec<pricing::ActivePriceListProjectionSnapshot>, PortError> {
        Err(PortError::invariant_violation(
            "test.unexpected_read",
            "Unexpected price-list listing",
        ))
    }
    async fn read_admin_product_pricing_projection(
        &self,
        _: PortContext,
        _: pricing::AdminProductPricingProjectionRequest,
    ) -> Result<pricing::AdminPricingProductDetail, PortError> {
        Err(PortError::invariant_violation(
            "test.unexpected_read",
            "Unexpected admin read",
        ))
    }
    async fn read_storefront_product_pricing_projection(
        &self,
        _: PortContext,
        _: pricing::StorefrontProductPricingProjectionRequest,
    ) -> Result<Option<pricing::StorefrontPricingProductDetail>, PortError> {
        Err(PortError::invariant_violation(
            "test.unexpected_read",
            "Unexpected storefront read",
        ))
    }
    async fn preview_variant_discount(
        &self,
        _: PortContext,
        _: pricing::PreviewVariantDiscountRequest,
    ) -> Result<pricing::PriceAdjustmentPreview, PortError> {
        Err(PortError::invariant_violation(
            "test.unexpected_read",
            "Unexpected preview",
        ))
    }
}

fn target_input(cart: &CartResponse) -> UpdateCartContextInput {
    UpdateCartContextInput {
        email: cart.email.clone(),
        region_id: Some(Uuid::from_u128(70)),
        country_code: Some("fr".into()),
        locale_code: Some("fr_CA".into()),
        selected_shipping_option_id: None,
        shipping_selections: None,
    }
}

fn add_line(cart: &mut CartResponse, id: u128, variant: u128, parent: u128, quantity: i32) {
    let mut line = cart.line_items[0].clone();
    line.id = Uuid::from_u128(id);
    line.variant_id = Some(Uuid::from_u128(variant));
    line.product_id = Some(Uuid::from_u128(parent));
    line.quantity = quantity;
    line.total_price = line.unit_price * Decimal::from(quantity);
    cart.line_items.push(line);
}

#[tokio::test]
async fn original_observation_and_caller_metadata_survive_target_projection_and_per_line_tiers() {
    let (context, mut cart, _) = super::tests::cart_fixture();
    add_line(&mut cart, 10, 4, 3, 3);
    add_line(&mut cart, 11, 4, 3, 5);
    add_line(&mut cart, 12, 41, 31, 3);
    let original = CartPricingContextGuard::capture(&cart).unwrap();
    let owner = PricingProbe::new(Reply::Exact);
    let input = target_input(&cart);
    let command = prepare_storefront_cart_context(&owner, context.clone(), &cart, input.clone())
        .await
        .unwrap();
    assert_eq!(command.pricing_context, original);
    assert_eq!(command.pricing_context.locale_code.as_deref(), Some("ru"));
    let CartContextPricing::Reprice(plan) = command.pricing else {
        panic!("a buyer context must be repriced")
    };
    assert_eq!(
        plan.scope,
        CartPricingScope::capture(&cart.project_context(&input).unwrap())
    );
    assert_eq!(plan.scope.locale_code.as_deref(), Some("fr-ca"));
    assert_eq!(plan.updates.len(), 4);
    let requests = owner.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    for (actual, request) in requests.iter() {
        let mut expected = context.clone();
        expected.channel = cart.channel_slug.clone();
        expected.locale = "fr-CA".into();
        assert_eq!(*actual, expected);
        assert_eq!(request.pricing.region_id, input.region_id);
        assert_eq!(request.variant_ids.len(), 1);
        assert!(request.pricing.quantity == Some(3) || request.pricing.quantity == Some(5));
    }
    for update in plan.updates {
        let original = cart
            .line_items
            .iter()
            .find(|line| line.id == update.line_item_id)
            .unwrap();
        assert_eq!(update.quantity, original.quantity);
        assert_eq!(
            update.pricing_adjustment.unwrap().metadata["effective_amount"],
            original.quantity.to_string()
        );
    }
    assert_eq!(owner.singles.load(Ordering::SeqCst), 0);
    assert_eq!(cart.locale_code.as_deref(), Some("ru"));
    assert_eq!(cart.line_items[0].unit_price, Decimal::from(999));
}

#[tokio::test]
async fn unique_variant_batches_are_chunked_at_owner_bound_and_fan_out_to_original_line_ids() {
    let (context, mut cart, _) = super::tests::cart_fixture();
    for index in 0..256u128 {
        add_line(&mut cart, 100 + index, 1000 + index, 3, 3);
    }
    add_line(&mut cart, 500, 4, 3, 3);
    let owner = PricingProbe::new(Reply::Exact);
    let command = prepare_storefront_cart_context(&owner, context, &cart, target_input(&cart))
        .await
        .unwrap();
    let CartContextPricing::Reprice(plan) = command.pricing else {
        panic!("missing context plan")
    };
    assert_eq!(plan.updates.len(), 258);
    let requests = owner.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].1.variant_ids.len(),
        pricing::MAX_PRICE_RESOLUTION_VARIANTS
    );
    assert_eq!(requests[1].1.variant_ids.len(), 1);
    assert!(
        requests
            .iter()
            .all(|(_, request)| request.product_id == Some(Uuid::from_u128(3)))
    );
    assert_eq!(owner.singles.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn missing_reordered_foreign_or_unpriced_batch_responses_are_never_partially_adopted() {
    let (context, mut cart, _) = super::tests::cart_fixture();
    add_line(&mut cart, 10, 44, 3, 3);
    for reply in [
        Reply::MissingItem,
        Reply::Duplicate,
        Reply::Reordered,
        Reply::WrongVariant,
        Reply::WrongParent,
        Reply::WrongCurrency,
        Reply::WrongRegion,
        Reply::WrongQuantity,
        Reply::Preview,
        Reply::MissingPrice,
        Reply::WrongChannel,
    ] {
        let owner = PricingProbe::new(reply);
        assert!(
            prepare_storefront_cart_context(&owner, context.clone(), &cart, target_input(&cart))
                .await
                .is_err()
        );
        assert_eq!(owner.singles.load(Ordering::SeqCst), 0);
        assert_eq!(cart.locale_code.as_deref(), Some("ru"));
    }
}

#[tokio::test]
async fn selected_batch_capability_refusal_keeps_exact_retry_bit_without_single_or_local_fallback()
{
    let (context, cart, _) = super::tests::cart_fixture();
    let owner = PricingProbe::new(Reply::Unavailable);
    let error = prepare_storefront_cart_context(&owner, context, &cart, target_input(&cart))
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Unavailable);
    assert_eq!(error.code, "pricing.batch_capability_unavailable");
    assert!(!error.retryable);
    assert_eq!(owner.singles.load(Ordering::SeqCst), 0);
    assert_eq!(cart.line_items[0].unit_price, Decimal::from(999));
}

#[tokio::test]
async fn locked_missing_sku_duplicate_or_oversized_cart_refuses_before_pricing_io() {
    let (context, cart, _) = super::tests::cart_fixture();
    let owner = PricingProbe::new(Reply::Exact);
    for status in ["checking_out", "completed", "abandoned"] {
        let mut invalid = cart.clone();
        invalid.status = status.into();
        assert!(
            prepare_storefront_cart_context(&owner, context.clone(), &invalid, target_input(&cart))
                .await
                .is_err()
        );
    }
    let mut invalid = cart.clone();
    invalid.line_items[0].variant_id = None;
    assert!(
        prepare_storefront_cart_context(&owner, context.clone(), &invalid, target_input(&cart))
            .await
            .is_err()
    );
    let mut invalid = cart.clone();
    invalid.line_items.push(invalid.line_items[0].clone());
    assert!(
        prepare_storefront_cart_context(&owner, context.clone(), &invalid, target_input(&cart))
            .await
            .is_err()
    );
    let mut invalid = cart.clone();
    invalid
        .line_items
        .resize(1025, invalid.line_items[0].clone());
    assert!(
        prepare_storefront_cart_context(&owner, context, &invalid, target_input(&cart))
            .await
            .is_err()
    );
    assert!(owner.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn all_quantity_groups_share_one_inherited_deadline_not_a_new_budget_per_batch() {
    let (context, mut cart, _) = super::tests::cart_fixture();
    add_line(&mut cart, 10, 4, 3, 5);
    add_line(&mut cart, 11, 4, 3, 7);
    let mut owner = PricingProbe::new(Reply::Exact);
    owner.delay = Duration::from_millis(40);
    let context = context.with_deadline(Duration::from_millis(60));
    let error = prepare_storefront_cart_context(&owner, context, &cart, target_input(&cart))
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Timeout);
    assert_eq!(error.code, "commerce.cart_context_pricing_timeout");
    assert!(owner.requests.lock().unwrap().len() <= 2);
    assert_eq!(owner.singles.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn id_only_cart_uses_owner_channel_completion_and_empty_cart_needs_no_money_reads() {
    let (context, mut cart, _) = super::tests::cart_fixture();
    cart.channel_slug = None;
    let owner = PricingProbe::new(Reply::Exact);
    prepare_storefront_cart_context(&owner, context.clone(), &cart, target_input(&cart))
        .await
        .unwrap();
    assert_eq!(owner.requests.lock().unwrap()[0].0.channel, None);
    assert_eq!(
        owner.requests.lock().unwrap()[0].1.pricing.channel_slug,
        None
    );
    cart.line_items.clear();
    let empty_owner = PricingProbe::new(Reply::Unavailable);
    let command =
        prepare_storefront_cart_context(&empty_owner, context, &cart, target_input(&cart))
            .await
            .unwrap();
    let CartContextPricing::Reprice(plan) = command.pricing else {
        panic!("missing empty context plan")
    };
    assert!(plan.updates.is_empty());
    assert!(empty_owner.requests.lock().unwrap().is_empty());
}

#[test]
fn wildcard_shipping_selection_cannot_overlap_a_resolved_seller_group() {
    let (_, mut cart, _) = super::tests::cart_fixture();
    cart.line_items[0].fulfillment_requirement =
        rustok_cart::CartLineFulfillmentRequirement::Physical;
    cart.line_items[0].shipping_profile_slug = Some("default".into());
    cart.line_items[0].seller_id = Some("seller-a".into());
    add_line(&mut cart, 10, 44, 3, 3);
    cart.line_items[1].seller_id = Some("seller-b".into());
    cart.delivery_groups = cart
        .line_items
        .iter()
        .map(|line| rustok_cart::CartDeliveryGroupResponse {
            shipping_profile_slug: "default".into(),
            seller_id: line.seller_id.clone(),
            seller_scope: None,
            line_item_ids: vec![line.id],
            selected_shipping_option_id: None,
            available_shipping_options: Vec::new(),
        })
        .collect();
    let mut input = target_input(&cart);
    input.shipping_selections = Some(vec![
        rustok_cart::CartShippingSelectionInput {
            shipping_profile_slug: "default".into(),
            seller_id: None,
            seller_scope: None,
            selected_shipping_option_id: None,
        },
        rustok_cart::CartShippingSelectionInput {
            shipping_profile_slug: "DEFAULT".into(),
            seller_id: Some(" seller-b ".into()),
            seller_scope: None,
            selected_shipping_option_id: None,
        },
    ]);
    assert!(cart.project_context(&input).is_err());
    let mut duplicate = input;
    duplicate.shipping_selections.as_mut().unwrap()[0].seller_id = Some("seller-b".into());
    assert!(cart.project_context(&duplicate).is_err());
}
