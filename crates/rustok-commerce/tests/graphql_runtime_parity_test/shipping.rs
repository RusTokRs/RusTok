use super::*;

#[tokio::test]
async fn admin_graphql_exposes_shipping_profile_slug_for_products() {
    let (db, catalog, _) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let mut input = create_product_input();
    input.shipping_profile_slug = Some("Bulky".to_string());
    let created = catalog
        .create_product(tenant_id, actor_id, input)
        .await
        .expect("product should be created");

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
              products(tenantId: "{tenant_id}", locale: "en", filter: {{ page: 1, perPage: 20 }}) {{
                items {{
                  id
                  shippingProfileSlug
                }}
              }}
              product(tenantId: "{tenant_id}", id: "{product_id}", locale: "en") {{
                shippingProfileSlug
              }}
            }}
            "#,
            product_id = created.id
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected admin GraphQL shipping profile errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["products"]["items"][0]["shippingProfileSlug"],
        Value::from("bulky")
    );
    assert_eq!(json["product"]["shippingProfileSlug"], Value::from("bulky"));
}

#[tokio::test]
async fn admin_graphql_supports_shipping_option_create_update_and_list() {
    let (db, _, _) = setup().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    ShippingProfileService::new(db.clone())
        .create_shipping_profile(
            tenant_id,
            rustok_commerce::dto::CreateShippingProfileInput {
                slug: "bulky".to_string(),
                translations: vec![ShippingProfileTranslationInput {
                    locale: "en".to_string(),
                    name: "Bulky".to_string(),
                    description: None,
                }],
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("bulky profile should be created");
    ShippingProfileService::new(db.clone())
        .create_shipping_profile(
            tenant_id,
            rustok_commerce::dto::CreateShippingProfileInput {
                slug: "cold-chain".to_string(),
                translations: vec![ShippingProfileTranslationInput {
                    locale: "en".to_string(),
                    name: "Cold Chain".to_string(),
                    description: None,
                }],
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("cold-chain profile should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_fulfillment_auth_context(tenant_id)),
    );

    let created = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              createShippingOption(
                tenantId: "{tenant_id}",
                input: {{
                  translations: [{{ locale: "en", name: "Bulky Freight" }}],
                  currencyCode: "eur",
                  amount: "29.99",
                  providerId: "manual",
                  allowedShippingProfileSlugs: [" bulky ", "cold-chain", "bulky"],
                  metadata: "{{\"source\":\"graphql-admin-shipping-option\"}}"
                }}
              ) {{
                id
                name
                currencyCode
                providerId
                allowedShippingProfileSlugs
              }}
            }}
            "#
        )))
        .await;
    assert!(
        created.errors.is_empty(),
        "unexpected admin shipping option create errors: {:?}",
        created.errors
    );
    let created_json = created
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    let shipping_option_id = created_json["createShippingOption"]["id"]
        .as_str()
        .expect("shipping option id should be present")
        .to_string();
    assert_eq!(
        created_json["createShippingOption"]["allowedShippingProfileSlugs"],
        serde_json::json!(["bulky", "cold-chain"])
    );

    let updated = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              updateShippingOption(
                tenantId: "{tenant_id}",
                id: "{shipping_option_id}",
                input: {{
                  translations: [{{ locale: "en", name: "Cold Chain Freight" }}],
                  currencyCode: "usd",
                  amount: "39.99",
                  providerId: "custom-provider",
                  allowedShippingProfileSlugs: ["cold-chain"],
                  metadata: "{{\"updated\":true}}"
                }}
              ) {{
                id
                name
                currencyCode
                providerId
                allowedShippingProfileSlugs
              }}
            }}
            "#
        )))
        .await;
    assert!(
        updated.errors.is_empty(),
        "unexpected admin shipping option update errors: {:?}",
        updated.errors
    );
    let updated_json = updated
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    assert_eq!(
        updated_json["updateShippingOption"]["name"],
        Value::from("Cold Chain Freight")
    );
    assert_eq!(
        updated_json["updateShippingOption"]["currencyCode"],
        Value::from("USD")
    );
    assert_eq!(
        updated_json["updateShippingOption"]["allowedShippingProfileSlugs"],
        serde_json::json!(["cold-chain"])
    );

    let queried = schema
        .execute(Request::new(format!(
            r#"
            query {{
              shippingOptions(
                tenantId: "{tenant_id}",
                filter: {{ search: "chain", page: 1, perPage: 20 }}
              ) {{
                total
                items {{
                  id
                  name
                  currencyCode
                  allowedShippingProfileSlugs
                }}
              }}
              shippingOption(tenantId: "{tenant_id}", id: "{shipping_option_id}") {{
                id
                providerId
                metadata
                allowedShippingProfileSlugs
              }}
            }}
            "#
        )))
        .await;
    assert!(
        queried.errors.is_empty(),
        "unexpected admin shipping option query errors: {:?}",
        queried.errors
    );
    let queried_json = queried
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    assert_eq!(queried_json["shippingOptions"]["total"], Value::from(1));
    assert_eq!(
        queried_json["shippingOptions"]["items"][0]["id"],
        Value::from(shipping_option_id.clone())
    );
    assert_eq!(
        queried_json["shippingOption"]["providerId"],
        Value::from("custom-provider")
    );
    assert_eq!(
        queried_json["shippingOption"]["allowedShippingProfileSlugs"],
        serde_json::json!(["cold-chain"])
    );

    let deactivated = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              deactivateShippingOption(tenantId: "{tenant_id}", id: "{shipping_option_id}") {{
                id
                active
              }}
            }}
            "#
        )))
        .await;
    assert!(
        deactivated.errors.is_empty(),
        "unexpected admin shipping option deactivate errors: {:?}",
        deactivated.errors
    );
    let deactivated_json = deactivated
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    assert_eq!(
        deactivated_json["deactivateShippingOption"]["active"],
        Value::from(false)
    );

    let inactive_query = schema
        .execute(Request::new(format!(
            r#"
            query {{
              shippingOptions(
                tenantId: "{tenant_id}",
                filter: {{ active: false, page: 1, perPage: 20 }}
              ) {{
                total
                items {{
                  id
                  active
                }}
              }}
            }}
            "#
        )))
        .await;
    assert!(
        inactive_query.errors.is_empty(),
        "unexpected inactive shipping option query errors: {:?}",
        inactive_query.errors
    );
    let inactive_json = inactive_query
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    assert_eq!(inactive_json["shippingOptions"]["total"], Value::from(1));
    assert_eq!(
        inactive_json["shippingOptions"]["items"][0]["id"],
        Value::from(shipping_option_id.clone())
    );
    assert_eq!(
        inactive_json["shippingOptions"]["items"][0]["active"],
        Value::from(false)
    );

    let reactivated = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              reactivateShippingOption(tenantId: "{tenant_id}", id: "{shipping_option_id}") {{
                id
                active
              }}
            }}
            "#
        )))
        .await;
    assert!(
        reactivated.errors.is_empty(),
        "unexpected admin shipping option reactivate errors: {:?}",
        reactivated.errors
    );
    let reactivated_json = reactivated
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    assert_eq!(
        reactivated_json["reactivateShippingOption"]["active"],
        Value::from(true)
    );
}

