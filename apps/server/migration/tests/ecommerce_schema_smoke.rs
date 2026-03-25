use migration::Migrator;
use rust_decimal::Decimal;
use rustok_commerce::dto::{
    CreateProductInput, CreateVariantInput, PriceInput, ProductOptionInput, ProductTranslationInput,
};
use rustok_commerce::services::{CatalogService, PricingService};
use rustok_test_utils::{db::setup_test_db_with_migrations, mock_transactional_event_bus};
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use std::collections::BTreeSet;
use std::str::FromStr;
use uuid::Uuid;

async fn load_sqlite_tables(db: &DatabaseConnection) -> BTreeSet<String> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type = 'table'".to_string(),
        ))
        .await
        .expect("failed to query sqlite_master");

    rows.into_iter()
        .map(|row| {
            row.try_get::<String>("", "name")
                .expect("sqlite_master row must expose table name")
        })
        .collect()
}

#[tokio::test]
async fn pricing_service_supports_decimal_prices_on_migrated_schema() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let event_bus = mock_transactional_event_bus();
    let catalog = CatalogService::new(db.clone(), event_bus.clone());
    let pricing = PricingService::new(db.clone(), event_bus);
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("catalog create_product should work before pricing smoke");

    let variant_id = created.variants[0].id;
    pricing
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "EUR",
            Decimal::from_str("89.99").expect("valid decimal"),
            Some(Decimal::from_str("109.99").expect("valid decimal")),
        )
        .await
        .expect("pricing service should write decimal price on migrated schema");

    let fetched = pricing
        .get_price(variant_id, "EUR")
        .await
        .expect("pricing service should read decimal price on migrated schema");

    assert_eq!(fetched, Some(Decimal::from_str("89.99").expect("valid decimal")));
}

async fn seed_tenant(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            tenant_id.into(),
            "Migration Test Tenant".into(),
            format!("migration-test-tenant-{tenant_id}").into(),
            sea_orm_migration::sea_orm::Value::String(None),
            serde_json::json!({}).to_string().into(),
            "en".into(),
            true.into(),
        ],
    ))
    .await
    .expect("failed to seed tenant");
}

fn create_product_input() -> CreateProductInput {
    CreateProductInput {
        translations: vec![
            ProductTranslationInput {
                locale: "en".to_string(),
                title: "Migration-backed Product".to_string(),
                description: Some("English translation".to_string()),
                handle: Some(format!("migration-backed-{}", Uuid::new_v4())),
                meta_title: Some("EN meta".to_string()),
                meta_description: Some("EN description".to_string()),
            },
            ProductTranslationInput {
                locale: "ru".to_string(),
                title: "Товар из миграций".to_string(),
                description: Some("Русская локализация".to_string()),
                handle: Some(format!("tovar-iz-migraciy-{}", Uuid::new_v4())),
                meta_title: Some("RU meta".to_string()),
                meta_description: Some("RU description".to_string()),
            },
        ],
        options: vec![ProductOptionInput {
            name: "Size".to_string(),
            values: vec!["S".to_string(), "M".to_string()],
        }],
        variants: vec![CreateVariantInput {
            sku: Some(format!("SKU-{}", Uuid::new_v4())),
            barcode: None,
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
            prices: vec![PriceInput {
                currency_code: "USD".to_string(),
                amount: Decimal::from_str("99.99").expect("valid decimal"),
                compare_at_amount: Some(Decimal::from_str("149.99").expect("valid decimal")),
            }],
            inventory_quantity: 10,
            inventory_policy: "deny".to_string(),
            weight: Some(Decimal::from_str("1.5").expect("valid decimal")),
            weight_unit: Some("kg".to_string()),
        }],
        vendor: Some("Migration Test Vendor".to_string()),
        product_type: Some("Physical".to_string()),
        publish: false,
        metadata: serde_json::json!({ "source": "migration-smoke" }),
    }
}

#[tokio::test]
async fn ecommerce_migrations_create_expected_tables() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let tables = load_sqlite_tables(&db).await;

    for table in [
        "products",
        "product_translations",
        "product_images",
        "product_image_translations",
        "product_options",
        "product_option_translations",
        "product_option_values",
        "product_option_value_translations",
        "product_variants",
        "product_variant_translations",
        "variant_option_values",
        "price_lists",
        "prices",
        "regions",
        "stock_locations",
        "inventory_items",
        "inventory_levels",
        "reservation_items",
    ] {
        assert!(
            tables.contains(table),
            "expected migrated schema to include table `{table}`, found: {tables:?}"
        );
    }
}

#[tokio::test]
async fn catalog_service_supports_multilingual_catalog_data_on_migrated_schema() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let event_bus = mock_transactional_event_bus();
    let service = CatalogService::new(db.clone(), event_bus);
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let created = service
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("catalog create_product should work on migrated schema");

    assert_eq!(created.translations.len(), 2);
    assert!(created.translations.iter().any(|item| item.locale == "en"));
    assert!(created.translations.iter().any(|item| item.locale == "ru"));
    assert_eq!(created.options.len(), 1);
    assert_eq!(created.options[0].translations.len(), 2);
    assert_eq!(created.variants[0].translations.len(), 2);

    let fetched = service
        .get_product(tenant_id, created.id)
        .await
        .expect("catalog get_product should work on migrated schema");

    assert_eq!(fetched.translations.len(), 2);
    assert_eq!(fetched.options[0].translations.len(), 2);
    assert_eq!(fetched.options[0].translations[0].values.len(), 2);
    assert_eq!(fetched.variants[0].translations.len(), 2);
    assert_eq!(
        fetched
            .translations
            .iter()
            .find(|item| item.locale == "ru")
            .map(|item| item.title.as_str()),
        Some("Товар из миграций")
    );
}
