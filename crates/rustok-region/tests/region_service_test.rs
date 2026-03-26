use rust_decimal::Decimal;
use rustok_region::dto::{CreateRegionInput, UpdateRegionInput};
use rustok_region::services::RegionService;
use rustok_test_utils::db::setup_test_db;
use std::str::FromStr;
use uuid::Uuid;

mod support;

async fn setup() -> RegionService {
    let db = setup_test_db().await;
    support::ensure_region_schema(&db).await;
    RegionService::new(db)
}

fn create_region_input() -> CreateRegionInput {
    CreateRegionInput {
        name: "European Union".to_string(),
        currency_code: "eur".to_string(),
        tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
        tax_included: true,
        countries: vec!["de".to_string(), "fr".to_string()],
        metadata: serde_json::json!({ "source": "region-test" }),
    }
}

#[tokio::test]
async fn create_and_resolve_region_by_country() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();

    let created = service
        .create_region(tenant_id, create_region_input())
        .await
        .expect("region should be created");
    assert_eq!(created.currency_code, "EUR");
    assert_eq!(created.countries, vec!["DE".to_string(), "FR".to_string()]);

    let resolved = service
        .resolve_region_for_country(tenant_id, "fr")
        .await
        .expect("region lookup should succeed")
        .expect("region should resolve");
    assert_eq!(resolved.id, created.id);
}

#[tokio::test]
async fn update_region_changes_currency_and_countries() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let created = service
        .create_region(tenant_id, create_region_input())
        .await
        .expect("region should be created");

    let updated = service
        .update_region(
            tenant_id,
            created.id,
            UpdateRegionInput {
                currency_code: Some("usd".to_string()),
                countries: Some(vec!["us".to_string(), "ca".to_string()]),
                ..Default::default()
            },
        )
        .await
        .expect("region should be updated");

    assert_eq!(updated.currency_code, "USD");
    assert_eq!(updated.countries, vec!["US".to_string(), "CA".to_string()]);
}
