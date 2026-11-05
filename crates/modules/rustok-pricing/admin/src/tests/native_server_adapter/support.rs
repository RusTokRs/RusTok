use uuid::Uuid;
use leptos::prelude::*;
pub use rustok_api::HostRuntimeContext;
use crate::core::*;
use crate::model::*;
use crate::transport::native_server_adapter::*;
pub use rustok_api::{AuthContext, Permission, TenantContext};
pub use rustok_product::CatalogService;
pub use rustok_test_utils::db::setup_test_db;
pub use rustok_test_utils::mock_transactional_event_bus;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde_json::json;
use std::str::FromStr;

pub mod support {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../rustok-commerce/tests/support/mod.rs"
    ));
}

pub fn pricing_projection_test_context(tenant_id: Uuid, locale: &str) -> rustok_api::PortContext {
    rustok_api::PortContext::new(
        tenant_id.to_string(),
        rustok_api::PortActor::service("pricing-native-test"),
        locale,
        "pricing-native-test",
    )
    .with_deadline(std::time::Duration::from_secs(2))
}

pub fn test_app_context(db: sea_orm::DatabaseConnection) -> HostRuntimeContext {
    HostRuntimeContext::new(db).with_shared_value(mock_transactional_event_bus())
}

pub fn test_catalog(db: sea_orm::DatabaseConnection) -> CatalogService {
    CatalogService::new(db.clone(), mock_transactional_event_bus())
        .with_pricing_port(rustok_pricing::integrations::in_process_product_pricing_port(db))
}

    pub async fn seed_tenant_context(db: &sea_orm::DatabaseConnection, tenant_id: Uuid) {
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

    pub async fn seed_channel_context(
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

    pub async fn load_base_price(
        db: &sea_orm::DatabaseConnection,
        variant_id: Uuid,
    ) -> (rust_decimal::Decimal, Option<rust_decimal::Decimal>) {
        let row = db
            .query_one_raw(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT amount_decimal, compare_at_amount_decimal FROM prices WHERE variant_id = ? AND currency_code = 'USD' AND price_list_id IS NULL AND channel_id IS NULL AND min_quantity IS NULL AND max_quantity IS NULL LIMIT 1",
                vec![variant_id.into()],
            ))
            .await
            .expect("price row query should succeed")
            .expect("base row should exist");

        (
            row.try_get("", "amount_decimal").expect("valid amount"),
            row.try_get("", "compare_at_amount_decimal").expect("valid compare_at"),
        )
    }

    pub async fn load_base_price_full(
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
                "SELECT amount_decimal, compare_at_amount_decimal, amount, compare_at_amount FROM prices WHERE variant_id = ? AND currency_code = 'USD' AND price_list_id IS NULL AND channel_id IS NULL AND min_quantity IS NULL AND max_quantity IS NULL LIMIT 1",
                vec![variant_id.into()],
            ))
            .await
            .expect("price row query should succeed")
            .expect("base row should exist");

        (
            row.try_get("", "amount_decimal").expect("valid amount"),
            row.try_get("", "compare_at_amount_decimal").expect("valid compare_at"),
            row.try_get("", "amount").expect("valid legacy_amount"),
            row.try_get("", "compare_at_amount").expect("valid legacy_compare_at"),
        )
    }

    pub async fn count_prices(
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

    pub async fn create_test_product(
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

    pub async fn create_price_list(
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

    pub async fn create_future_price_list(
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

    pub async fn create_expired_price_list(
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

