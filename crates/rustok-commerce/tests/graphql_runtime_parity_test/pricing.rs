use super::*;

#[tokio::test]
async fn storefront_graphql_pricing_helpers_respect_explicit_channel_override() {
    let (db, _catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let web_channel_id = Uuid::new_v4();
    let mobile_channel_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    seed_channel_binding(&db, tenant_id, web_channel_id, "web-store", true).await;
    seed_channel_binding(&db, tenant_id, mobile_channel_id, "mobile-app", true).await;

    let global_list_id =
        seed_active_price_list(&db, tenant_id, "Global Sale", None, None, Some("12.5")).await;
    let web_list_id = seed_active_price_list(
        &db,
        tenant_id,
        "Web Sale",
        Some(web_channel_id),
        Some("web-store"),
        None,
    )
    .await;
    let mobile_list_id = seed_active_price_list(
        &db,
        tenant_id,
        "Mobile Sale",
        Some(mobile_channel_id),
        Some("mobile-app"),
        None,
    )
    .await;

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context_with_channel(tenant_id, "de", web_channel_id, "web-store"),
        None,
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              storefrontPricingChannels {{
                id
                slug
                name
              }}
              requestScoped: storefrontActivePriceLists {{
                id
                channelSlug
                adjustmentPercent
              }}
              explicitMobile: storefrontActivePriceLists(
                channelId: "{mobile_channel_id}",
                channelSlug: "mobile-app"
              ) {{
                id
                channelSlug
              }}
            }}
            "#
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected storefront pricing helper GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    let channel_slugs = json["storefrontPricingChannels"]
        .as_array()
        .expect("channels should be an array")
        .iter()
        .filter_map(|item| item["slug"].as_str().map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    assert!(channel_slugs.contains(&"web-store".to_string()));
    assert!(channel_slugs.contains(&"mobile-app".to_string()));

    let request_scoped_ids = json["requestScoped"]
        .as_array()
        .expect("request-scoped lists should be an array")
        .iter()
        .filter_map(|item| item["id"].as_str().map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    assert!(request_scoped_ids.contains(&global_list_id.to_string()));
    assert!(request_scoped_ids.contains(&web_list_id.to_string()));
    assert!(!request_scoped_ids.contains(&mobile_list_id.to_string()));

    let explicit_mobile_ids = json["explicitMobile"]
        .as_array()
        .expect("explicit mobile lists should be an array")
        .iter()
        .filter_map(|item| item["id"].as_str().map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    assert!(explicit_mobile_ids.contains(&global_list_id.to_string()));
    assert!(explicit_mobile_ids.contains(&mobile_list_id.to_string()));
    assert!(!explicit_mobile_ids.contains(&web_list_id.to_string()));

    let global_rule = json["requestScoped"]
        .as_array()
        .expect("request-scoped lists should be an array")
        .iter()
        .find(|item| item["id"] == global_list_id.to_string())
        .expect("global list should be present");
    assert_eq!(global_rule["adjustmentPercent"], Value::from("12.5"));
}

#[tokio::test]
async fn storefront_graphql_active_price_lists_clear_rule_metadata_without_stale_state() {
    let (db, _catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let price_list_id =
        seed_active_price_list(&db, tenant_id, "Global Sale", None, None, Some("12.5")).await;

    PricingService::new(db.clone(), mock_transactional_event_bus())
        .set_price_list_percentage_rule(tenant_id, actor_id, price_list_id, None)
        .await
        .expect("rule clear should succeed");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        None,
    );
    let response = schema
        .execute(Request::new(
            r#"
            query {
              storefrontActivePriceLists {
                id
                ruleKind
                adjustmentPercent
              }
            }
            "#,
        ))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected storefront active price lists GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    let option = json["storefrontActivePriceLists"]
        .as_array()
        .expect("price lists should be an array")
        .iter()
        .find(|item| item["id"] == price_list_id.to_string())
        .expect("cleared price list should stay visible");

    assert_eq!(option["ruleKind"], Value::Null);
    assert_eq!(option["adjustmentPercent"], Value::Null);
}

#[tokio::test]
async fn storefront_graphql_active_price_lists_respect_scope_update_boundary() {
    let (db, _catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let web_channel_id = Uuid::new_v4();
    let mobile_channel_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    seed_channel_binding(&db, tenant_id, web_channel_id, "web-store", true).await;
    seed_channel_binding(&db, tenant_id, mobile_channel_id, "mobile-app", true).await;
    let price_list_id =
        seed_active_price_list(&db, tenant_id, "Movable Sale", None, None, Some("10")).await;

    PricingService::new(db.clone(), mock_transactional_event_bus())
        .set_price_list_scope(
            tenant_id,
            actor_id,
            price_list_id,
            Some(web_channel_id),
            Some("web-store".to_string()),
        )
        .await
        .expect("scope update should succeed");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context_with_channel(tenant_id, "de", web_channel_id, "web-store"),
        None,
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              requestScoped: storefrontActivePriceLists {{
                id
                channelSlug
              }}
              explicitMobile: storefrontActivePriceLists(
                channelId: "{mobile_channel_id}",
                channelSlug: "mobile-app"
              ) {{
                id
              }}
            }}
            "#
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected storefront scope-update GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert!(
        json["requestScoped"]
            .as_array()
            .expect("request-scoped lists should be an array")
            .iter()
            .any(|item| item["id"] == price_list_id.to_string()
                && item["channelSlug"] == "web-store"),
        "updated list should be visible in matching channel scope"
    );
    assert!(
        !json["explicitMobile"]
            .as_array()
            .expect("mobile lists should be an array")
            .iter()
            .any(|item| item["id"] == price_list_id.to_string()),
        "updated list should not leak into a different channel scope"
    );
}

#[tokio::test]
async fn admin_graphql_pricing_product_applies_price_list_rule_without_override() {
    let (db, catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("product should be created");
    let variant = created
        .variants
        .first()
        .expect("product should include a variant");
    let price_list_id = seed_active_price_list(&db, tenant_id, "Rule Sale", None, None, None).await;

    let pricing = PricingService::new(db.clone(), mock_transactional_event_bus());
    pricing
        .set_price(
            tenant_id,
            actor_id,
            variant.id,
            "EUR",
            Decimal::from_str("20.00").unwrap(),
            None,
        )
        .await
        .expect("base price should be updated");
    pricing
        .set_price_list_percentage_rule(
            tenant_id,
            actor_id,
            price_list_id,
            Some(Decimal::from_str("15").unwrap()),
        )
        .await
        .expect("rule should be stored");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              adminPricingProduct(
                tenantId: "{tenant_id}",
                id: "{product_id}",
                locale: "en",
                currencyCode: "EUR",
                priceListId: "{price_list_id}",
                quantity: 1
              ) {{
                variants {{
                  effectivePrice {{
                    amount
                    compareAtAmount
                    discountPercent
                    priceListId
                    onSale
                  }}
                }}
              }}
            }}
            "#,
            product_id = created.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected admin rule-driven pricing GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["amount"],
        Value::from("17")
    );
    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["compareAtAmount"],
        Value::from("20")
    );
    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["discountPercent"],
        Value::from("15")
    );
    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["priceListId"],
        Value::from(price_list_id.to_string())
    );
    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["onSale"],
        Value::from(true)
    );
}

