use super::*;

#[tokio::test]
async fn checkout_without_fulfillment_flag_skips_fulfillment_creation() {
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
                metadata: serde_json::json!({ "source": "checkout-without-fulfillment-test" }),
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
                metadata: serde_json::json!({ "source": "checkout-without-fulfillment-test" }),
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
                metadata: serde_json::json!({ "source": "checkout-without-fulfillment-test" }),
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
                sku: Some("CHK-NO-FULFILL-1".to_string()),
                title: "Checkout Without Fulfillment Product".to_string(),
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
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: false,
                metadata: serde_json::json!({ "flow": "checkout-without-fulfillment-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(completed.cart.status, "completed");
    assert_eq!(completed.order.status, "paid");
    assert_eq!(completed.payment_collection.status, "captured");
    assert!(completed.fulfillment.is_none());
}

#[tokio::test]
async fn mixed_cart_creates_delivery_groups_and_uses_typed_shipping_selections() {
    let (db, cart_service, _, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
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
                tax_rate: Decimal::from_str("0.00").expect("valid decimal"),
                tax_included: false,
                country_tax_policies: None,
                countries: vec!["us".to_string()],
                metadata: serde_json::json!({ "source": "delivery-groups-test" }),
            },
        )
        .await
        .unwrap();
    let cold_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Cold Chain".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("12.50").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: Some(vec!["cold".to_string()]),
                metadata: serde_json::json!({ "source": "delivery-groups-test" }),
            },
        )
        .await
        .unwrap();
    let bulky_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Bulky Freight".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("34.00").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: Some(vec!["bulky".to_string()]),
                metadata: serde_json::json!({ "source": "delivery-groups-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("split@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("us".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: None,
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "delivery-groups-test" }),
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
                shipping_profile_slug: Some("cold".to_string()),
                sku: Some("COLD-1".to_string()),
                title: "Cold Shipment".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("15.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
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
                shipping_profile_slug: Some("bulky".to_string()),
                sku: Some("BULKY-1".to_string()),
                title: "Bulky Shipment".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("40.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 2 }),
            },
        )
        .await
        .unwrap();

    assert_eq!(cart.delivery_groups.len(), 2);
    assert_eq!(cart.selected_shipping_option_id, None);
    let delivery_group_slugs = cart
        .delivery_groups
        .iter()
        .map(|group| group.shipping_profile_slug.as_str())
        .collect::<Vec<_>>();
    assert_eq!(delivery_group_slugs, vec!["bulky", "cold"]);

    let cart = cart_service
        .update_context(
            tenant_id,
            cart.id,
            UpdateCartContextInput {
                email: cart.email.clone(),
                region_id: cart.region_id,
                country_code: cart.country_code.clone(),
                locale_code: cart.locale_code.clone(),
                selected_shipping_option_id: None,
                shipping_selections: Some(vec![
                    CartShippingSelectionInput {
                        shipping_profile_slug: "cold".to_string(),
                        seller_id: None,
                        seller_scope: None,
                        selected_shipping_option_id: Some(cold_option.id),
                    },
                    CartShippingSelectionInput {
                        shipping_profile_slug: "bulky".to_string(),
                        seller_id: None,
                        seller_scope: None,
                        selected_shipping_option_id: Some(bulky_option.id),
                    },
                ]),
            },
        )
        .await
        .unwrap();

    assert_eq!(cart.selected_shipping_option_id, None);
    assert_eq!(cart.delivery_groups.len(), 2);
    let delivery_groups = cart
        .delivery_groups
        .iter()
        .map(|group| {
            (
                group.shipping_profile_slug.clone(),
                group.selected_shipping_option_id,
            )
        })
        .collect::<Vec<_>>();
    assert!(delivery_groups.contains(&(String::from("cold"), Some(cold_option.id))));
    assert!(delivery_groups.contains(&(String::from("bulky"), Some(bulky_option.id))));
}

