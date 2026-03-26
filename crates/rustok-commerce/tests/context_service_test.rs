use rust_decimal::Decimal;
use rustok_commerce::dto::ResolveStoreContextInput;
use rustok_commerce::services::StoreContextService;
use rustok_region::dto::CreateRegionInput;
use rustok_region::services::RegionService;
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use std::str::FromStr;
use uuid::Uuid;

mod support;

async fn setup() -> (DatabaseConnection, StoreContextService, RegionService) {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    (
        db.clone(),
        StoreContextService::new(db.clone()),
        RegionService::new(db),
    )
}

#[tokio::test]
async fn resolve_context_uses_tenant_locales_and_region_currency() {
    let (db, service, regions) = setup().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = regions
        .create_region(
            tenant_id,
            CreateRegionInput {
                name: "Europe".to_string(),
                currency_code: "eur".to_string(),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                countries: vec!["de".to_string(), "fr".to_string()],
                metadata: serde_json::json!({ "source": "context-test" }),
            },
        )
        .await
        .expect("region should be created");

    let resolved = service
        .resolve_context(
            tenant_id,
            ResolveStoreContextInput {
                region_id: Some(region.id),
                country_code: None,
                locale: Some("de".to_string()),
                currency_code: None,
            },
        )
        .await
        .expect("context should resolve");

    assert_eq!(resolved.locale, "de");
    assert_eq!(resolved.default_locale, "en");
    assert_eq!(
        resolved.available_locales,
        vec!["en".to_string(), "de".to_string()]
    );
    assert_eq!(resolved.currency_code.as_deref(), Some("EUR"));
    assert_eq!(
        resolved.region.as_ref().map(|value| value.id),
        Some(region.id)
    );
}

async fn seed_tenant_context(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            tenant_id.into(),
            "Context Tenant".into(),
            format!("context-tenant-{tenant_id}").into(),
            sea_orm::Value::String(None),
            serde_json::json!({}).to_string().into(),
            "en".into(),
            true.into(),
        ],
    ))
    .await
    .unwrap();
    for (locale, name, native_name, is_default) in [
        ("en", "English", "English", true),
        ("de", "German", "Deutsch", false),
    ] {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
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
        .unwrap();
    }
}
