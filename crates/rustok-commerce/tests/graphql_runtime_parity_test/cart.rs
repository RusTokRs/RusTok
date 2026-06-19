use super::*;

#[tokio::test]
async fn storefront_graphql_read_path_is_stable_after_cart_snapshot_creation() {
    let (db, catalog, cart_service) = setup().await;
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
        "unexpected GraphQL errors before cart snapshot: {:?}",
        before.errors
    );

    let products_before = before
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: Some(Uuid::new_v4()),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(Uuid::new_v4()),
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "graphql-parity-test" }),
            },
        )
        .await
        .unwrap();

    let after = schema
        .execute(Request::new(storefront_query(&handle)))
        .await;
    assert!(
        after.errors.is_empty(),
        "unexpected GraphQL errors after cart snapshot: {:?}",
        after.errors
    );

    let products_after = after
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(products_before, products_after);
    assert_eq!(
        products_after["storefrontProducts"]["total"],
        Value::from(1)
    );
    assert_eq!(
        products_after["storefrontProducts"]["items"][0]["title"],
        Value::from("Paritaet Produkt")
    );
}



#[tokio::test]
async fn admin_graphql_catalog_query_is_stable_after_cart_snapshot_creation() {
    let (db, catalog, cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .unwrap();

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
        "unexpected admin GraphQL errors before cart snapshot: {:?}",
        before.errors
    );
    let before_json = before
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: Some(Uuid::new_v4()),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(Uuid::new_v4()),
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "graphql-admin-parity-test" }),
            },
        )
        .await
        .unwrap();

    let after = schema.execute(Request::new(query)).await;
    assert!(
        after.errors.is_empty(),
        "unexpected admin GraphQL errors after cart snapshot: {:?}",
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
async fn storefront_graphql_cart_flow_creates_reads_updates_and_removes_line_items() {
    let (db, catalog, _cart_service) = setup().await;
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
        request_context(tenant_id, "de"),
        None,
    );

    let created_cart = schema
        .execute(Request::new(storefront_cart_flow_mutation(tenant_id)))
        .await;
    assert!(
        created_cart.errors.is_empty(),
        "unexpected create cart GraphQL errors: {:?}",
        created_cart.errors
    );
    let created_cart_json = created_cart
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    let cart_id = Uuid::parse_str(
        created_cart_json["createStorefrontCart"]["cart"]["id"]
            .as_str()
            .expect("cart id must be a string"),
    )
    .expect("cart id must parse");
    assert_eq!(
        created_cart_json["createStorefrontCart"]["context"]["currencyCode"],
        Value::from("EUR")
    );

    let added = schema
        .execute(Request::new(storefront_cart_add_line_item_mutation(
            tenant_id,
            cart_id,
            published_variant.id,
        )))
        .await;
    assert!(
        added.errors.is_empty(),
        "unexpected add line item GraphQL errors: {:?}",
        added.errors
    );
    let added_json = added
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    let line_id = Uuid::parse_str(
        added_json["addStorefrontCartLineItem"]["lineItems"][0]["id"]
            .as_str()
            .expect("line id must be a string"),
    )
    .expect("line id must parse");
    assert_eq!(
        added_json["addStorefrontCartLineItem"]["totalAmount"],
        Value::from("39.98")
    );

    let queried = schema
        .execute(Request::new(storefront_cart_query(tenant_id, cart_id)))
        .await;
    assert!(
        queried.errors.is_empty(),
        "unexpected cart query GraphQL errors: {:?}",
        queried.errors
    );
    let queried_json = queried
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    assert_eq!(
        queried_json["storefrontCart"]["lineItems"][0]["title"],
        Value::from("Paritaet Produkt / Default")
    );
    assert_eq!(
        queried_json["storefrontCart"]["lineItems"][0]["quantity"],
        Value::from(2)
    );

    let updated = schema
        .execute(Request::new(storefront_cart_update_line_item_mutation(
            tenant_id, cart_id, line_id,
        )))
        .await;
    assert!(
        updated.errors.is_empty(),
        "unexpected update line item GraphQL errors: {:?}",
        updated.errors
    );
    let updated_json = updated
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    assert_eq!(
        updated_json["updateStorefrontCartLineItem"]["totalAmount"],
        Value::from("59.97")
    );
    assert_eq!(
        updated_json["updateStorefrontCartLineItem"]["lineItems"][0]["quantity"],
        Value::from(3)
    );

    let removed = schema
        .execute(Request::new(storefront_cart_remove_line_item_mutation(
            tenant_id, cart_id, line_id,
        )))
        .await;
    assert!(
        removed.errors.is_empty(),
        "unexpected remove line item GraphQL errors: {:?}",
        removed.errors
    );
    let removed_json = removed
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    assert_eq!(
        removed_json["removeStorefrontCartLineItem"]["totalAmount"],
        Value::from("0")
    );
    assert_eq!(
        removed_json["removeStorefrontCartLineItem"]["lineItems"],
        serde_json::json!([])
    );
}



#[tokio::test]
async fn storefront_graphql_cart_query_rejects_foreign_customer_access() {
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
                email: "owner-query@example.com".to_string(),
                first_name: Some("Owner".to_string()),
                last_name: None,
                phone: None,
                locale: Some("en".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-cart-owner" }),
            },
        )
        .await
        .expect("owner customer should be created");
    CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(foreign_user_id),
                email: "foreign-query@example.com".to_string(),
                first_name: Some("Foreign".to_string()),
                last_name: None,
                phone: None,
                locale: Some("en".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-cart-foreign" }),
            },
        )
        .await
        .expect("foreign customer should be created");
    let cart = CartService::new(db.clone())
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(owner_customer.id),
                email: Some("owner-query@example.com".to_string()),
                region_id: None,
                country_code: Some("de".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: None,
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "storefront-graphql-cart-foreign" }),
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
        .execute(Request::new(storefront_cart_query(tenant_id, cart.id)))
        .await;

    assert_eq!(response.errors.len(), 1);
    assert_eq!(
        response.errors[0].message,
        "Cart belongs to another customer"
    );
}



#[tokio::test]
async fn storefront_graphql_cart_context_patch_keeps_tristate_semantics() {
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
                metadata: serde_json::json!({ "source": "storefront-graphql-cart-context" }),
            },
        )
        .await
        .expect("region should be created");
    let shipping_option = FulfillmentService::new(db.clone())
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
                metadata: serde_json::json!({ "source": "storefront-graphql-cart-context" }),
            },
        )
        .await
        .expect("shipping option should be created");
    let cart = CartService::new(db.clone())
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("context@example.com".to_string()),
                region_id: None,
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: None,
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "storefront-graphql-cart-context" }),
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
        .execute(Request::new(storefront_cart_context_update_mutation(
            tenant_id,
            cart.id,
            region.id,
            shipping_option.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected cart context patch GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["updateStorefrontCartContext"]["cart"]["email"],
        Value::Null
    );
    assert_eq!(
        json["updateStorefrontCartContext"]["cart"]["regionId"],
        Value::from(region.id.to_string())
    );
    assert_eq!(
        json["updateStorefrontCartContext"]["cart"]["countryCode"],
        Value::Null
    );
    assert_eq!(
        json["updateStorefrontCartContext"]["cart"]["selectedShippingOptionId"],
        Value::Null
    );
    assert_eq!(
        json["updateStorefrontCartContext"]["context"]["region"]["id"],
        Value::from(region.id.to_string())
    );
    assert_eq!(
        json["updateStorefrontCartContext"]["context"]["currencyCode"],
        Value::from("EUR")
    );
}