#[tokio::test]
async fn complete_checkout_rejects_missing_shipping_selection_for_delivery_group() {
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
                tax_rate: Decimal::from_str("0.00").expect("valid decimal"),
                tax_included: false,
                country_tax_policies: None,
                countries: vec!["us".to_string()],
                metadata: serde_json::json!({ "source": "missing-selection-test" }),
            },
        )
        .await
        .unwrap();
    let cold_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Cold Chain".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("12.50").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: Some(vec!["cold".to_string()]),
                metadata: serde_json::json!({ "source": "missing-selection-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("split@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("us".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: None,
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "missing-selection-test" }),
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
                shipping_profile_slug: Some("cold".to_string()),
                sku: Some("COLD-1".to_string()),
                title: "Cold Shipment".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("15.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
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
                shipping_profile_slug: Some("bulky".to_string()),
                sku: Some("BULKY-1".to_string()),
                title: "Bulky Shipment".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("40.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 2 }),
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
                shipping_selections: Some(vec![CartShippingSelectionInput {
                    shipping_profile_slug: "cold".to_string(),
                    seller_id: None,
                    seller_scope: None,
                    selected_shipping_option_id: Some(cold_option.id),
                }]),
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "missing-selection-test" }),
            },
        )
        .await
        .expect_err("checkout must reject a delivery group without shipping selection");

    match error {
        CheckoutError::Validation(message) => {
            assert!(
                message.contains("Delivery group bulky does not have a selected shipping option"),
                "unexpected validation message: {message}"
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn complete_checkout_creates_multiple_fulfillments_for_delivery_groups() {
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
                tax_rate: Decimal::from_str("0.00").expect("valid decimal"),
                tax_included: false,
                country_tax_policies: None,
                countries: vec!["us".to_string()],
                metadata: serde_json::json!({ "source": "multi-fulfillment-test" }),
            },
        )
        .await
        .unwrap();
    let cold_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Cold Chain".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("12.50").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: Some(vec!["cold".to_string()]),
                metadata: serde_json::json!({ "source": "multi-fulfillment-test" }),
            },
        )
        .await
        .unwrap();
    let bulky_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Bulky Freight".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("34.00").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: Some(vec!["bulky".to_string()]),
                metadata: serde_json::json!({ "source": "multi-fulfillment-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("split@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("us".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: None,
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "multi-fulfillment-test" }),
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
                shipping_profile_slug: Some("cold".to_string()),
                sku: Some("COLD-1".to_string()),
                title: "Cold Shipment".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("15.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
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
                shipping_profile_slug: Some("bulky".to_string()),
                sku: Some("BULKY-1".to_string()),
                title: "Bulky Shipment".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("40.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 2 }),
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
                shipping_selections: Some(vec![
                    CartShippingSelectionInput {
                        shipping_profile_slug: "cold".to_string(),
                        seller_id: None,
                        seller_scope: None,
                        selected_shipping_option_id: Some(cold_option.id),
                    },
                    CartShippingSelectionInput {
                        shipping_profile_slug: "bulky".to_string(),
                        seller_id: None,
                        seller_scope: None,
                        selected_shipping_option_id: Some(bulky_option.id),
                    },
                ]),
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "multi-fulfillment-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(completed.cart.status, "completed");
    assert_eq!(completed.order.status, "paid");
    assert_eq!(completed.fulfillments.len(), 2);
    assert!(completed.fulfillment.is_none());
    assert_eq!(completed.cart.selected_shipping_option_id, None);
    assert_eq!(completed.cart.delivery_groups.len(), 2);

    let delivery_group_options = completed
        .cart
        .delivery_groups
        .iter()
        .map(|group| {
            (
                group.shipping_profile_slug.clone(),
                group.selected_shipping_option_id,
            )
        })
        .collect::<Vec<_>>();
    assert!(delivery_group_options.contains(&(String::from("cold"), Some(cold_option.id))));
    assert!(delivery_group_options.contains(&(String::from("bulky"), Some(bulky_option.id))));

    let fulfillment_profiles = completed
        .fulfillments
        .iter()
        .map(|item| {
            (
                item.metadata["delivery_group"]["shipping_profile_slug"]
                    .as_str()
                    .expect("delivery group profile slug should be present")
                    .to_string(),
                item.shipping_option_id,
            )
        })
        .collect::<Vec<_>>();
    assert!(fulfillment_profiles.contains(&(String::from("cold"), Some(cold_option.id))));
    assert!(fulfillment_profiles.contains(&(String::from("bulky"), Some(bulky_option.id))));
}

#[tokio::test]
async fn complete_checkout_keeps_seller_aware_delivery_groups_for_same_shipping_profile() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let seller_a_id = "seller-a-id";
    let seller_b_id = "seller-b-id";
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
                tax_rate: Decimal::from_str("0.00").expect("valid decimal"),
                tax_included: false,
                country_tax_policies: None,
                countries: vec!["us".to_string()],
                metadata: serde_json::json!({ "source": "seller-aware-fulfillment-test" }),
            },
        )
        .await
        .unwrap();
    let seller_a_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Seller A Standard".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("10.00").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: Some(vec!["default".to_string()]),
                metadata: serde_json::json!({ "source": "seller-aware-fulfillment-test" }),
            },
        )
        .await
        .unwrap();
    let seller_b_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Seller B Standard".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("12.00").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: Some(vec!["default".to_string()]),
                metadata: serde_json::json!({ "source": "seller-aware-fulfillment-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("seller-aware@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("us".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: None,
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "seller-aware-fulfillment-test" }),
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
                sku: Some("SELLER-A-1".to_string()),
                title: "Seller A Shipment".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("15.00").expect("valid decimal"),
                metadata: serde_json::json!({
                    "seller": {
                        "id": seller_a_id,
                        "scope": "seller-a",
                        "label": "Seller A"
                    }
                }),
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
                sku: Some("SELLER-B-1".to_string()),
                title: "Seller B Shipment".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("18.00").expect("valid decimal"),
                metadata: serde_json::json!({
                    "seller": {
                        "id": seller_b_id,
                        "scope": "seller-b",
                        "label": "Seller B"
                    }
                }),
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
                shipping_selections: Some(vec![
                    CartShippingSelectionInput {
                        shipping_profile_slug: "default".to_string(),
                        seller_id: Some(seller_a_id.to_string()),
                        seller_scope: None,
                        selected_shipping_option_id: Some(seller_a_option.id),
                    },
                    CartShippingSelectionInput {
                        shipping_profile_slug: "default".to_string(),
                        seller_id: Some(seller_b_id.to_string()),
                        seller_scope: None,
                        selected_shipping_option_id: Some(seller_b_option.id),
                    },
                ]),
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "seller-aware-fulfillment-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(completed.cart.delivery_groups.len(), 2);
    assert_eq!(completed.fulfillments.len(), 2);
    assert!(completed.fulfillment.is_none());

    let delivery_groups = completed
        .cart
        .delivery_groups
        .iter()
        .map(|group| {
            (
                group.shipping_profile_slug.clone(),
                group.seller_id.clone(),
                group.selected_shipping_option_id,
            )
        })
        .collect::<Vec<_>>();
    assert!(delivery_groups.contains(&(
        String::from("default"),
        Some(seller_a_id.to_string()),
        Some(seller_a_option.id),
    )));
    assert!(delivery_groups.contains(&(
        String::from("default"),
        Some(seller_b_id.to_string()),
        Some(seller_b_option.id),
    )));
    let fulfillment_groups = completed
        .fulfillments
        .iter()
        .map(|item| {
            (
                item.metadata["delivery_group"]["shipping_profile_slug"]
                    .as_str()
                    .expect("delivery group profile slug should be present")
                    .to_string(),
                item.metadata["delivery_group"]["seller_id"]
                    .as_str()
                    .expect("delivery group seller id should be present")
                    .to_string(),
                item.shipping_option_id,
                item.items.len(),
            )
        })
        .collect::<Vec<_>>();
    assert!(fulfillment_groups.contains(&(
        String::from("default"),
        seller_a_id.to_string(),
        Some(seller_a_option.id),
        1,
    )));
    assert!(fulfillment_groups.contains(&(
        String::from("default"),
        seller_b_id.to_string(),
        Some(seller_b_option.id),
        1,
    )));
    assert!(completed.fulfillments.iter().all(|item| {
        item.metadata
            .get("delivery_group")
            .and_then(|delivery_group| delivery_group.get("seller_scope"))
            .is_none()
    }));
    assert!(completed.fulfillments.iter().all(|item| {
        item.metadata
            .get("delivery_group")
            .and_then(|delivery_group| delivery_group.get("seller_label"))
            .is_none()
    }));

    let fulfillment_item_order_line_ids = completed
        .fulfillments
        .iter()
        .flat_map(|fulfillment| fulfillment.items.iter().map(|item| item.order_line_item_id))
        .collect::<Vec<_>>();
    assert_eq!(fulfillment_item_order_line_ids.len(), 2);
    assert_eq!(
        fulfillment_item_order_line_ids
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        2
    );
}

