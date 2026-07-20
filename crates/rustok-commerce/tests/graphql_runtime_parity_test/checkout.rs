use super::*;

#[tokio::test]
async fn storefront_graphql_read_path_is_stable_after_complete_checkout() {
    let (db, catalog, cart_service, checkout, fulfillment) = setup_checkout().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .unwrap();
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .unwrap();
    let published_variant = published
        .variants
        .first()
        .expect("published product must have variant");
    let handle = published
        .translations
        .iter()
        .find(|translation| translation.locale == "de")
        .map(|translation| translation.handle.clone())
        .expect("published product must keep de handle");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        None,
    );

    let before = schema
        .execute(Request::new(storefront_query(&handle)))
        .await;
    assert!(
        before.errors.is_empty(),
        "unexpected GraphQL errors before checkout: {:?}",
        before.errors
    );
    let before_json = before
        .data
        .into_json()
        .expect("GraphQL response must serialize");

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
                metadata: serde_json::json!({ "source": "graphql-checkout-parity" }),
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
                metadata: serde_json::json!({ "source": "graphql-checkout-parity" }),
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
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "graphql-checkout-parity" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(published.id),
                variant_id: Some(published_variant.id),
                shipping_profile_slug: None,
                sku: published_variant.sku.clone(),
                title: "Parity Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                metadata: serde_json::json!({ "source": "graphql-checkout-parity" }),
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
                metadata: serde_json::json!({ "source": "graphql-checkout-parity" }),
            },
        )
        .await
        .unwrap();
    assert_eq!(completed.cart.status, "completed");
    assert_eq!(completed.order.status, "paid");

    let after = schema
        .execute(Request::new(storefront_query(&handle)))
        .await;
    assert!(
        after.errors.is_empty(),
        "unexpected GraphQL errors after checkout: {:?}",
        after.errors
    );
    let after_json = after
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(before_json, after_json);
    assert_eq!(after_json["storefrontProducts"]["total"], Value::from(1));
    assert_eq!(
        after_json["storefrontProducts"]["items"][0]["title"],
        Value::from("Paritaet Produkt")
    );
}

#[tokio::test]
async fn admin_graphql_catalog_query_is_stable_after_complete_checkout() {
    let (db, catalog, cart_service, checkout, fulfillment) = setup_checkout().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .unwrap();
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .unwrap();
    let published_variant = published
        .variants
        .first()
        .expect("published product must have variant");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(auth_context(tenant_id)),
    );
    let query = admin_query(tenant_id, created.id);

    let before = schema.execute(Request::new(query.clone())).await;
    assert!(
        before.errors.is_empty(),
        "unexpected admin GraphQL errors before checkout: {:?}",
        before.errors
    );
    let before_json = before
        .data
        .into_json()
        .expect("GraphQL response must serialize");

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
                metadata: serde_json::json!({ "source": "admin-graphql-checkout-parity" }),
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
                metadata: serde_json::json!({ "source": "admin-graphql-checkout-parity" }),
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
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "admin-graphql-checkout-parity" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(published.id),
                variant_id: Some(published_variant.id),
                shipping_profile_slug: None,
                sku: published_variant.sku.clone(),
                title: "Parity Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                metadata: serde_json::json!({ "source": "admin-graphql-checkout-parity" }),
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
                metadata: serde_json::json!({ "source": "admin-graphql-checkout-parity" }),
            },
        )
        .await
        .unwrap();
    assert_eq!(completed.cart.status, "completed");
    assert_eq!(completed.order.status, "paid");

    let after = schema.execute(Request::new(query)).await;
    assert!(
        after.errors.is_empty(),
        "unexpected admin GraphQL errors after checkout: {:?}",
        after.errors
    );
    let after_json = after
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(before_json, after_json);
    assert_eq!(after_json["products"]["total"], Value::from(1));
    assert_eq!(
        after_json["product"]["translations"][0]["title"],
        Value::from("Parity Product")
    );
}

#[tokio::test]
async fn admin_graphql_order_payment_and_fulfillment_surface_matches_runtime_services() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let order_service = OrderService::new(db.clone(), mock_transactional_event_bus());
    let payment_service = PaymentService::new(db.clone());
    let fulfillment_service = FulfillmentService::new(db.clone());

    let created_order = order_service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("GRAPHQL-ADMIN-ORDER-1".to_string()),
                    title: "GraphQL Admin Order".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-admin-order-parity" }),
                }],
                adjustments: Vec::new(),
                tax_lines: vec![rustok_order::dto::CreateOrderTaxLineInput {
                    line_item_index: Some(0),
                    shipping_option_id: None,
                    rate: Decimal::from_str("20.00").expect("valid decimal"),
                    amount: Decimal::from_str("5.00").expect("valid decimal"),
                    currency_code: "eur".to_string(),
                    description: Some("VAT".to_string()),
                    provider_id: "region_default".to_string(),
                    metadata: serde_json::json!({ "tax_included": false }),
                }],
                metadata: serde_json::json!({ "source": "graphql-admin-order-parity" }),
            },
        )
        .await
        .expect("order should be created");
    let confirmed_order = order_service
        .confirm_order(tenant_id, actor_id, created_order.id)
        .await
        .expect("order should be confirmed");
    let payment_collection = payment_service
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(confirmed_order.id),
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "source": "graphql-admin-order-parity" }),
            },
        )
        .await
        .expect("payment collection should be created");
    let fulfillment = fulfillment_service
        .create_fulfillment(
            tenant_id,
            CreateFulfillmentInput {
                order_id: confirmed_order.id,
                shipping_option_id: None,
                customer_id: Some(customer_id),
                carrier: None,
                tracking_number: None,
                items: None,
                metadata: serde_json::json!({ "source": "graphql-admin-order-parity" }),
            },
        )
        .await
        .expect("fulfillment should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_order_auth_context(tenant_id)),
    );

    let mutation = schema
        .execute(Request::new(admin_order_mutation(
            tenant_id,
            actor_id,
            confirmed_order.id,
            payment_collection.id,
            fulfillment.id,
        )))
        .await;
    assert!(
        mutation.errors.is_empty(),
        "unexpected admin GraphQL mutation errors: {:?}",
        mutation.errors
    );
    let mutation_json = mutation
        .data
        .into_json()
        .expect("GraphQL mutation response must serialize");
    assert_eq!(
        mutation_json["authorizePaymentCollection"]["status"],
        Value::from("authorized")
    );
    assert_eq!(
        mutation_json["capturePaymentCollection"]["status"],
        Value::from("captured")
    );
    assert_eq!(
        mutation_json["markOrderPaid"]["status"],
        Value::from("paid")
    );
    assert_eq!(mutation_json["shipOrder"]["status"], Value::from("shipped"));
    assert_eq!(
        mutation_json["deliverOrder"]["status"],
        Value::from("delivered")
    );
    assert_eq!(
        mutation_json["deliverFulfillment"]["status"],
        Value::from("delivered")
    );

    let query = schema
        .execute(Request::new(admin_order_parity_query(
            tenant_id,
            confirmed_order.id,
            payment_collection.id,
            fulfillment.id,
        )))
        .await;
    assert!(
        query.errors.is_empty(),
        "unexpected admin GraphQL query errors: {:?}",
        query.errors
    );
    let query_json = query
        .data
        .into_json()
        .expect("GraphQL query response must serialize");

    assert_eq!(
        query_json["order"]["order"]["status"],
        Value::from("delivered")
    );
    assert_eq!(
        query_json["order"]["order"]["totalAmount"],
        Value::from("30")
    );
    assert_eq!(query_json["order"]["order"]["taxTotal"], Value::from("5"));
    assert_eq!(
        query_json["order"]["order"]["taxIncluded"],
        Value::from(false)
    );
    assert_eq!(
        query_json["order"]["order"]["taxLines"][0]["providerId"],
        Value::from("region_default")
    );
    assert_eq!(
        query_json["order"]["order"]["paymentId"],
        Value::from("graphql-pay-1")
    );
    assert_eq!(
        query_json["order"]["order"]["trackingNumber"],
        Value::from("TRACK-789")
    );
    assert_eq!(
        query_json["order"]["paymentCollection"]["status"],
        Value::from("captured")
    );
    assert_eq!(
        query_json["order"]["fulfillment"]["status"],
        Value::from("delivered")
    );
    assert_eq!(query_json["orders"]["total"], Value::from(1));
    assert_eq!(
        query_json["orders"]["items"][0]["id"],
        Value::from(confirmed_order.id.to_string())
    );
    assert_eq!(
        query_json["orders"]["items"][0]["totalAmount"],
        Value::from("30")
    );
    assert_eq!(
        query_json["orders"]["items"][0]["taxTotal"],
        Value::from("5")
    );
    assert_eq!(
        query_json["orders"]["items"][0]["taxIncluded"],
        Value::from(false)
    );
    assert_eq!(
        query_json["orders"]["items"][0]["taxLines"][0]["providerId"],
        Value::from("region_default")
    );
    assert_eq!(
        query_json["paymentCollection"]["payments"][0]["status"],
        Value::from("captured")
    );
    assert_eq!(
        query_json["fulfillment"]["deliveredNote"],
        Value::from("Left at reception")
    );
    assert_eq!(query_json["paymentCollections"]["total"], Value::from(1));
    assert_eq!(
        query_json["paymentCollections"]["items"][0]["id"],
        Value::from(payment_collection.id.to_string())
    );
    assert_eq!(query_json["fulfillments"]["total"], Value::from(1));
    assert_eq!(
        query_json["fulfillments"]["items"][0]["id"],
        Value::from(fulfillment.id.to_string())
    );
}

