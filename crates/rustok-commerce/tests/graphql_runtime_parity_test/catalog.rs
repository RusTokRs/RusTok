use super::*;

#[tokio::test]
async fn storefront_graphql_filters_channel_hidden_products() {
    let (db, catalog, _) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    seed_channel_binding(&db, tenant_id, channel_id, "web-store", true).await;

    let mut visible_input = create_product_input();
    visible_input.translations[0].title = "Visible Product".to_string();
    visible_input.translations[0].handle = Some(unique_slug("visible-storefront-product-en"));
    visible_input.translations[1].title = "Sichtbares Produkt".to_string();
    visible_input.translations[1].handle = Some(unique_slug("sichtbares-storefront-product-de"));
    visible_input.variants[0].sku = Some("GRAPHQL-VISIBLE-SKU-1".to_string());
    let visible = catalog
        .create_product(tenant_id, actor_id, visible_input)
        .await
        .expect("visible product should be created");
    let visible = catalog
        .publish_product(tenant_id, actor_id, visible.id)
        .await
        .expect("visible product should be published");
    let visible_handle = visible
        .translations
        .iter()
        .find(|translation| translation.locale == "de")
        .map(|translation| translation.handle.clone())
        .expect("visible product should have de handle");

    let mut hidden_input = create_product_input();
    hidden_input.translations[0].title = "Hidden Product".to_string();
    hidden_input.translations[0].handle = Some(unique_slug("hidden-storefront-product-en"));
    hidden_input.translations[1].title = "Verstecktes Produkt".to_string();
    hidden_input.translations[1].handle = Some(unique_slug("verstecktes-storefront-product-de"));
    hidden_input.variants[0].sku = Some("GRAPHQL-HIDDEN-SKU-1".to_string());
    hidden_input.metadata = serde_json::json!({
        "channel_visibility": {
            "allowed_channel_slugs": ["mobile-app"]
        }
    });
    let hidden = catalog
        .create_product(tenant_id, actor_id, hidden_input)
        .await
        .expect("hidden product should be created");
    let hidden = catalog
        .publish_product(tenant_id, actor_id, hidden.id)
        .await
        .expect("hidden product should be published");
    let hidden_handle = hidden
        .translations
        .iter()
        .find(|translation| translation.locale == "de")
        .map(|translation| translation.handle.clone())
        .expect("hidden product should have de handle");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context_with_channel(tenant_id, "de", channel_id, "web-store"),
        None,
    );

    let visible_query = format!(
        r#"
        query {{
          storefrontProducts(locale: "de") {{
            total
            items {{ title handle }}
          }}
          storefrontProduct(locale: "de", handle: "{visible_handle}") {{
            translations {{ locale title handle }}
          }}
        }}
        "#
    );
    let visible_response = schema.execute(Request::new(visible_query)).await;
    assert!(
        visible_response.errors.is_empty(),
        "unexpected GraphQL errors for visible product: {:?}",
        visible_response.errors
    );
    let visible_json = visible_response
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    assert_eq!(visible_json["storefrontProducts"]["total"], Value::from(1));
    assert_eq!(
        visible_json["storefrontProducts"]["items"][0]["title"],
        Value::from("Sichtbares Produkt")
    );
    assert_eq!(
        visible_json["storefrontProduct"]["translations"][0]["handle"],
        Value::from(visible_handle)
    );

    let hidden_query = format!(
        r#"
        query {{
          storefrontProduct(locale: "de", handle: "{hidden_handle}") {{
            id
          }}
        }}
        "#
    );
    let hidden_response = schema.execute(Request::new(hidden_query)).await;
    assert!(
        hidden_response.errors.is_empty(),
        "unexpected GraphQL errors for hidden product: {:?}",
        hidden_response.errors
    );
    let hidden_json = hidden_response
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    assert!(hidden_json["storefrontProduct"].is_null());
}

