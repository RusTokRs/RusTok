
    use super::{
        cart_context_metadata, checkout_actor_id, ensure_store_cart_access, merge_metadata,
        requested_cart_context, resolve_store_line_item_input, RequestedCartContext,
        StoreAddCartLineItemInput, StoreCartContextPatch, MODULE_SLUG,
    };
    use axum::body::{to_bytes, Body};
    use axum::extract::{Path, State};
    use axum::http::{Request, StatusCode};
    use axum::middleware::{from_fn_with_state, Next};
    use axum::response::Response;
    use axum::Router;
    use loco_rs::app::{AppContext, SharedStore};
    use loco_rs::cache;
    use loco_rs::environment::Environment;
    use loco_rs::storage::{self, Storage};
    use loco_rs::tests_cfg::config::test_config;
    use rust_decimal::Decimal;
    use rustok_api::context::ChannelResolutionSource;
    use rustok_api::RequestContext;
    use rustok_api::{
        AuthContext, AuthContextExtension, ChannelContext, ChannelContextExtension, TenantContext,
        TenantContextExtension,
    };
    use rustok_cart::dto::SetCartAdjustmentInput;
    use rustok_core::events::EventTransport;
    use rustok_core::Permission;
    use rustok_pricing::PriceResolutionContext;
    use rustok_region::dto::{CreateRegionInput, RegionResponse, RegionTranslationInput};
    use rustok_region::services::RegionService;
    use rustok_test_utils::db::setup_test_db;
    use rustok_test_utils::{mock_transactional_event_bus, MockEventTransport};
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
    use serde_json::json;
    use std::str::FromStr;
    use std::sync::Arc;
    use tower::util::ServiceExt;
    use uuid::Uuid;

    use crate::dto::{
        AddCartLineItemInput, CartResponse, CreateCartInput, CreateProductInput,
        CreateShippingOptionInput, CreateVariantInput, PriceInput, ProductTranslationInput,
        ShippingOptionTranslationInput, StoreContextResponse,
    };
    use crate::{CartService, CatalogService, CustomerService, FulfillmentService, PricingService};
    use rustok_customer::dto::CreateCustomerInput;

    mod support {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support.rs"));
    }

    fn sample_cart(customer_id: Option<Uuid>) -> CartResponse {
        CartResponse {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            channel_id: None,
            channel_slug: None,
            customer_id,
            email: Some("buyer@example.com".to_string()),
            region_id: None,
            country_code: None,
            locale_code: None,
            selected_shipping_option_id: None,
            status: "active".to_string(),
            currency_code: "USD".to_string(),
            subtotal_amount: Decimal::ZERO,
            adjustment_total: Decimal::ZERO,
            shipping_total: Decimal::ZERO,
            total_amount: Decimal::ZERO,
            tax_total: Decimal::ZERO,
            metadata: serde_json::json!({}),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            completed_at: None,
            line_items: Vec::new(),
            adjustments: Vec::new(),
            tax_lines: Vec::new(),
            delivery_groups: Vec::new(),
        }
    }

    fn pricing_context(currency_code: &str, quantity: i32) -> PriceResolutionContext {
        PriceResolutionContext {
            currency_code: currency_code.to_ascii_uppercase(),
            region_id: None,
            price_list_id: None,
            channel_id: None,
            channel_slug: None,
            quantity: Some(quantity),
        }
    }

    #[test]
    fn guest_cart_allows_missing_customer_context() {
        let cart = sample_cart(None);
        assert!(ensure_store_cart_access(&cart, None).is_ok());
    }

    #[test]
    fn customer_owned_cart_allows_matching_customer() {
        let customer_id = Uuid::new_v4();
        let cart = sample_cart(Some(customer_id));
        assert!(ensure_store_cart_access(&cart, Some(customer_id)).is_ok());
    }

    #[test]
    fn customer_owned_cart_rejects_missing_customer_context() {
        let cart = sample_cart(Some(Uuid::new_v4()));
        let error = ensure_store_cart_access(&cart, None).expect_err("customer auth required");
        assert_eq!(error.to_string(), "Cart belongs to another customer");
    }

    #[test]
    fn customer_owned_cart_rejects_different_customer() {
        let cart = sample_cart(Some(Uuid::new_v4()));
        let error = ensure_store_cart_access(&cart, Some(Uuid::new_v4()))
            .expect_err("foreign customer access must be rejected");
        assert_eq!(error.to_string(), "Cart belongs to another customer");
    }

    #[test]
    fn payment_collection_allows_non_completed_cart() {
        let mut cart = sample_cart(None);
        cart.status = "open".to_string();
        assert!(super::ensure_cart_allows_payment_collection(&cart).is_ok());
    }

    #[test]
    fn payment_collection_rejects_completed_cart() {
        let mut cart = sample_cart(None);
        cart.status = "completed".to_string();
        let error = super::ensure_cart_allows_payment_collection(&cart)
            .expect_err("completed carts must reject payment collection creation");
        assert_eq!(
            error.to_string(),
            "Cannot create payment collection for completed cart"
        );
    }

    #[test]
    fn guest_checkout_uses_nil_actor_without_auth() {
        assert_eq!(checkout_actor_id(None), Uuid::nil());
    }

    #[test]
    fn authenticated_checkout_uses_user_actor() {
        let user_id = Uuid::new_v4();
        let auth = AuthContext {
            user_id,
            session_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            permissions: vec![],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };

        assert_eq!(checkout_actor_id(Some(&auth)), user_id);
    }

    fn sample_request_context(locale: &str) -> RequestContext {
        RequestContext {
            tenant_id: Uuid::new_v4(),
            user_id: None,
            channel_id: None,
            channel_slug: None,
            channel_resolution_source: None,
            locale: locale.to_string(),
        }
    }

    fn sample_channel_context(slug: &str) -> ChannelContext {
        ChannelContext {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            slug: slug.to_string(),
            name: format!("Channel {slug}"),
            is_active: true,
            status: "active".to_string(),
            target_type: Some("web_domain".to_string()),
            target_value: Some(format!("{slug}.example.test")),
            settings: json!({}),
            resolution_source: ChannelResolutionSource::Host,
            resolution_trace: Vec::new(),
        }
    }

    async fn seed_channel_binding(
        db: &sea_orm::DatabaseConnection,
        channel: &ChannelContext,
        module_slug: &str,
        is_enabled: bool,
    ) {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO channels (id, tenant_id, slug, name, is_active, is_default, status, settings, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                channel.id.into(),
                channel.tenant_id.into(),
                channel.slug.clone().into(),
                channel.name.clone().into(),
                channel.is_active.into(),
                false.into(),
                channel.status.clone().into(),
                channel.settings.to_string().into(),
            ],
        ))
        .await
        .expect("channel should be inserted for test");

        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO channel_module_bindings (id, channel_id, module_slug, is_enabled, settings, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                Uuid::new_v4().into(),
                channel.id.into(),
                module_slug.into(),
                is_enabled.into(),
                json!({}).to_string().into(),
            ],
        ))
        .await
        .expect("channel module binding should be inserted for test");
    }

    async fn set_stock_location_channel_visibility(
        db: &sea_orm::DatabaseConnection,
        tenant_id: Uuid,
        allowed_channel_slugs: &[&str],
    ) {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE stock_locations SET metadata = ? WHERE tenant_id = ?",
            vec![
                json!({
                    "channel_visibility": {
                        "allowed_channel_slugs": allowed_channel_slugs
                    }
                })
                .to_string()
                .into(),
                tenant_id.into(),
            ],
        ))
        .await
        .expect("stock location visibility should be updated");
    }

    #[test]
    fn cart_context_patch_keeps_existing_values_when_fields_are_omitted() {
        let region_id = Uuid::new_v4();
        let shipping_option_id = Uuid::new_v4();
        let mut cart = sample_cart(None);
        cart.email = Some("keep@example.com".to_string());
        cart.region_id = Some(region_id);
        cart.country_code = Some("DE".to_string());
        cart.locale_code = Some("de".to_string());
        cart.selected_shipping_option_id = Some(shipping_option_id);

        let requested = requested_cart_context(
            &cart,
            &sample_request_context("en"),
            StoreCartContextPatch {
                email: None,
                region_id: None,
                country_code: None,
                locale: None,
                selected_shipping_option_id: None,
                shipping_selections: None,
            },
        );

        assert_eq!(
            requested,
            RequestedCartContext {
                email: Some("keep@example.com".to_string()),
                region_id: Some(region_id),
                country_code: Some("DE".to_string()),
                locale: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option_id),
                shipping_selections: Vec::new(),
            }
        );
    }

    #[test]
    fn cart_context_patch_applies_explicit_values() {
        let region_id = Uuid::new_v4();
        let shipping_option_id = Uuid::new_v4();
        let cart = sample_cart(None);

        let requested = requested_cart_context(
            &cart,
            &sample_request_context("en"),
            StoreCartContextPatch {
                email: Some(Some("set@example.com".to_string())),
                region_id: Some(Some(region_id)),
                country_code: Some(Some("fr".to_string())),
                locale: Some(Some("fr".to_string())),
                selected_shipping_option_id: Some(Some(shipping_option_id)),
                shipping_selections: None,
            },
        );

        assert_eq!(
            requested,
            RequestedCartContext {
                email: Some("set@example.com".to_string()),
                region_id: Some(region_id),
                country_code: Some("fr".to_string()),
                locale: Some("fr".to_string()),
                selected_shipping_option_id: Some(shipping_option_id),
                shipping_selections: Vec::new(),
            }
        );
    }

    #[test]
    fn cart_context_patch_clears_country_when_region_is_explicitly_cleared() {
        let mut cart = sample_cart(None);
        cart.region_id = Some(Uuid::new_v4());
        cart.country_code = Some("DE".to_string());
        cart.locale_code = Some("de".to_string());

        let requested = requested_cart_context(
            &cart,
            &sample_request_context("en"),
            StoreCartContextPatch {
                email: None,
                region_id: Some(None),
                country_code: None,
                locale: None,
                selected_shipping_option_id: None,
                shipping_selections: None,
            },
        );

        assert_eq!(
            requested,
            RequestedCartContext {
                email: Some("buyer@example.com".to_string()),
                region_id: None,
                country_code: None,
                locale: Some("de".to_string()),
                selected_shipping_option_id: None,
                shipping_selections: Vec::new(),
            }
        );
    }

    #[test]
    fn cart_context_patch_can_clear_individual_fields_and_falls_back_to_request_locale() {
        let region_id = Uuid::new_v4();
        let shipping_option_id = Uuid::new_v4();
        let mut cart = sample_cart(None);
        cart.region_id = Some(region_id);
        cart.country_code = Some("DE".to_string());
        cart.locale_code = Some("de".to_string());
        cart.selected_shipping_option_id = Some(shipping_option_id);

        let requested = requested_cart_context(
            &cart,
            &sample_request_context("en"),
            StoreCartContextPatch {
                email: Some(None),
                region_id: None,
                country_code: Some(None),
                locale: Some(None),
                selected_shipping_option_id: Some(None),
                shipping_selections: None,
            },
        );

        assert_eq!(
            requested,
            RequestedCartContext {
                email: None,
                region_id: Some(region_id),
                country_code: None,
                locale: Some("en".to_string()),
                selected_shipping_option_id: None,
                shipping_selections: Vec::new(),
            }
        );
    }

    #[test]
    fn merge_metadata_keeps_existing_fields_and_overrides_conflicts() {
        let merged = merge_metadata(
            json!({
                "source": "request",
                "cart_context": { "locale": "de", "currency_code": "EUR" }
            }),
            json!({
                "cart_context": { "locale": "en" },
                "attempt": 2
            }),
        );

        assert_eq!(
            merged,
            json!({
                "source": "request",
                "cart_context": { "locale": "en" },
                "attempt": 2
            })
        );
    }

    #[test]
    fn cart_context_metadata_embeds_storefront_context_for_payment_collection() {
        let tenant_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();
        let region_id = Uuid::new_v4();
        let shipping_option_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let mut cart = sample_cart(Some(customer_id));
        cart.channel_id = Some(channel_id);
        cart.channel_slug = Some("web-store".to_string());
        cart.region_id = Some(region_id);
        cart.country_code = Some("DE".to_string());
        cart.locale_code = Some("de".to_string());
        cart.selected_shipping_option_id = Some(shipping_option_id);

        let metadata = cart_context_metadata(
            &cart,
            &StoreContextResponse {
                region: Some(RegionResponse {
                    id: region_id,
                    tenant_id,
                    name: "Europe".to_string(),
                    currency_code: "EUR".to_string(),
                    tax_provider_id: None,
                    tax_rate: Decimal::from(20),
                    tax_included: true,
                    country_tax_policies: vec![],
                    countries: vec!["DE".to_string()],
                    metadata: json!({}),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                    requested_locale: Some("de".to_string()),
                    effective_locale: Some("de".to_string()),
                    available_locales: vec!["en".to_string(), "de".to_string()],
                    translations: Vec::new(),
                }),
                locale: "de".to_string(),
                default_locale: "en".to_string(),
                available_locales: vec!["en".to_string(), "de".to_string()],
                currency_code: Some("EUR".to_string()),
            },
        );

        assert_eq!(
            metadata,
            json!({
                "cart_context": {
                    "channel_id": channel_id,
                    "channel_slug": "web-store",
                    "region_id": region_id,
                    "country_code": "DE",
                    "locale": "de",
                    "currency_code": "USD",
                    "selected_shipping_option_id": shipping_option_id,
                    "shipping_selections": [],
                    "customer_id": customer_id,
                    "email": "buyer@example.com"
                }
            })
        );
    }

    #[tokio::test]
    async fn store_cart_transport_persists_channel_snapshot() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let mut channel = sample_channel_context("web-store");
        channel.tenant_id = tenant_id;
        let channel_id = channel.id;
        seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
        let app = commerce_transport_router_with_context(
            test_app_context(db),
            tenant,
            None,
            Some(channel),
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "channel-cart@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(response.status(), StatusCode::CREATED);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&body).expect("create cart response should be JSON");
        assert_eq!(created_cart["cart"]["channel_id"], json!(channel_id));
        assert_eq!(created_cart["cart"]["channel_slug"], json!("web-store"));
    }

    #[tokio::test]
    async fn store_products_transport_rejects_disabled_channel_module() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let mut channel = sample_channel_context("web-store");
        channel.tenant_id = tenant_id;
        seed_channel_binding(&db, &channel, MODULE_SLUG, false).await;
        let app = commerce_transport_router_with_context(
            test_app_context(db),
            tenant,
            None,
            Some(channel),
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/store/products")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("store products request should complete");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn store_products_transport_filters_channel_hidden_products() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());

        let mut visible_input = storefront_product_input();
        visible_input.translations[0].title = "Visible Product".to_string();
        visible_input.translations[0].handle = Some("visible-storefront-product-en".to_string());
        visible_input.translations[1].title = "Sichtbares Produkt".to_string();
        visible_input.translations[1].handle = Some("sichtbares-storefront-product-de".to_string());
        visible_input.variants[0].sku = Some("STOREFRONT-VISIBLE-SKU-1".to_string());
        let visible = catalog
            .create_product(tenant_id, actor_id, visible_input)
            .await
            .expect("visible product should be created");
        catalog
            .publish_product(tenant_id, actor_id, visible.id)
            .await
            .expect("visible product should be published");

        let mut hidden_input = storefront_product_input();
        hidden_input.translations[0].title = "Hidden Product".to_string();
        hidden_input.translations[0].handle = Some("hidden-storefront-product-en".to_string());
        hidden_input.translations[1].title = "Verstecktes Produkt".to_string();
        hidden_input.translations[1].handle = Some("verstecktes-storefront-product-de".to_string());
        hidden_input.variants[0].sku = Some("STOREFRONT-HIDDEN-SKU-1".to_string());
        hidden_input.metadata = json!({
            "channel_visibility": {
                "allowed_channel_slugs": ["mobile-app"]
            }
        });
        let hidden = catalog
            .create_product(tenant_id, actor_id, hidden_input)
            .await
            .expect("hidden product should be created");
        catalog
            .publish_product(tenant_id, actor_id, hidden.id)
            .await
            .expect("hidden product should be published");

        let mut channel = sample_channel_context("web-store");
        channel.tenant_id = tenant_id;
        seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
        let app = commerce_transport_router_with_context(
            test_app_context(db),
            tenant,
            None,
            Some(channel),
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/store/products?locale=de")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("store products request should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("store products body should read");
        let json: serde_json::Value =
            serde_json::from_slice(&body).expect("store products response should be JSON");
        let items = json["data"]
            .as_array()
            .expect("product list should be an array");
        assert_eq!(json["meta"]["total"], json!(1));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["title"], json!("Sichtbares Produkt"));
    }

    #[tokio::test]
    async fn store_shipping_options_transport_filters_channel_hidden_options() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let fulfillment = FulfillmentService::new(db.clone());
        let visible_option = fulfillment
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Visible Shipping".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("9.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: None,
                    metadata: json!({}),
                },
            )
            .await
            .expect("visible shipping option should be created");
        fulfillment
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Hidden Shipping".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("19.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: None,
                    metadata: json!({
                        "channel_visibility": {
                            "allowed_channel_slugs": ["mobile-app"]
                        }
                    }),
                },
            )
            .await
            .expect("hidden shipping option should be created");

        let mut channel = sample_channel_context("web-store");
        channel.tenant_id = tenant_id;
        seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
        let app = commerce_transport_router_with_context(
            test_app_context(db),
            tenant,
            None,
            Some(channel),
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/store/shipping-options?currency_code=eur&locale=de")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("shipping options request should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("shipping options body should read");
        let json: serde_json::Value =
            serde_json::from_slice(&body).expect("shipping options response should be JSON");
        let options = json
            .as_array()
            .expect("shipping options should be an array");
        assert_eq!(options.len(), 1);
        assert_eq!(options[0]["id"], json!(visible_option.id));
    }

    #[tokio::test]
    async fn store_shipping_options_transport_filters_incompatible_shipping_profiles() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };

        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let mut product_input = storefront_product_input();
        product_input.metadata = json!({
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

        let fulfillment = FulfillmentService::new(db.clone());
        fulfillment
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
                    metadata: json!({
                        "shipping_profiles": {
                            "allowed_slugs": ["default"]
                        }
                    }),
                },
            )
            .await
            .expect("default shipping option should be created");
        let bulky_option = fulfillment
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
                    metadata: json!({
                        "shipping_profiles": {
                            "allowed_slugs": ["bulky"]
                        }
                    }),
                },
            )
            .await
            .expect("bulky shipping option should be created");

        let cart_service = CartService::new(db.clone());
        let cart = cart_service
            .create_cart_with_channel(
                tenant_id,
                CreateCartInput {
                    customer_id: None,
                    email: Some("buyer@example.com".to_string()),
                    region_id: None,
                    country_code: Some("de".to_string()),
                    locale_code: Some("de".to_string()),
                    selected_shipping_option_id: None,
                    currency_code: "eur".to_string(),
                    metadata: json!({ "source": "store-shipping-profile-filter" }),
                },
                Some(channel_id),
                Some("web-store".to_string()),
            )
            .await
            .expect("cart should be created");
        cart_service
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
                    metadata: json!({ "slot": 1 }),
                },
            )
            .await
            .expect("line item should be added");

        let mut channel = sample_channel_context("web-store");
        channel.id = channel_id;
        channel.tenant_id = tenant_id;
        seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
        let app = commerce_transport_router_with_context(
            test_app_context(db),
            tenant,
            None,
            Some(channel),
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/store/shipping-options?cart_id={}&currency_code=eur&locale=de",
                        cart.id
                    ))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("shipping options request should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("shipping options body should read");
        let json: serde_json::Value =
            serde_json::from_slice(&body).expect("shipping options response should be JSON");
        let options = json
            .as_array()
            .expect("shipping options should be an array");
        assert_eq!(options.len(), 1);
        assert_eq!(options[0]["id"], json!(bulky_option.id));
        assert_eq!(
            options[0]["allowed_shipping_profile_slugs"],
            json!(["bulky"])
        );
    }

    #[tokio::test]
    async fn store_update_cart_context_rejects_incompatible_shipping_profile_option() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };

        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let mut product_input = storefront_product_input();
        product_input.metadata = json!({
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
                    metadata: json!({
                        "shipping_profiles": {
                            "allowed_slugs": ["default"]
                        }
                    }),
                },
            )
            .await
            .expect("shipping option should be created");

        let cart_service = CartService::new(db.clone());
        let cart = cart_service
            .create_cart_with_channel(
                tenant_id,
                CreateCartInput {
                    customer_id: None,
                    email: Some("buyer@example.com".to_string()),
                    region_id: None,
                    country_code: Some("de".to_string()),
                    locale_code: Some("de".to_string()),
                    selected_shipping_option_id: None,
                    currency_code: "eur".to_string(),
                    metadata: json!({ "source": "store-shipping-profile-update" }),
                },
                Some(channel_id),
                Some("web-store".to_string()),
            )
            .await
            .expect("cart should be created");
        cart_service
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
                    metadata: json!({ "slot": 1 }),
                },
            )
            .await
            .expect("line item should be added");

        let mut channel = sample_channel_context("web-store");
        channel.id = channel_id;
        channel.tenant_id = tenant_id;
        seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
        let app = commerce_transport_router_with_context(
            test_app_context(db),
            tenant,
            None,
            Some(channel),
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{}", cart.id))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "selected_shipping_option_id": incompatible_option.id
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update cart request should complete");

        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("update cart body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected update cart body: {}",
            String::from_utf8_lossy(&body)
        );
        let updated_cart: serde_json::Value =
            serde_json::from_slice(&body).expect("updated cart response should be JSON");
        assert_eq!(
            updated_cart["cart"]["selected_shipping_option_id"],
            json!(null)
        );
        assert_eq!(
            updated_cart["cart"]["delivery_groups"][0]["shipping_profile_slug"],
            json!("bulky")
        );
        assert_eq!(
            updated_cart["cart"]["delivery_groups"][0]["available_shipping_options"],
            json!([])
        );
    }

    #[tokio::test]
    async fn store_cart_line_item_transport_rejects_channel_hidden_product() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let mut hidden_input = storefront_product_input();
        hidden_input.translations[0].handle = Some("channel-hidden-variant-en".to_string());
        hidden_input.translations[1].handle = Some("channel-hidden-variant-de".to_string());
        hidden_input.variants[0].sku = Some("STOREFRONT-CHANNEL-HIDDEN-SKU-1".to_string());
        hidden_input.metadata = json!({
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
        let variant = hidden
            .variants
            .first()
            .expect("hidden product should have variant");

        let mut channel = sample_channel_context("web-store");
        channel.tenant_id = tenant_id;
        seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
        let app = commerce_transport_router_with_context(
            test_app_context(db),
            tenant,
            None,
            Some(channel),
        );

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "buyer@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");

        let add_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": variant.id,
                            "quantity": 1,
                            "metadata": {}
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should complete");

        assert_eq!(add_response.status(), StatusCode::NOT_FOUND);
    }

    fn storefront_product_input() -> CreateProductInput {
        CreateProductInput {
            translations: vec![
                ProductTranslationInput {
                    locale: "en".to_string(),
                    title: "Storefront Product".to_string(),
                    description: Some("English description".to_string()),
                    handle: Some("storefront-product-en".to_string()),
                    meta_title: None,
                    meta_description: None,
                },
                ProductTranslationInput {
                    locale: "de".to_string(),
                    title: "Storefront Produkt".to_string(),
                    description: Some("German description".to_string()),
                    handle: Some("storefront-product-de".to_string()),
                    meta_title: None,
                    meta_description: None,
                },
            ],
            options: vec![],
            variants: vec![CreateVariantInput {
                sku: Some("STOREFRONT-SKU-1".to_string()),
                barcode: None,
                shipping_profile_slug: None,
                option1: Some("Default".to_string()),
                option2: None,
                option3: None,
                prices: vec![PriceInput {
                    currency_code: "EUR".to_string(),
                    channel_id: None,
                    channel_slug: None,
                    amount: Decimal::from_str("19.99").expect("valid decimal"),
                    compare_at_amount: None,
                }],
                inventory_quantity: 0,
                inventory_policy: "deny".to_string(),
                weight: None,
                weight_unit: None,
            }],
            seller_id: None,
            vendor: Some("Storefront Vendor".to_string()),
            product_type: Some("physical".to_string()),
            shipping_profile_slug: None,
            tags: vec![],
            publish: false,
            metadata: json!({}),
        }
    }

    fn test_app_context(db: sea_orm::DatabaseConnection) -> AppContext {
        let shared_store = Arc::new(SharedStore::default());
        let event_transport: Arc<dyn EventTransport> = Arc::new(MockEventTransport::new());
        shared_store.insert(event_transport);

        AppContext {
            environment: Environment::Test,
            db,
            queue_provider: None,
            config: test_config(),
            mailer: None,
            storage: Storage::single(storage::drivers::mem::new()).into(),
            cache: Arc::new(cache::Cache::new(cache::drivers::null::new())),
            shared_store,
        }
    }

    async fn seed_store_tenant_context(db: &sea_orm::DatabaseConnection, tenant_id: Uuid) {
        db.execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                tenant_id.into(),
                "Store Test Tenant".into(),
                format!("store-test-{tenant_id}").into(),
                sea_orm::Value::String(None),
                json!({}).to_string().into(),
                "en".into(),
                true.into(),
            ],
        ))
        .await
        .expect("tenant should be inserted");

        for (locale, name, native_name, is_default) in [
            ("en", "English", "English", true),
            ("de", "German", "Deutsch", false),
        ] {
            db.execute(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                "INSERT INTO tenant_locales (id, tenant_id, locale, name, native_name, is_default, is_enabled, fallback_locale, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
                vec![
                    Uuid::new_v4().into(),
                    tenant_id.into(),
                    locale.into(),
                    name.into(),
                    native_name.into(),
                    is_default.into(),
                    true.into(),
                    sea_orm::Value::String(None),
                ],
            ))
            .await
            .expect("tenant locale should be inserted");
        }

        db.execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                Uuid::new_v4().into(),
                tenant_id.into(),
                "commerce".into(),
                true.into(),
                json!({}).to_string().into(),
            ],
        ))
        .await
        .expect("commerce module should be enabled for tenant");
    }

    async fn create_customer_for_user(
        db: &sea_orm::DatabaseConnection,
        tenant_id: Uuid,
        user_id: Uuid,
        email: &str,
    ) -> Uuid {
        CustomerService::new(db.clone())
            .create_customer(
                tenant_id,
                CreateCustomerInput {
                    user_id: Some(user_id),
                    email: email.to_string(),
                    first_name: Some("Store".to_string()),
                    last_name: Some("Customer".to_string()),
                    phone: None,
                    locale: Some("de".to_string()),
                    metadata: json!({}),
                },
            )
            .await
            .expect("customer should be created")
            .id
    }

    #[derive(Clone)]
    struct TransportRequestContext {
        tenant: TenantContext,
        auth: Option<AuthContext>,
        channel: Option<ChannelContext>,
    }

    async fn inject_transport_context(
        State(context): State<TransportRequestContext>,
        mut req: axum::extract::Request,
        next: Next,
    ) -> Response {
        req.extensions_mut()
            .insert(TenantContextExtension(context.tenant));
        if let Some(auth) = context.auth {
            req.extensions_mut().insert(AuthContextExtension(auth));
        }
        if let Some(channel) = context.channel {
            req.extensions_mut()
                .insert(ChannelContextExtension(channel));
        }
        next.run(req).await
    }

    fn commerce_transport_router(ctx: AppContext, tenant: TenantContext) -> Router {
        commerce_transport_router_with_auth(ctx, tenant, None)
    }

    fn commerce_transport_router_with_auth(
        ctx: AppContext,
        tenant: TenantContext,
        auth: Option<AuthContext>,
    ) -> Router {
        commerce_transport_router_with_context(ctx, tenant, auth, None)
    }

    fn commerce_transport_router_with_context(
        ctx: AppContext,
        tenant: TenantContext,
        auth: Option<AuthContext>,
        channel: Option<ChannelContext>,
    ) -> Router {
        let routes = crate::controllers::routes();
        let mut router = Router::new();
        for handler in routes.handlers {
            router = router.route(&handler.uri, handler.method.with_state(ctx.clone()));
        }

        router.layer(from_fn_with_state(
            TransportRequestContext {
                tenant,
                auth,
                channel,
            },
            inject_transport_context,
        ))
    }

    #[tokio::test]
    async fn storefront_line_item_resolution_uses_backend_variant_title_and_price() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let service = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;

        let created = service
            .create_product(tenant_id, actor_id, product_input)
            .await
            .expect("product should be created");
        let published = service
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
        let pricing_service = PricingService::new(db.clone(), mock_transactional_event_bus());
        let pricing_context = pricing_context("EUR", 2);

        let resolved = resolve_store_line_item_input(
            &db,
            tenant_id,
            &pricing_service,
            &pricing_context,
            "de",
            "en",
            None,
            StoreAddCartLineItemInput {
                variant_id: variant.id,
                quantity: 2,
                metadata: json!({ "source": "store-line-item-test" }),
            },
        )
        .await
        .expect("store line item should resolve from backend catalog");

        assert_eq!(resolved.add_line_item.product_id, Some(published.id));
        assert_eq!(resolved.add_line_item.variant_id, Some(variant.id));
        assert_eq!(
            resolved.add_line_item.sku.as_deref(),
            Some("STOREFRONT-SKU-1")
        );
        assert_eq!(resolved.add_line_item.title, "Storefront Produkt / Default");
        assert_eq!(
            resolved.add_line_item.unit_price,
            Decimal::from_str("19.99").expect("valid decimal")
        );
        assert_eq!(resolved.add_line_item.quantity, 2);
        assert_eq!(
            resolved.add_line_item.metadata,
            json!({
                "seller": { "id": null, "scope": null },
                "source": "store-line-item-test"
            })
        );
    }

    #[tokio::test]
    async fn storefront_line_item_resolution_rejects_missing_price_for_cart_currency() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let service = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();

        let created = service
            .create_product(tenant_id, actor_id, storefront_product_input())
            .await
            .expect("product should be created");
        let published = service
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
        let pricing_service = PricingService::new(db.clone(), mock_transactional_event_bus());
        let pricing_context = pricing_context("USD", 1);

        let error = resolve_store_line_item_input(
            &db,
            tenant_id,
            &pricing_service,
            &pricing_context,
            "de",
            "en",
            None,
            StoreAddCartLineItemInput {
                variant_id: variant.id,
                quantity: 1,
                metadata: json!({}),
            },
        )
        .await
        .expect_err("store line item must reject missing price in cart currency");

        assert_eq!(
            error.to_string(),
            format!(
                "No storefront price for variant {} in currency USD",
                variant.id
            )
        );
    }

    #[tokio::test]
    async fn storefront_line_item_resolution_falls_back_to_first_product_translation_when_locale_missing(
    ) {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let service = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;

        let created = service
            .create_product(tenant_id, actor_id, product_input)
            .await
            .expect("product should be created");
        let published = service
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
        let pricing_service = PricingService::new(db.clone(), mock_transactional_event_bus());
        let pricing_context = pricing_context("EUR", 1);

        let resolved = resolve_store_line_item_input(
            &db,
            tenant_id,
            &pricing_service,
            &pricing_context,
            "fr",
            "en",
            None,
            StoreAddCartLineItemInput {
                variant_id: variant.id,
                quantity: 1,
                metadata: json!({}),
            },
        )
        .await
        .expect("store line item should fall back to an existing product translation");

        assert_eq!(resolved.add_line_item.product_id, Some(published.id));
        assert_eq!(resolved.add_line_item.variant_id, Some(variant.id));
        assert_eq!(
            resolved.add_line_item.sku.as_deref(),
            Some("STOREFRONT-SKU-1")
        );
        assert_eq!(resolved.add_line_item.title, "Storefront Product / Default");
        assert_eq!(
            resolved.add_line_item.unit_price,
            Decimal::from_str("19.99").expect("valid decimal")
        );
    }

    #[tokio::test]
    async fn storefront_line_item_resolution_returns_not_found_for_unknown_variant() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let pricing_service = PricingService::new(db.clone(), mock_transactional_event_bus());
        let pricing_context = pricing_context("EUR", 1);

        let error = resolve_store_line_item_input(
            &db,
            tenant_id,
            &pricing_service,
            &pricing_context,
            "de",
            "en",
            None,
            StoreAddCartLineItemInput {
                variant_id: Uuid::new_v4(),
                quantity: 1,
                metadata: json!({}),
            },
        )
        .await
        .expect_err("unknown variant must not resolve");

        assert_eq!(error.to_string(), "not found");
    }

    #[tokio::test]
    async fn storefront_line_item_resolution_rejects_quantity_above_channel_visible_inventory() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let service = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();

        let mut input = storefront_product_input();
        input.variants[0].inventory_quantity = 5;
        let created = service
            .create_product(tenant_id, actor_id, input)
            .await
            .expect("product should be created");
        let published = service
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
        let pricing_service = PricingService::new(db.clone(), mock_transactional_event_bus());
        let pricing_context = pricing_context("EUR", 1);
        set_stock_location_channel_visibility(&db, tenant_id, &["mobile-app"]).await;

        let error = resolve_store_line_item_input(
            &db,
            tenant_id,
            &pricing_service,
            &pricing_context,
            "de",
            "en",
            Some("web-store"),
            StoreAddCartLineItemInput {
                variant_id: variant.id,
                quantity: 1,
                metadata: json!({}),
            },
        )
        .await
        .expect_err("hidden inventory should reject storefront line item resolution");

        assert_eq!(
            error.to_string(),
            format!(
                "Variant {} does not have enough available inventory for the current channel",
                variant.id
            )
        );
    }

    #[tokio::test]
    async fn store_product_transport_uses_channel_visible_inventory() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let service = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let mut channel = sample_channel_context("web-store");
        channel.tenant_id = tenant_id;

        let mut input = storefront_product_input();
        input.variants[0].inventory_quantity = 7;
        let created = service
            .create_product(tenant_id, actor_id, input)
            .await
            .expect("product should be created");
        let published = service
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        set_stock_location_channel_visibility(&db, tenant_id, &["mobile-app"]).await;
        seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
        let request_context = RequestContext {
            tenant_id,
            user_id: None,
            channel_id: Some(channel.id),
            channel_slug: Some(channel.slug.clone()),
            channel_resolution_source: Some(ChannelResolutionSource::Host),
            locale: "de".to_string(),
        };

        let product = super::products::show_product(
            State(test_app_context(db)),
            tenant,
            request_context,
            Path(published.id),
        )
        .await
        .expect("store product handler should succeed")
        .0;

        assert_eq!(product.variants.len(), 1);
        assert_eq!(product.variants[0].inventory_quantity, 0);
        assert!(!product.variants[0].in_stock);
    }

    #[tokio::test]
    async fn store_cart_transport_uses_tristate_update_semantics_end_to_end() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let app = commerce_transport_router(test_app_context(db.clone()), tenant);

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "buyer@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        let create_status = create_response.status();
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        assert_eq!(
            create_status,
            StatusCode::CREATED,
            "unexpected create cart body: {}",
            String::from_utf8_lossy(&create_body)
        );

        let created: serde_json::Value =
            serde_json::from_slice(&create_body).expect("create cart response should be JSON");
        let cart_id = created["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");
        assert_eq!(created["cart"]["email"], json!("buyer@example.com"));
        assert_eq!(created["cart"]["locale_code"], json!("de"));
        assert_eq!(created["context"]["locale"], json!("de"));

        let update_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .header("x-medusa-locale", "en")
                    .body(Body::from(
                        json!({
                            "email": null,
                            "locale": null
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update cart request should succeed");
        let update_status = update_response.status();
        let update_body = to_bytes(update_response.into_body(), usize::MAX)
            .await
            .expect("update cart body should read");
        assert_eq!(
            update_status,
            StatusCode::OK,
            "unexpected update cart body: {}",
            String::from_utf8_lossy(&update_body)
        );

        let updated: serde_json::Value =
            serde_json::from_slice(&update_body).expect("update cart response should be JSON");
        assert_eq!(updated["cart"]["id"], json!(cart_id));
        assert!(updated["cart"]["email"].is_null());
        assert_eq!(updated["cart"]["locale_code"], json!("en"));
        assert_eq!(updated["context"]["locale"], json!("en"));
    }

    #[tokio::test]
    async fn store_cart_transport_rejects_currency_mismatch_for_region() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
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
                    metadata: json!({ "source": "store-cart-region-mismatch" }),
                },
            )
            .await
            .expect("region should be created");
        let app = commerce_transport_router(test_app_context(db.clone()), tenant);

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "buyer@example.com",
                            "region_id": region.id,
                            "currency_code": "usd",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should complete");
        let status = create_response.status();
        let body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let body_text = String::from_utf8_lossy(&body);
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "unexpected create cart body: {body_text}",
        );
        assert!(
            body_text.contains("USD"),
            "body should mention requested currency: {body_text}"
        );
        assert!(
            body_text.contains("EUR"),
            "body should mention region currency: {body_text}"
        );
        assert!(
            body_text.contains(&region.id.to_string()),
            "body should mention conflicting region: {body_text}"
        );
    }

    #[tokio::test]
    async fn store_shipping_options_transport_uses_cart_context_currency_over_query_drift() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let fulfillment = FulfillmentService::new(db.clone());
        let eur_option = fulfillment
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "EU Standard".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("9.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: None,
                    metadata: json!({ "source": "store-shipping-options-eur" }),
                },
            )
            .await
            .expect("EUR shipping option should be created");
        let usd_option = fulfillment
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "US Express".to_string(),
                    }],
                    currency_code: "usd".to_string(),
                    amount: Decimal::from_str("19.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: None,
                    metadata: json!({ "source": "store-shipping-options-usd" }),
                },
            )
            .await
            .expect("USD shipping option should be created");
        let app = commerce_transport_router(test_app_context(db), tenant);

        let create_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "buyer@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");

        let shipping_options_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/store/shipping-options?cart_id={cart_id}&currency_code=usd"
                    ))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("shipping options request should succeed");
        let status = shipping_options_response.status();
        let body = to_bytes(shipping_options_response.into_body(), usize::MAX)
            .await
            .expect("shipping options body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected shipping options body: {}",
            String::from_utf8_lossy(&body)
        );

        let shipping_options: serde_json::Value =
            serde_json::from_slice(&body).expect("shipping options response should be JSON");
        let options = shipping_options
            .as_array()
            .expect("shipping options should be an array");
        assert_eq!(options.len(), 1, "cart context should override query drift");
        assert_eq!(options[0]["id"], json!(eur_option.id));
        assert_eq!(options[0]["currency_code"], json!("EUR"));
        assert_ne!(options[0]["id"], json!(usd_option.id));
    }

    #[tokio::test]
    async fn store_cart_line_item_transport_resolves_backend_title_and_price() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
            .expect("published product must include variant");
        let app = commerce_transport_router(test_app_context(db.clone()), tenant);

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "buyer@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");

        let line_item_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": variant.id,
                            "quantity": 2,
                            "metadata": { "source": "transport-line-item-test" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should succeed");
        let line_item_status = line_item_response.status();
        let line_item_body = to_bytes(line_item_response.into_body(), usize::MAX)
            .await
            .expect("line item body should read");
        assert_eq!(
            line_item_status,
            StatusCode::OK,
            "unexpected add line item body: {}",
            String::from_utf8_lossy(&line_item_body)
        );

        let updated_cart: serde_json::Value =
            serde_json::from_slice(&line_item_body).expect("updated cart should be JSON");
        assert_eq!(
            updated_cart["line_items"][0]["variant_id"],
            json!(variant.id)
        );
        assert_eq!(
            updated_cart["line_items"][0]["product_id"],
            json!(published.id)
        );
        assert_eq!(
            updated_cart["line_items"][0]["sku"],
            json!("STOREFRONT-SKU-1")
        );
        assert_eq!(
            updated_cart["line_items"][0]["title"],
            json!("Storefront Produkt / Default")
        );
        assert_eq!(updated_cart["line_items"][0]["unit_price"], json!("19.99"));
        assert_eq!(updated_cart["line_items"][0]["quantity"], json!(2));
        assert_eq!(
            updated_cart["line_items"][0]["metadata"],
            json!({
                "seller": { "id": null, "scope": null },
                "source": "transport-line-item-test"
            })
        );
    }

    #[tokio::test]
    async fn store_cart_line_item_transport_returns_not_found_for_unknown_variant() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let app = commerce_transport_router(test_app_context(db), tenant);

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "buyer@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");

        let line_item_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": Uuid::new_v4(),
                            "quantity": 1,
                            "metadata": {}
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should complete");

        assert_eq!(line_item_response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn store_payment_collection_transport_reuses_active_collection_and_preserves_cart_context_metadata(
    ) {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
            .expect("published product must include variant");
        let app = commerce_transport_router(test_app_context(db.clone()), tenant);

        let create_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "buyer@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");
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
                    metadata: json!({ "source": "transport-checkout-test-shipping-option" }),
                },
            )
            .await
            .expect("shipping option should be created");
        let update_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "selected_shipping_option_id": shipping_option.id
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update cart request should succeed");
        assert_eq!(update_cart_response.status(), StatusCode::OK);

        let add_line_item_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": variant.id,
                            "quantity": 1,
                            "metadata": { "source": "transport-payment-test-line-item" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should succeed");
        assert_eq!(add_line_item_response.status(), StatusCode::OK);

        let create_collection_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/payment-collections")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .header("x-medusa-locale", "de")
                    .body(Body::from(
                        json!({
                            "cart_id": cart_id,
                            "metadata": { "source": "transport-payment-test" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create payment collection request should succeed");
        let create_collection_status = create_collection_response.status();
        let create_collection_body = to_bytes(create_collection_response.into_body(), usize::MAX)
            .await
            .expect("payment collection body should read");
        assert_eq!(
            create_collection_status,
            StatusCode::CREATED,
            "unexpected payment collection body: {}",
            String::from_utf8_lossy(&create_collection_body)
        );

        let first_collection: serde_json::Value = serde_json::from_slice(&create_collection_body)
            .expect("payment collection response should be JSON");
        assert_eq!(first_collection["status"], json!("pending"));
        assert_eq!(first_collection["currency_code"], json!("EUR"));
        assert_eq!(
            first_collection["metadata"]["source"],
            json!("transport-payment-test")
        );
        assert_eq!(
            first_collection["metadata"]["cart_context"]["locale"],
            json!("de")
        );
        assert_eq!(
            first_collection["metadata"]["cart_context"]["currency_code"],
            json!("EUR")
        );
        assert_eq!(
            first_collection["metadata"]["cart_context"]["email"],
            json!("buyer@example.com")
        );

        let reuse_collection_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/payment-collections")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .header("x-medusa-locale", "de")
                    .body(Body::from(
                        json!({
                            "cart_id": cart_id,
                            "metadata": { "source": "transport-payment-test-retry" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("retry payment collection request should succeed");
        let reuse_collection_status = reuse_collection_response.status();
        let reuse_collection_body = to_bytes(reuse_collection_response.into_body(), usize::MAX)
            .await
            .expect("reused payment collection body should read");
        assert_eq!(
            reuse_collection_status,
            StatusCode::OK,
            "unexpected reused payment collection body: {}",
            String::from_utf8_lossy(&reuse_collection_body)
        );

        let reused_collection: serde_json::Value = serde_json::from_slice(&reuse_collection_body)
            .expect("reused payment collection response should be JSON");
        assert_eq!(reused_collection["id"], first_collection["id"]);
        assert_eq!(reused_collection["metadata"], first_collection["metadata"]);
    }

    #[tokio::test]
    async fn store_checkout_transport_end_to_end_preserves_updated_cart_context() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
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
                    metadata: json!({ "source": "store-checkout-flow-region" }),
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
                    metadata: json!({ "source": "store-checkout-flow-shipping-option" }),
                },
            )
            .await
            .expect("shipping option should be created");
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
            .expect("published product must include variant");
        let app = commerce_transport_router(test_app_context(db), tenant);

        let create_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "buyer@example.com",
                            "currency_code": "eur",
                            "locale": "en"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");

        let update_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "checkout@example.com",
                            "region_id": region.id,
                            "country_code": "de",
                            "locale": "de",
                            "selected_shipping_option_id": shipping_option.id
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update cart request should succeed");
        let update_cart_status = update_cart_response.status();
        let update_cart_body = to_bytes(update_cart_response.into_body(), usize::MAX)
            .await
            .expect("update cart body should read");
        assert_eq!(
            update_cart_status,
            StatusCode::OK,
            "unexpected update cart body: {}",
            String::from_utf8_lossy(&update_cart_body)
        );
        let updated_cart: serde_json::Value =
            serde_json::from_slice(&update_cart_body).expect("update cart response should be JSON");
        assert_eq!(updated_cart["cart"]["email"], json!("checkout@example.com"));
        assert_eq!(updated_cart["cart"]["country_code"], json!("DE"));
        assert_eq!(updated_cart["cart"]["locale_code"], json!("de"));
        assert_eq!(updated_cart["cart"]["region_id"], json!(region.id));
        assert_eq!(
            updated_cart["cart"]["selected_shipping_option_id"],
            json!(null)
        );
        assert_eq!(updated_cart["context"]["locale"], json!("de"));
        assert_eq!(updated_cart["context"]["region"]["id"], json!(region.id));

        let add_line_item_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": variant.id,
                            "quantity": 1,
                            "metadata": { "source": "store-checkout-flow-line-item" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should succeed");
        assert_eq!(add_line_item_response.status(), StatusCode::OK);

        let shipping_options_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/store/shipping-options?cart_id={cart_id}&currency_code=usd"
                    ))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("shipping options request should succeed");
        let shipping_options_status = shipping_options_response.status();
        let shipping_options_body = to_bytes(shipping_options_response.into_body(), usize::MAX)
            .await
            .expect("shipping options body should read");
        assert_eq!(
            shipping_options_status,
            StatusCode::OK,
            "unexpected shipping options body: {}",
            String::from_utf8_lossy(&shipping_options_body)
        );
        let shipping_options: serde_json::Value = serde_json::from_slice(&shipping_options_body)
            .expect("shipping options response should be JSON");
        let options = shipping_options
            .as_array()
            .expect("shipping options should be an array");
        assert_eq!(options.len(), 1);
        assert_eq!(options[0]["id"], json!(shipping_option.id));

        let payment_collection_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/payment-collections")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .header("x-medusa-locale", "de")
                    .body(Body::from(
                        json!({
                            "cart_id": cart_id,
                            "metadata": { "source": "store-checkout-flow-payment" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create payment collection request should succeed");
        let payment_collection_status = payment_collection_response.status();
        let payment_collection_body = to_bytes(payment_collection_response.into_body(), usize::MAX)
            .await
            .expect("payment collection body should read");
        assert_eq!(
            payment_collection_status,
            StatusCode::CREATED,
            "unexpected payment collection body: {}",
            String::from_utf8_lossy(&payment_collection_body)
        );
        let payment_collection: serde_json::Value =
            serde_json::from_slice(&payment_collection_body)
                .expect("payment collection response should be JSON");
        assert_eq!(
            payment_collection["metadata"]["cart_context"]["region_id"],
            json!(region.id)
        );
        assert_eq!(
            payment_collection["metadata"]["cart_context"]["country_code"],
            json!("DE")
        );
        assert_eq!(
            payment_collection["metadata"]["cart_context"]["locale"],
            json!("de")
        );
        assert_eq!(
            payment_collection["metadata"]["cart_context"]["selected_shipping_option_id"],
            json!(shipping_option.id)
        );
        assert_eq!(
            payment_collection["metadata"]["cart_context"]["email"],
            json!("checkout@example.com")
        );

        let complete_checkout_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/complete"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "create_fulfillment": false,
                            "metadata": { "source": "store-checkout-flow-complete" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("complete checkout request should succeed");
        let complete_checkout_status = complete_checkout_response.status();
        let complete_checkout_body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
            .await
            .expect("complete checkout body should read");
        assert_eq!(
            complete_checkout_status,
            StatusCode::OK,
            "unexpected complete checkout body: {}",
            String::from_utf8_lossy(&complete_checkout_body)
        );
        let completed: serde_json::Value = serde_json::from_slice(&complete_checkout_body)
            .expect("complete checkout response should be JSON");
        assert_eq!(completed["cart"]["status"], json!("completed"));
        assert_eq!(completed["cart"]["country_code"], json!("DE"));
        assert_eq!(completed["cart"]["locale_code"], json!("de"));
        assert_eq!(completed["cart"]["region_id"], json!(region.id));
        assert_eq!(
            completed["cart"]["selected_shipping_option_id"],
            json!(shipping_option.id)
        );
        assert_eq!(completed["context"]["locale"], json!("de"));
        assert_eq!(completed["context"]["region"]["id"], json!(region.id));
        assert_eq!(completed["order"]["status"], json!("paid"));
        assert_eq!(completed["cart"]["tax_included"], json!(true));
        assert_eq!(completed["order"]["tax_included"], json!(true));
        assert_eq!(
            completed["cart"]["tax_total"],
            completed["order"]["tax_total"]
        );
        assert_eq!(
            completed["cart"]["tax_lines"][0]["provider_id"],
            json!("region_default")
        );
        assert_eq!(
            completed["order"]["tax_lines"][0]["provider_id"],
            json!("region_default")
        );
        assert_eq!(
            completed["payment_collection"]["id"],
            payment_collection["id"]
        );
        assert_eq!(completed["payment_collection"]["status"], json!("captured"));
        assert_eq!(
            completed["payment_collection"]["amount"],
            completed["order"]["total_amount"]
        );
        assert!(completed["fulfillment"].is_null());
    }

    #[tokio::test]
    async fn store_checkout_transport_completes_guest_cart_with_existing_payment_and_no_fulfillment(
    ) {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_CREATE, Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
            .expect("published product must include variant");
        let app = commerce_transport_router(test_app_context(db.clone()), tenant.clone());
        let authed_app =
            commerce_transport_router_with_auth(test_app_context(db.clone()), tenant, Some(auth));

        let create_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "guest@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");
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
                    metadata: json!({ "source": "transport-checkout-test-shipping-option" }),
                },
            )
            .await
            .expect("shipping option should be created");
        let update_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "selected_shipping_option_id": shipping_option.id
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update cart request should succeed");
        assert_eq!(update_cart_response.status(), StatusCode::OK);

        let add_line_item_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": variant.id,
                            "quantity": 1,
                            "metadata": { "source": "transport-checkout-test-line-item" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should succeed");
        assert_eq!(add_line_item_response.status(), StatusCode::OK);

        let payment_collection_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/payment-collections")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .header("x-medusa-locale", "de")
                    .body(Body::from(
                        json!({
                            "cart_id": cart_id,
                            "metadata": { "source": "transport-checkout-test-payment" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create payment collection request should succeed");
        assert_eq!(payment_collection_response.status(), StatusCode::CREATED);

        let complete_checkout_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/complete"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "create_fulfillment": false,
                            "metadata": { "source": "transport-checkout-test-complete" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("complete checkout request should succeed");
        let complete_checkout_status = complete_checkout_response.status();
        let complete_checkout_body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
            .await
            .expect("complete checkout body should read");
        assert_eq!(
            complete_checkout_status,
            StatusCode::OK,
            "unexpected complete checkout body: {}",
            String::from_utf8_lossy(&complete_checkout_body)
        );

        let completed: serde_json::Value = serde_json::from_slice(&complete_checkout_body)
            .expect("complete checkout response should be JSON");
        assert_eq!(completed["cart"]["status"], json!("completed"));
        assert_eq!(completed["order"]["status"], json!("paid"));
        assert_eq!(completed["payment_collection"]["status"], json!("captured"));
        assert!(completed["fulfillment"].is_null());
        assert_eq!(completed["context"]["locale"], json!("de"));

        let get_order_response = authed_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/store/orders/{}",
                        completed["order"]["id"]
                            .as_str()
                            .expect("order id should exist")
                    ))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("get order request should complete");
        assert_eq!(get_order_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn store_shipping_options_transport_rejects_customer_owned_cart_for_foreign_customer() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let owner_user_id = Uuid::new_v4();
        let other_user_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let owner_auth = AuthContext {
            user_id: owner_user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let other_auth = AuthContext {
            user_id: other_user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        create_customer_for_user(&db, tenant_id, owner_user_id, "owner@example.com").await;
        create_customer_for_user(&db, tenant_id, other_user_id, "other@example.com").await;

        let owner_app = commerce_transport_router_with_auth(
            test_app_context(db.clone()),
            tenant.clone(),
            Some(owner_auth),
        );
        let other_app =
            commerce_transport_router_with_auth(test_app_context(db), tenant, Some(other_auth));

        let create_cart_response = owner_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "owner@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");

        let shipping_options_response = other_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/store/shipping-options?cart_id={cart_id}"))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("shipping options request should complete");
        let status = shipping_options_response.status();
        let body = to_bytes(shipping_options_response.into_body(), usize::MAX)
            .await
            .expect("shipping options body should read");
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "unexpected shipping options body: {}",
            String::from_utf8_lossy(&body)
        );
    }

    #[tokio::test]
    async fn store_checkout_transport_rejects_customer_owned_cart_without_auth() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let owner_user_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let owner_auth = AuthContext {
            user_id: owner_user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_CREATE, Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        create_customer_for_user(&db, tenant_id, owner_user_id, "owner@example.com").await;

        let owner_app = commerce_transport_router_with_auth(
            test_app_context(db.clone()),
            tenant.clone(),
            Some(owner_auth),
        );
        let guest_app = commerce_transport_router(test_app_context(db), tenant);

        let create_cart_response = owner_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "owner@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");

        let complete_checkout_response = guest_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/complete"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "create_fulfillment": false,
                            "metadata": { "source": "transport-checkout-owner-guard" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("complete checkout request should complete");
        let status = complete_checkout_response.status();
        let body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
            .await
            .expect("complete checkout body should read");
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "unexpected complete checkout body: {}",
            String::from_utf8_lossy(&body)
        );
    }

    #[tokio::test]
    async fn store_payment_collection_transport_returns_not_found_for_unknown_cart() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let app = commerce_transport_router(test_app_context(db), tenant);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/payment-collections")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "cart_id": Uuid::new_v4(),
                            "metadata": { "source": "unknown-cart-payment-guard" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("payment collection request should complete");

        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("payment collection body should read");
        assert_eq!(status, StatusCode::NOT_FOUND);
        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("payment collection error should be JSON");
        assert_eq!(payload["error"], json!("not_found"));
    }

    #[tokio::test]
    async fn store_checkout_transport_rejects_payment_collection_for_completed_cart() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let app = commerce_transport_router(test_app_context(db.clone()), tenant);

        let create_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "guest@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");

        let actor_id = Uuid::new_v4();
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
            .expect("published product must include variant");

        let add_line_item_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": variant.id,
                            "quantity": 1,
                            "metadata": { "source": "completed-cart-payment-guard" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should succeed");
        assert_eq!(add_line_item_response.status(), StatusCode::OK);

        let cart_service = CartService::new(db.clone());
        let cart_uuid = Uuid::parse_str(cart_id).expect("cart id should be valid uuid");
        let completed = cart_service
            .complete_cart(tenant_id, cart_uuid)
            .await
            .expect("cart should transition to completed");
        assert_eq!(completed.status, "completed");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/payment-collections")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "cart_id": cart_id,
                            "metadata": { "source": "completed-cart-payment-guard" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("payment collection request should complete");

        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("payment collection body should read");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("payment collection error should be JSON");
        assert_eq!(payload["error"], json!("Bad Request"));
        assert_eq!(
            payload["description"],
            json!("Cannot create payment collection for completed cart")
        );
    }

    #[tokio::test]
    async fn store_checkout_transport_carries_cart_channel_snapshot_into_order() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
            .expect("published product must include variant");
        let mut channel = sample_channel_context("marketplace-eu");
        channel.tenant_id = tenant_id;
        let channel_id = channel.id;
        seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
        let app = commerce_transport_router_with_context(
            test_app_context(db.clone()),
            tenant,
            None,
            Some(channel),
        );

        let create_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "guest@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");
        assert_eq!(created_cart["cart"]["channel_id"], json!(channel_id));
        assert_eq!(
            created_cart["cart"]["channel_slug"],
            json!("marketplace-eu")
        );
        let shipping_option = FulfillmentService::new(db.clone())
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Channel Shipping".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("9.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: None,
                    metadata: json!({ "source": "channel-checkout-shipping-option" }),
                },
            )
            .await
            .expect("shipping option should be created");
        let update_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "selected_shipping_option_id": shipping_option.id
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update cart request should succeed");
        assert_eq!(update_cart_response.status(), StatusCode::OK);

        let add_line_item_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": variant.id,
                            "quantity": 1,
                            "metadata": { "source": "channel-checkout-line-item" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should succeed");
        assert_eq!(add_line_item_response.status(), StatusCode::OK);

        let payment_collection_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/payment-collections")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "cart_id": cart_id,
                            "metadata": { "source": "channel-checkout-payment" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create payment collection request should succeed");
        assert_eq!(payment_collection_response.status(), StatusCode::CREATED);

        let complete_checkout_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/complete"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "create_fulfillment": false,
                            "metadata": { "source": "channel-checkout-complete" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("complete checkout request should succeed");
        assert_eq!(complete_checkout_response.status(), StatusCode::OK);

        let complete_checkout_body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
            .await
            .expect("complete checkout body should read");
        let completed: serde_json::Value = serde_json::from_slice(&complete_checkout_body)
            .expect("complete checkout response should be JSON");
        assert_eq!(completed["order"]["channel_id"], json!(channel_id));
        assert_eq!(completed["order"]["channel_slug"], json!("marketplace-eu"));
    }

    #[tokio::test]
    async fn store_order_transport_returns_customer_owned_order_after_checkout() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_CREATE, Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let customer_id =
            create_customer_for_user(&db, tenant_id, actor_id, "customer@example.com").await;
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
            .expect("published product must include variant");
        let app =
            commerce_transport_router_with_auth(test_app_context(db.clone()), tenant, Some(auth));

        let create_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "customer@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");
        assert_eq!(created_cart["cart"]["customer_id"], json!(customer_id));
        let shipping_option = FulfillmentService::new(db.clone())
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Order Shipping".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("9.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: None,
                    metadata: json!({ "source": "transport-order-test-shipping-option" }),
                },
            )
            .await
            .expect("shipping option should be created");
        let update_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "selected_shipping_option_id": shipping_option.id
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update cart request should succeed");
        assert_eq!(update_cart_response.status(), StatusCode::OK);

        let add_line_item_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": variant.id,
                            "quantity": 1,
                            "metadata": { "source": "transport-order-test-line-item" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should succeed");
        assert_eq!(add_line_item_response.status(), StatusCode::OK);

        let payment_collection_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/payment-collections")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .header("x-medusa-locale", "de")
                    .body(Body::from(
                        json!({
                            "cart_id": cart_id,
                            "metadata": { "source": "transport-order-test-payment" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create payment collection request should succeed");
        assert_eq!(payment_collection_response.status(), StatusCode::CREATED);

        let complete_checkout_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/complete"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "create_fulfillment": false,
                            "metadata": { "source": "transport-order-test-complete" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("complete checkout request should succeed");
        assert_eq!(complete_checkout_response.status(), StatusCode::OK);
        let complete_checkout_body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
            .await
            .expect("complete checkout body should read");
        let completed: serde_json::Value = serde_json::from_slice(&complete_checkout_body)
            .expect("complete checkout response should be JSON");
        let order_id = completed["order"]["id"]
            .as_str()
            .expect("order id should be returned");

        let get_order_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/store/orders/{order_id}"))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("get order request should succeed");
        let get_order_status = get_order_response.status();
        let get_order_body = to_bytes(get_order_response.into_body(), usize::MAX)
            .await
            .expect("get order body should read");
        assert_eq!(
            get_order_status,
            StatusCode::OK,
            "unexpected get order body: {}",
            String::from_utf8_lossy(&get_order_body)
        );

        let order: serde_json::Value =
            serde_json::from_slice(&get_order_body).expect("order response should be JSON");
        assert_eq!(order["id"], completed["order"]["id"]);
        assert_eq!(order["customer_id"], json!(customer_id));
        assert_eq!(order["status"], json!("paid"));
        assert_eq!(order["currency_code"], json!("EUR"));
        assert_eq!(
            order["subtotal_amount"],
            completed["order"]["subtotal_amount"]
        );
        assert_eq!(order["total_amount"], completed["order"]["total_amount"]);
        assert_eq!(order["tax_included"], completed["order"]["tax_included"]);
        assert_eq!(order["tax_total"], completed["order"]["tax_total"]);
        assert_eq!(order["tax_lines"], completed["order"]["tax_lines"]);
        assert_eq!(
            order["tax_lines"]
                .as_array()
                .expect("tax lines array")
                .len(),
            2
        );
        assert_eq!(
            order["tax_lines"][0]["provider_id"],
            completed["order"]["tax_lines"][0]["provider_id"]
        );
        assert!(order["tax_lines"][0]["line_item_id"].as_str().is_some());
        assert!(order["tax_lines"][0]["shipping_option_id"].is_null());
        assert!(order["tax_lines"][1]["line_item_id"].is_null());
        assert!(order["tax_lines"][1]["shipping_option_id"]
            .as_str()
            .is_some());
    }

    #[tokio::test]
    async fn store_cart_transport_returns_typed_adjustments_and_totals() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let created = catalog
            .create_product(tenant_id, actor_id, storefront_product_input())
            .await
            .expect("product should be created");
        let published = catalog
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
        let cart_service = CartService::new(db.clone());
        let app = commerce_transport_router(test_app_context(db.clone()), tenant);
        let cart = cart_service
            .create_cart(
                tenant_id,
                CreateCartInput {
                    customer_id: None,
                    email: Some("buyer@example.com".to_string()),
                    region_id: None,
                    country_code: None,
                    currency_code: "eur".to_string(),
                    metadata: json!({ "source": "store-cart-adjustment-cart" }),
                    locale_code: Some("de".to_string()),
                    selected_shipping_option_id: None,
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
                    title: variant.title.clone(),
                    quantity: 1,
                    unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                    metadata: json!({ "source": "store-cart-adjustment-line-item" }),
                },
            )
            .await
            .expect("line item should be added");
        let cart_id = cart.id;
        let line_item_id = cart.line_items[0].id;

        cart_service
            .set_adjustments(
                tenant_id,
                cart_id,
                vec![SetCartAdjustmentInput {
                    line_item_id: Some(line_item_id),
                    source_type: "Promotion".to_string(),
                    source_id: Some("promo-store".to_string()),
                    amount: Decimal::from_str("4.99").expect("valid decimal"),
                    metadata: json!({
                        "rule_code": "store-adjustment",
                        "display_label": "Store promotion"
                    }),
                }],
            )
            .await
            .expect("cart adjustment should be stored");

        let get_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("get cart request should succeed");
        let get_cart_status = get_cart_response.status();
        let get_cart_body = to_bytes(get_cart_response.into_body(), usize::MAX)
            .await
            .expect("get cart body should read");
        assert_eq!(
            get_cart_status,
            StatusCode::OK,
            "unexpected get cart adjustment body: {}",
            String::from_utf8_lossy(&get_cart_body)
        );

        let cart: serde_json::Value =
            serde_json::from_slice(&get_cart_body).expect("cart response should be JSON");
        assert_eq!(cart["subtotal_amount"], json!("19.99"));
        assert_eq!(cart["adjustment_total"], json!("4.99"));
        assert_eq!(cart["total_amount"], json!("15"));
        assert_eq!(cart["adjustments"][0]["line_item_id"], json!(line_item_id));
        assert_eq!(cart["adjustments"][0]["source_type"], json!("promotion"));
        assert_eq!(cart["adjustments"][0]["source_id"], json!("promo-store"));
        assert_eq!(cart["adjustments"][0]["amount"], json!("4.99"));
        assert_eq!(cart["adjustments"][0]["currency_code"], json!("EUR"));
        assert_eq!(
            cart["adjustments"][0]["metadata"],
            json!({ "rule_code": "store-adjustment" })
        );
    }

    #[tokio::test]
    async fn store_cart_transport_returns_shipping_total_and_shipping_scoped_promotion() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let created = catalog
            .create_product(tenant_id, actor_id, storefront_product_input())
            .await
            .expect("product should be created");
        let published = catalog
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
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
                    metadata: json!({ "source": "store-cart-shipping-promotion" }),
                },
            )
            .await
            .expect("shipping option should be created");
        let cart_service = CartService::new(db.clone());
        let app = commerce_transport_router(test_app_context(db.clone()), tenant);
        let cart = cart_service
            .create_cart(
                tenant_id,
                CreateCartInput {
                    customer_id: None,
                    email: Some("buyer@example.com".to_string()),
                    region_id: None,
                    country_code: None,
                    currency_code: "eur".to_string(),
                    metadata: json!({ "source": "store-cart-shipping-promotion" }),
                    locale_code: Some("de".to_string()),
                    selected_shipping_option_id: Some(shipping_option.id),
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
                    title: variant.title.clone(),
                    quantity: 1,
                    unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                    metadata: json!({ "source": "store-cart-shipping-promotion-line-item" }),
                },
            )
            .await
            .expect("line item should be added");
        let cart_id = cart.id;

        cart_service
            .apply_fixed_shipping_promotion(
                tenant_id,
                cart_id,
                "promo-shipping-store",
                Decimal::from_str("4.99").expect("valid decimal"),
                json!({
                    "campaign": "shipping-half-off",
                    "display_label": "Shipping half off"
                }),
            )
            .await
            .expect("shipping promotion should be stored");

        let get_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("get cart request should succeed");
        let get_cart_status = get_cart_response.status();
        let get_cart_body = to_bytes(get_cart_response.into_body(), usize::MAX)
            .await
            .expect("get cart body should read");
        assert_eq!(
            get_cart_status,
            StatusCode::OK,
            "unexpected get cart shipping promotion body: {}",
            String::from_utf8_lossy(&get_cart_body)
        );

        let cart: serde_json::Value =
            serde_json::from_slice(&get_cart_body).expect("cart response should be JSON");
        assert_eq!(cart["subtotal_amount"], json!("19.99"));
        assert_eq!(cart["shipping_total"], json!("9.99"));
        assert_eq!(cart["adjustment_total"], json!("4.99"));
        assert_eq!(cart["total_amount"], json!("24.99"));
        assert_eq!(cart["adjustments"][0]["line_item_id"], json!(null));
        assert_eq!(cart["adjustments"][0]["source_type"], json!("promotion"));
        assert_eq!(
            cart["adjustments"][0]["source_id"],
            json!("promo-shipping-store")
        );
        assert_eq!(cart["adjustments"][0]["amount"], json!("4.99"));
        assert_eq!(cart["adjustments"][0]["currency_code"], json!("EUR"));
        assert_eq!(
            cart["adjustments"][0]["metadata"],
            json!({ "campaign": "shipping-half-off", "kind": "fixed_discount", "scope": "shipping", "fixed_amount": "4.99" })
        );
    }

    #[tokio::test]
    async fn store_order_transport_rejects_order_for_another_customer() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let owner_user_id = Uuid::new_v4();
        let other_user_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let owner_auth = AuthContext {
            user_id: owner_user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_CREATE, Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let other_auth = AuthContext {
            user_id: other_user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        create_customer_for_user(&db, tenant_id, owner_user_id, "owner@example.com").await;
        create_customer_for_user(&db, tenant_id, other_user_id, "other@example.com").await;
        let actor_id = owner_user_id;
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
            .expect("published product must include variant");
        let owner_app = commerce_transport_router_with_auth(
            test_app_context(db.clone()),
            tenant.clone(),
            Some(owner_auth),
        );
        let other_app = commerce_transport_router_with_auth(
            test_app_context(db.clone()),
            tenant,
            Some(other_auth),
        );

        let create_cart_response = owner_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "owner@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");
        let shipping_option = FulfillmentService::new(db.clone())
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Ownership Shipping".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("9.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: None,
                    metadata: json!({ "source": "transport-order-ownership-shipping-option" }),
                },
            )
            .await
            .expect("shipping option should be created");
        let update_cart_response = owner_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "selected_shipping_option_id": shipping_option.id
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update cart request should succeed");
        assert_eq!(update_cart_response.status(), StatusCode::OK);

        let add_line_item_response = owner_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": variant.id,
                            "quantity": 1,
                            "metadata": { "source": "transport-order-ownership-line-item" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should succeed");
        assert_eq!(add_line_item_response.status(), StatusCode::OK);

        let payment_collection_response = owner_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/payment-collections")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .header("x-medusa-locale", "de")
                    .body(Body::from(
                        json!({
                            "cart_id": cart_id,
                            "metadata": { "source": "transport-order-ownership-payment" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create payment collection request should succeed");
        assert_eq!(payment_collection_response.status(), StatusCode::CREATED);

        let complete_checkout_response = owner_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/complete"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "create_fulfillment": false,
                            "metadata": { "source": "transport-order-ownership-complete" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("complete checkout request should succeed");
        assert_eq!(complete_checkout_response.status(), StatusCode::OK);
        let complete_checkout_body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
            .await
            .expect("complete checkout body should read");
        let completed: serde_json::Value = serde_json::from_slice(&complete_checkout_body)
            .expect("complete checkout response should be JSON");
        let order_id = completed["order"]["id"]
            .as_str()
            .expect("order id should be returned");

        let get_order_response = other_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/store/orders/{order_id}"))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("get order request should complete");
        let get_order_status = get_order_response.status();
        let get_order_body = to_bytes(get_order_response.into_body(), usize::MAX)
            .await
            .expect("get order body should read");
        assert_eq!(
            get_order_status,
            StatusCode::UNAUTHORIZED,
            "unexpected get order body: {}",
            String::from_utf8_lossy(&get_order_body)
        );
    }

    #[tokio::test]
    async fn store_customer_me_transport_returns_customer_for_authenticated_user() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let customer_id =
            create_customer_for_user(&db, tenant_id, user_id, "customer-me@example.com").await;
        let app = commerce_transport_router_with_auth(test_app_context(db), tenant, Some(auth));

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/store/customers/me")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("get me request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("get me body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected get me body: {}",
            String::from_utf8_lossy(&body)
        );

        let customer: serde_json::Value =
            serde_json::from_slice(&body).expect("get me response should be JSON");
        assert_eq!(customer["id"], json!(customer_id));
        assert_eq!(customer["user_id"], json!(user_id));
        assert_eq!(customer["email"], json!("customer-me@example.com"));
        assert_eq!(customer["locale"], json!("de"));
    }

    #[tokio::test]
    async fn store_cart_transport_rejects_customer_owned_cart_for_another_customer() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let owner_user_id = Uuid::new_v4();
        let other_user_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let owner_auth = AuthContext {
            user_id: owner_user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let other_auth = AuthContext {
            user_id: other_user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let owner_customer_id =
            create_customer_for_user(&db, tenant_id, owner_user_id, "cart-owner@example.com").await;
        create_customer_for_user(&db, tenant_id, other_user_id, "cart-other@example.com").await;
        let owner_app = commerce_transport_router_with_auth(
            test_app_context(db.clone()),
            tenant.clone(),
            Some(owner_auth),
        );
        let other_app =
            commerce_transport_router_with_auth(test_app_context(db), tenant, Some(other_auth));

        let create_cart_response = owner_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "cart-owner@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");
        assert_eq!(
            created_cart["cart"]["customer_id"],
            json!(owner_customer_id)
        );

        let get_cart_response = other_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("get cart request should complete");
        let get_cart_status = get_cart_response.status();
        let get_cart_body = to_bytes(get_cart_response.into_body(), usize::MAX)
            .await
            .expect("get cart body should read");
        assert_eq!(
            get_cart_status,
            StatusCode::UNAUTHORIZED,
            "unexpected get cart body: {}",
            String::from_utf8_lossy(&get_cart_body)
        );
    }