#[tokio::test]
async fn admin_graphql_refund_surface_matches_runtime_services() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let event_bus = mock_transactional_event_bus();
    let order_service = OrderService::new(db.clone(), event_bus.clone());
    let payment_service = PaymentService::new(db.clone());

    let created_order = order_service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("GRAPHQL-ADMIN-REFUND-1".to_string()),
                    title: "GraphQL Admin Refund".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-admin-refund-parity" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "graphql-admin-refund-parity" }),
            },
        )
        .await
        .expect("order should be created");
    let confirmed_order = order_service
        .confirm_order(tenant_id, actor_id, created_order.id)
        .await
        .expect("order should be confirmed");
    let payment_collection = payment_service
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(confirmed_order.id),
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "source": "graphql-admin-refund-parity" }),
            },
        )
        .await
        .expect("payment collection should be created");
    payment_service
        .authorize_collection(
            tenant_id,
            payment_collection.id,
            rustok_payment::dto::AuthorizePaymentInput {
                provider_id: Some("manual".to_string()),
                provider_payment_id: Some("graphql-refund-pay-1".to_string()),
                amount: None,
                metadata: serde_json::json!({ "step": "authorized" }),
            },
        )
        .await
        .expect("payment collection should be authorized");
    payment_service
        .capture_collection(
            tenant_id,
            payment_collection.id,
            rustok_payment::dto::CapturePaymentInput {
                amount: Some(Decimal::from_str("25.00").expect("valid decimal")),
                metadata: serde_json::json!({ "step": "captured" }),
            },
        )
        .await
        .expect("payment collection should be captured");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_order_auth_context(tenant_id)),
    );

    let create_first = schema
        .execute(Request::new(admin_create_refund_mutation(
            tenant_id,
            payment_collection.id,
            "10.00",
            "customer-request",
            "create-1",
        )))
        .await;
    assert!(
        create_first.errors.is_empty(),
        "unexpected create refund errors: {:?}",
        create_first.errors
    );
    let create_first_json = create_first
        .data
        .into_json()
        .expect("GraphQL create refund response must serialize");
    let first_refund_id = Uuid::parse_str(
        create_first_json["createRefund"]["id"]
            .as_str()
            .expect("refund id should be returned"),
    )
    .expect("refund id should parse");
    assert_eq!(
        create_first_json["createRefund"]["status"],
        Value::from("pending")
    );

    let complete_first = schema
        .execute(Request::new(admin_complete_refund_mutation(
            tenant_id,
            first_refund_id,
        )))
        .await;
    assert!(
        complete_first.errors.is_empty(),
        "unexpected complete refund errors: {:?}",
        complete_first.errors
    );
    let complete_first_json = complete_first
        .data
        .into_json()
        .expect("GraphQL complete refund response must serialize");
    assert_eq!(
        complete_first_json["completeRefund"]["status"],
        Value::from("refunded")
    );

    let create_second = schema
        .execute(Request::new(admin_create_refund_mutation(
            tenant_id,
            payment_collection.id,
            "5.00",
            "ops-review",
            "create-2",
        )))
        .await;
    assert!(
        create_second.errors.is_empty(),
        "unexpected second create refund errors: {:?}",
        create_second.errors
    );
    let create_second_json = create_second
        .data
        .into_json()
        .expect("GraphQL second create refund response must serialize");
    let second_refund_id = Uuid::parse_str(
        create_second_json["createRefund"]["id"]
            .as_str()
            .expect("refund id should be returned"),
    )
    .expect("refund id should parse");

    let cancel_second = schema
        .execute(Request::new(admin_cancel_refund_mutation(
            tenant_id,
            second_refund_id,
        )))
        .await;
    assert!(
        cancel_second.errors.is_empty(),
        "unexpected cancel refund errors: {:?}",
        cancel_second.errors
    );
    let cancel_second_json = cancel_second
        .data
        .into_json()
        .expect("GraphQL cancel refund response must serialize");
    assert_eq!(
        cancel_second_json["cancelRefund"]["status"],
        Value::from("cancelled")
    );

    let query = schema
        .execute(Request::new(admin_refund_query(
            tenant_id,
            first_refund_id,
            payment_collection.id,
        )))
        .await;
    assert!(
        query.errors.is_empty(),
        "unexpected refund query errors: {:?}",
        query.errors
    );
    let query_json = query
        .data
        .into_json()
        .expect("GraphQL refund query response must serialize");
    assert_eq!(query_json["refund"]["status"], Value::from("refunded"));
    assert_eq!(query_json["refunds"]["total"], Value::from(2));
    assert_eq!(
        query_json["paymentCollection"]["refundedAmount"],
        Value::from("10")
    );
    assert_eq!(
        query_json["paymentCollection"]["refunds"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn admin_graphql_refund_query_hides_foreign_tenant_refund() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    seed_tenant_context(&db, foreign_tenant_id).await;

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("GRAPHQL-FOREIGN-REFUND-1".to_string()),
                    title: "GraphQL Foreign Refund".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("20.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-refund-foreign" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "graphql-refund-foreign" }),
            },
        )
        .await
        .expect("order should be created");

    let payment_collection = PaymentService::new(db.clone())
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(order.id),
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                amount: order.total_amount,
                metadata: serde_json::json!({ "source": "graphql-refund-foreign" }),
            },
        )
        .await
        .expect("payment collection should be created");

    capture_payment_collection_for_refund(
        &db,
        tenant_id,
        payment_collection.id,
        order.total_amount,
    )
    .await;

    let refund = PaymentRefundCreationService::new(db.clone())
        .create_or_replay(
            tenant_id,
            payment_collection.id,
            "test-refund-foreign",
            CreateRefundInput {
                amount: Decimal::from_str("5.00").expect("valid decimal"),
                reason: Some("test".to_string()),
                metadata: serde_json::json!({ "source": "graphql-refund-foreign" }),
            },
        )
        .await
        .expect("refund should be created");

    let foreign_schema = build_schema(
        &db,
        tenant_context(foreign_tenant_id),
        request_context(foreign_tenant_id, "en"),
        Some(admin_order_auth_context(foreign_tenant_id)),
    );

    let response = foreign_schema
        .execute(Request::new(admin_refund_query(
            foreign_tenant_id,
            refund.id,
            payment_collection.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected foreign-tenant refund query errors: {:?}",
        response.errors
    );

    let json = response
        .data
        .into_json()
        .expect("foreign tenant refund query response should serialize");
    assert_eq!(json["refund"], Value::Null);
    assert_eq!(json["refunds"]["total"], Value::from(0));
    assert_eq!(json["paymentCollection"], Value::Null);
}

#[tokio::test]
async fn admin_graphql_refunds_list_ignores_foreign_tenant_payment_collection_filter() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    seed_tenant_context(&db, foreign_tenant_id).await;

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("GRAPHQL-FOREIGN-REFUND-LIST-1".to_string()),
                    title: "GraphQL Foreign Refund List".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("20.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-refund-foreign-list" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "graphql-refund-foreign-list" }),
            },
        )
        .await
        .expect("order should be created");

    let payment_collection = PaymentService::new(db.clone())
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(order.id),
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                amount: order.total_amount,
                metadata: serde_json::json!({ "source": "graphql-refund-foreign-list" }),
            },
        )
        .await
        .expect("payment collection should be created");

    capture_payment_collection_for_refund(
        &db,
        tenant_id,
        payment_collection.id,
        order.total_amount,
    )
    .await;

    PaymentRefundCreationService::new(db.clone())
        .create_or_replay(
            tenant_id,
            payment_collection.id,
            "test-refund-foreign-list",
            CreateRefundInput {
                amount: Decimal::from_str("5.00").expect("valid decimal"),
                reason: Some("test".to_string()),
                metadata: serde_json::json!({ "source": "graphql-refund-foreign-list" }),
            },
        )
        .await
        .expect("refund should be created");

    let foreign_schema = build_schema(
        &db,
        tenant_context(foreign_tenant_id),
        request_context(foreign_tenant_id, "en"),
        Some(admin_order_auth_context(foreign_tenant_id)),
    );

    let response = foreign_schema
        .execute(Request::new(admin_refunds_list_query(
            foreign_tenant_id,
            payment_collection.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected foreign-tenant refunds list errors: {:?}",
        response.errors
    );

    let json = response
        .data
        .into_json()
        .expect("foreign tenant refunds list response should serialize");
    assert_eq!(json["refunds"]["total"], Value::from(0));
    assert_eq!(json["refunds"]["items"], Value::from(Vec::<Value>::new()));
}

#[tokio::test]
async fn admin_graphql_create_refund_rejects_foreign_tenant_payment_collection() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    seed_tenant_context(&db, foreign_tenant_id).await;

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("GRAPHQL-FOREIGN-REFUND-CREATE-1".to_string()),
                    title: "GraphQL Foreign Refund Create".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("20.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-refund-foreign-create" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "graphql-refund-foreign-create" }),
            },
        )
        .await
        .expect("order should be created");

    let payment_collection = PaymentService::new(db.clone())
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(order.id),
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                amount: order.total_amount,
                metadata: serde_json::json!({ "source": "graphql-refund-foreign-create" }),
            },
        )
        .await
        .expect("payment collection should be created");

    let foreign_schema = build_schema(
        &db,
        tenant_context(foreign_tenant_id),
        request_context(foreign_tenant_id, "en"),
        Some(admin_order_auth_context(foreign_tenant_id)),
    );

    let response = foreign_schema
        .execute(Request::new(admin_create_refund_mutation(
            foreign_tenant_id,
            payment_collection.id,
            "5.00",
            "test",
            "foreign-create",
        )))
        .await;

    assert!(
        !response.errors.is_empty(),
        "foreign tenant createRefund should return GraphQL error"
    );
    let error_message = response.errors[0].message.to_lowercase();
    assert!(
        error_message.contains("not found") || error_message.contains("payment collection"),
        "unexpected createRefund error message: {}",
        response.errors[0].message
    );
}

#[tokio::test]
async fn admin_graphql_complete_refund_hides_foreign_tenant_refund() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    seed_tenant_context(&db, foreign_tenant_id).await;

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("GRAPHQL-FOREIGN-REFUND-COMPLETE-1".to_string()),
                    title: "GraphQL Foreign Refund Complete".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("20.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-refund-foreign-complete" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "graphql-refund-foreign-complete" }),
            },
        )
        .await
        .expect("order should be created");

    let payment_collection = PaymentService::new(db.clone())
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(order.id),
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                amount: order.total_amount,
                metadata: serde_json::json!({ "source": "graphql-refund-foreign-complete" }),
            },
        )
        .await
        .expect("payment collection should be created");

    capture_payment_collection_for_refund(
        &db,
        tenant_id,
        payment_collection.id,
        order.total_amount,
    )
    .await;

    let refund = PaymentRefundCreationService::new(db.clone())
        .create_or_replay(
            tenant_id,
            payment_collection.id,
            "test-refund-foreign-complete",
            CreateRefundInput {
                amount: Decimal::from_str("5.00").expect("valid decimal"),
                reason: Some("test".to_string()),
                metadata: serde_json::json!({ "source": "graphql-refund-foreign-complete" }),
            },
        )
        .await
        .expect("refund should be created");

    let foreign_schema = build_schema(
        &db,
        tenant_context(foreign_tenant_id),
        request_context(foreign_tenant_id, "en"),
        Some(admin_order_auth_context(foreign_tenant_id)),
    );

    let response = foreign_schema
        .execute(Request::new(admin_complete_refund_mutation(
            foreign_tenant_id,
            refund.id,
        )))
        .await;

    assert!(
        !response.errors.is_empty(),
        "foreign tenant completeRefund should return GraphQL error"
    );
    let error_message = response.errors[0].message.to_lowercase();
    assert!(
        error_message.contains("not found") || error_message.contains("refund"),
        "unexpected completeRefund error message: {}",
        response.errors[0].message
    );
}

