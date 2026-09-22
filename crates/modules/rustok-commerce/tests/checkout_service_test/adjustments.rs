use super::*;

#[tokio::test]
async fn complete_checkout_snapshots_cart_adjustments_into_order_and_payment_total() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "United States".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::ZERO,
                tax_included: false,
                country_tax_policies: None,
                countries: vec!["us".to_string()],
                metadata: serde_json::json!({ "source": "checkout-adjustment-test" }),
            },
        )
        .await
        .unwrap();

    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Standard".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({ "source": "checkout-adjustment-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("adjusted@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("us".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-adjustment-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: None,
                variant_id: None,
                shipping_profile_slug: None,
                sku: Some("ADJ-1".to_string()),
                title: "Adjusted Checkout Product".to_string(),
                quantity: 2,
                unit_price: Decimal::from_str("30.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();
    let line_item_id = cart.line_items[0].id;
    let cart = cart_service
        .set_adjustments(
            tenant_id,
            cart.id,
            vec![SetCartAdjustmentInput {
                line_item_id: Some(line_item_id),
                source_type: "Promotion".to_string(),
                source_id: Some("promo-checkout".to_string()),
                amount: Decimal::from_str("10.00").expect("valid decimal"),
                metadata: serde_json::json!({
                    "rule_code": "checkout",
                    "localized_label": "Checkout promo"
                }),
            }],
        )
        .await
        .unwrap();

    assert_eq!(cart.subtotal_amount, Decimal::from_str("60.00").unwrap());
    assert_eq!(cart.adjustment_total, Decimal::from_str("10.00").unwrap());
    assert_eq!(cart.shipping_total, Decimal::from_str("9.99").unwrap());
    assert_eq!(cart.total_amount, Decimal::from_str("59.99").unwrap());

    let completed = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-adjustment-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        completed.cart.subtotal_amount,
        Decimal::from_str("60.00").unwrap()
    );
    assert_eq!(
        completed.cart.adjustment_total,
        Decimal::from_str("10.00").unwrap()
    );
    assert_eq!(
        completed.cart.shipping_total,
        Decimal::from_str("9.99").unwrap()
    );
    assert_eq!(
        completed.cart.total_amount,
        Decimal::from_str("59.99").unwrap()
    );
    assert_eq!(
        completed.order.subtotal_amount,
        Decimal::from_str("60.00").unwrap()
    );
    assert_eq!(
        completed.order.adjustment_total,
        Decimal::from_str("10.00").unwrap()
    );
    assert_eq!(
        completed.order.shipping_total,
        Decimal::from_str("9.99").unwrap()
    );
    assert_eq!(
        completed.order.total_amount,
        Decimal::from_str("59.99").unwrap()
    );
    assert_eq!(completed.order.adjustments.len(), 1);
    assert_eq!(completed.order.adjustments[0].source_type, "promotion");
    assert_eq!(
        completed.order.adjustments[0].source_id.as_deref(),
        Some("promo-checkout")
    );
    assert_eq!(
        completed.order.adjustments[0].line_item_id,
        Some(completed.order.line_items[0].id)
    );
    assert!(
        completed.order.adjustments[0]
            .metadata
            .get("localized_label")
            .is_none()
    );
    assert_eq!(
        completed.payment_collection.amount,
        Decimal::from_str("59.99").unwrap()
    );
    assert_eq!(
        completed.payment_collection.captured_amount,
        Decimal::from_str("59.99").unwrap()
    );
}

#[tokio::test]
async fn complete_checkout_snapshots_typed_percentage_promotion_into_order() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "United States".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::ZERO,
                tax_included: false,
                country_tax_policies: None,
                countries: vec!["us".to_string()],
                metadata: serde_json::json!({ "source": "checkout-typed-promotion-test" }),
            },
        )
        .await
        .unwrap();

    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Standard".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({ "source": "checkout-typed-promotion-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("typed-promo@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("us".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-typed-promotion-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: None,
                variant_id: None,
                shipping_profile_slug: None,
                sku: Some("PROMO-1".to_string()),
                title: "Promotion Checkout Product".to_string(),
                quantity: 2,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .apply_percentage_promotion(
            tenant_id,
            cart.id,
            None,
            "promo-typed-cart-10",
            Decimal::from_str("10").unwrap(),
            serde_json::json!({
                "display_label": "Ten percent off"
            }),
        )
        .await
        .unwrap();

    assert_eq!(cart.subtotal_amount, Decimal::from_str("50.00").unwrap());
    assert_eq!(cart.adjustment_total, Decimal::from_str("5.00").unwrap());
    assert_eq!(cart.shipping_total, Decimal::from_str("9.99").unwrap());
    assert_eq!(cart.total_amount, Decimal::from_str("54.99").unwrap());

    let completed = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-typed-promotion-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(completed.order.adjustments.len(), 1);
    assert_eq!(completed.order.adjustments[0].source_type, "promotion");
    assert_eq!(
        completed.order.adjustments[0].source_id.as_deref(),
        Some("promo-typed-cart-10")
    );
    assert_eq!(completed.order.adjustments[0].line_item_id, None);
    assert_eq!(
        completed.order.adjustments[0].metadata["kind"],
        serde_json::json!("percentage_discount")
    );
    assert_eq!(
        completed.order.adjustments[0].metadata["scope"],
        serde_json::json!("cart")
    );
    assert!(
        completed.order.adjustments[0]
            .metadata
            .get("display_label")
            .is_none()
    );
    assert_eq!(
        completed.payment_collection.amount,
        Decimal::from_str("54.99").unwrap()
    );
}