#[tokio::test]
async fn admin_graphql_supports_shipping_profile_create_update_and_list() {
    let (db, _, _) = setup().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_fulfillment_auth_context(tenant_id)),
    );

    let created = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              createShippingProfile(
                tenantId: "{tenant_id}",
                input: {{
                  slug: " bulky-freight "
                  translations: [{{ locale: "en", name: "Bulky Freight", description: "Large parcel handling" }}]
                  metadata: "{{\"source\":\"graphql-admin-shipping-profile\"}}"
                }}
              ) {{
                id
                slug
                name
                description
                active
              }}
            }}
            "#
        )))
        .await;
    assert!(
        created.errors.is_empty(),
        "unexpected admin shipping profile create errors: {:?}",
        created.errors
    );
    let created_json = created
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    let profile_id = created_json["createShippingProfile"]["id"]
        .as_str()
        .expect("shipping profile id should be present")
        .to_string();
    assert_eq!(
        created_json["createShippingProfile"]["slug"],
        Value::from("bulky-freight")
    );

    let updated = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              updateShippingProfile(
                tenantId: "{tenant_id}",
                id: "{profile_id}",
                input: {{
                  translations: [{{ locale: "en", name: "Oversize Freight", description: "Updated profile" }}]
                  metadata: "{{\"updated\":true}}"
                }}
              ) {{
                id
                slug
                name
                description
              }}
            }}
            "#
        )))
        .await;
    assert!(
        updated.errors.is_empty(),
        "unexpected admin shipping profile update errors: {:?}",
        updated.errors
    );
    let updated_json = updated
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    assert_eq!(
        updated_json["updateShippingProfile"]["name"],
        Value::from("Oversize Freight")
    );

    let queried = schema
        .execute(Request::new(format!(
            r#"
            query {{
              shippingProfiles(
                tenantId: "{tenant_id}",
                filter: {{ search: "oversize", page: 1, perPage: 20 }}
              ) {{
                total
                items {{
                  id
                  slug
                  name
                  active
                }}
              }}
              shippingProfile(tenantId: "{tenant_id}", id: "{profile_id}") {{
                id
                slug
                metadata
              }}
            }}
            "#
        )))
        .await;
    assert!(
        queried.errors.is_empty(),
        "unexpected admin shipping profile query errors: {:?}",
        queried.errors
    );
    let queried_json = queried
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    assert_eq!(queried_json["shippingProfiles"]["total"], Value::from(1));
    assert_eq!(
        queried_json["shippingProfiles"]["items"][0]["id"],
        Value::from(profile_id.clone())
    );
    assert_eq!(
        queried_json["shippingProfile"]["slug"],
        Value::from("bulky-freight")
    );

    let deactivated = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              deactivateShippingProfile(tenantId: "{tenant_id}", id: "{profile_id}") {{
                id
                active
              }}
            }}
            "#
        )))
        .await;
    assert!(
        deactivated.errors.is_empty(),
        "unexpected admin shipping profile deactivate errors: {:?}",
        deactivated.errors
    );
    let deactivated_json = deactivated
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    assert_eq!(
        deactivated_json["deactivateShippingProfile"]["active"],
        Value::from(false)
    );

    let reactivated = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              reactivateShippingProfile(tenantId: "{tenant_id}", id: "{profile_id}") {{
                id
                active
              }}
            }}
            "#
        )))
        .await;
    assert!(
        reactivated.errors.is_empty(),
        "unexpected admin shipping profile reactivate errors: {:?}",
        reactivated.errors
    );
    let reactivated_json = reactivated
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    assert_eq!(
        reactivated_json["reactivateShippingProfile"]["active"],
        Value::from(true)
    );
}