#[tokio::test]
async fn admin_graphql_refunds_filter_normalizes_status_and_rejects_unknown_values() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("GRAPHQL-REFUND-STATUS-FILTER-1".to_string()),
                    title: "GraphQL Refund Status Filter".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("20.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-refund-status-filter" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "graphql-refund-status-filter" }),
            },
        )
        .await
        .expect("order should be created");

    let payment_collection = PaymentService::new(db.clone())
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(order.id),
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                amount: order.total_amount,
                metadata: serde_json::json!({ "source": "graphql-refund-status-filter" }),
            },
        )
        .await
        .expect("payment collection should be created");

    capture_payment_collection_for_refund(
        &db,
        tenant_id,
        payment_collection.id,
        order.total_amount,
    )
    .await;

    PaymentRefundCreationService::new(db.clone())
        .create_or_replay(
            tenant_id,
            payment_collection.id,
            "test-refund-status-filter",
            CreateRefundInput {
                amount: Decimal::from_str("5.00").expect("valid decimal"),
                reason: Some("test".to_string()),
                metadata: serde_json::json!({ "source": "graphql-refund-status-filter" }),
            },
        )
        .await
        .expect("refund should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_order_auth_context(tenant_id)),
    );

    let normalized_response = schema
        .execute(Request::new(admin_refunds_list_query_with_status(
            tenant_id,
            payment_collection.id,
            " PENDING ",
        )))
        .await;
    assert!(
        normalized_response.errors.is_empty(),
        "unexpected refunds filter normalization errors: {:?}",
        normalized_response.errors
    );
    let normalized_json = normalized_response
        .data
        .into_json()
        .expect("normalized refunds response should serialize");
    assert_eq!(normalized_json["refunds"]["total"], Value::from(1));

    let invalid_response = schema
        .execute(Request::new(admin_refunds_list_query_with_status(
            tenant_id,
            payment_collection.id,
            "processing",
        )))
        .await;
    assert!(
        !invalid_response.errors.is_empty(),
        "invalid refunds status should return GraphQL error"
    );
    assert!(
        invalid_response.errors[0]
            .message
            .contains("invalid refund status filter"),
        "unexpected invalid refunds status error: {}",
        invalid_response.errors[0].message
    );
}

#[tokio::test]
async fn admin_graphql_refunds_filter_supports_order_id() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let order_service = OrderService::new(db.clone(), mock_transactional_event_bus());
    let first_order = order_service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("GRAPHQL-REFUND-ORDER-FILTER-1".to_string()),
                    title: "GraphQL Refund Order Filter 1".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("20.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-refund-order-filter" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "graphql-refund-order-filter" }),
            },
        )
        .await
        .expect("first order should be created");
    let second_order = order_service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(Uuid::new_v4()),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("GRAPHQL-REFUND-ORDER-FILTER-2".to_string()),
                    title: "GraphQL Refund Order Filter 2".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("22.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-refund-order-filter" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "graphql-refund-order-filter" }),
            },
        )
        .await
        .expect("second order should be created");

    let first_collection = PaymentService::new(db.clone())
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(first_order.id),
                customer_id: first_order.customer_id,
                currency_code: "eur".to_string(),
                amount: first_order.total_amount,
                metadata: serde_json::json!({ "source": "graphql-refund-order-filter" }),
            },
        )
        .await
        .expect("first collection should be created");
    capture_payment_collection_for_refund(
        &db,
        tenant_id,
        first_collection.id,
        first_order.total_amount,
    )
    .await;

    let second_collection = PaymentService::new(db.clone())
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(second_order.id),
                customer_id: second_order.customer_id,
                currency_code: "eur".to_string(),
                amount: second_order.total_amount,
                metadata: serde_json::json!({ "source": "graphql-refund-order-filter" }),
            },
        )
        .await
        .expect("second collection should be created");
    capture_payment_collection_for_refund(
        &db,
        tenant_id,
        second_collection.id,
        second_order.total_amount,
    )
    .await;

    PaymentRefundCreationService::new(db.clone())
        .create_or_replay(
            tenant_id,
            first_collection.id,
            "test-refund-order-filter-first",
            CreateRefundInput {
                amount: Decimal::from_str("4.00").expect("valid decimal"),
                reason: Some("test".to_string()),
                metadata: serde_json::json!({ "source": "graphql-refund-order-filter" }),
            },
        )
        .await
        .expect("first refund should be created");
    PaymentRefundCreationService::new(db.clone())
        .create_or_replay(
            tenant_id,
            second_collection.id,
            "test-refund-order-filter-second",
            CreateRefundInput {
                amount: Decimal::from_str("6.00").expect("valid decimal"),
                reason: Some("test".to_string()),
                metadata: serde_json::json!({ "source": "graphql-refund-order-filter" }),
            },
        )
        .await
        .expect("second refund should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_order_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(admin_refunds_list_query_with_order(
            tenant_id,
            first_order.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected refunds-by-order errors: {:?}",
        response.errors
    );

    let json = response
        .data
        .into_json()
        .expect("refunds-by-order response should serialize");
    assert_eq!(json["refunds"]["total"], Value::from(1));
    let items = json["refunds"]["items"]
        .as_array()
        .expect("refunds items should be array");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0]["paymentCollectionId"],
        Value::from(first_collection.id.to_string())
    );
}

#[tokio::test]
async fn admin_graphql_order_query_exposes_typed_adjustments_and_totals() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(Uuid::new_v4()),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("ADMIN-ADJUSTMENT-1".to_string()),
                    title: "Admin Adjusted Order".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-admin-adjustment-order" }),
                }],
                adjustments: vec![rustok_order::dto::CreateOrderAdjustmentInput {
                    line_item_index: Some(0),
                    source_type: "Promotion".to_string(),
                    source_id: Some("promo-admin".to_string()),
                    amount: Decimal::from_str("5.00").expect("valid decimal"),
                    metadata: serde_json::json!({
                        "rule_code": "admin-adjustment",
                        "display_label": "Admin promotion"
                    }),
                }],
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "graphql-admin-adjustment-order" }),
            },
        )
        .await
        .expect("order should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_order_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              order(tenantId: "{tenant_id}", id: "{order_id}") {{
                order {{
                  id
                  subtotalAmount
                  adjustmentTotal
                  totalAmount
                  lineItems {{
                    id
                  }}
                  adjustments {{
                    lineItemId
                    sourceType
                    sourceId
                    amount
                    currencyCode
                    metadata
                  }}
                }}
              }}
            }}
            "#,
            order_id = order.id
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected admin order adjustment GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(json["order"]["order"]["subtotalAmount"], Value::from("25"));
    assert_eq!(json["order"]["order"]["adjustmentTotal"], Value::from("5"));
    assert_eq!(json["order"]["order"]["totalAmount"], Value::from("20"));
    assert_eq!(
        json["order"]["order"]["adjustments"][0]["lineItemId"],
        json["order"]["order"]["lineItems"][0]["id"]
    );
    assert_eq!(
        json["order"]["order"]["adjustments"][0]["sourceType"],
        Value::from("promotion")
    );
    assert_eq!(
        json["order"]["order"]["adjustments"][0]["sourceId"],
        Value::from("promo-admin")
    );
    assert_eq!(
        json["order"]["order"]["adjustments"][0]["amount"],
        Value::from("5")
    );
    assert_eq!(
        json["order"]["order"]["adjustments"][0]["currencyCode"],
        Value::from("EUR")
    );
    let metadata: Value = serde_json::from_str(
        json["order"]["order"]["adjustments"][0]["metadata"]
            .as_str()
            .expect("order adjustment metadata should be JSON string"),
    )
    .expect("order adjustment metadata should parse");
    assert_eq!(metadata["rule_code"], Value::from("admin-adjustment"));
    assert!(metadata.get("display_label").is_none());
}

#[tokio::test]
async fn admin_graphql_order_query_exposes_shipping_total_and_shipping_scoped_adjustments() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(Uuid::new_v4()),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::from_str("9.99").expect("valid decimal"),
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("ADMIN-SHIPPING-ADJUSTMENT-1".to_string()),
                    title: "Admin Shipping Adjusted Order".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-admin-shipping-adjustment-order" }),
                }],
                adjustments: vec![rustok_order::dto::CreateOrderAdjustmentInput {
                    line_item_index: None,
                    source_type: "Promotion".to_string(),
                    source_id: Some("promo-shipping-admin".to_string()),
                    amount: Decimal::from_str("4.99").expect("valid decimal"),
                    metadata: serde_json::json!({
                        "rule_code": "admin-shipping-adjustment",
                        "scope": "shipping",
                        "display_label": "Admin shipping promotion"
                    }),
                }],
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "graphql-admin-shipping-adjustment-order" }),
            },
        )
        .await
        .expect("order should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_order_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              order(tenantId: "{tenant_id}", id: "{order_id}") {{
                order {{
                  id
                  shippingTotal
                  adjustmentTotal
                  totalAmount
                  adjustments {{
                    lineItemId
                    sourceType
                    sourceId
                    amount
                    currencyCode
                    metadata
                  }}
                }}
              }}
            }}
            "#,
            order_id = order.id
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected admin shipping adjustment GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(json["order"]["order"]["shippingTotal"], Value::from("9.99"));
    assert_eq!(
        json["order"]["order"]["adjustmentTotal"],
        Value::from("4.99")
    );
    assert_eq!(json["order"]["order"]["totalAmount"], Value::from("30"));
    assert_eq!(
        json["order"]["order"]["adjustments"][0]["lineItemId"],
        Value::Null
    );
    assert_eq!(
        json["order"]["order"]["adjustments"][0]["sourceType"],
        Value::from("promotion")
    );
    assert_eq!(
        json["order"]["order"]["adjustments"][0]["sourceId"],
        Value::from("promo-shipping-admin")
    );
    assert_eq!(
        json["order"]["order"]["adjustments"][0]["amount"],
        Value::from("4.99")
    );
    assert_eq!(
        json["order"]["order"]["adjustments"][0]["currencyCode"],
        Value::from("EUR")
    );
    let metadata: Value = serde_json::from_str(
        json["order"]["order"]["adjustments"][0]["metadata"]
            .as_str()
            .expect("order adjustment metadata should be JSON string"),
    )
    .expect("order adjustment metadata should parse");
    assert_eq!(
        metadata["rule_code"],
        Value::from("admin-shipping-adjustment")
    );
    assert_eq!(metadata["scope"], Value::from("shipping"));
    assert!(metadata.get("display_label").is_none());
}