#[tokio::test]
async fn complete_checkout_rejects_stale_shipping_profile_snapshot_after_variant_binding_change() {
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
                metadata: serde_json::json!({ "source": "stale-shipping-profile-test" }),
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
                    name: "Cold Chain".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: Some(vec!["cold".to_string()]),
                metadata: serde_json::json!({ "source": "stale-shipping-profile-test" }),
            },
        )
        .await
        .unwrap();

    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let mut product_input = create_product_input();
    product_input.publish = true;
    product_input.shipping_profile_slug = Some("cold".to_string());
    let product = catalog
        .create_product(tenant_id, actor_id, product_input)
        .await
        .unwrap();
    let variant = product
        .variants
        .first()
        .expect("product must have a variant")
        .clone();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("stale@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "stale-shipping-profile-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(product.id),
                variant_id: Some(variant.id),
                shipping_profile_slug: Some("cold".to_string()),
                sku: variant.sku.clone(),
                title: variant.title,
                quantity: 1,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "UPDATE product_variants SET shipping_profile_slug = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        vec!["frozen".into(), variant.id.into()],
    ))
    .await
    .expect("variant shipping profile should be updated");

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
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "stale-shipping-profile-test" }),
            },
        )
        .await
        .expect_err("checkout must reject stale shipping profile snapshots");

    match error {
        CheckoutError::Validation(message) => {
            assert!(
                message.contains("stale shipping profile snapshot cold (current: frozen)"),
                "unexpected validation message: {message}"
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}