#[tokio::test]
async fn admin_graphql_pricing_product_prefers_explicit_override_over_price_list_rule() {
    let (db, catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("product should be created");
    let variant = created
        .variants
        .first()
        .expect("product should include a variant");
    let price_list_id = seed_active_price_list(&db, tenant_id, "Rule Sale", None, None, None).await;

    let pricing = PricingService::new(db.clone(), mock_transactional_event_bus());
    pricing
        .set_price(
            tenant_id,
            actor_id,
            variant.id,
            "EUR",
            Decimal::from_str("20.00").unwrap(),
            None,
        )
        .await
        .expect("base price should be updated");
    pricing
        .set_price_list_percentage_rule(
            tenant_id,
            actor_id,
            price_list_id,
            Some(Decimal::from_str("15").unwrap()),
        )
        .await
        .expect("rule should be stored");
    pricing
        .set_price_list_tier(
            tenant_id,
            actor_id,
            variant.id,
            price_list_id,
            "EUR",
            Decimal::from_str("14.00").unwrap(),
            Some(Decimal::from_str("20.00").unwrap()),
            None,
            None,
        )
        .await
        .expect("override row should be stored");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              adminPricingProduct(
                tenantId: "{tenant_id}",
                id: "{product_id}",
                locale: "en",
                currencyCode: "EUR",
                priceListId: "{price_list_id}",
                quantity: 1
              ) {{
                variants {{
                  effectivePrice {{
                    amount
                    compareAtAmount
                    discountPercent
                    priceListId
                  }}
                }}
              }}
            }}
            "#,
            product_id = created.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected admin override precedence GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["amount"],
        Value::from("14")
    );
    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["compareAtAmount"],
        Value::from("20")
    );
    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["discountPercent"],
        Value::from("30")
    );
    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["priceListId"],
        Value::from(price_list_id.to_string())
    );
}

#[tokio::test]
async fn admin_graphql_pricing_product_resolves_effective_price_for_explicit_channel() {
    let (db, catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("product should be created");
    let variant = created
        .variants
        .first()
        .expect("product should include a variant");
    PricingService::new(db.clone(), mock_transactional_event_bus())
        .set_prices(
            tenant_id,
            actor_id,
            variant.id,
            vec![PriceInput {
                currency_code: "EUR".to_string(),
                channel_id: Some(channel_id),
                channel_slug: Some("web-store".to_string()),
                amount: Decimal::from_str("15.99").expect("valid decimal"),
                compare_at_amount: Some(Decimal::from_str("19.99").expect("valid decimal")),
            }],
        )
        .await
        .expect("channel-scoped price should be stored");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              adminPricingProduct(
                tenantId: "{tenant_id}",
                id: "{product_id}",
                locale: "en",
                currencyCode: "EUR",
                channelId: "{channel_id}",
                channelSlug: "web-store",
                quantity: 1
              ) {{
                id
                variants {{
                  id
                  prices {{
                    currencyCode
                    amount
                    channelId
                    channelSlug
                  }}
                  effectivePrice {{
                    currencyCode
                    amount
                    compareAtAmount
                    onSale
                    channelId
                    channelSlug
                  }}
                }}
              }}
            }}
            "#,
            product_id = created.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected admin pricing product GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    let prices = json["adminPricingProduct"]["variants"][0]["prices"]
        .as_array()
        .expect("prices should be an array");
    assert!(prices.iter().any(|item| {
        item["channelId"] == channel_id.to_string()
            && item["channelSlug"] == "web-store"
            && item["amount"] == "15.99"
    }));

    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["amount"],
        Value::from("15.99")
    );
    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["channelId"],
        Value::from(channel_id.to_string())
    );
    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["channelSlug"],
        Value::from("web-store")
    );
    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["compareAtAmount"],
        Value::from("19.99")
    );
    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["onSale"],
        Value::from(true)
    );
}