#[tokio::test]
async fn admin_graphql_order_query_exposes_tax_breakdown_with_provider_ids() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(Uuid::new_v4()),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("ADMIN-TAX-LINE-1".to_string()),
                    title: "Admin Taxed Order".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("100.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-admin-tax-order" }),
                }],
                adjustments: Vec::new(),
                tax_lines: vec![
                    rustok_order::dto::CreateOrderTaxLineInput {
                        line_item_index: Some(0),
                        shipping_option_id: None,
                        rate: Decimal::from_str("19.00").expect("valid decimal"),
                        amount: Decimal::from_str("19.00").expect("valid decimal"),
                        currency_code: "eur".to_string(),
                        description: Some("VAT line item".to_string()),
                        provider_id: "region_default".to_string(),
                        metadata: serde_json::json!({"tax_included": false, "scope": "line_item"}),
                    },
                    rustok_order::dto::CreateOrderTaxLineInput {
                        line_item_index: None,
                        shipping_option_id: Some(Uuid::new_v4()),
                        rate: Decimal::from_str("19.00").expect("valid decimal"),
                        amount: Decimal::from_str("1.00").expect("valid decimal"),
                        currency_code: "eur".to_string(),
                        description: Some("VAT shipping".to_string()),
                        provider_id: "region_default".to_string(),
                        metadata: serde_json::json!({"tax_included": false, "scope": "shipping"}),
                    },
                    rustok_order::dto::CreateOrderTaxLineInput {
                        line_item_index: None,
                        shipping_option_id: None,
                        rate: Decimal::from_str("19.00").expect("valid decimal"),
                        amount: Decimal::from_str("0.50").expect("valid decimal"),
                        currency_code: "eur".to_string(),
                        description: Some("VAT order".to_string()),
                        provider_id: "region_default".to_string(),
                        metadata: serde_json::json!({"tax_included": false, "scope": "order"}),
                    },
                ],
                metadata: serde_json::json!({ "source": "graphql-admin-tax-order" }),
            },
        )
        .await
        .expect("order should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_order_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              order(tenantId: "{tenant_id}", id: "{order_id}") {{
                order {{
                  id
                  taxTotal
                  taxIncluded
                  taxLines {{
                    providerId
                    description
                    amount
                    rate
                    lineItemId
                    shippingOptionId
                    metadata
                  }}
                }}
              }}
            }}
            "#,
            order_id = order.id
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected admin tax GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("admin tax query response should serialize");
    assert_eq!(json["order"]["order"]["taxTotal"], Value::from("20.5"));
    assert_eq!(json["order"]["order"]["taxIncluded"], Value::from(false));
    let tax_lines = json["order"]["order"]["taxLines"]
        .as_array()
        .expect("tax lines array");
    assert_eq!(tax_lines.len(), 3);
    assert!(tax_lines
        .iter()
        .all(|line| line["providerId"] == "region_default"));
    assert!(tax_lines
        .iter()
        .any(|line| line["lineItemId"].as_str().is_some()));
    assert!(tax_lines
        .iter()
        .any(|line| line["shippingOptionId"].as_str().is_some()));
    assert!(tax_lines
        .iter()
        .any(|line| line["lineItemId"].is_null() && line["shippingOptionId"].is_null()));
}

#[tokio::test]
async fn admin_graphql_return_decision_creates_completed_claim_order_change() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: Some("merchant-claim-id".to_string()),
                    sku: Some("GRAPHQL-RETURN-CLAIM-1".to_string()),
                    title: "GraphQL Return Claim Order".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("27.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-return-claim-decision" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "graphql-return-claim-decision" }),
            },
        )
        .await
        .expect("order should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_order_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(admin_return_claim_decision_mutation(
            tenant_id,
            order.id,
            order.line_items[0].id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected admin GraphQL return claim decision errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    let decision = &json["createOrderReturnDecision"];
    let order_return = &decision["orderReturn"];
    let order_change = &decision["orderChange"];
    let order_return_id = order_return["id"]
        .as_str()
        .expect("return id should be a string");
    let order_change_id = order_change["id"]
        .as_str()
        .expect("order change id should be a string");
    let decision_metadata: Value = serde_json::from_str(
        decision["metadata"]
            .as_str()
            .expect("decision metadata should be a JSON string"),
    )
    .expect("decision metadata should parse");
    let return_metadata: Value = serde_json::from_str(
        order_return["metadata"]
            .as_str()
            .expect("return metadata should be a JSON string"),
    )
    .expect("return metadata should parse");
    let change_metadata: Value = serde_json::from_str(
        order_change["metadata"]
            .as_str()
            .expect("order change metadata should be a JSON string"),
    )
    .expect("order change metadata should parse");
    let change_preview: Value = serde_json::from_str(
        order_change["preview"]
            .as_str()
            .expect("order change preview should be a JSON string"),
    )
    .expect("order change preview should parse");

    assert_eq!(decision["action"], Value::from("claim"));
    assert_eq!(decision_metadata["flow"], Value::from("claim"));
    assert_eq!(order_return["orderId"], Value::from(order.id.to_string()));
    assert_eq!(order_return["status"], Value::from("completed"));
    assert_eq!(order_return["resolutionType"], Value::from("claim"));
    assert_eq!(
        order_return["orderChangeId"],
        Value::from(order_change_id.to_string())
    );
    assert_eq!(
        return_metadata["source"],
        Value::from("graphql-return-claim-decision")
    );
    assert_eq!(order_change["orderId"], Value::from(order.id.to_string()));
    assert_eq!(order_change["changeType"], Value::from("claim"));
    assert_eq!(
        order_change["description"],
        Value::from("Operator claim review")
    );
    assert_eq!(change_metadata["operator"], Value::from("claims-desk"));
    assert_eq!(
        change_metadata["order_return_id"],
        Value::from(order_return_id)
    );
    assert_eq!(
        change_preview["order_return_id"],
        Value::from(order_return_id)
    );
    assert_eq!(change_preview["claim_type"], Value::from("damaged_item"));
    assert!(decision["refund"].is_null());
}

#[tokio::test]
async fn admin_graphql_complete_return_with_exchange_helper() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let order_service = OrderService::new(db.clone(), mock_transactional_event_bus());
    let order = order_service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: Some("merchant-exchange-id".to_string()),
                    sku: Some("GRAPHQL-COMPLETE-EXCHANGE-1".to_string()),
                    title: "GraphQL Complete Return Exchange Order".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("27.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-complete-exchange" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "graphql-complete-exchange" }),
            },
        )
        .await
        .expect("order should be created");

    let order_return = order_service
        .create_return(
            tenant_id,
            order.id,
            rustok_order::dto::CreateOrderReturnInput {
                reason: Some("wrong-size".to_string()),
                note: Some("needs larger size".to_string()),
                items: vec![rustok_order::dto::CreateOrderReturnItemInput {
                    line_item_id: order.line_items[0].id,
                    quantity: 1,
                    reason: Some("too small".to_string()),
                    note: None,
                    metadata: serde_json::json!({}),
                }],
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("return should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_order_auth_context(tenant_id)),
    );
    let preview_json = r#"{"exchange_type":"size_exchange","items":[{"sku":"GRAPHQL-COMPLETE-EXCHANGE-2","quantity":1}]}"#;
    let response = schema
        .execute(Request::new(
            admin_complete_order_return_with_exchange_mutation(
                tenant_id,
                order_return.id,
                preview_json,
            ),
        ))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected admin GraphQL complete return exchange errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    let completed_return = &json["completeOrderReturn"];
    let order_change_id = completed_return["orderChangeId"]
        .as_str()
        .expect("order change id should be a string");

    assert_eq!(completed_return["status"], Value::from("completed"));
    assert_eq!(completed_return["resolutionType"], Value::from("exchange"));

    // Verify created order change has correct context attached
    let order_change = order_service
        .get_order_change(tenant_id, Uuid::parse_str(order_change_id).unwrap())
        .await
        .expect("order change should exist");

    assert_eq!(order_change.change_type, "exchange");
    assert_eq!(
        order_change.metadata["order_return_id"],
        serde_json::json!(order_return.id.to_string())
    );
    assert_eq!(
        order_change.metadata["return_decision_action"],
        serde_json::json!("exchange")
    );
    assert_eq!(
        order_change.metadata["return_decision_source"],
        serde_json::json!("rustok-commerce")
    );
    assert_eq!(
        order_change.preview["order_return_id"],
        serde_json::json!(order_return.id.to_string())
    );
    assert_eq!(
        order_change.preview["return_decision_action"],
        serde_json::json!("exchange")
    );
}

#[tokio::test]
async fn admin_graphql_complete_return_with_claim_helper() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let order_service = OrderService::new(db.clone(), mock_transactional_event_bus());
    let order = order_service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: Some("merchant-claim-id".to_string()),
                    sku: Some("GRAPHQL-COMPLETE-CLAIM-1".to_string()),
                    title: "GraphQL Complete Return Claim Order".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("27.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-complete-claim" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "graphql-complete-claim" }),
            },
        )
        .await
        .expect("order should be created");

    let order_return = order_service
        .create_return(
            tenant_id,
            order.id,
            rustok_order::dto::CreateOrderReturnInput {
                reason: Some("damaged".to_string()),
                note: Some("damaged on delivery".to_string()),
                items: vec![rustok_order::dto::CreateOrderReturnItemInput {
                    line_item_id: order.line_items[0].id,
                    quantity: 1,
                    reason: Some("broken".to_string()),
                    note: None,
                    metadata: serde_json::json!({}),
                }],
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("return should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_order_auth_context(tenant_id)),
    );
    let preview_json = r#"{"claim_type":"damaged_item","resolution":"replacement"}"#;
    let response = schema
        .execute(Request::new(
            admin_complete_order_return_with_claim_mutation(
                tenant_id,
                order_return.id,
                preview_json,
            ),
        ))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected admin GraphQL complete return claim errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    let completed_return = &json["completeOrderReturn"];
    let order_change_id = completed_return["orderChangeId"]
        .as_str()
        .expect("order change id should be a string");

    assert_eq!(completed_return["status"], Value::from("completed"));
    assert_eq!(completed_return["resolutionType"], Value::from("claim"));

    // Verify created order change has correct context attached
    let order_change = order_service
        .get_order_change(tenant_id, Uuid::parse_str(order_change_id).unwrap())
        .await
        .expect("order change should exist");

    assert_eq!(order_change.change_type, "claim");
    assert_eq!(
        order_change.metadata["order_return_id"],
        serde_json::json!(order_return.id.to_string())
    );
    assert_eq!(
        order_change.metadata["return_decision_action"],
        serde_json::json!("claim")
    );
    assert_eq!(
        order_change.metadata["return_decision_source"],
        serde_json::json!("rustok-commerce")
    );
    assert_eq!(
        order_change.preview["order_return_id"],
        serde_json::json!(order_return.id.to_string())
    );
    assert_eq!(
        order_change.preview["return_decision_action"],
        serde_json::json!("claim")
    );
}

#[tokio::test]
async fn admin_graphql_create_fulfillment_supports_typed_manual_post_order_items() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: Some("merchant-alpha-id".to_string()),
                    sku: Some("GRAPHQL-MANUAL-FULFILLMENT-1".to_string()),
                    title: "GraphQL Manual Fulfillment Order".to_string(),
                    quantity: 3,
                    unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                    metadata: serde_json::json!({
                        "source": "graphql-manual-fulfillment",
                        "seller": {
                            "scope": "merchant-alpha",
                            "label": "Merchant Alpha"
                        }
                    }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "graphql-manual-fulfillment" }),
            },
        )
        .await
        .expect("order should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_fulfillment_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(admin_create_fulfillment_mutation(
            tenant_id,
            order.id,
            order.line_items[0].id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected admin GraphQL create fulfillment errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    let fulfillment_metadata: Value = serde_json::from_str(
        json["createFulfillment"]["metadata"]
            .as_str()
            .expect("fulfillment metadata should be JSON string"),
    )
    .expect("fulfillment metadata should parse");

    assert_eq!(
        json["createFulfillment"]["orderId"],
        Value::from(order.id.to_string())
    );
    assert_eq!(
        json["createFulfillment"]["customerId"],
        Value::from(customer_id.to_string())
    );
    assert_eq!(json["createFulfillment"]["status"], Value::from("pending"));
    assert_eq!(
        json["createFulfillment"]["items"][0]["orderLineItemId"],
        Value::from(order.line_items[0].id.to_string())
    );
    assert_eq!(
        json["createFulfillment"]["items"][0]["quantity"],
        Value::from(2)
    );
    assert_eq!(
        fulfillment_metadata["delivery_group"]["seller_id"],
        Value::from("merchant-alpha-id")
    );
    assert!(fulfillment_metadata["delivery_group"]
        .get("seller_scope")
        .is_none());
    assert!(fulfillment_metadata["delivery_group"]
        .get("seller_label")
        .is_none());
    assert_eq!(
        fulfillment_metadata["post_order"]["manual"],
        Value::from(true)
    );
}

