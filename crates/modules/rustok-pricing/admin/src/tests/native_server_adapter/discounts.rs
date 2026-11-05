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
    async fn admin_pricing_native_preview_variant_discount_returns_typed_preview() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

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
        let app_ctx = test_app_context(db.clone());
        let catalog = test_catalog(db.clone());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;

        let preview = preview_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            &rustok_api::RequestContext {
                tenant_id,
                user_id: Some(auth.user_id),
                channel_id: None,
                channel_slug: None,
                channel_resolution_source: None,
                locale: "en".to_owned(),
                correlation_id: "pricing-preview-test".to_owned(),
            },
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "usd".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect("preview should succeed");

        assert_eq!(preview.kind, "percentage_discount");
        assert_eq!(preview.currency_code, "USD");
        assert_eq!(preview.base_amount, "100");
        assert_eq!(preview.adjusted_amount, "90");
        assert_eq!(preview.adjustment_percent, "10");
    }

    #[tokio::test]
    async fn admin_pricing_native_apply_variant_discount_updates_base_row_only() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

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
            .set_price_tier(
                tenant_id,
                actor_id,
                variant_id,
                "USD",
                rust_decimal::Decimal::from_str("85.00").expect("valid decimal"),
                Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal")),
                Some(10),
                None,
            )
            .await
            .expect("tier should be created");

        let preview = apply_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect("apply should succeed");

        assert_eq!(preview.adjusted_amount, "90");

        let detail = rustok_pricing::PricingService::new(db, mock_transactional_event_bus())
            .get_admin_product_pricing_with_locale_fallback(
                pricing_projection_test_context(tenant_id, "en"),
                product_id,
                Some("en"),
                None,
            )
            .await
            .expect("admin detail should load");

        let variant = detail
            .variants
            .into_iter()
            .find(|variant| variant.id == variant_id)
            .expect("variant should be present");
        let base_price = variant
            .prices
            .iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.price_list_id.is_none()
                    && price.min_quantity.is_none()
                    && price.max_quantity.is_none()
            })
            .expect("base price should exist");
        let tier_price = variant
            .prices
            .iter()
            .find(|price| price.currency_code == "USD" && price.min_quantity == Some(10))
            .expect("tier price should exist");

        assert_eq!(
            base_price.amount,
            rust_decimal::Decimal::from_str("90.00").expect("valid decimal")
        );
        assert_eq!(
            tier_price.amount,
            rust_decimal::Decimal::from_str("85.00").expect("valid decimal")
        );
    }

    #[tokio::test]
    async fn admin_pricing_native_apply_variant_discount_targets_channel_scoped_base_row_only() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        seed_channel_context(&db, tenant_id, channel_id, "web-store").await;

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
            .set_price_tier_with_channel(
                tenant_id,
                actor_id,
                variant_id,
                "USD",
                rust_decimal::Decimal::from_str("80.00").expect("valid decimal"),
                Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal")),
                Some(channel_id),
                Some("web-store".to_string()),
                None,
                None,
            )
            .await
            .expect("channel scoped base row should be created");

        let preview = apply_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: "".to_string(),
                channel_id: channel_id.to_string(),
                channel_slug: "web-store".to_string(),
            },
        )
        .await
        .expect("apply should succeed");

        assert_eq!(preview.adjusted_amount, "90");
        assert_eq!(preview.channel_id, Some(channel_id.to_string()));
        assert_eq!(preview.channel_slug.as_deref(), Some("web-store"));

        let detail = rustok_pricing::PricingService::new(db, mock_transactional_event_bus())
            .get_admin_product_pricing_with_locale_fallback(
                pricing_projection_test_context(tenant_id, "en"),
                product_id,
                Some("en"),
                None,
            )
            .await
            .expect("admin detail should load");

        let variant = detail
            .variants
            .into_iter()
            .find(|variant| variant.id == variant_id)
            .expect("variant should be present");
        let global_base = variant
            .prices
            .iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.price_list_id.is_none()
                    && price.channel_id.is_none()
                    && price.channel_slug.is_none()
                    && price.min_quantity.is_none()
                    && price.max_quantity.is_none()
            })
            .expect("global base price should exist");
        let channel_base = variant
            .prices
            .iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.price_list_id.is_none()
                    && price.channel_id == Some(channel_id)
                    && price.channel_slug.as_deref() == Some("web-store")
                    && price.min_quantity.is_none()
                    && price.max_quantity.is_none()
            })
            .expect("channel scoped base price should exist");

        assert_eq!(
            global_base.amount,
            rust_decimal::Decimal::from_str("100.00").expect("valid decimal")
        );
        assert_eq!(
            channel_base.amount,
            rust_decimal::Decimal::from_str("90.00").expect("valid decimal")
        );
    }

    #[tokio::test]
    async fn admin_pricing_native_apply_variant_discount_requires_products_update_permission() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

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
        let app_ctx = test_app_context(db.clone());
        let catalog = test_catalog(db);
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;

        let error = apply_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect_err("permission should be required");

        assert!(error.to_string().contains("products:update required"));
    }

    #[tokio::test]
    async fn admin_pricing_native_preview_variant_discount_supports_active_price_list_override() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

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
        let app_ctx = test_app_context(db.clone());
        let catalog = test_catalog(db.clone());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let pricing =
            rustok_pricing::PricingService::new(db.clone(), mock_transactional_event_bus());
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        pricing
            .set_price_list_tier(
                tenant_id,
                actor_id,
                variant_id,
                price_list_id,
                "USD",
                rust_decimal::Decimal::from_str("80.00").expect("valid decimal"),
                Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal")),
                None,
                None,
            )
            .await
            .expect("price-list row should be created");

        let preview = preview_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            &rustok_api::RequestContext {
                tenant_id,
                user_id: Some(auth.user_id),
                channel_id: None,
                channel_slug: None,
                channel_resolution_source: None,
                locale: "en".to_owned(),
                correlation_id: "pricing-preview-test".to_owned(),
            },
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect("preview should succeed");

        assert_eq!(preview.adjusted_amount, "90");
        assert_eq!(preview.price_list_id, Some(price_list_id.to_string()));
    }

    #[tokio::test]
    async fn admin_pricing_native_preview_variant_discount_rejects_future_price_list_override() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

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
        let app_ctx = test_app_context(db.clone());
        let catalog = test_catalog(db.clone());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let price_list_id = create_future_price_list(&db, tenant_id, "active").await;

        let error = preview_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            &rustok_api::RequestContext {
                tenant_id,
                user_id: Some(auth.user_id),
                channel_id: None,
                channel_slug: None,
                channel_resolution_source: None,
                locale: "en".to_owned(),
                correlation_id: "pricing-preview-test".to_owned(),
            },
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect_err("future list should be rejected");

        assert!(error.to_string().contains("not active yet"));
    }

    #[tokio::test]
    async fn admin_pricing_native_preview_variant_discount_rejects_expired_price_list_override() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

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
        let app_ctx = test_app_context(db.clone());
        let catalog = test_catalog(db.clone());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let price_list_id = create_expired_price_list(&db, tenant_id, "active").await;

        let error = preview_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            &rustok_api::RequestContext {
                tenant_id,
                user_id: Some(auth.user_id),
                channel_id: None,
                channel_slug: None,
                channel_resolution_source: None,
                locale: "en".to_owned(),
                correlation_id: "pricing-preview-test".to_owned(),
            },
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect_err("expired list should be rejected");

        assert!(error.to_string().contains("already expired"));
    }

    #[tokio::test]
    async fn admin_pricing_native_preview_variant_discount_rejects_channel_mismatch_for_price_list_override()
     {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        seed_channel_context(&db, tenant_id, channel_id, "web-store").await;

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
        let app_ctx = test_app_context(db.clone());
        let catalog = test_catalog(db.clone());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let pricing =
            rustok_pricing::PricingService::new(db.clone(), mock_transactional_event_bus());
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        pricing
            .set_price_list_scope(
                tenant_id,
                actor_id,
                price_list_id,
                Some(channel_id),
                Some("web-store".to_string()),
            )
            .await
            .expect("scope update should succeed");

        let error = preview_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            &rustok_api::RequestContext {
                tenant_id,
                user_id: Some(auth.user_id),
                channel_id: None,
                channel_slug: None,
                channel_resolution_source: None,
                locale: "en".to_owned(),
                correlation_id: "pricing-preview-test".to_owned(),
            },
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: Uuid::new_v4().to_string(),
                channel_slug: "mobile-app".to_string(),
            },
        )
        .await
        .expect_err("mismatched channel should be rejected");

        assert!(
            error
                .to_string()
                .contains("price_list_id is not available for the requested channel")
        );
    }

    #[tokio::test]
    async fn admin_pricing_native_apply_variant_discount_targets_active_price_list_override_only() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

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
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        pricing
            .set_price_list_tier(
                tenant_id,
                actor_id,
                variant_id,
                price_list_id,
                "USD",
                rust_decimal::Decimal::from_str("80.00").expect("valid decimal"),
                Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal")),
                None,
                None,
            )
            .await
            .expect("price-list row should be created");

        let preview = apply_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect("apply should succeed");

        assert_eq!(preview.adjusted_amount, "90");
        assert_eq!(preview.price_list_id, Some(price_list_id.to_string()));

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
        let base_price = variant
            .prices
            .iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.price_list_id.is_none()
                    && price.min_quantity.is_none()
                    && price.max_quantity.is_none()
            })
            .expect("base price should exist");
        let price_list_price = variant
            .prices
            .iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.price_list_id == Some(price_list_id)
                    && price.min_quantity.is_none()
                    && price.max_quantity.is_none()
            })
            .expect("price-list row should exist");

        assert_eq!(
            base_price.amount,
            rust_decimal::Decimal::from_str("100.00").expect("valid decimal")
        );
        assert_eq!(
            price_list_price.amount,
            rust_decimal::Decimal::from_str("90.00").expect("valid decimal")
        );
    }

    #[tokio::test]
    async fn admin_pricing_native_apply_variant_discount_rejects_future_price_list_override_without_writing()
     {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

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
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let price_list_id = create_future_price_list(&db, tenant_id, "active").await;

        let count_before = count_prices(&db, variant_id, price_list_id).await;

        let error = apply_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect_err("future list should be rejected");

        let count_after = count_prices(&db, variant_id, price_list_id).await;

        assert!(error.to_string().contains("not active yet"));
        assert_eq!(count_before, 0);
        assert_eq!(count_after, 0);
    }

    #[tokio::test]
    async fn admin_pricing_native_apply_variant_discount_rejects_expired_price_list_override_without_writing()
     {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

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
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let price_list_id = create_expired_price_list(&db, tenant_id, "active").await;

        let count_before = count_prices(&db, variant_id, price_list_id).await;

        let error = apply_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect_err("expired list should be rejected");

        let count_after = count_prices(&db, variant_id, price_list_id).await;

        assert!(error.to_string().contains("already expired"));
        assert_eq!(count_before, 0);
        assert_eq!(count_after, 0);
    }

    #[tokio::test]
    async fn admin_pricing_native_apply_variant_discount_rejects_channel_mismatch_without_mutating_scoped_override()
     {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        seed_channel_context(&db, tenant_id, channel_id, "web-store").await;

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
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        pricing
            .set_price_list_scope(
                tenant_id,
                actor_id,
                price_list_id,
                Some(channel_id),
                Some("web-store".to_string()),
            )
            .await
            .expect("scope update should succeed");
        pricing
            .set_price_list_tier_with_channel(
                tenant_id,
                actor_id,
                variant_id,
                price_list_id,
                "USD",
                rust_decimal::Decimal::from_str("80.00").expect("valid decimal"),
                Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal")),
                Some(channel_id),
                Some("web-store".to_string()),
                None,
                None,
            )
            .await
            .expect("scoped override should be created");

        let count_before = count_prices(&db, variant_id, price_list_id).await;

        let error = apply_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: Uuid::new_v4().to_string(),
                channel_slug: "mobile-app".to_string(),
            },
        )
        .await
        .expect_err("mismatched channel should be rejected");

        let count_after = count_prices(&db, variant_id, price_list_id).await;

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
            .expect("scoped price should exist");

        assert!(
            error
                .to_string()
                .contains("price_list_id is not available for the requested channel")
        );
        assert_eq!(count_before, 1);
        assert_eq!(count_after, 1);
        assert_eq!(
            scoped_price.amount,
            rust_decimal::Decimal::from_str("80.00").expect("valid decimal")
        );
        assert_eq!(
            scoped_price.compare_at_amount,
            Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal"))
        );
        assert_eq!(scoped_price.channel_id, Some(channel_id));
        assert_eq!(scoped_price.channel_slug.as_deref(), Some("web-store"));
    }