#[tokio::test]
async fn admin_graphql_pricing_product_keeps_compare_at_without_sale_semantics_when_amount_matches()
{
    let (db, catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("product should be created");
    let variant = created
        .variants
        .first()
        .expect("product should include a variant");
    PricingService::new(db.clone(), mock_transactional_event_bus())
        .set_price(
            tenant_id,
            actor_id,
            variant.id,
            "EUR",
            Decimal::from_str("19.99").expect("valid decimal"),
            Some(Decimal::from_str("19.99").expect("valid decimal")),
        )
        .await
        .expect("price should be updated");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              adminPricingProduct(
                tenantId: "{tenant_id}",
                id: "{product_id}",
                locale: "en",
                currencyCode: "EUR",
                quantity: 1
              ) {{
                variants {{
                  effectivePrice {{
                    amount
                    compareAtAmount
                    discountPercent
                    onSale
                  }}
                }}
              }}
            }}
            "#,
            product_id = created.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected admin compare-at parity GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["amount"],
        Value::from("19.99")
    );
    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["compareAtAmount"],
        Value::from("19.99")
    );
    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["discountPercent"],
        Value::Null
    );
    assert_eq!(
        json["adminPricingProduct"]["variants"][0]["effectivePrice"]["onSale"],
        Value::from(false)
    );
}

#[tokio::test]
async fn storefront_graphql_pricing_product_applies_channel_scoped_rule_only_for_matching_context()
{
    let (db, catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let web_channel_id = Uuid::new_v4();
    let mobile_channel_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    seed_channel_binding(&db, tenant_id, web_channel_id, "web-store", true).await;
    seed_channel_binding(&db, tenant_id, mobile_channel_id, "mobile-app", true).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("product should be created");
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let handle = published
        .translations
        .iter()
        .find(|item| item.locale == "en")
        .map(|item| item.handle.clone())
        .expect("published product should expose an English handle");
    let variant = published
        .variants
        .first()
        .expect("published product should include a variant");
    let price_list_id = seed_active_price_list(
        &db,
        tenant_id,
        "Web Rule Sale",
        Some(web_channel_id),
        Some("web-store"),
        None,
    )
    .await;

    let pricing = PricingService::new(db.clone(), mock_transactional_event_bus());
    pricing
        .set_price(
            tenant_id,
            actor_id,
            variant.id,
            "EUR",
            Decimal::from_str("20.00").unwrap(),
            None,
        )
        .await
        .expect("base price should be updated");
    pricing
        .set_price_list_percentage_rule(
            tenant_id,
            actor_id,
            price_list_id,
            Some(Decimal::from_str("12.5").unwrap()),
        )
        .await
        .expect("rule should be stored");

    let web_schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context_with_channel(tenant_id, "en", web_channel_id, "web-store"),
        None,
    );
    let web_response = web_schema
        .execute(Request::new(format!(
            r#"
            query {{
              storefrontPricingProduct(
                tenantId: "{tenant_id}",
                handle: "{handle}",
                locale: "en",
                currencyCode: "EUR",
                priceListId: "{price_list_id}",
                quantity: 1
              ) {{
                variants {{
                  effectivePrice {{
                    amount
                    compareAtAmount
                    discountPercent
                    priceListId
                    channelSlug
                  }}
                }}
              }}
            }}
            "#
        )))
        .await;
    assert!(
        web_response.errors.is_empty(),
        "unexpected storefront matching-channel GraphQL errors: {:?}",
        web_response.errors
    );
    let web_json = web_response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        web_json["storefrontPricingProduct"]["variants"][0]["effectivePrice"]["amount"],
        Value::from("17.5")
    );
    assert_eq!(
        web_json["storefrontPricingProduct"]["variants"][0]["effectivePrice"]["channelSlug"],
        Value::Null
    );

    let mobile_schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context_with_channel(tenant_id, "en", mobile_channel_id, "mobile-app"),
        None,
    );
    let mobile_response = mobile_schema
        .execute(Request::new(format!(
            r#"
            query {{
              storefrontPricingProduct(
                tenantId: "{tenant_id}",
                handle: "{handle}",
                locale: "en",
                currencyCode: "EUR",
                quantity: 1
              ) {{
                variants {{
                  effectivePrice {{
                    amount
                    priceListId
                    channelSlug
                  }}
                }}
              }}
            }}
            "#
        )))
        .await;
    assert!(
        mobile_response.errors.is_empty(),
        "unexpected storefront non-matching-channel GraphQL errors: {:?}",
        mobile_response.errors
    );
    let mobile_json = mobile_response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        mobile_json["storefrontPricingProduct"]["variants"][0]["effectivePrice"]["amount"],
        Value::from("20")
    );
    assert_eq!(
        mobile_json["storefrontPricingProduct"]["variants"][0]["effectivePrice"]["priceListId"],
        Value::Null
    );
    assert_eq!(
        mobile_json["storefrontPricingProduct"]["variants"][0]["effectivePrice"]["channelSlug"],
        Value::Null
    );
}