#[tokio::test]
async fn admin_graphql_ship_and_deliver_support_partial_item_progress() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("GRAPHQL-PARTIAL-FULFILLMENT-1".to_string()),
                    title: "GraphQL Partial Fulfillment Order".to_string(),
                    quantity: 3,
                    unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-partial-fulfillment" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "graphql-partial-fulfillment" }),
            },
        )
        .await
        .expect("order should be created");
    let fulfillment = FulfillmentService::new(db.clone())
        .create_fulfillment(
            tenant_id,
            CreateFulfillmentInput {
                order_id: order.id,
                shipping_option_id: None,
                customer_id: Some(customer_id),
                carrier: None,
                tracking_number: None,
                items: Some(vec![rustok_fulfillment::dto::CreateFulfillmentItemInput {
                    order_line_item_id: order.line_items[0].id,
                    quantity: 3,
                    metadata: serde_json::json!({ "source": "graphql-partial-fulfillment" }),
                }]),
                metadata: serde_json::json!({ "source": "graphql-partial-fulfillment" }),
            },
        )
        .await
        .expect("fulfillment should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_fulfillment_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(admin_partial_fulfillment_progress_mutation(
            tenant_id,
            fulfillment.id,
            fulfillment.items[0].id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected admin GraphQL partial fulfillment errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    let deliver_metadata: Value = serde_json::from_str(
        json["deliverFulfillment"]["metadata"]
            .as_str()
            .expect("deliver metadata should be JSON string"),
    )
    .expect("deliver metadata should parse");

    assert_eq!(json["shipFulfillment"]["status"], Value::from("shipped"));
    assert_eq!(
        json["shipFulfillment"]["items"][0]["shippedQuantity"],
        Value::from(2)
    );
    assert_eq!(json["deliverFulfillment"]["status"], Value::from("shipped"));
    assert_eq!(
        json["deliverFulfillment"]["items"][0]["deliveredQuantity"],
        Value::from(1)
    );
    assert_eq!(
        deliver_metadata["audit"]["events"][1]["type"],
        Value::from("deliver")
    );
}

#[tokio::test]
async fn admin_graphql_reopen_fulfillment_restores_shipped_progress() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("GRAPHQL-REOPEN-FULFILLMENT-1".to_string()),
                    title: "GraphQL Reopen Fulfillment Order".to_string(),
                    quantity: 3,
                    unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-reopen-fulfillment" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "graphql-reopen-fulfillment" }),
            },
        )
        .await
        .expect("order should be created");
    let fulfillment = FulfillmentService::new(db.clone())
        .create_fulfillment(
            tenant_id,
            CreateFulfillmentInput {
                order_id: order.id,
                shipping_option_id: None,
                customer_id: Some(customer_id),
                carrier: None,
                tracking_number: None,
                items: Some(vec![rustok_fulfillment::dto::CreateFulfillmentItemInput {
                    order_line_item_id: order.line_items[0].id,
                    quantity: 3,
                    metadata: serde_json::json!({ "source": "graphql-reopen-fulfillment" }),
                }]),
                metadata: serde_json::json!({ "source": "graphql-reopen-fulfillment" }),
            },
        )
        .await
        .expect("fulfillment should be created");
    FulfillmentService::new(db.clone())
        .ship_fulfillment(
            tenant_id,
            fulfillment.id,
            ShipFulfillmentInput {
                carrier: "manual".to_string(),
                tracking_number: "GRAPHQL-REOPEN".to_string(),
                items: Some(vec![
                    rustok_fulfillment::dto::FulfillmentItemQuantityInput {
                        fulfillment_item_id: fulfillment.items[0].id,
                        quantity: 3,
                    },
                ]),
                metadata: serde_json::json!({ "source": "graphql-reopen-ship" }),
            },
        )
        .await
        .expect("fulfillment should ship");
    FulfillmentService::new(db.clone())
        .deliver_fulfillment(
            tenant_id,
            fulfillment.id,
            DeliverFulfillmentInput {
                delivered_note: Some("done".to_string()),
                items: Some(vec![
                    rustok_fulfillment::dto::FulfillmentItemQuantityInput {
                        fulfillment_item_id: fulfillment.items[0].id,
                        quantity: 3,
                    },
                ]),
                metadata: serde_json::json!({ "source": "graphql-reopen-deliver" }),
            },
        )
        .await
        .expect("fulfillment should deliver");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_fulfillment_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(admin_reopen_fulfillment_mutation(
            tenant_id,
            fulfillment.id,
            fulfillment.items[0].id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected admin GraphQL reopen fulfillment errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    let reopen_metadata: Value = serde_json::from_str(
        json["reopenFulfillment"]["metadata"]
            .as_str()
            .expect("reopen metadata should be JSON string"),
    )
    .expect("reopen metadata should parse");

    assert_eq!(json["reopenFulfillment"]["status"], Value::from("shipped"));
    assert_eq!(
        json["reopenFulfillment"]["items"][0]["deliveredQuantity"],
        Value::from(2)
    );
    assert_eq!(json["reopenFulfillment"]["deliveredNote"], Value::Null);
    assert_eq!(
        reopen_metadata["audit"]["events"][2]["type"],
        Value::from("reopen")
    );
}

#[tokio::test]
async fn admin_graphql_reship_fulfillment_reopens_delivery_with_new_tracking() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("GRAPHQL-RESHIP-FULFILLMENT-1".to_string()),
                    title: "GraphQL Reship Fulfillment Order".to_string(),
                    quantity: 2,
                    unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-reship-fulfillment" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "graphql-reship-fulfillment" }),
            },
        )
        .await
        .expect("order should be created");
    let fulfillment = FulfillmentService::new(db.clone())
        .create_fulfillment(
            tenant_id,
            CreateFulfillmentInput {
                order_id: order.id,
                shipping_option_id: None,
                customer_id: Some(customer_id),
                carrier: None,
                tracking_number: None,
                items: Some(vec![rustok_fulfillment::dto::CreateFulfillmentItemInput {
                    order_line_item_id: order.line_items[0].id,
                    quantity: 2,
                    metadata: serde_json::json!({ "source": "graphql-reship-fulfillment" }),
                }]),
                metadata: serde_json::json!({ "source": "graphql-reship-fulfillment" }),
            },
        )
        .await
        .expect("fulfillment should be created");
    FulfillmentService::new(db.clone())
        .ship_fulfillment(
            tenant_id,
            fulfillment.id,
            ShipFulfillmentInput {
                carrier: "manual".to_string(),
                tracking_number: "GRAPHQL-RESHIP-OLD".to_string(),
                items: Some(vec![
                    rustok_fulfillment::dto::FulfillmentItemQuantityInput {
                        fulfillment_item_id: fulfillment.items[0].id,
                        quantity: 2,
                    },
                ]),
                metadata: serde_json::json!({ "source": "graphql-reship-ship" }),
            },
        )
        .await
        .expect("fulfillment should ship");
    FulfillmentService::new(db.clone())
        .deliver_fulfillment(
            tenant_id,
            fulfillment.id,
            DeliverFulfillmentInput {
                delivered_note: Some("done".to_string()),
                items: Some(vec![
                    rustok_fulfillment::dto::FulfillmentItemQuantityInput {
                        fulfillment_item_id: fulfillment.items[0].id,
                        quantity: 2,
                    },
                ]),
                metadata: serde_json::json!({ "source": "graphql-reship-deliver" }),
            },
        )
        .await
        .expect("fulfillment should deliver");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_fulfillment_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(admin_reship_fulfillment_mutation(
            tenant_id,
            fulfillment.id,
            fulfillment.items[0].id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected admin GraphQL reship fulfillment errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    let reship_metadata: Value = serde_json::from_str(
        json["reshipFulfillment"]["metadata"]
            .as_str()
            .expect("reship metadata should be JSON string"),
    )
    .expect("reship metadata should parse");

    assert_eq!(json["reshipFulfillment"]["status"], Value::from("shipped"));
    assert_eq!(
        json["reshipFulfillment"]["trackingNumber"],
        Value::from("GRAPHQL-RESHIP")
    );
    assert_eq!(
        json["reshipFulfillment"]["items"][0]["deliveredQuantity"],
        Value::from(0)
    );
    assert_eq!(json["reshipFulfillment"]["deliveredNote"], Value::Null);
    assert_eq!(
        reship_metadata["audit"]["events"][2]["type"],
        Value::from("reship")
    );
}

#[tokio::test]
async fn storefront_graphql_customer_and_order_queries_match_customer_owned_read_path() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let customer = CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(user_id),
                email: "buyer@example.com".to_string(),
                first_name: Some("GraphQL".to_string()),
                last_name: Some("Buyer".to_string()),
                phone: None,
                locale: Some("de".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-order-parity" }),
            },
        )
        .await
        .expect("customer should be created");

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            user_id,
            CreateOrderInput {
                customer_id: Some(customer.id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("STOREFRONT-ORDER-1".to_string()),
                    title: "Storefront Order".to_string(),
                    quantity: 2,
                    unit_price: Decimal::from_str("15.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "storefront-graphql-order-parity" }),
                }],
                adjustments: Vec::new(),
                tax_lines: vec![
                    rustok_order::dto::CreateOrderTaxLineInput {
                        line_item_index: Some(0),
                        shipping_option_id: None,
                        rate: Decimal::from_str("19.00").expect("valid decimal"),
                        amount: Decimal::from_str("5.70").expect("valid decimal"),
                        currency_code: "eur".to_string(),
                        description: Some("VAT line item".to_string()),
                        provider_id: "region_default".to_string(),
                        metadata: serde_json::json!({ "tax_included": false, "scope": "line_item" }),
                    },
                    rustok_order::dto::CreateOrderTaxLineInput {
                        line_item_index: None,
                        shipping_option_id: Some(Uuid::new_v4()),
                        rate: Decimal::from_str("19.00").expect("valid decimal"),
                        amount: Decimal::from_str("1.50").expect("valid decimal"),
                        currency_code: "eur".to_string(),
                        description: Some("VAT shipping".to_string()),
                        provider_id: "region_default".to_string(),
                        metadata: serde_json::json!({ "tax_included": false, "scope": "shipping" }),
                    },
                    rustok_order::dto::CreateOrderTaxLineInput {
                        line_item_index: None,
                        shipping_option_id: None,
                        rate: Decimal::from_str("19.00").expect("valid decimal"),
                        amount: Decimal::from_str("0.25").expect("valid decimal"),
                        currency_code: "eur".to_string(),
                        description: Some("VAT order".to_string()),
                        provider_id: "region_default".to_string(),
                        metadata: serde_json::json!({ "tax_included": false, "scope": "order" }),
                    },
                ],
                metadata: serde_json::json!({ "source": "storefront-graphql-order-parity" }),
            },
        )
        .await
        .expect("order should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        Some(customer_auth_context(tenant_id, user_id)),
    );
    let response = schema
        .execute(Request::new(storefront_customer_order_query(
            tenant_id, order.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected storefront GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["storefrontMe"]["email"],
        Value::from("buyer@example.com")
    );
    assert_eq!(json["storefrontMe"]["locale"], Value::from("de"));
    assert_eq!(
        json["storefrontOrder"]["id"],
        Value::from(order.id.to_string())
    );
    assert_eq!(
        json["storefrontOrder"]["customerId"],
        Value::from(customer.id.to_string())
    );
    assert_eq!(json["storefrontOrder"]["status"], Value::from("pending"));
    assert_eq!(json["storefrontOrder"]["currencyCode"], Value::from("EUR"));
    assert_eq!(json["storefrontOrder"]["taxTotal"], Value::from("7.45"));
    assert_eq!(json["storefrontOrder"]["taxIncluded"], Value::from(false));
    let tax_lines = json["storefrontOrder"]["taxLines"]
        .as_array()
        .expect("tax lines array");
    assert_eq!(tax_lines.len(), 3);
    assert!(tax_lines
        .iter()
        .all(|line| line["providerId"] == "region_default"));
    assert!(tax_lines
        .iter()
        .any(|line| line["lineItemId"].as_str().is_some() && line["shippingOptionId"].is_null()));
    assert!(tax_lines
        .iter()
        .any(|line| line["lineItemId"].is_null() && line["shippingOptionId"].as_str().is_some()));
    assert!(tax_lines
        .iter()
        .any(|line| line["lineItemId"].is_null() && line["shippingOptionId"].is_null()));
    assert_eq!(json["storefrontOrder"]["totalAmount"], Value::from("37.45"));
    assert_eq!(
        json["storefrontOrder"]["lineItems"][0]["title"],
        Value::from("Storefront Order")
    );
    assert_eq!(
        json["storefrontOrder"]["lineItems"][0]["quantity"],
        Value::from(2)
    );
}

#[tokio::test]
async fn storefront_graphql_refunds_query_returns_customer_order_refunds_only() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let customer_user_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let customer = CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(customer_user_id),
                email: "refund-buyer@example.com".to_string(),
                first_name: Some("Refund".to_string()),
                last_name: Some("Buyer".to_string()),
                phone: None,
                locale: Some("de".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-refunds" }),
            },
        )
        .await
        .expect("customer should be created");

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            customer_user_id,
            CreateOrderInput {
                customer_id: Some(customer.id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("STOREFRONT-REFUND-1".to_string()),
                    title: "Storefront Refundable Order".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("30.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "storefront-graphql-refunds" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "storefront-graphql-refunds" }),
            },
        )
        .await
        .expect("order should be created");

    let payment = PaymentService::new(db.clone())
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(order.id),
                amount: Decimal::from_str("30.00").expect("valid decimal"),
                currency_code: "EUR".to_string(),
                customer_id: Some(customer.id),
                metadata: serde_json::json!({ "source": "storefront-graphql-refunds" }),
            },
        )
        .await
        .expect("payment collection should be created");

    capture_payment_collection_for_refund(&db, tenant_id, payment.id, order.total_amount).await;

    let created_refund = PaymentRefundCreationService::new(db.clone())
        .create_or_replay(
            tenant_id,
            payment.id,
            "test-storefront-graphql-refunds",
            CreateRefundInput {
                amount: Decimal::from_str("10.00").expect("valid decimal"),
                reason: Some("customer-request".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-refunds" }),
            },
        )
        .await
        .expect("refund should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        Some(customer_auth_context(tenant_id, customer_user_id)),
    );
    let response = schema
        .execute(Request::new(storefront_refunds_query(tenant_id, order.id)))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected storefront refunds errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("response should serialize");
    assert_eq!(json["storefrontRefunds"]["total"], Value::from(1));
    let items = json["storefrontRefunds"]["items"]
        .as_array()
        .expect("refund items should be array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], Value::from(created_refund.id.to_string()));
    assert_eq!(
        items[0]["paymentCollectionId"],
        Value::from(payment.id.to_string())
    );
    assert_eq!(items[0]["amount"], Value::from("10"));
}

#[tokio::test]
async fn storefront_graphql_refunds_query_rejects_foreign_customer_order() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let owner_user_id = Uuid::new_v4();
    let foreign_user_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let owner = CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(owner_user_id),
                email: "owner-refund@example.com".to_string(),
                first_name: Some("Owner".to_string()),
                last_name: None,
                phone: None,
                locale: Some("de".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-refunds-forbidden" }),
            },
        )
        .await
        .expect("owner customer should be created");

    CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(foreign_user_id),
                email: "foreign-refund@example.com".to_string(),
                first_name: Some("Foreign".to_string()),
                last_name: None,
                phone: None,
                locale: Some("de".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-refunds-forbidden" }),
            },
        )
        .await
        .expect("foreign customer should be created");

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            owner_user_id,
            CreateOrderInput {
                customer_id: Some(owner.id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("STOREFRONT-REFUND-FORBIDDEN".to_string()),
                    title: "Foreign Order".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("20.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "storefront-graphql-refunds-forbidden" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "storefront-graphql-refunds-forbidden" }),
            },
        )
        .await
        .expect("order should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        Some(customer_auth_context(tenant_id, foreign_user_id)),
    );
    let response = schema
        .execute(Request::new(storefront_refunds_query(tenant_id, order.id)))
        .await;

    assert_eq!(response.errors.len(), 1, "expected ownership error");
    assert!(
        response.errors[0]
            .message
            .contains("Order does not belong to the current customer"),
        "unexpected ownership error: {}",
        response.errors[0].message
    );
}