#[tokio::test]
async fn complete_checkout_snapshots_pricing_reprice_adjustments_into_order() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "United States".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::ZERO,
                tax_included: false,
                country_tax_policies: None,
                countries: vec!["us".to_string()],
                metadata: serde_json::json!({ "source": "checkout-pricing-adjustment-test" }),
            },
        )
        .await
        .unwrap();

    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Standard".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({ "source": "checkout-pricing-adjustment-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("pricing@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("us".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-pricing-adjustment-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: None,
                variant_id: None,
                shipping_profile_slug: None,
                sku: Some("PRICE-1".to_string()),
                title: "Priced Checkout Product".to_string(),
                quantity: 2,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();
    let line_item_id = cart.line_items[0].id;
    let cart = cart_service
        .reprice_line_items(
            tenant_id,
            cart.id,
            vec![rustok_cart::services::cart::CartLineItemPricingUpdate {
                line_item_id,
                unit_price: Decimal::from_str("30.00").expect("valid decimal"),
                pricing_adjustment: Some(
                    rustok_cart::services::cart::CartPricingAdjustmentUpdate {
                        source_id: Some("price-list-checkout".to_string()),
                        amount: Decimal::from_str("10.00").expect("valid decimal"),
                        metadata: serde_json::json!({
                            "kind": "price_list",
                            "discount_percent": "16.67",
                            "display_label": "Pricing sale"
                        }),
                    },
                ),
            }],
        )
        .await
        .unwrap();

    assert_eq!(cart.subtotal_amount, Decimal::from_str("60.00").unwrap());
    assert_eq!(cart.adjustment_total, Decimal::from_str("10.00").unwrap());
    assert_eq!(cart.shipping_total, Decimal::from_str("9.99").unwrap());
    assert_eq!(cart.total_amount, Decimal::from_str("59.99").unwrap());
    assert_eq!(cart.adjustments[0].source_type, "pricing");

    let completed = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-pricing-adjustment-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        completed.order.subtotal_amount,
        Decimal::from_str("60.00").unwrap()
    );
    assert_eq!(
        completed.order.adjustment_total,
        Decimal::from_str("10.00").unwrap()
    );
    assert_eq!(
        completed.order.shipping_total,
        Decimal::from_str("9.99").unwrap()
    );
    assert_eq!(
        completed.order.total_amount,
        Decimal::from_str("59.99").unwrap()
    );
    assert_eq!(completed.order.adjustments.len(), 1);
    assert_eq!(completed.order.adjustments[0].source_type, "pricing");
    assert_eq!(
        completed.order.adjustments[0].source_id.as_deref(),
        Some("price-list-checkout")
    );
    assert!(
        completed.order.adjustments[0]
            .metadata
            .get("display_label")
            .is_none()
    );
    assert_eq!(
        completed.payment_collection.amount,
        Decimal::from_str("59.99").unwrap()
    );
}

#[tokio::test]
async fn complete_checkout_snapshots_shipping_promotion_into_order_and_payment_total() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "United States".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::ZERO,
                tax_included: false,
                country_tax_policies: None,
                countries: vec!["us".to_string()],
                metadata: serde_json::json!({ "source": "checkout-shipping-promotion-test" }),
            },
        )
        .await
        .unwrap();

    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Standard".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({ "source": "checkout-shipping-promotion-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("shipping-promo@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("us".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-shipping-promotion-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: None,
                variant_id: None,
                shipping_profile_slug: None,
                sku: Some("SHIP-PROMO-1".to_string()),
                title: "Shipping Promo Product".to_string(),
                quantity: 2,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .apply_fixed_shipping_promotion(
            tenant_id,
            cart.id,
            "promo-shipping-fixed",
            Decimal::from_str("4.99").unwrap(),
            serde_json::json!({
                "display_label": "Half off shipping"
            }),
        )
        .await
        .unwrap();

    assert_eq!(cart.subtotal_amount, Decimal::from_str("50.00").unwrap());
    assert_eq!(cart.shipping_total, Decimal::from_str("9.99").unwrap());
    assert_eq!(cart.adjustment_total, Decimal::from_str("4.99").unwrap());
    assert_eq!(cart.total_amount, Decimal::from_str("55.00").unwrap());

    let completed = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-shipping-promotion-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        completed.order.shipping_total,
        Decimal::from_str("9.99").unwrap()
    );
    assert_eq!(completed.order.adjustments.len(), 1);
    assert_eq!(completed.order.adjustments[0].source_type, "promotion");
    assert_eq!(
        completed.order.adjustments[0].source_id.as_deref(),
        Some("promo-shipping-fixed")
    );
    assert_eq!(
        completed.order.adjustments[0].metadata["scope"],
        serde_json::json!("shipping")
    );
    assert!(
        completed.order.adjustments[0]
            .metadata
            .get("display_label")
            .is_none()
    );
    assert_eq!(
        completed.order.total_amount,
        Decimal::from_str("55.00").unwrap()
    );
    assert_eq!(
        completed.payment_collection.amount,
        Decimal::from_str("55.00").unwrap()
    );
}