#[tokio::test]
async fn admin_graphql_update_pricing_variant_price_returns_written_row() {
    let (db, catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("product should be created");
    let variant = created
        .variants
        .first()
        .expect("product should include a variant");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(pricing_update_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              updateAdminPricingVariantPrice(
                tenantId: "{tenant_id}",
                variantId: "{variant_id}",
                input: {{
                  currencyCode: "usd",
                  amount: "79.00",
                  compareAtAmount: "100.00",
                  channelSlug: "WEB-STORE"
                }}
              ) {{
                currencyCode
                amount
                compareAtAmount
                discountPercent
                onSale
                channelSlug
              }}
            }}
            "#,
            variant_id = variant.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected update pricing GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["updateAdminPricingVariantPrice"]["currencyCode"],
        Value::from("USD")
    );
    assert_eq!(
        json["updateAdminPricingVariantPrice"]["amount"],
        Value::from("79")
    );
    assert_eq!(
        json["updateAdminPricingVariantPrice"]["compareAtAmount"],
        Value::from("100")
    );
    assert_eq!(
        json["updateAdminPricingVariantPrice"]["discountPercent"],
        Value::from("21")
    );
    assert_eq!(
        json["updateAdminPricingVariantPrice"]["onSale"],
        Value::from(true)
    );
    assert_eq!(
        json["updateAdminPricingVariantPrice"]["channelSlug"],
        Value::from("web-store")
    );
}

#[tokio::test]
async fn admin_graphql_update_pricing_variant_price_supports_price_list_tier_scope() {
    let (db, catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("product should be created");
    let variant = created
        .variants
        .first()
        .expect("product should include a variant");
    let price_list_id = seed_active_price_list(
        &db,
        tenant_id,
        "Tiered Sale",
        Some(channel_id),
        Some("web-store"),
        None,
    )
    .await;

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(pricing_update_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              updateAdminPricingVariantPrice(
                tenantId: "{tenant_id}",
                variantId: "{variant_id}",
                input: {{
                  currencyCode: "eur",
                  amount: "14.50",
                  compareAtAmount: "19.99",
                  priceListId: "{price_list_id}",
                  channelId: "{channel_id}",
                  channelSlug: "WEB-STORE",
                  minQuantity: 5,
                  maxQuantity: 9
                }}
              ) {{
                currencyCode
                amount
                compareAtAmount
                discountPercent
                onSale
                priceListId
                channelId
                channelSlug
                minQuantity
                maxQuantity
              }}
            }}
            "#,
            variant_id = variant.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected price-list tier GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["updateAdminPricingVariantPrice"]["currencyCode"],
        Value::from("EUR")
    );
    assert_eq!(
        json["updateAdminPricingVariantPrice"]["amount"],
        Value::from("14.5")
    );
    assert_eq!(
        json["updateAdminPricingVariantPrice"]["compareAtAmount"],
        Value::from("19.99")
    );
    assert_eq!(
        json["updateAdminPricingVariantPrice"]["discountPercent"],
        Value::from("27.46")
    );
    assert_eq!(
        json["updateAdminPricingVariantPrice"]["onSale"],
        Value::from(true)
    );
    assert_eq!(
        json["updateAdminPricingVariantPrice"]["priceListId"],
        Value::from(price_list_id.to_string())
    );
    assert_eq!(
        json["updateAdminPricingVariantPrice"]["channelId"],
        Value::from(channel_id.to_string())
    );
    assert_eq!(
        json["updateAdminPricingVariantPrice"]["channelSlug"],
        Value::from("web-store")
    );
    assert_eq!(
        json["updateAdminPricingVariantPrice"]["minQuantity"],
        Value::from(5)
    );
    assert_eq!(
        json["updateAdminPricingVariantPrice"]["maxQuantity"],
        Value::from(9)
    );

    let persisted = PricingService::new(db.clone(), mock_transactional_event_bus())
        .get_variant_prices(variant.id)
        .await
        .expect("variant prices should load")
        .into_iter()
        .find(|price| {
            price.price_list_id == Some(price_list_id)
                && price.channel_id == Some(channel_id)
                && price.channel_slug.as_deref() == Some("web-store")
                && price.min_quantity == Some(5)
                && price.max_quantity == Some(9)
        })
        .expect("scoped price-list tier should persist");
    assert_eq!(persisted.amount, Decimal::from_str("14.50").unwrap());
    assert_eq!(
        persisted.compare_at_amount,
        Some(Decimal::from_str("19.99").unwrap())
    );
}

#[tokio::test]
async fn admin_graphql_update_pricing_variant_price_rejects_price_list_scope_mismatch() {
    let (db, catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let web_channel_id = Uuid::new_v4();
    let mobile_channel_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("product should be created");
    let variant = created
        .variants
        .first()
        .expect("product should include a variant");
    let price_list_id = seed_active_price_list(
        &db,
        tenant_id,
        "Tiered Sale",
        Some(web_channel_id),
        Some("web-store"),
        None,
    )
    .await;

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(pricing_update_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              updateAdminPricingVariantPrice(
                tenantId: "{tenant_id}",
                variantId: "{variant_id}",
                input: {{
                  currencyCode: "eur",
                  amount: "14.50",
                  compareAtAmount: "19.99",
                  priceListId: "{price_list_id}",
                  channelId: "{mobile_channel_id}",
                  channelSlug: "mobile-app",
                  minQuantity: 5,
                  maxQuantity: 9
                }}
              ) {{
                currencyCode
              }}
            }}
            "#,
            variant_id = variant.id,
        )))
        .await;

    assert_eq!(response.errors.len(), 1);
    assert!(response.errors[0].message.contains(
        "price rows for a selected price_list_id must match the price list channel scope"
    ));

    let scoped_override = PricingService::new(db.clone(), mock_transactional_event_bus())
        .get_variant_prices(variant.id)
        .await
        .expect("variant prices should load")
        .into_iter()
        .find(|price| price.price_list_id == Some(price_list_id));
    assert!(scoped_override.is_none());
}