#[tokio::test]
async fn storefront_graphql_refunds_query_requires_authentication() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let response = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        None,
    )
    .execute(Request::new(storefront_refunds_query(
        tenant_id,
        Uuid::new_v4(),
    )))
    .await;

    assert_eq!(response.errors.len(), 1, "expected unauthenticated error");
    assert!(
        response.errors[0]
            .message
            .to_ascii_lowercase()
            .contains("auth"),
        "unexpected unauthenticated error: {}",
        response.errors[0].message
    );
}

#[tokio::test]
async fn storefront_graphql_refunds_query_returns_empty_for_unknown_order() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let customer_user_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(customer_user_id),
                email: "refund-empty@example.com".to_string(),
                first_name: Some("Empty".to_string()),
                last_name: Some("Refunds".to_string()),
                phone: None,
                locale: Some("de".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-refunds-empty" }),
            },
        )
        .await
        .expect("customer should be created");

    let response = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        Some(customer_auth_context(tenant_id, customer_user_id)),
    )
    .execute(Request::new(storefront_refunds_query_with_paging(
        tenant_id,
        Uuid::new_v4(),
        3,
        7,
    )))
    .await;

    assert!(
        response.errors.is_empty(),
        "unexpected errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("response should serialize");
    assert_eq!(json["storefrontRefunds"]["total"], Value::from(0));
    assert_eq!(json["storefrontRefunds"]["page"], Value::from(3));
    assert_eq!(json["storefrontRefunds"]["perPage"], Value::from(7));
    assert_eq!(
        json["storefrontRefunds"]["items"],
        Value::from(Vec::<Value>::new())
    );
}

#[tokio::test]
async fn storefront_graphql_refunds_query_normalizes_status_and_rejects_unknown_values() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let customer_user_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let customer = CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(customer_user_id),
                email: "refund-status@example.com".to_string(),
                first_name: Some("Refund".to_string()),
                last_name: Some("Status".to_string()),
                phone: None,
                locale: Some("de".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-refunds-status" }),
            },
        )
        .await
        .expect("customer should be created");

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            customer_user_id,
            CreateOrderInput {
                customer_id: Some(customer.id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("STOREFRONT-REFUND-STATUS".to_string()),
                    title: "Storefront Refund Status".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("40.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "storefront-graphql-refunds-status" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "storefront-graphql-refunds-status" }),
            },
        )
        .await
        .expect("order should be created");

    let payment = PaymentService::new(db.clone())
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(order.id),
                amount: Decimal::from_str("40.00").expect("valid decimal"),
                currency_code: "EUR".to_string(),
                customer_id: Some(customer.id),
                metadata: serde_json::json!({ "source": "storefront-graphql-refunds-status" }),
            },
        )
        .await
        .expect("payment collection should be created");

    capture_payment_collection_for_refund(&db, tenant_id, payment.id, order.total_amount).await;

    PaymentRefundCreationService::new(db.clone())
        .create_or_replay(
            tenant_id,
            payment.id,
            "test-storefront-graphql-refunds-status",
            CreateRefundInput {
                amount: Decimal::from_str("5.00").expect("valid decimal"),
                reason: Some("status-normalization".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-refunds-status" }),
            },
        )
        .await
        .expect("refund should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        Some(customer_auth_context(tenant_id, customer_user_id)),
    );

    let normalized = schema
        .execute(Request::new(storefront_refunds_query_with_status(
            tenant_id,
            order.id,
            " PENDING ",
        )))
        .await;
    assert!(
        normalized.errors.is_empty(),
        "unexpected normalized errors: {:?}",
        normalized.errors
    );
    let normalized_json = normalized
        .data
        .into_json()
        .expect("normalized response should serialize");
    assert_eq!(
        normalized_json["storefrontRefunds"]["total"],
        Value::from(1)
    );

    let invalid = schema
        .execute(Request::new(storefront_refunds_query_with_status(
            tenant_id,
            order.id,
            "processing",
        )))
        .await;
    assert_eq!(invalid.errors.len(), 1, "invalid status should fail");
    assert!(
        invalid.errors[0]
            .message
            .contains("invalid refund status filter"),
        "unexpected invalid-status error: {}",
        invalid.errors[0].message
    );
}