#[tokio::test]
async fn admin_graphql_rejects_unknown_shipping_profile_references() {
    let (db, _, _) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let auth = AuthContext {
        user_id: actor_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![
            Permission::PRODUCTS_CREATE,
            Permission::PRODUCTS_UPDATE,
            Permission::FULFILLMENTS_CREATE,
            Permission::FULFILLMENTS_UPDATE,
        ],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    };
    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(auth),
    );

    let shipping_option_response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              createShippingOption(
                tenantId: "{tenant_id}",
                input: {{
                  translations: [{{ locale: "en", name: "Invalid Option" }}]
                  currencyCode: "eur"
                  amount: "9.99"
                  allowedShippingProfileSlugs: ["missing-profile"]
                }}
              ) {{
                id
              }}
            }}
            "#
        )))
        .await;
    assert_eq!(shipping_option_response.errors.len(), 1);
    assert!(
        shipping_option_response.errors[0]
            .message
            .contains("Unknown shipping profile slug: missing-profile"),
        "unexpected shipping option error: {}",
        shipping_option_response.errors[0].message
    );

    let product_response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              createProduct(
                input: {{
                  translations: [{{
                    locale: "en"
                    title: "Shipping Profile Product"
                    handle: "shipping-profile-product"
                  }}]
                  variants: [{{
                    sku: "PROFILE-SKU-1"
                    prices: [{{ currencyCode: "EUR", amount: "19.99" }}]
                  }}]
                  shippingProfileSlug: "missing-profile"
                }}
              ) {{
                id
              }}
            }}
            "#
        )))
        .await;
    assert_eq!(product_response.errors.len(), 1);
    assert!(
        product_response.errors[0]
            .message
            .contains("Unknown shipping profile slug: missing-profile"),
        "unexpected product error: {}",
        product_response.errors[0].message
    );
}