#[tokio::test]
async fn admin_graphql_preview_pricing_variant_discount_returns_typed_preview() {
    let (db, catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("product should be created");
    let variant = created
        .variants
        .first()
        .expect("product should include a variant");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(pricing_update_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              previewAdminPricingVariantDiscount(
                tenantId: "{tenant_id}",
                variantId: "{variant_id}",
                input: {{
                  currencyCode: "eur",
                  discountPercent: "10"
                }}
              ) {{
                kind
                currencyCode
                currentAmount
                baseAmount
                adjustedAmount
                adjustmentPercent
                compareAtAmount
              }}
            }}
            "#,
            variant_id = variant.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected preview pricing GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["previewAdminPricingVariantDiscount"]["kind"],
        Value::from("percentage_discount")
    );
    assert_eq!(
        json["previewAdminPricingVariantDiscount"]["currencyCode"],
        Value::from("EUR")
    );
    assert_eq!(
        json["previewAdminPricingVariantDiscount"]["currentAmount"],
        Value::from("19.99")
    );
    assert_eq!(
        json["previewAdminPricingVariantDiscount"]["baseAmount"],
        Value::from("19.99")
    );
    assert_eq!(
        json["previewAdminPricingVariantDiscount"]["adjustedAmount"],
        Value::from("17.99")
    );
    assert_eq!(
        json["previewAdminPricingVariantDiscount"]["adjustmentPercent"],
        Value::from("10")
    );
    assert_eq!(
        json["previewAdminPricingVariantDiscount"]["compareAtAmount"],
        Value::from("19.99")
    );
}

#[tokio::test]
async fn admin_graphql_apply_pricing_variant_discount_updates_base_row() {
    let (db, catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("product should be created");
    let variant = created
        .variants
        .first()
        .expect("product should include a variant");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(pricing_update_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              applyAdminPricingVariantDiscount(
                tenantId: "{tenant_id}",
                variantId: "{variant_id}",
                input: {{
                  currencyCode: "eur",
                  discountPercent: "10"
                }}
              ) {{
                kind
                adjustedAmount
                compareAtAmount
              }}
            }}
            "#,
            variant_id = variant.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected apply pricing GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["applyAdminPricingVariantDiscount"]["kind"],
        Value::from("percentage_discount")
    );
    assert_eq!(
        json["applyAdminPricingVariantDiscount"]["adjustedAmount"],
        Value::from("17.99")
    );
    assert_eq!(
        json["applyAdminPricingVariantDiscount"]["compareAtAmount"],
        Value::from("19.99")
    );

    let updated = PricingService::new(db.clone(), mock_transactional_event_bus())
        .get_price(variant.id, "EUR")
        .await
        .expect("updated price should load");
    assert_eq!(updated, Some(Decimal::from_str("17.99").unwrap()));
}