#[tokio::test]
async fn storefront_graphql_product_uses_channel_visible_inventory() {
    let (db, catalog, _) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    seed_channel_binding(&db, tenant_id, channel_id, "web-store", true).await;

    let mut input = create_product_input();
    input.translations[0].handle = Some(unique_slug("inventory-visible-product-en"));
    input.translations[1].handle = Some(unique_slug("inventory-visible-product-de"));
    input.variants[0].sku = Some("GRAPHQL-INVENTORY-SKU-1".to_string());
    input.variants[0].inventory_quantity = 8;
    let created = catalog
        .create_product(tenant_id, actor_id, input)
        .await
        .expect("product should be created");
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let handle = published
        .translations
        .iter()
        .find(|translation| translation.locale == "de")
        .map(|translation| translation.handle.clone())
        .expect("product should have de handle");

    set_stock_location_channel_visibility(&db, tenant_id, &["mobile-app"]).await;

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context_with_channel(tenant_id, "de", channel_id, "web-store"),
        None,
    );

    let query = format!(
        r#"
        query {{
          storefrontProduct(locale: "de", handle: "{handle}") {{
            variants {{
              sku
              inventoryQuantity
              inStock
            }}
          }}
        }}
        "#
    );
    let response = schema.execute(Request::new(query)).await;
    assert!(
        response.errors.is_empty(),
        "unexpected GraphQL errors for inventory visibility: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["storefrontProduct"]["variants"][0]["sku"],
        Value::from("GRAPHQL-INVENTORY-SKU-1")
    );
    assert_eq!(
        json["storefrontProduct"]["variants"][0]["inventoryQuantity"],
        Value::from(0)
    );
    assert_eq!(
        json["storefrontProduct"]["variants"][0]["inStock"],
        Value::from(false)
    );
}

#[tokio::test]
async fn storefront_graphql_rejects_disabled_channel_module() {
    let (db, _, _) = setup().await;
    let tenant_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    seed_channel_binding(&db, tenant_id, channel_id, "web-store", false).await;

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context_with_channel(tenant_id, "de", channel_id, "web-store"),
        None,
    );

    let mutation = r#"
        mutation {
          createStorefrontCart(
            input: {
              email: "buyer@example.com"
              currencyCode: "eur"
              locale: "de"
            }
          ) {
            cart { id }
          }
        }
    "#;
    let response = schema.execute(Request::new(mutation)).await;
    assert_eq!(response.errors.len(), 1, "expected module gate error");
    let error = &response.errors[0];
    assert!(
        error.message.contains("not enabled"),
        "unexpected error message: {}",
        error.message
    );
    assert!(matches!(
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code")),
        Some(async_graphql::Value::String(code)) if code == "MODULE_NOT_ENABLED"
    ));

    let query = r#"
        query {
          storefrontProduct(locale: "de", id: "00000000-0000-0000-0000-000000000000") {
            id
          }
        }
    "#;
    let query_response = schema.execute(Request::new(query)).await;
    assert_eq!(
        query_response.errors.len(),
        1,
        "expected module gate error for storefrontProduct"
    );
    let query_error = &query_response.errors[0];
    assert!(
        query_error.message.contains("not enabled"),
        "unexpected query error message: {}",
        query_error.message
    );
    assert!(matches!(
        query_error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code")),
        Some(async_graphql::Value::String(code)) if code == "MODULE_NOT_ENABLED"
    ));
}

#[tokio::test]
async fn legacy_catalog_read_path_is_stable_after_complete_checkout() {
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

    let before = serde_json::to_value(
        catalog
            .get_product(tenant_id, published.id)
            .await
            .expect("legacy catalog read path must resolve published product before checkout"),
    )
    .expect("product response must serialize");

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
                metadata: serde_json::json!({ "source": "legacy-checkout-parity" }),
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
                metadata: serde_json::json!({ "source": "legacy-checkout-parity" }),
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
                metadata: serde_json::json!({ "source": "legacy-checkout-parity" }),
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
                metadata: serde_json::json!({ "source": "legacy-checkout-parity" }),
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
                metadata: serde_json::json!({ "source": "legacy-checkout-parity" }),
            },
        )
        .await
        .unwrap();
    assert_eq!(completed.cart.status, "completed");
    assert_eq!(completed.order.status, "paid");

    let after = serde_json::to_value(
        catalog
            .get_product(tenant_id, published.id)
            .await
            .expect("legacy catalog read path must resolve published product after checkout"),
    )
    .expect("product response must serialize");

    assert_eq!(before, after);
    assert_eq!(
        after["translations"][0]["title"],
        Value::from("Parity Product")
    );
}