#[tokio::test]
async fn storefront_graphql_shipping_options_filter_incompatible_shipping_profiles() {
    let (db, catalog, cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let mut product_input = create_product_input();
    product_input.metadata = serde_json::json!({
        "shipping_profile": {
            "slug": "bulky"
        }
    });
    let created = catalog
        .create_product(tenant_id, actor_id, product_input)
        .await
        .expect("product should be created");
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let variant = published
        .variants
        .first()
        .expect("published product should include variant");

    FulfillmentService::new(db.clone())
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Default Shipping".to_string(),
                }],
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: Some(vec!["default".to_string()]),
                metadata: serde_json::json!({
                    "shipping_profiles": {
                        "allowed_slugs": ["default"]
                    }
                }),
            },
        )
        .await
        .expect("default shipping option should be created");
    let bulky_option = FulfillmentService::new(db.clone())
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Bulky Freight".to_string(),
                }],
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("29.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: Some(vec!["bulky".to_string()]),
                metadata: serde_json::json!({
                    "shipping_profiles": {
                        "allowed_slugs": ["bulky"]
                    }
                }),
            },
        )
        .await
        .expect("bulky shipping option should be created");

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("shipping-profile@example.com".to_string()),
                region_id: None,
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: None,
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "storefront-graphql-shipping-profile" }),
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
                shipping_profile_slug: Some("bulky".to_string()),
                sku: variant.sku.clone(),
                title: variant.title.clone(),
                quantity: 1,
                unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .expect("line item should be added");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        None,
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              storefrontShippingOptions(
                tenantId: "{tenant_id}",
                filter: {{ cartId: "{cart_id}" currencyCode: "eur" }}
              ) {{
                id
                name
                allowedShippingProfileSlugs
              }}
            }}
            "#,
            cart_id = cart.id
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected storefront shipping profile GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["storefrontShippingOptions"],
        serde_json::json!([{
            "id": bulky_option.id.to_string(),
            "name": "Bulky Freight",
            "allowedShippingProfileSlugs": ["bulky"]
        }])
    );
}

#[tokio::test]
async fn storefront_graphql_update_cart_context_rejects_incompatible_shipping_profile_option() {
    let (db, catalog, cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let mut product_input = create_product_input();
    product_input.metadata = serde_json::json!({
        "shipping_profile": {
            "slug": "bulky"
        }
    });
    let created = catalog
        .create_product(tenant_id, actor_id, product_input)
        .await
        .expect("product should be created");
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let variant = published
        .variants
        .first()
        .expect("published product should include variant");

    let incompatible_option = FulfillmentService::new(db.clone())
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Default Shipping".to_string(),
                }],
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: Some(vec!["default".to_string()]),
                metadata: serde_json::json!({
                    "shipping_profiles": {
                        "allowed_slugs": ["default"]
                    }
                }),
            },
        )
        .await
        .expect("shipping option should be created");

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("shipping-profile@example.com".to_string()),
                region_id: None,
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: None,
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "storefront-graphql-shipping-profile" }),
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
                shipping_profile_slug: Some("bulky".to_string()),
                sku: variant.sku.clone(),
                title: variant.title.clone(),
                quantity: 1,
                unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .expect("line item should be added");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        None,
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              updateStorefrontCartContext(
                tenantId: "{tenant_id}",
                cartId: "{cart_id}",
                input: {{ selectedShippingOptionId: "{shipping_option_id}" }}
              ) {{
                cart {{ id }}
              }}
            }}
            "#,
            cart_id = cart.id,
            shipping_option_id = incompatible_option.id
        )))
        .await;

    assert_eq!(response.errors.len(), 1);
    assert!(
        response.errors[0]
            .message
            .contains("not compatible with shipping profile bulky"),
        "unexpected GraphQL error: {}",
        response.errors[0].message
    );
}

#[tokio::test]
async fn storefront_graphql_shipping_options_reject_foreign_customer_cart_access() {
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
                email: "shipping-owner@example.com".to_string(),
                first_name: Some("Owner".to_string()),
                last_name: None,
                phone: None,
                locale: Some("en".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-shipping-owner" }),
            },
        )
        .await
        .expect("owner customer should be created");
    CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(foreign_user_id),
                email: "shipping-foreign@example.com".to_string(),
                first_name: Some("Foreign".to_string()),
                last_name: None,
                phone: None,
                locale: Some("en".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-shipping-foreign" }),
            },
        )
        .await
        .expect("foreign customer should be created");
    let cart = CartService::new(db.clone())
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(owner_customer.id),
                email: Some("shipping-owner@example.com".to_string()),
                region_id: None,
                country_code: Some("de".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: None,
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "storefront-graphql-shipping-foreign" }),
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
            query {{
              storefrontShippingOptions(
                tenantId: "{tenant_id}",
                filter: {{ cartId: "{cart_id}" }}
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