#[tokio::test]
async fn admin_graphql_preview_and_apply_cart_shipping_promotion() {
    let (db, catalog, cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("product should be created");
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let variant = published
        .variants
        .first()
        .expect("published product should include a variant")
        .clone();
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
                metadata: serde_json::json!({ "source": "graphql-admin-cart-promotion" }),
            },
        )
        .await
        .expect("shipping option should be created");
    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("admin-preview@example.com".to_string()),
                region_id: None,
                country_code: Some("de".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "graphql-admin-cart-promotion" }),
            },
        )
        .await
        .expect("cart should be created");
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(published.id),
                variant_id: Some(variant.id),
                shipping_profile_slug: None,
                sku: variant.sku.clone(),
                title: "Admin Cart Promotion Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                metadata: serde_json::json!({ "source": "graphql-admin-cart-promotion" }),
            },
        )
        .await
        .expect("line item should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_order_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              previewAdminCartPromotion(
                tenantId: "{tenant_id}",
                cartId: "{cart_id}",
                input: {{
                  kind: PERCENTAGE_DISCOUNT
                  scope: SHIPPING
                  sourceId: "promo-admin-cart"
                  discountPercent: "50"
                }}
              ) {{
                kind
                scope
                lineItemId
                currencyCode
                baseAmount
                adjustmentAmount
                adjustedAmount
              }}
              applyAdminCartPromotion(
                tenantId: "{tenant_id}",
                cartId: "{cart_id}",
                input: {{
                  kind: FIXED_DISCOUNT
                  scope: SHIPPING
                  sourceId: "promo-admin-cart"
                  amount: "4.99"
                  metadata: "{{\"campaign\":\"admin-shipping\"}}"
                }}
              ) {{
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
            cart_id = cart.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected admin cart promotion GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["previewAdminCartPromotion"]["kind"],
        Value::from("percentage_discount")
    );
    assert_eq!(
        json["previewAdminCartPromotion"]["scope"],
        Value::from("shipping")
    );
    assert_eq!(json["previewAdminCartPromotion"]["lineItemId"], Value::Null);
    assert_eq!(
        json["previewAdminCartPromotion"]["currencyCode"],
        Value::from("EUR")
    );
    assert_eq!(
        json["previewAdminCartPromotion"]["baseAmount"],
        Value::from("9.99")
    );
    assert_eq!(
        json["previewAdminCartPromotion"]["adjustmentAmount"],
        Value::from("4.99")
    );
    assert_eq!(
        json["previewAdminCartPromotion"]["adjustedAmount"],
        Value::from("5.00")
    );

    assert_eq!(
        json["applyAdminCartPromotion"]["shippingTotal"],
        Value::from("9.99")
    );
    assert_eq!(
        json["applyAdminCartPromotion"]["adjustmentTotal"],
        Value::from("4.99")
    );
    assert_eq!(
        json["applyAdminCartPromotion"]["totalAmount"],
        Value::from("24.99")
    );
    assert_eq!(
        json["applyAdminCartPromotion"]["adjustments"][0]["lineItemId"],
        Value::Null
    );
    assert_eq!(
        json["applyAdminCartPromotion"]["adjustments"][0]["sourceType"],
        Value::from("promotion")
    );
    assert_eq!(
        json["applyAdminCartPromotion"]["adjustments"][0]["sourceId"],
        Value::from("promo-admin-cart")
    );
    assert_eq!(
        json["applyAdminCartPromotion"]["adjustments"][0]["amount"],
        Value::from("4.99")
    );
    assert_eq!(
        json["applyAdminCartPromotion"]["adjustments"][0]["currencyCode"],
        Value::from("EUR")
    );
    let metadata: Value = serde_json::from_str(
        json["applyAdminCartPromotion"]["adjustments"][0]["metadata"]
            .as_str()
            .expect("adjustment metadata should be JSON string"),
    )
    .expect("adjustment metadata should parse");
    assert_eq!(metadata["scope"], Value::from("shipping"));
    assert_eq!(metadata["campaign"], Value::from("admin-shipping"));
    assert!(metadata.get("display_label").is_none());
}

#[tokio::test]
async fn admin_graphql_preview_cart_promotion_rejects_missing_line_item_target() {
    let (db, _catalog, cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("admin-invalid@example.com".to_string()),
                region_id: None,
                country_code: Some("de".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: None,
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "graphql-admin-cart-promotion-invalid" }),
            },
        )
        .await
        .expect("cart should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_order_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              previewAdminCartPromotion(
                tenantId: "{tenant_id}",
                cartId: "{cart_id}",
                input: {{
                  kind: FIXED_DISCOUNT
                  scope: LINE_ITEM
                  sourceId: "promo-invalid"
                  amount: "1.00"
                }}
              ) {{
                kind
              }}
            }}
            "#,
            cart_id = cart.id,
        )))
        .await;

    assert_eq!(response.errors.len(), 1);
    assert!(
        response.errors[0]
            .message
            .contains("line_item_id is required for line_item scope")
    );
}

#[tokio::test]
async fn admin_graphql_update_price_list_rule_updates_active_option() {
    let (db, _catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let price_list_id = seed_active_price_list(&db, tenant_id, "Rule Sale", None, None, None).await;

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(pricing_update_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              updateAdminPricingPriceListRule(
                tenantId: "{tenant_id}",
                priceListId: "{price_list_id}",
                input: {{
                  adjustmentPercent: "15"
                }}
              ) {{
                id
                ruleKind
                adjustmentPercent
              }}
            }}
            "#
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected price-list rule GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["updateAdminPricingPriceListRule"]["id"],
        Value::from(price_list_id.to_string())
    );
    assert_eq!(
        json["updateAdminPricingPriceListRule"]["ruleKind"],
        Value::from("percentage_discount")
    );
    assert_eq!(
        json["updateAdminPricingPriceListRule"]["adjustmentPercent"],
        Value::from("15")
    );
}

#[tokio::test]
async fn admin_graphql_update_price_list_rule_rejects_future_price_list() {
    let (db, _catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let future_list_id = seed_active_price_list_with_window(
        &db,
        tenant_id,
        "Future Rule Sale",
        None,
        None,
        None,
        Some(chrono::Utc::now() + chrono::Duration::days(1)),
        None,
    )
    .await;

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(pricing_update_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              updateAdminPricingPriceListRule(
                tenantId: "{tenant_id}",
                priceListId: "{future_list_id}",
                input: {{
                  adjustmentPercent: "15"
                }}
              ) {{
                id
              }}
            }}
            "#
        )))
        .await;

    assert_eq!(response.errors.len(), 1);
    assert!(
        response.errors[0]
            .message
            .contains("price_list_id is not active yet")
    );
}

#[tokio::test]
async fn admin_graphql_update_price_list_rule_clears_metadata() {
    let (db, _catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let price_list_id =
        seed_active_price_list(&db, tenant_id, "Rule Sale", None, None, Some("12.5")).await;

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(pricing_update_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              updateAdminPricingPriceListRule(
                tenantId: "{tenant_id}",
                priceListId: "{price_list_id}",
                input: {{
                  adjustmentPercent: null
                }}
              ) {{
                id
                ruleKind
                adjustmentPercent
              }}
            }}
            "#
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected clear-rule GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["updateAdminPricingPriceListRule"]["id"],
        Value::from(price_list_id.to_string())
    );
    assert_eq!(
        json["updateAdminPricingPriceListRule"]["ruleKind"],
        Value::Null
    );
    assert_eq!(
        json["updateAdminPricingPriceListRule"]["adjustmentPercent"],
        Value::Null
    );
}