#[tokio::test]
async fn storefront_graphql_order_query_exposes_typed_adjustments_and_totals() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let owner_user_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let customer = CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(owner_user_id),
                email: "buyer@example.com".to_string(),
                first_name: Some("Buyer".to_string()),
                last_name: None,
                phone: None,
                locale: Some("de".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-adjusted-order" }),
            },
        )
        .await
        .expect("customer should be created");

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            owner_user_id,
            CreateOrderInput {
                customer_id: Some(customer.id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("STOREFRONT-ADJUSTMENT-1".to_string()),
                    title: "Storefront Adjusted Order".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("30.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "storefront-graphql-adjusted-order" }),
                }],
                adjustments: vec![rustok_order::dto::CreateOrderAdjustmentInput {
                    line_item_index: Some(0),
                    source_type: "Promotion".to_string(),
                    source_id: Some("promo-storefront".to_string()),
                    amount: Decimal::from_str("7.50").expect("valid decimal"),
                    metadata: serde_json::json!({
                        "rule_code": "storefront-adjustment",
                        "display_label": "Storefront promotion"
                    }),
                }],
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "storefront-graphql-adjusted-order" }),
            },
        )
        .await
        .expect("order should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        Some(customer_auth_context(tenant_id, owner_user_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              storefrontOrder(tenantId: "{tenant_id}", id: "{order_id}") {{
                id
                taxTotal
                taxIncluded
                taxLines {{
                  providerId
                }}
                subtotalAmount
                adjustmentTotal
                totalAmount
                lineItems {{
                  id
                }}
                adjustments {{
                  lineItemId
                  sourceType
                  sourceId
                  amount
                  currencyCode
                  metadata
                }}
              }}
            }}
            "#,
            order_id = order.id
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected storefront order adjustment GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(json["storefrontOrder"]["taxTotal"], Value::from("0"));
    assert_eq!(json["storefrontOrder"]["taxIncluded"], Value::from(false));
    assert_eq!(
        json["storefrontOrder"]["taxLines"]
            .as_array()
            .map(|items| items.len()),
        Some(0)
    );
    assert_eq!(json["storefrontOrder"]["subtotalAmount"], Value::from("30"));
    assert_eq!(
        json["storefrontOrder"]["adjustmentTotal"],
        Value::from("7.5")
    );
    assert_eq!(json["storefrontOrder"]["totalAmount"], Value::from("22.5"));
    assert_eq!(
        json["storefrontOrder"]["adjustments"][0]["lineItemId"],
        json["storefrontOrder"]["lineItems"][0]["id"]
    );
    assert_eq!(
        json["storefrontOrder"]["adjustments"][0]["sourceType"],
        Value::from("promotion")
    );
    assert_eq!(
        json["storefrontOrder"]["adjustments"][0]["sourceId"],
        Value::from("promo-storefront")
    );
    assert_eq!(
        json["storefrontOrder"]["adjustments"][0]["amount"],
        Value::from("7.5")
    );
    assert_eq!(
        json["storefrontOrder"]["adjustments"][0]["currencyCode"],
        Value::from("EUR")
    );
    let metadata: Value = serde_json::from_str(
        json["storefrontOrder"]["adjustments"][0]["metadata"]
            .as_str()
            .expect("order adjustment metadata should be JSON string"),
    )
    .expect("order adjustment metadata should parse");
    assert_eq!(metadata["rule_code"], Value::from("storefront-adjustment"));
    assert!(metadata.get("display_label").is_none());
}

#[tokio::test]
async fn storefront_graphql_order_query_rejects_foreign_customer_access() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let owner_user_id = Uuid::new_v4();
    let foreign_user_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let owner_customer = CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(owner_user_id),
                email: "owner@example.com".to_string(),
                first_name: Some("Owner".to_string()),
                last_name: None,
                phone: None,
                locale: Some("en".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-order-owner" }),
            },
        )
        .await
        .expect("owner customer should be created");
    CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(foreign_user_id),
                email: "foreign@example.com".to_string(),
                first_name: Some("Foreign".to_string()),
                last_name: None,
                phone: None,
                locale: Some("en".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-order-foreign" }),
            },
        )
        .await
        .expect("foreign customer should be created");

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            owner_user_id,
            CreateOrderInput {
                customer_id: Some(owner_customer.id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("FOREIGN-ORDER-1".to_string()),
                    title: "Foreign Guard".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("9.99").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "storefront-graphql-order-foreign" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "storefront-graphql-order-foreign" }),
            },
        )
        .await
        .expect("order should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(customer_auth_context(tenant_id, foreign_user_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              storefrontOrder(tenantId: "{tenant_id}", id: "{order_id}") {{
                id
              }}
            }}
            "#,
            order_id = order.id
        )))
        .await;

    assert_eq!(response.errors.len(), 1);
    assert_eq!(
        response.errors[0].message,
        "Order does not belong to the current customer"
    );
}

#[tokio::test]
async fn storefront_graphql_checkout_reuses_cart_payment_collection_for_guest_cart() {
    let (db, catalog, cart_service, _checkout, fulfillment) = setup_checkout().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .unwrap();
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .unwrap();
    let published_variant = published
        .variants
        .first()
        .expect("published product must have variant");

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
                metadata: serde_json::json!({ "source": "storefront-graphql-checkout" }),
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
                metadata: serde_json::json!({ "source": "storefront-graphql-checkout" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("guest@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "storefront-graphql-checkout" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(published.id),
                variant_id: Some(published_variant.id),
                shipping_profile_slug: None,
                sku: published_variant.sku.clone(),
                title: "Parity Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                metadata: serde_json::json!({ "source": "storefront-graphql-checkout" }),
            },
        )
        .await
        .unwrap();

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        None,
    );
    let response = schema
        .execute(Request::new(storefront_checkout_mutation(
            tenant_id, cart.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected storefront checkout GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["createStorefrontPaymentCollection"]["status"],
        Value::from("pending")
    );
    assert_eq!(
        json["completeStorefrontCheckout"]["cart"]["status"],
        Value::from("completed")
    );
    assert_eq!(
        json["completeStorefrontCheckout"]["order"]["status"],
        Value::from("paid")
    );
    assert_eq!(
        json["completeStorefrontCheckout"]["paymentCollection"]["status"],
        Value::from("captured")
    );
    assert_eq!(
        json["createStorefrontPaymentCollection"]["id"],
        json["completeStorefrontCheckout"]["paymentCollection"]["id"]
    );
    assert_eq!(
        json["completeStorefrontCheckout"]["fulfillment"]["status"],
        Value::from("pending")
    );
    assert_eq!(
        json["completeStorefrontCheckout"]["fulfillments"][0]["status"],
        Value::from("pending")
    );
    assert_eq!(
        json["completeStorefrontCheckout"]["cart"]["selectedShippingOptionId"],
        Value::from(shipping_option.id.to_string())
    );
    assert_eq!(
        json["completeStorefrontCheckout"]["cart"]["deliveryGroups"][0]["shippingProfileSlug"],
        Value::from("default")
    );
    assert_eq!(
        json["completeStorefrontCheckout"]["cart"]["deliveryGroups"][0]["selectedShippingOptionId"],
        Value::from(shipping_option.id.to_string())
    );
    assert_eq!(
        json["completeStorefrontCheckout"]["context"]["currencyCode"],
        Value::from("EUR")
    );
}

#[tokio::test]
async fn storefront_graphql_checkout_preserves_typed_adjustments_and_net_payment_amount() {
    let (db, catalog, cart_service, _checkout, fulfillment) = setup_checkout().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .unwrap();
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .unwrap();
    let published_variant = published
        .variants
        .first()
        .expect("published product must have variant");

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
                metadata: serde_json::json!({ "source": "storefront-graphql-adjustments" }),
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
                metadata: serde_json::json!({ "source": "storefront-graphql-adjustments" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("guest@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "storefront-graphql-adjustments" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(published.id),
                variant_id: Some(published_variant.id),
                shipping_profile_slug: None,
                sku: published_variant.sku.clone(),
                title: "Parity Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                metadata: serde_json::json!({ "source": "storefront-graphql-adjustments" }),
            },
        )
        .await
        .unwrap();
    let line_item_id = cart.line_items[0].id;
    cart_service
        .set_adjustments(
            tenant_id,
            cart.id,
            vec![SetCartAdjustmentInput {
                line_item_id: Some(line_item_id),
                source_type: "Promotion".to_string(),
                source_id: Some("promo-spring".to_string()),
                amount: Decimal::from_str("4.99").expect("valid decimal"),
                metadata: serde_json::json!({
                    "rule_code": "spring",
                    "display_label": "Spring sale"
                }),
            }],
        )
        .await
        .expect("cart adjustment should be stored");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        None,
    );
    let cart_query = schema
        .execute(Request::new(format!(
            r#"
            query {{
              storefrontCart(tenantId: "{tenant_id}", id: "{cart_id}") {{
                id
                subtotalAmount
                adjustmentTotal
                totalAmount
                adjustments {{
                  lineItemId
                  sourceType
                  sourceId
                  amount
                  currencyCode
                  metadata
                }}
              }}
            }}
            "#,
            cart_id = cart.id
        )))
        .await;
    assert!(
        cart_query.errors.is_empty(),
        "unexpected storefront cart adjustment errors: {:?}",
        cart_query.errors
    );
    let cart_json = cart_query
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        cart_json["storefrontCart"]["subtotalAmount"],
        Value::from("19.99")
    );
    assert_eq!(
        cart_json["storefrontCart"]["adjustmentTotal"],
        Value::from("4.99")
    );
    assert_eq!(
        cart_json["storefrontCart"]["totalAmount"],
        Value::from("24.99")
    );
    assert_eq!(
        cart_json["storefrontCart"]["adjustments"][0]["lineItemId"],
        Value::from(line_item_id.to_string())
    );
    assert_eq!(
        cart_json["storefrontCart"]["adjustments"][0]["sourceType"],
        Value::from("promotion")
    );
    assert_eq!(
        cart_json["storefrontCart"]["adjustments"][0]["sourceId"],
        Value::from("promo-spring")
    );
    assert_eq!(
        cart_json["storefrontCart"]["adjustments"][0]["amount"],
        Value::from("4.99")
    );
    let cart_adjustment_metadata: Value = serde_json::from_str(
        cart_json["storefrontCart"]["adjustments"][0]["metadata"]
            .as_str()
            .expect("cart adjustment metadata should be a JSON string"),
    )
    .expect("cart adjustment metadata should parse");
    assert_eq!(cart_adjustment_metadata["rule_code"], Value::from("spring"));
    assert!(cart_adjustment_metadata.get("display_label").is_none());

    let checkout_response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              createStorefrontPaymentCollection(
                tenantId: "{tenant_id}",
                input: {{
                  cartId: "{cart_id}"
                  metadata: "{{\"source\":\"storefront-graphql-adjustments\",\"step\":\"payment\"}}"
                }}
              ) {{
                id
                status
                amount
              }}
              completeStorefrontCheckout(
                tenantId: "{tenant_id}",
                input: {{
                  cartId: "{cart_id}"
                  createFulfillment: true
                  metadata: "{{\"source\":\"storefront-graphql-adjustments\",\"step\":\"complete\"}}"
                }}
              ) {{
                cart {{
                  id
                  status
                  subtotalAmount
                  adjustmentTotal
                  totalAmount
                  adjustments {{
                    sourceType
                    sourceId
                    amount
                    currencyCode
                    metadata
                  }}
                }}
                order {{
                  id
                  status
                  subtotalAmount
                  adjustmentTotal
                  totalAmount
                  lineItems {{
                    id
                  }}
                  adjustments {{
                    lineItemId
                    sourceType
                    sourceId
                    amount
                    currencyCode
                    metadata
                  }}
                }}
                paymentCollection {{
                  id
                  status
                  amount
                  authorizedAmount
                  capturedAmount
                }}
              }}
            }}
            "#,
            cart_id = cart.id
        )))
        .await;
    assert!(
        checkout_response.errors.is_empty(),
        "unexpected storefront checkout adjustment errors: {:?}",
        checkout_response.errors
    );
    let checkout_json = checkout_response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        checkout_json["createStorefrontPaymentCollection"]["amount"],
        Value::from("24.99")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["paymentCollection"]["amount"],
        Value::from("24.99")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["paymentCollection"]["authorizedAmount"],
        Value::from("24.99")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["paymentCollection"]["capturedAmount"],
        Value::from("24.99")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["cart"]["adjustmentTotal"],
        Value::from("4.99")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["cart"]["totalAmount"],
        Value::from("24.99")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["order"]["subtotalAmount"],
        Value::from("19.99")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["order"]["adjustmentTotal"],
        Value::from("4.99")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["order"]["totalAmount"],
        Value::from("24.99")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["order"]["adjustments"][0]["sourceType"],
        Value::from("promotion")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["order"]["adjustments"][0]["sourceId"],
        Value::from("promo-spring")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["order"]["adjustments"][0]["amount"],
        Value::from("4.99")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["order"]["adjustments"][0]["currencyCode"],
        Value::from("EUR")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["order"]["adjustments"][0]["lineItemId"],
        checkout_json["completeStorefrontCheckout"]["order"]["lineItems"][0]["id"]
    );
    let order_adjustment_metadata: Value = serde_json::from_str(
        checkout_json["completeStorefrontCheckout"]["order"]["adjustments"][0]["metadata"]
            .as_str()
            .expect("order adjustment metadata should be a JSON string"),
    )
    .expect("order adjustment metadata should parse");
    assert_eq!(
        order_adjustment_metadata["rule_code"],
        Value::from("spring")
    );
    assert!(order_adjustment_metadata.get("display_label").is_none());
}

