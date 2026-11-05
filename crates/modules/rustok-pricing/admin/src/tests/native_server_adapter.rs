use uuid::Uuid;
use leptos::prelude::*;
use rustok_api::HostRuntimeContext;
use crate::core::*;
use crate::model::*;
use crate::transport::native_server_adapter::*;
    fn pricing_projection_test_context(tenant_id: Uuid, locale: &str) -> rustok_api::PortContext {
        rustok_api::PortContext::new(
            tenant_id.to_string(),
            rustok_api::PortActor::service("pricing-native-test"),
            locale,
            "pricing-native-test",
        )
        .with_deadline(std::time::Duration::from_secs(2))
    }

    use super::*;
    use rustok_api::{AuthContext, Permission, TenantContext};
    use rustok_product::CatalogService;
    use rustok_test_utils::db::setup_test_db;
    use rustok_test_utils::mock_transactional_event_bus;
    use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter};
    use serde_json::json;
    use std::str::FromStr;

    mod support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../rustok-commerce/tests/support/mod.rs"
        ));
    }

    fn test_app_context(db: sea_orm::DatabaseConnection) -> HostRuntimeContext {
        HostRuntimeContext::new(db).with_shared_value(mock_transactional_event_bus())
    }

    async fn seed_tenant_context(db: &sea_orm::DatabaseConnection, tenant_id: Uuid) {
        db.execute_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                tenant_id.into(),
                "Pricing Admin Test Tenant".into(),
                format!("pricing-admin-test-{tenant_id}").into(),
                sea_orm::Value::String(None),
                json!({}).to_string().into(),
                "en".into(),
                true.into(),
            ],
        ))
        .await
        .expect("tenant should be inserted");
    }

    async fn seed_channel_context(
        db: &sea_orm::DatabaseConnection,
        tenant_id: Uuid,
        channel_id: Uuid,
        channel_slug: &str,
    ) {
        db.execute_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO channels (id, tenant_id, slug, name, is_active, is_default, status, settings, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                channel_id.into(),
                tenant_id.into(),
                channel_slug.into(),
                format!("Channel {channel_slug}").into(),
                true.into(),
                false.into(),
                "active".into(),
                json!({}).to_string().into(),
            ],
        ))
        .await
        .expect("channel should be inserted");
    }

    async fn load_base_price(
        db: &sea_orm::DatabaseConnection,
        variant_id: Uuid,
    ) -> (rust_decimal::Decimal, Option<rust_decimal::Decimal>) {
        let row = db
            .query_one_raw(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT amount, compare_at_amount FROM prices WHERE variant_id = ? AND currency_code = 'USD' AND price_list_id IS NULL AND channel_id IS NULL AND min_quantity IS NULL AND max_quantity IS NULL LIMIT 1",
                vec![variant_id.into()],
            ))
            .await
            .expect("price row query should succeed")
            .expect("base row should exist");

        (
            row.try_get("", "amount").expect("valid amount"),
            row.try_get("", "compare_at_amount").expect("valid compare_at"),
        )
    }

    async fn load_base_price_full(
        db: &sea_orm::DatabaseConnection,
        variant_id: Uuid,
    ) -> (
        rust_decimal::Decimal,
        Option<rust_decimal::Decimal>,
        Option<i64>,
        Option<i64>,
    ) {
        let row = db
            .query_one_raw(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT amount, compare_at_amount, legacy_amount, legacy_compare_at_amount FROM prices WHERE variant_id = ? AND currency_code = 'USD' AND price_list_id IS NULL AND channel_id IS NULL AND min_quantity IS NULL AND max_quantity IS NULL LIMIT 1",
                vec![variant_id.into()],
            ))
            .await
            .expect("price row query should succeed")
            .expect("base row should exist");

        (
            row.try_get("", "amount").expect("valid amount"),
            row.try_get("", "compare_at_amount").expect("valid compare_at"),
            row.try_get("", "legacy_amount").expect("valid legacy_amount"),
            row.try_get("", "legacy_compare_at_amount").expect("valid legacy_compare_at"),
        )
    }

    async fn count_prices(
        db: &sea_orm::DatabaseConnection,
        variant_id: Uuid,
        price_list_id: Uuid,
    ) -> u64 {
        let row = db
            .query_one_raw(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT count(*) as count FROM prices WHERE variant_id = ? AND price_list_id = ?",
                vec![variant_id.into(), price_list_id.into()],
            ))
            .await
            .expect("query should succeed")
            .expect("count row exists");
        let count: i64 = row.try_get("", "count").expect("valid count");
        count as u64
    }

    async fn create_test_product(
        catalog: &CatalogService,
        tenant_id: Uuid,
        actor_id: Uuid,
    ) -> (Uuid, Uuid) {
        let product = catalog
            .create_product(
                tenant_id,
                actor_id,
                rustok_product::dto::CreateProductInput {
                    attribute_values: Default::default(),
                    brands: vec![],
                    translations: vec![rustok_product::dto::ProductTranslationInput {
                        locale: "en".to_string(),
                        title: "Tiered Pricing Product".to_string(),
                        description: Some("Admin pricing transport test".to_string()),
                        handle: Some(format!("tiered-pricing-{tenant_id}")),
                        meta_title: None,
                        meta_description: None,
                    }],
                    variant_axes: vec![],
                    variants: vec![rustok_product::dto::CreateVariantInput {
                        sku: Some("PRICE-TIER-1".to_string()),
                        barcode: None,
                        shipping_profile_slug: None,
                        axis_values: vec![],
                        prices: vec![rustok_product::dto::PriceInput {
                            currency_code: "USD".to_string(),
                            channel_id: None,
                            channel_slug: None,
                            amount: rust_decimal::Decimal::from_str("100.00")
                                .expect("valid decimal"),
                            compare_at_amount: None,
                        }],
                        inventory_quantity: 0,
                        inventory_policy: "deny".to_string(),
                        weight: None,
                        weight_unit: None,
                    }],
                    seller_id: None,
                    vendor: Some("Pricing Vendor".to_string()),
                    product_type: Some("Physical".to_string()),
                    shipping_profile_slug: None,
                    primary_category_id: None,
                    tags: vec![],
                    publish: false,
                    metadata: json!({}),
                },
            )
            .await
            .expect("product should be created");

        (product.id, product.variants[0].id)
    }

    async fn create_price_list(
        db: &sea_orm::DatabaseConnection,
        tenant_id: Uuid,
        status: &str,
    ) -> Uuid {
        let price_list_id = Uuid::new_v4();
        db.execute_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO price_lists (
                id,
                tenant_id,
                type,
                status,
                starts_at,
                ends_at,
                created_at,
                updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                price_list_id.into(),
                tenant_id.into(),
                "sale".into(),
                status.to_string().into(),
                sea_orm::Value::String(None),
                sea_orm::Value::String(None),
            ],
        ))
        .await
        .expect("price list should be inserted");

        db.execute_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO price_list_translations (
                id,
                price_list_id,
                locale,
                name,
                description
            ) VALUES (?, ?, ?, ?, ?)",
            vec![
                Uuid::new_v4().into(),
                price_list_id.into(),
                "en".into(),
                format!("Admin List-{price_list_id}").into(),
                Some("Admin pricing transport test list".to_string()).into(),
            ],
        ))
        .await
        .expect("price list translation should be inserted");

        price_list_id
    }

    async fn create_future_price_list(
        db: &sea_orm::DatabaseConnection,
        tenant_id: Uuid,
        status: &str,
    ) -> Uuid {
        let price_list_id = Uuid::new_v4();
        db.execute_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO price_lists (
                id,
                tenant_id,
                type,
                status,
                starts_at,
                ends_at,
                created_at,
                updated_at
            ) VALUES (?, ?, ?, ?, datetime('now', '+1 day'), NULL, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                price_list_id.into(),
                tenant_id.into(),
                "sale".into(),
                status.to_string().into(),
            ],
        ))
        .await
        .expect("price list should be inserted");

        db.execute_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO price_list_translations (
                id,
                price_list_id,
                locale,
                name,
                description
            ) VALUES (?, ?, ?, ?, ?)",
            vec![
                Uuid::new_v4().into(),
                price_list_id.into(),
                "en".into(),
                format!("Admin List-{price_list_id}").into(),
                Some("Admin pricing transport test list".to_string()).into(),
            ],
        ))
        .await
        .expect("price list translation should be inserted");

        price_list_id
    }

    async fn create_expired_price_list(
        db: &sea_orm::DatabaseConnection,
        tenant_id: Uuid,
        status: &str,
    ) -> Uuid {
        let price_list_id = Uuid::new_v4();
        db.execute_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO price_lists (
                id,
                tenant_id,
                type,
                status,
                starts_at,
                ends_at,
                created_at,
                updated_at
            ) VALUES (?, ?, ?, ?, NULL, datetime('now', '-1 day'), CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                price_list_id.into(),
                tenant_id.into(),
                "sale".into(),
                status.to_string().into(),
            ],
        ))
        .await
        .expect("price list should be inserted");

        db.execute_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO price_list_translations (
                id,
                price_list_id,
                locale,
                name,
                description
            ) VALUES (?, ?, ?, ?, ?)",
            vec![
                Uuid::new_v4().into(),
                price_list_id.into(),
                "en".into(),
                format!("Admin List-{price_list_id}").into(),
                Some("Admin pricing transport test list".to_string()).into(),
            ],
        ))
        .await
        .expect("price list translation should be inserted");

        price_list_id
    }

    #[test]
    fn admin_pricing_resolution_context_rejects_non_letter_currency_code() {
        let error = sanitize_resolution_context(Some("US1".to_string()), None, None, Some(1))
            .expect_err("invalid currency should be rejected");

        assert!(
            error
                .to_string()
                .contains("currency_code must be a 3-letter code")
        );
    }

    #[test]
    fn admin_pricing_resolution_context_rejects_non_positive_quantity() {
        let error = sanitize_resolution_context(Some("USD".to_string()), None, None, Some(0))
            .expect_err("invalid quantity should be rejected");

        assert!(error.to_string().contains("quantity must be at least 1"));
    }

    #[test]
    fn admin_pricing_resolution_context_rejects_modifiers_without_currency_code() {
        let error = sanitize_resolution_context(None, None, Some(Uuid::new_v4().to_string()), None)
            .expect_err("price_list_id without currency should be rejected");

        assert!(
            error
                .to_string()
                .contains("currency_code is required for pricing resolution context")
        );
    }

    #[test]
    fn admin_pricing_write_scope_rejects_invalid_price_list_id() {
        let error = parse_optional_uuid("not-a-uuid", "price_list_id")
            .expect_err("invalid price_list_id should be rejected");

        assert!(error.to_string().contains("Invalid price_list_id"));

        let nil_error = parse_optional_uuid(Uuid::nil().to_string().as_str(), "price_list_id")
            .expect_err("nil price_list_id should be rejected");
        assert!(nil_error.to_string().contains("Invalid price_list_id"));
    }

    #[test]
    fn admin_pricing_transport_rejects_invalid_channel_id() {
        let error = parse_optional_uuid_string(Some("not-a-uuid".to_string()), "channel_id")
            .expect_err("invalid channel_id should be rejected");

        assert!(error.to_string().contains("Invalid channel_id"));

        let nil_error = parse_optional_uuid_string(Some(Uuid::nil().to_string()), "channel_id")
            .expect_err("nil channel_id should be rejected");
        assert!(nil_error.to_string().contains("Invalid channel_id"));
    }

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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db, mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db, mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