#[tokio::test]
async fn admin_graphql_update_price_list_scope_updates_active_option_and_rows() {
    let (db, catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("product should be created");
    let variant = created
        .variants
        .first()
        .expect("product should include a variant");
    let price_list_id =
        seed_active_price_list(&db, tenant_id, "Scoped Sale", None, None, None).await;

    PricingService::new(db.clone(), mock_transactional_event_bus())
        .set_price_list_tier(
            tenant_id,
            actor_id,
            variant.id,
            price_list_id,
            "EUR",
            Decimal::from_str("14.00").unwrap(),
            Some(Decimal::from_str("19.99").unwrap()),
            None,
            None,
        )
        .await
        .expect("override row should be stored");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(pricing_update_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              updateAdminPricingPriceListScope(
                tenantId: "{tenant_id}",
                priceListId: "{price_list_id}",
                input: {{
                  channelId: "{channel_id}",
                  channelSlug: "WEB-STORE"
                }}
              ) {{
                id
                channelId
                channelSlug
              }}
            }}
            "#
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected price-list scope GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["updateAdminPricingPriceListScope"]["id"],
        Value::from(price_list_id.to_string())
    );
    assert_eq!(
        json["updateAdminPricingPriceListScope"]["channelId"],
        Value::from(channel_id.to_string())
    );
    assert_eq!(
        json["updateAdminPricingPriceListScope"]["channelSlug"],
        Value::from("web-store")
    );

    let scoped_row = PricingService::new(db.clone(), mock_transactional_event_bus())
        .get_variant_prices(variant.id)
        .await
        .expect("variant prices should load")
        .into_iter()
        .find(|price| price.price_list_id == Some(price_list_id))
        .expect("scoped override should exist");
    assert_eq!(scoped_row.channel_id, Some(channel_id));
    assert_eq!(scoped_row.channel_slug.as_deref(), Some("web-store"));
}

#[tokio::test]
async fn admin_graphql_update_price_list_scope_clears_boundary_and_rows() {
    let (db, catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("product should be created");
    let variant = created
        .variants
        .first()
        .expect("product should include a variant");
    let price_list_id = seed_active_price_list(
        &db,
        tenant_id,
        "Scoped Sale",
        Some(channel_id),
        Some("web-store"),
        None,
    )
    .await;

    PricingService::new(db.clone(), mock_transactional_event_bus())
        .set_price_list_tier_with_channel(
            tenant_id,
            actor_id,
            variant.id,
            price_list_id,
            "EUR",
            Decimal::from_str("14.00").unwrap(),
            Some(Decimal::from_str("19.99").unwrap()),
            Some(channel_id),
            Some("web-store".to_string()),
            None,
            None,
        )
        .await
        .expect("scoped override row should be stored");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(pricing_update_auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              updateAdminPricingPriceListScope(
                tenantId: "{tenant_id}",
                priceListId: "{price_list_id}",
                input: {{
                  channelId: null,
                  channelSlug: null
                }}
              ) {{
                id
                channelId
                channelSlug
              }}
            }}
            "#
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected clear-scope GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["updateAdminPricingPriceListScope"]["id"],
        Value::from(price_list_id.to_string())
    );
    assert_eq!(
        json["updateAdminPricingPriceListScope"]["channelId"],
        Value::Null
    );
    assert_eq!(
        json["updateAdminPricingPriceListScope"]["channelSlug"],
        Value::Null
    );

    let global_row = PricingService::new(db.clone(), mock_transactional_event_bus())
        .get_variant_prices(variant.id)
        .await
        .expect("variant prices should load")
        .into_iter()
        .find(|price| price.price_list_id == Some(price_list_id))
        .expect("override row should remain present");
    assert_eq!(global_row.channel_id, None);
    assert_eq!(global_row.channel_slug, None);
}

#[tokio::test]
async fn pricing_graphql_facades_reject_price_list_channel_mismatch() {
    let (db, catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let web_channel_id = Uuid::new_v4();
    let mobile_channel_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    seed_channel_binding(&db, tenant_id, web_channel_id, "web-store", true).await;
    seed_channel_binding(&db, tenant_id, mobile_channel_id, "mobile-app", true).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("product should be created");
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let handle = published
        .translations
        .iter()
        .find(|item| item.locale == "en")
        .map(|item| item.handle.clone())
        .expect("published product should expose an English handle");
    let price_list_id = seed_active_price_list(
        &db,
        tenant_id,
        "Web Rule Sale",
        Some(web_channel_id),
        Some("web-store"),
        Some("12.5"),
    )
    .await;

    let admin_schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(auth_context(tenant_id)),
    );
    let admin_response = admin_schema
        .execute(Request::new(format!(
            r#"
            query {{
              adminPricingProduct(
                tenantId: "{tenant_id}",
                id: "{product_id}",
                locale: "en",
                currencyCode: "EUR",
                priceListId: "{price_list_id}",
                channelId: "{mobile_channel_id}",
                channelSlug: "mobile-app",
                quantity: 1
              ) {{
                id
              }}
            }}
            "#,
            product_id = created.id,
        )))
        .await;
    assert!(
        admin_response.errors.iter().any(|error| error
            .message
            .contains("price_list_id is not available for the requested channel")),
        "expected admin channel mismatch validation error, got {:?}",
        admin_response.errors
    );

    let storefront_schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context_with_channel(tenant_id, "en", web_channel_id, "web-store"),
        None,
    );
    let storefront_response = storefront_schema
        .execute(Request::new(format!(
            r#"
            query {{
              storefrontPricingProduct(
                tenantId: "{tenant_id}",
                handle: "{handle}",
                locale: "en",
                currencyCode: "EUR",
                priceListId: "{price_list_id}",
                channelId: "{mobile_channel_id}",
                channelSlug: "mobile-app",
                quantity: 1
              ) {{
                id
              }}
            }}
            "#
        )))
        .await;
    assert!(
        storefront_response.errors.iter().any(|error| error
            .message
            .contains("price_list_id is not available for the requested channel")),
        "expected storefront channel mismatch validation error, got {:?}",
        storefront_response.errors
    );
}

