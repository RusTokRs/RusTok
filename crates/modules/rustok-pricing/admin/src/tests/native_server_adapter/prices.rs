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
    async fn admin_pricing_native_update_variant_price_supports_quantity_tiers() {
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
            permissions: vec![Permission::PRODUCTS_UPDATE, Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = test_catalog(db.clone());
        let (product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;

        update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "usd".to_string(),
                amount: "85.00".to_string(),
                compare_at_amount: "100.00".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
                min_quantity: "10".to_string(),
                max_quantity: "".to_string(),
            },
        )
        .await
        .expect("native update should succeed");

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
        let tier = variant
            .prices
            .into_iter()
            .find(|price| price.currency_code == "USD" && price.min_quantity == Some(10))
            .expect("tier price should be present");

        assert_eq!(
            tier.amount,
            rust_decimal::Decimal::from_str("85.00").expect("valid decimal")
        );
        assert_eq!(
            tier.compare_at_amount,
            Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal"))
        );
        assert_eq!(tier.max_quantity, None);
    }

    #[tokio::test]
    async fn admin_pricing_native_update_variant_price_supports_active_price_list_overrides() {
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
            permissions: vec![Permission::PRODUCTS_UPDATE, Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = test_catalog(db.clone());
        let (product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;

        update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "usd".to_string(),
                amount: "79.00".to_string(),
                compare_at_amount: "100.00".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
                min_quantity: "2".to_string(),
                max_quantity: "5".to_string(),
            },
        )
        .await
        .expect("native price list override should succeed");

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
        let override_price = variant
            .prices
            .into_iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.price_list_id == Some(price_list_id)
                    && price.min_quantity == Some(2)
                    && price.max_quantity == Some(5)
            })
            .expect("price list override should be present");

        assert_eq!(
            override_price.amount,
            rust_decimal::Decimal::from_str("79.00").expect("valid decimal")
        );
        assert_eq!(
            override_price.compare_at_amount,
            Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal"))
        );
    }

    #[tokio::test]
    async fn admin_pricing_native_update_variant_price_supports_channel_scoped_base_rows() {
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
            permissions: vec![Permission::PRODUCTS_UPDATE, Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = test_catalog(db.clone());
        let (product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;

        update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "usd".to_string(),
                amount: "79.00".to_string(),
                compare_at_amount: "100.00".to_string(),
                price_list_id: "".to_string(),
                channel_id: channel_id.to_string(),
                channel_slug: "web-store".to_string(),
                min_quantity: "".to_string(),
                max_quantity: "".to_string(),
            },
        )
        .await
        .expect("native update should support channel scope");

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
        let scoped_price = variant
            .prices
            .into_iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.channel_id == Some(channel_id)
                    && price.channel_slug.as_deref() == Some("web-store")
            })
            .expect("channel-scoped row should be present");

        assert_eq!(
            scoped_price.amount,
            rust_decimal::Decimal::from_str("79.00").expect("valid decimal")
        );
        assert_eq!(
            scoped_price.compare_at_amount,
            Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal"))
        );
    }

    #[tokio::test]
    async fn admin_pricing_native_update_variant_price_persists_decimal_and_legacy_cents() {
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
            permissions: vec![Permission::PRODUCTS_UPDATE, Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = test_catalog(db.clone());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;

        update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "usd".to_string(),
                amount: "79.994".to_string(),
                compare_at_amount: "99.996".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
                min_quantity: "".to_string(),
                max_quantity: "".to_string(),
            },
        )
        .await
        .expect("fractional update should succeed");

        let (amount, compare_at_amount, legacy_amount, legacy_compare_at_amount) =
            load_base_price_full(&db, variant_id).await;

        assert_eq!(
            amount,
            rust_decimal::Decimal::from_str("79.994").expect("valid decimal")
        );
        assert_eq!(
            compare_at_amount,
            Some(rust_decimal::Decimal::from_str("99.996").expect("valid decimal"))
        );
        assert_eq!(legacy_amount, Some(7999));
        assert_eq!(legacy_compare_at_amount, Some(10000));
    }

    #[tokio::test]
    async fn admin_pricing_native_update_variant_price_clears_compare_at_on_existing_row() {
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
            permissions: vec![Permission::PRODUCTS_UPDATE, Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = test_catalog(db.clone());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;

        update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "usd".to_string(),
                amount: "79.99".to_string(),
                compare_at_amount: "99.99".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
                min_quantity: "".to_string(),
                max_quantity: "".to_string(),
            },
        )
        .await
        .expect("initial update should succeed");

        update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "usd".to_string(),
                amount: "89.99".to_string(),
                compare_at_amount: "".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
                min_quantity: "".to_string(),
                max_quantity: "".to_string(),
            },
        )
        .await
        .expect("compare_at clear should succeed");

        let (amount, compare_at_amount, legacy_amount, legacy_compare_at_amount) =
            load_base_price_full(&db, variant_id).await;

        assert_eq!(
            amount,
            rust_decimal::Decimal::from_str("89.99").expect("valid decimal")
        );
        assert_eq!(compare_at_amount, None);
        assert_eq!(legacy_amount, Some(8999));
        assert_eq!(legacy_compare_at_amount, None);
    }

    #[tokio::test]
    async fn admin_pricing_native_update_variant_price_requires_products_update_permission() {
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

        let error = update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "USD".to_string(),
                amount: "85.00".to_string(),
                compare_at_amount: "".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
                min_quantity: "".to_string(),
                max_quantity: "".to_string(),
            },
        )
        .await
        .expect_err("permission should be required");

        assert!(error.to_string().contains("products:update required"));
    }

    #[tokio::test]
    async fn admin_pricing_native_update_variant_price_rejects_compare_at_below_amount_without_mutating_base_row()
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
            permissions: vec![Permission::PRODUCTS_UPDATE, Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = test_catalog(db.clone());
        let (product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;

        let error = update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "usd".to_string(),
                amount: "120.00".to_string(),
                compare_at_amount: "110.00".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
                min_quantity: "".to_string(),
                max_quantity: "".to_string(),
            },
        )
        .await
        .expect_err("invalid compare_at should be rejected");

        assert!(
            error
                .to_string()
                .contains("Compare at price must be greater than amount")
        );

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
            .into_iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.price_list_id.is_none()
                    && price.channel_id.is_none()
                    && price.min_quantity.is_none()
                    && price.max_quantity.is_none()
            })
            .expect("base price should exist");

        assert_eq!(
            base_price.amount,
            rust_decimal::Decimal::from_str("100.00").expect("valid decimal")
        );
        assert_eq!(base_price.compare_at_amount, None);
    }

    #[tokio::test]
    async fn admin_pricing_native_update_variant_price_rejects_compare_at_below_amount_without_mutating_price_list_override()
     {
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
            permissions: vec![Permission::PRODUCTS_UPDATE, Permission::PRODUCTS_READ],
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
                rust_decimal::Decimal::from_str("79.00").expect("valid decimal"),
                Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal")),
                None,
                None,
            )
            .await
            .expect("valid override should be stored");

        let error = update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "usd".to_string(),
                amount: "120.00".to_string(),
                compare_at_amount: "110.00".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
                min_quantity: "".to_string(),
                max_quantity: "".to_string(),
            },
        )
        .await
        .expect_err("invalid compare_at should be rejected for override");

        assert!(
            error
                .to_string()
                .contains("Compare at price must be greater than amount")
        );

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
        let override_price = variant
            .prices
            .into_iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.price_list_id == Some(price_list_id)
                    && price.channel_id.is_none()
                    && price.min_quantity.is_none()
                    && price.max_quantity.is_none()
            })
            .expect("price list override should exist");

        assert_eq!(
            override_price.amount,
            rust_decimal::Decimal::from_str("79.00").expect("valid decimal")
        );
        assert_eq!(
            override_price.compare_at_amount,
            Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal"))
        );
    }
