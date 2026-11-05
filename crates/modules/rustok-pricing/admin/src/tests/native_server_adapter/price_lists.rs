use std::str::FromStr;
use serde_json::json;
use uuid::Uuid;
use rustok_api::{AuthContext, Permission, TenantContext};
use rustok_test_utils::db::setup_test_db;
use crate::core::*;
use crate::model::*;
use crate::transport::native_server_adapter::*;
use super::support::support;
use super::support::*;

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_rule_updates_active_option() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db);

        let option = update_price_list_rule_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListRuleDraft {
                adjustment_percent: "15".to_string(),
            },
        )
        .await
        .expect("rule update should succeed");

        assert_eq!(option.id, price_list_id.to_string());
        assert_eq!(option.rule_kind.as_deref(), Some("percentage_discount"));
        assert_eq!(option.adjustment_percent.as_deref(), Some("15"));
    }

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_rule_clears_active_option_metadata() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());

        update_price_list_rule_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListRuleDraft {
                adjustment_percent: "15".to_string(),
            },
        )
        .await
        .expect("rule update should succeed");

        let option = update_price_list_rule_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListRuleDraft {
                adjustment_percent: "".to_string(),
            },
        )
        .await
        .expect("rule clear should succeed");

        assert_eq!(option.id, price_list_id.to_string());
        assert!(option.rule_kind.is_none());
        assert!(option.adjustment_percent.is_none());
    }

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_rule_rejects_draft_price_list() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let price_list_id = create_price_list(&db, tenant_id, "draft").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db);

        let error = update_price_list_rule_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListRuleDraft {
                adjustment_percent: "15".to_string(),
            },
        )
        .await
        .expect_err("draft list should be rejected");

        assert!(
            error
                .to_string()
                .contains("price_list_id must reference an active price list")
        );
    }

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_rule_rejects_future_price_list() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let price_list_id = create_future_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db);

        let error = update_price_list_rule_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListRuleDraft {
                adjustment_percent: "15".to_string(),
            },
        )
        .await
        .expect_err("future list should be rejected");

        assert!(error.to_string().contains("not active yet"));
    }

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_rule_rejects_expired_price_list() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let price_list_id = create_expired_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db);

        let error = update_price_list_rule_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListRuleDraft {
                adjustment_percent: "15".to_string(),
            },
        )
        .await
        .expect_err("expired list should be rejected");

        assert!(error.to_string().contains("already expired"));
    }

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_scope_updates_active_option_and_rows() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        seed_channel_context(&db, tenant_id, channel_id, "web-store").await;
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = test_catalog(db.clone());
        let (product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let pricing =
            rustok_pricing::PricingService::new(db.clone(), mock_transactional_event_bus());

        pricing
            .set_price_list_tier(
                tenant_id,
                actor_id,
                variant_id,
                price_list_id,
                "USD",
                rust_decimal::Decimal::from_str("81.00").expect("valid decimal"),
                Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal")),
                None,
                None,
            )
            .await
            .expect("price-list row should be created");

        let option = update_price_list_scope_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListScopeDraft {
                channel_id: channel_id.to_string(),
                channel_slug: "web-store".to_string(),
            },
        )
        .await
        .expect("scope update should succeed");

        assert_eq!(option.id, price_list_id.to_string());
        assert_eq!(option.channel_id, Some(channel_id.to_string()));
        assert_eq!(option.channel_slug.as_deref(), Some("web-store"));

        let detail = rustok_pricing::PricingService::new(db, mock_transactional_event_bus())
            .get_admin_product_pricing_with_locale_fallback(
                pricing_projection_test_context(tenant_id, "en"),
                product_id,
                Some("en"),
                Some(price_list_id),
            )
            .await
            .expect("admin detail should load");

        let variant = detail
            .variants
            .into_iter()
            .find(|variant| variant.id == variant_id)
            .expect("variant should be present");
        let scoped_price = variant
            .prices
            .into_iter()
            .find(|price| price.price_list_id == Some(price_list_id))
            .expect("price-list row should exist");

        assert_eq!(scoped_price.channel_id, Some(channel_id));
        assert_eq!(scoped_price.channel_slug.as_deref(), Some("web-store"));
    }

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_scope_clears_active_option_and_rows() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        seed_channel_context(&db, tenant_id, channel_id, "web-store").await;
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = test_catalog(db.clone());
        let (product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let pricing =
            rustok_pricing::PricingService::new(db.clone(), mock_transactional_event_bus());

        pricing
            .set_price_list_tier(
                tenant_id,
                actor_id,
                variant_id,
                price_list_id,
                "USD",
                rust_decimal::Decimal::from_str("81.00").expect("valid decimal"),
                Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal")),
                None,
                None,
            )
            .await
            .expect("price-list row should be created");

        update_price_list_scope_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListScopeDraft {
                channel_id: channel_id.to_string(),
                channel_slug: "web-store".to_string(),
            },
        )
        .await
        .expect("scope update should succeed");

        let option = update_price_list_scope_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListScopeDraft {
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect("scope clear should succeed");

        assert!(option.channel_id.is_none());
        assert!(option.channel_slug.is_none());

        let detail = rustok_pricing::PricingService::new(db, mock_transactional_event_bus())
            .get_admin_product_pricing_with_locale_fallback(
                pricing_projection_test_context(tenant_id, "en"),
                product_id,
                Some("en"),
                Some(price_list_id),
            )
            .await
            .expect("admin detail should load");

        let variant = detail
            .variants
            .into_iter()
            .find(|variant| variant.id == variant_id)
            .expect("variant should be present");
        let scoped_price = variant
            .prices
            .into_iter()
            .find(|price| price.price_list_id == Some(price_list_id))
            .expect("price-list row should exist");

        assert_eq!(scoped_price.channel_id, None);
        assert_eq!(scoped_price.channel_slug, None);
    }

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_scope_refreshes_active_price_list_selector() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let other_channel_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        seed_channel_context(&db, tenant_id, channel_id, "web-store").await;
        seed_channel_context(&db, tenant_id, other_channel_id, "mobile-app").await;
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let request_context = rustok_api::RequestContext {
            tenant_id,
            user_id: Some(actor_id),
            channel_id: Some(other_channel_id),
            channel_slug: Some("mobile-app".to_string()),
            channel_resolution_source: None,
            locale: "en".to_string(),
            correlation_id: "pricing-admin-selector-regression".to_string(),
        };
        let app_ctx = test_app_context(db);

        update_price_list_scope_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListScopeDraft {
                channel_id: channel_id.to_string(),
                channel_slug: "web-store".to_string(),
            },
        )
        .await
        .expect("scope update should succeed");

        let scoped_lists = list_active_price_lists_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            &request_context,
            Some(channel_id.to_string()),
            Some("web-store".to_string()),
        )
        .await
        .expect("scoped list should load");
        assert!(
            scoped_lists
                .iter()
                .any(|item| item.id == price_list_id.to_string())
        );

        let mismatched_lists = list_active_price_lists_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            &request_context,
            Some(other_channel_id.to_string()),
            Some("mobile-app".to_string()),
        )
        .await
        .expect("mismatched list should load");
        assert!(
            !mismatched_lists
                .iter()
                .any(|item| item.id == price_list_id.to_string())
        );

        // A different host channel cannot supply the missing half of an explicit selector.
        for (selected_id, selected_slug) in [
            (Some(channel_id.to_string()), None),
            (None, Some("web-store".to_string())),
        ] {
            let lists = list_active_price_lists_native_with_context(
                &app_ctx,
                &auth,
                &tenant,
                &request_context,
                selected_id,
                selected_slug,
            )
            .await
            .expect("the owner should complete an explicit selector independently of the host");
            assert!(
                lists
                    .iter()
                    .any(|item| item.id == price_list_id.to_string())
            );
        }
        let contradictory = list_active_price_lists_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            &request_context,
            Some(channel_id.to_string()),
            Some("mobile-app".to_string()),
        )
        .await;
        assert!(
            contradictory.is_err(),
            "contradictory selectors must refuse, not broaden"
        );
        let current = list_active_price_lists_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            &request_context,
            None,
            None,
        )
        .await
        .expect("omitted selectors use the trusted current channel");
        assert!(
            !current
                .iter()
                .any(|item| item.id == price_list_id.to_string())
        );
        let mut foreign_request = request_context.clone();
        foreign_request.tenant_id = Uuid::new_v4();
        assert!(
            list_active_price_lists_native_with_context(
                &app_ctx,
                &auth,
                &tenant,
                &foreign_request,
                None,
                None,
            )
            .await
            .is_err()
        );

        update_price_list_scope_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListScopeDraft {
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect("scope clear should succeed");

        let global_lists = list_active_price_lists_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            &request_context,
            Some(other_channel_id.to_string()),
            Some("mobile-app".to_string()),
        )
        .await
        .expect("global list should load");
        assert!(
            global_lists
                .iter()
                .any(|item| item.id == price_list_id.to_string())
        );
    }

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_rule_requires_products_update_permission() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db);

        let error = update_price_list_rule_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListRuleDraft {
                adjustment_percent: "15".to_string(),
            },
        )
        .await
        .expect_err("permission should be required");

        assert!(error.to_string().contains("products:update required"));
    }