#[tokio::test]
async fn admin_graphql_pricing_product_rejects_non_letter_currency_code() {
    let (db, _catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              adminPricingProduct(
                tenantId: "{tenant_id}",
                id: "{product_id}",
                locale: "en",
                currencyCode: "US1",
                quantity: 1
              ) {{
                id
              }}
            }}
            "#,
            product_id = Uuid::new_v4(),
        )))
        .await;

    assert!(
        response.errors.iter().any(|error| error
            .message
            .contains("currency_code must be a 3-letter code")),
        "expected GraphQL currency_code validation error, got {:?}",
        response.errors
    );
}

#[tokio::test]
async fn admin_graphql_pricing_product_rejects_non_positive_quantity() {
    let (db, _catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              adminPricingProduct(
                tenantId: "{tenant_id}",
                id: "{product_id}",
                locale: "en",
                currencyCode: "USD",
                quantity: 0
              ) {{
                id
              }}
            }}
            "#,
            product_id = Uuid::new_v4(),
        )))
        .await;

    assert!(
        response
            .errors
            .iter()
            .any(|error| error.message.contains("quantity must be at least 1")),
        "expected GraphQL quantity validation error, got {:?}",
        response.errors
    );
}

#[tokio::test]
async fn admin_graphql_pricing_product_rejects_resolution_context_without_currency_code() {
    let (db, _catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              adminPricingProduct(
                tenantId: "{tenant_id}",
                id: "{product_id}",
                locale: "en",
                priceListId: "{price_list_id}",
                quantity: 1
              ) {{
                id
              }}
            }}
            "#,
            product_id = Uuid::new_v4(),
            price_list_id = Uuid::new_v4(),
        )))
        .await;

    assert!(
        response.errors.iter().any(|error| error
            .message
            .contains("currency_code is required for pricing resolution context")),
        "expected GraphQL missing currency_code validation error, got {:?}",
        response.errors
    );
}

#[tokio::test]
async fn storefront_graphql_pricing_product_rejects_invalid_resolution_context() {
    let (db, _catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              storefrontPricingProduct(
                tenantId: "{tenant_id}",
                handle: "missing-product",
                locale: "en",
                currencyCode: "EU1",
                quantity: 0
              ) {{
                id
              }}
            }}
            "#
        )))
        .await;

    assert!(
        response.errors.iter().any(|error| error
            .message
            .contains("currency_code must be a 3-letter code")),
        "expected storefront GraphQL currency_code validation error, got {:?}",
        response.errors
    );
}

#[tokio::test]
async fn storefront_graphql_pricing_product_rejects_resolution_context_without_currency_code() {
    let (db, _catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              storefrontPricingProduct(
                tenantId: "{tenant_id}",
                handle: "missing-product",
                locale: "en",
                priceListId: "{price_list_id}",
                quantity: 1
              ) {{
                id
              }}
            }}
            "#,
            price_list_id = Uuid::new_v4(),
        )))
        .await;

    assert!(
        response.errors.iter().any(|error| error
            .message
            .contains("currency_code is required for pricing resolution context")),
        "expected storefront GraphQL missing currency_code validation error, got {:?}",
        response.errors
    );
}

#[tokio::test]
async fn pricing_graphql_facades_preserve_seller_id_as_identity_boundary() {
    let (db, catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let mut input = create_product_input();
    input.seller_id = Some("seller-alpha-id".to_string());
    input.vendor = Some("Localized Vendor Display".to_string());
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
        .find(|item| item.locale == "en")
        .map(|item| item.handle.clone())
        .expect("published product should have an English handle");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(auth_context(tenant_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              adminPricingProduct(
                tenantId: "{tenant_id}",
                id: "{product_id}",
                locale: "en"
              ) {{
                sellerId
                vendor
              }}
              storefrontPricingProduct(
                tenantId: "{tenant_id}",
                handle: "{handle}",
                locale: "en"
              ) {{
                sellerId
                vendor
              }}
            }}
            "#,
            product_id = published.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected pricing facade GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["adminPricingProduct"]["sellerId"],
        Value::from("seller-alpha-id")
    );
    assert_eq!(
        json["storefrontPricingProduct"]["sellerId"],
        Value::from("seller-alpha-id")
    );
    assert_eq!(
        json["adminPricingProduct"]["vendor"],
        Value::from("Localized Vendor Display")
    );
    assert_eq!(
        json["storefrontPricingProduct"]["vendor"],
        Value::from("Localized Vendor Display")
    );
}
