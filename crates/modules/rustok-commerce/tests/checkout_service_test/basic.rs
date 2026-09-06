use super::*;

#[tokio::test]
async fn complete_checkout_builds_order_payment_and_fulfillment_flow() {
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
                    name: "Europe".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                country_tax_policies: None,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-test" }),
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
                metadata: serde_json::json!({ "source": "checkout-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-test" }),
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
                sku: Some("CHK-1".to_string()),
                title: "Checkout Product".to_string(),
                quantity: 2,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

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
                metadata: serde_json::json!({ "flow": "checkout-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(completed.cart.status, "completed");
    assert_eq!(completed.order.status, "paid");
    assert_eq!(completed.payment_collection.status, "captured");
    assert!(completed.fulfillment.is_some());
    assert_eq!(completed.fulfillments.len(), 1);
    assert_eq!(completed.cart.delivery_groups.len(), 1);
    assert_eq!(completed.context.locale, "de");
    assert_eq!(completed.context.currency_code.as_deref(), Some("USD"));
    assert_eq!(
        completed.cart.shipping_total,
        Decimal::from_str("9.99").unwrap()
    );
    assert_eq!(
        completed.order.shipping_total,
        Decimal::from_str("9.99").unwrap()
    );
    assert_eq!(
        completed.payment_collection.amount,
        Decimal::from_str("59.99").unwrap()
    );
    assert_eq!(completed.cart.region_id, Some(region.id));
    assert_eq!(completed.cart.country_code.as_deref(), Some("DE"));
    assert_eq!(completed.cart.locale_code.as_deref(), Some("de"));
    assert_eq!(
        completed.cart.selected_shipping_option_id,
        Some(shipping_option.id)
    );
    assert_eq!(
        completed.context.region.as_ref().map(|region| region.id),
        Some(region.id)
    );
    assert_eq!(
        completed
            .fulfillment
            .as_ref()
            .and_then(|value| value.shipping_option_id),
        Some(shipping_option.id)
    );
    assert!(!completed.cart.tax_lines.is_empty());
    assert!(!completed.order.tax_lines.is_empty());
    assert!(
        completed
            .cart
            .tax_lines
            .iter()
            .all(|line| line.provider_id == "region_default")
    );
    assert!(
        completed
            .order
            .tax_lines
            .iter()
            .all(|line| line.provider_id == "region_default")
    );
    assert!(completed.order.tax_lines.iter().all(|line| {
        line.metadata
            .get("tax_included")
            .and_then(|value| value.as_bool())
            == Some(true)
    }));
    assert_eq!(
        completed.fulfillments[0].metadata["delivery_group"]["shipping_profile_slug"],
        serde_json::json!("default")
    );
}

#[tokio::test]
async fn complete_checkout_rejects_empty_cart() {
    let (db, cart_service, checkout, _) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("empty@example.com".to_string()),
                region_id: None,
                country_code: None,
                locale_code: None,
                selected_shipping_option_id: None,
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();

    let error = checkout
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
                create_fulfillment: false,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap_err();

    match error {
        CheckoutError::EmptyCart(cart_id) => assert_eq!(cart_id, cart.id),
        other => panic!("expected empty cart error, got {other:?}"),
    }
}

#[tokio::test]
async fn complete_checkout_reuses_existing_cart_payment_collection() {
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
                    name: "Europe".to_string(),
                }],
                currency_code: "eur".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                country_tax_policies: None,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-existing-collection-test" }),
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
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({ "source": "checkout-existing-collection-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("buyer@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "checkout-existing-collection-test" }),
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
                sku: Some("CHK-EXISTING-1".to_string()),
                title: "Checkout Product".to_string(),
                quantity: 2,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();
    let existing_collection = PaymentService::new(db.clone())
        .create_collection(
            tenant_id,
            rustok_payment::dto::CreatePaymentCollectionInput {
                cart_id: Some(cart.id),
                order_id: None,
                customer_id: cart.customer_id,
                currency_code: cart.currency_code.clone(),
                amount: cart.total_amount,
                metadata: serde_json::json!({ "source": "checkout-existing-collection-test" }),
            },
        )
        .await
        .unwrap();

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
                create_fulfillment: false,
                metadata: serde_json::json!({ "flow": "checkout-existing-collection-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(completed.payment_collection.id, existing_collection.id);
    assert_eq!(
        completed.payment_collection.order_id,
        Some(completed.order.id)
    );
    assert_eq!(completed.payment_collection.status, "captured");
    assert_eq!(completed.order.status, "paid");
    assert_eq!(completed.cart.status, "completed");
}

#[tokio::test]
async fn complete_checkout_prefers_persisted_cart_context_over_conflicting_overrides() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region_de = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "Germany".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                country_tax_policies: None,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-context-priority-test" }),
            },
        )
        .await
        .unwrap();
    let region_fr = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "France".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                country_tax_policies: None,
                countries: vec!["fr".to_string()],
                metadata: serde_json::json!({ "source": "checkout-context-priority-test" }),
            },
        )
        .await
        .unwrap();

    let shipping_option_de = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "German Standard".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({ "source": "checkout-context-priority-test" }),
            },
        )
        .await
        .unwrap();
    let shipping_option_fr = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "French Standard".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("12.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({ "source": "checkout-context-priority-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: Some(region_de.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option_de.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-context-priority-test" }),
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
                sku: Some("CHK-CONTEXT-1".to_string()),
                title: "Checkout Context Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let completed = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: Some(shipping_option_fr.id),
                shipping_selections: None,
                region_id: Some(region_fr.id),
                country_code: Some("fr".to_string()),
                locale: Some("fr".to_string()),
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-context-priority-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(completed.cart.region_id, Some(region_de.id));
    assert_eq!(completed.cart.country_code.as_deref(), Some("DE"));
    assert_eq!(completed.cart.locale_code.as_deref(), Some("de"));
    assert_eq!(
        completed.cart.selected_shipping_option_id,
        Some(shipping_option_fr.id)
    );
    assert_eq!(
        completed.context.region.as_ref().map(|region| region.id),
        Some(region_de.id)
    );
    assert_eq!(completed.context.locale, "de");
    assert_eq!(
        completed
            .fulfillment
            .as_ref()
            .and_then(|value| value.shipping_option_id),
        Some(shipping_option_fr.id)
    );
}