#[tokio::test]
async fn storefront_graphql_checkout_preserves_shipping_total_and_shipping_promotion_amount() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let fulfillment = FulfillmentService::new(db.clone());
    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("product should be created");
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let published_variant = published
        .variants
        .first()
        .expect("published product must include a variant")
        .clone();
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
                metadata: serde_json::json!({ "source": "storefront-graphql-shipping-promotion" }),
            },
        )
        .await
        .unwrap();
    let cart_service = CartService::new(db.clone());
    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("guest@example.com".to_string()),
                region_id: None,
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "storefront-graphql-shipping-promotion" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(published.id),
                variant_id: Some(published_variant.id),
                shipping_profile_slug: None,
                sku: published_variant.sku.clone(),
                title: "Shipping Promotion Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                metadata: serde_json::json!({ "source": "storefront-graphql-shipping-promotion" }),
            },
        )
        .await
        .unwrap();
    cart_service
        .apply_fixed_shipping_promotion(
            tenant_id,
            cart.id,
            "promo-shipping-graphql",
            Decimal::from_str("4.99").expect("valid decimal"),
            serde_json::json!({
                "campaign": "shipping-half-off",
                "display_label": "Shipping half off"
            }),
        )
        .await
        .expect("shipping promotion should be stored");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        None,
    );
    let cart_query = schema
        .execute(Request::new(format!(
            r#"
            query {{
              storefrontCart(tenantId: "{tenant_id}", id: "{cart_id}") {{
                subtotalAmount
                shippingTotal
                adjustmentTotal
                totalAmount
                adjustments {{
                  lineItemId
                  sourceType
                  sourceId
                  amount
                  currencyCode
                  metadata
                }}
              }}
            }}
            "#,
            cart_id = cart.id
        )))
        .await;
    assert!(
        cart_query.errors.is_empty(),
        "unexpected storefront shipping promotion errors: {:?}",
        cart_query.errors
    );
    let cart_json = cart_query
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        cart_json["storefrontCart"]["subtotalAmount"],
        Value::from("19.99")
    );
    assert_eq!(
        cart_json["storefrontCart"]["shippingTotal"],
        Value::from("9.99")
    );
    assert_eq!(
        cart_json["storefrontCart"]["adjustmentTotal"],
        Value::from("4.99")
    );
    assert_eq!(
        cart_json["storefrontCart"]["totalAmount"],
        Value::from("24.99")
    );
    assert_eq!(
        cart_json["storefrontCart"]["adjustments"][0]["lineItemId"],
        Value::Null
    );
    let cart_adjustment_metadata: Value = serde_json::from_str(
        cart_json["storefrontCart"]["adjustments"][0]["metadata"]
            .as_str()
            .expect("cart adjustment metadata should be a JSON string"),
    )
    .expect("cart adjustment metadata should parse");
    assert_eq!(cart_adjustment_metadata["scope"], Value::from("shipping"));
    assert_eq!(
        cart_adjustment_metadata["campaign"],
        Value::from("shipping-half-off")
    );
    assert!(cart_adjustment_metadata.get("display_label").is_none());

    let checkout_response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              createStorefrontPaymentCollection(
                tenantId: "{tenant_id}",
                input: {{
                  cartId: "{cart_id}"
                  metadata: "{{\"source\":\"storefront-graphql-shipping-promotion\",\"step\":\"payment\"}}"
                }}
              ) {{
                amount
              }}
              completeStorefrontCheckout(
                tenantId: "{tenant_id}",
                input: {{
                  cartId: "{cart_id}"
                  createFulfillment: true
                  metadata: "{{\"source\":\"storefront-graphql-shipping-promotion\",\"step\":\"complete\"}}"
                }}
              ) {{
                cart {{
                  shippingTotal
                  adjustmentTotal
                  totalAmount
                }}
                order {{
                  shippingTotal
                  adjustmentTotal
                  totalAmount
                  adjustments {{
                    sourceType
                    sourceId
                    amount
                    metadata
                  }}
                }}
                paymentCollection {{
                  amount
                  capturedAmount
                }}
              }}
            }}
            "#,
            cart_id = cart.id
        )))
        .await;
    assert!(
        checkout_response.errors.is_empty(),
        "unexpected storefront shipping promotion checkout errors: {:?}",
        checkout_response.errors
    );
    let checkout_json = checkout_response
        .data
        .into_json()
        .expect("GraphQL checkout response must serialize");

    assert_eq!(
        checkout_json["createStorefrontPaymentCollection"]["amount"],
        Value::from("24.99")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["cart"]["shippingTotal"],
        Value::from("9.99")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["cart"]["adjustmentTotal"],
        Value::from("4.99")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["cart"]["totalAmount"],
        Value::from("24.99")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["order"]["shippingTotal"],
        Value::from("9.99")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["order"]["adjustmentTotal"],
        Value::from("4.99")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["order"]["totalAmount"],
        Value::from("24.99")
    );
    let order_adjustment_metadata: Value = serde_json::from_str(
        checkout_json["completeStorefrontCheckout"]["order"]["adjustments"][0]["metadata"]
            .as_str()
            .expect("order adjustment metadata should be a JSON string"),
    )
    .expect("order adjustment metadata should parse");
    assert_eq!(order_adjustment_metadata["scope"], Value::from("shipping"));
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["paymentCollection"]["amount"],
        Value::from("24.99")
    );
    assert_eq!(
        checkout_json["completeStorefrontCheckout"]["paymentCollection"]["capturedAmount"],
        Value::from("24.99")
    );
}

#[tokio::test]
async fn storefront_graphql_payment_collection_rejects_foreign_customer_cart_access() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let owner_user_id = Uuid::new_v4();
    let foreign_user_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let owner_customer = CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(owner_user_id),
                email: "owner-cart@example.com".to_string(),
                first_name: Some("Owner".to_string()),
                last_name: None,
                phone: None,
                locale: Some("en".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-payment-owner" }),
            },
        )
        .await
        .expect("owner customer should be created");
    CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(foreign_user_id),
                email: "foreign-cart@example.com".to_string(),
                first_name: Some("Foreign".to_string()),
                last_name: None,
                phone: None,
                locale: Some("en".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-payment-foreign" }),
            },
        )
        .await
        .expect("foreign customer should be created");

    let cart = CartService::new(db.clone())
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(owner_customer.id),
                email: Some("owner-cart@example.com".to_string()),
                region_id: None,
                country_code: Some("de".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: None,
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "storefront-graphql-payment-foreign" }),
            },
        )
        .await
        .expect("cart should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(customer_auth_context(tenant_id, foreign_user_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              createStorefrontPaymentCollection(
                tenantId: "{tenant_id}",
                input: {{ cartId: "{cart_id}" }}
              ) {{
                id
              }}
            }}
            "#,
            cart_id = cart.id
        )))
        .await;

    assert_eq!(response.errors.len(), 1);
    assert_eq!(
        response.errors[0].message,
        "Cart belongs to another customer"
    );
}

#[tokio::test]
async fn storefront_graphql_discovery_queries_follow_live_region_and_shipping_context_contract() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
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
                metadata: serde_json::json!({ "source": "storefront-graphql-discovery" }),
            },
        )
        .await
        .expect("region should be created");
    FulfillmentService::new(db.clone())
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "EUR Standard".to_string(),
                }],
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({ "source": "storefront-graphql-discovery" }),
            },
        )
        .await
        .expect("eur option should be created");
    FulfillmentService::new(db.clone())
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "USD Express".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("14.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({ "source": "storefront-graphql-discovery" }),
            },
        )
        .await
        .expect("usd option should be created");
    let cart = CartService::new(db.clone())
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("discovery@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: None,
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "storefront-graphql-discovery" }),
            },
        )
        .await
        .expect("cart should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        None,
    );
    let response = schema
        .execute(Request::new(storefront_discovery_query(tenant_id, cart.id)))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected storefront discovery GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["storefrontRegions"][0]["id"],
        Value::from(region.id.to_string())
    );
    assert_eq!(
        json["storefrontRegions"][0]["currencyCode"],
        Value::from("EUR")
    );
    assert_eq!(
        json["storefrontShippingOptions"],
        serde_json::json!([{
            "id": json["storefrontShippingOptions"][0]["id"].clone(),
            "name": "EUR Standard",
            "currencyCode": "EUR",
            "amount": "9.99"
        }])
    );
}
