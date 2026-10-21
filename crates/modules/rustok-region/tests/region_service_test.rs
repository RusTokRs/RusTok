use rust_decimal::Decimal;
use rustok_region::dto::{
    CreateRegionInput, RegionCountryTaxPolicyInput, RegionTranslationInput, UpdateRegionInput,
};
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
        translations: vec![RegionTranslationInput {
            locale: "en".to_string(),
            name: "European Union".to_string(),
        }],
        currency_code: "eur".to_string(),
        tax_provider_id: None,
        tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
        tax_included: true,
        country_tax_policies: Some(vec![RegionCountryTaxPolicyInput {
            country_code: "de".to_string(),
            tax_rate: Decimal::from_str("7.00").expect("valid decimal"),
            tax_included: true,
        }]),
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
    assert_eq!(created.country_tax_policies.len(), 1);
    assert_eq!(created.country_tax_policies[0].country_code, "DE");
    assert_eq!(
        created.country_tax_policies[0].tax_rate,
        Decimal::from_str("7.00").expect("valid decimal")
    );

    let resolved = service
        .resolve_region_for_country(tenant_id, "fr", Some("en"), Some("en"))
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
                country_tax_policies: Some(vec![RegionCountryTaxPolicyInput {
                    country_code: "ca".to_string(),
                    tax_rate: Decimal::from_str("5.00").expect("valid decimal"),
                    tax_included: false,
                }]),
                countries: Some(vec!["us".to_string(), "ca".to_string()]),
                ..Default::default()
            },
        )
        .await
        .expect("region should be updated");

    assert_eq!(updated.currency_code, "USD");
    assert_eq!(updated.countries, vec!["US".to_string(), "CA".to_string()]);
    assert_eq!(updated.country_tax_policies.len(), 1);
    assert_eq!(updated.country_tax_policies[0].country_code, "CA");
}

#[tokio::test]
async fn rejects_nil_tenant_or_region_id() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();

    let err = service
        .create_region(Uuid::nil(), create_region_input())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("tenant_id must not be nil"));

    let err = service
        .get_region(Uuid::nil(), Uuid::new_v4(), None, None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("tenant_id must not be nil"));

    let err = service
        .get_region(tenant_id, Uuid::nil(), None, None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("region_id must not be nil"));

    let err = service
        .list_regions(Uuid::nil(), None, None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("tenant_id must not be nil"));

    let err = service
        .update_region(Uuid::nil(), Uuid::new_v4(), UpdateRegionInput::default())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("tenant_id must not be nil"));

    let err = service
        .update_region(tenant_id, Uuid::nil(), UpdateRegionInput::default())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("region_id must not be nil"));

    let err = service
        .resolve_region_for_country(Uuid::nil(), "de", None, None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("tenant_id must not be nil"));
}

#[tokio::test]
async fn rejects_tax_rate_out_of_bounds() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();

    let mut input = create_region_input();
    input.tax_rate = Decimal::from_str("105.00").unwrap();
    let err = service.create_region(tenant_id, input).await.unwrap_err();
    assert!(err.to_string().contains("tax_rate must be between 0 and 100"));

    let mut input = create_region_input();
    input.tax_rate = Decimal::from_str("-1.00").unwrap();
    let err = service.create_region(tenant_id, input).await.unwrap_err();
    assert!(err.to_string().contains("tax_rate must be between 0 and 100"));

    let mut input = create_region_input();
    input.country_tax_policies = Some(vec![RegionCountryTaxPolicyInput {
        country_code: "de".to_string(),
        tax_rate: Decimal::from_str("150.00").unwrap(),
        tax_included: true,
    }]);
    let err = service.create_region(tenant_id, input).await.unwrap_err();
    assert!(err.to_string().contains("country tax policy tax_rate must be between 0 and 100"));
}

#[tokio::test]
async fn rejects_duplicate_countries() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();

    let mut input = create_region_input();
    input.countries = vec!["de".to_string(), "DE".to_string()];
    let err = service.create_region(tenant_id, input).await.unwrap_err();
    assert!(err.to_string().contains("duplicate country code `DE`"));
}

#[tokio::test]
async fn rejects_policy_for_unowned_country() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();

    let mut input = create_region_input();
    input.countries = vec!["de".to_string()];
    input.country_tax_policies = Some(vec![RegionCountryTaxPolicyInput {
        country_code: "it".to_string(),
        tax_rate: Decimal::from_str("22.00").unwrap(),
        tax_included: true,
    }]);
    let err = service.create_region(tenant_id, input).await.unwrap_err();
    assert!(err.to_string().contains("country tax policy targets `IT` which is not in region countries"));
}

#[tokio::test]
async fn rejects_invalid_currency_code() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();

    let mut input = create_region_input();
    input.currency_code = "123".to_string();
    let err = service.create_region(tenant_id, input).await.unwrap_err();
    assert!(err.to_string().contains("currency_code must be a 3-letter alphabetic code"));
}

#[tokio::test]
async fn prevents_removing_country_with_active_tax_policy() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let created = service
        .create_region(tenant_id, create_region_input())
        .await
        .expect("created");

    // Try updating countries to only FR without updating country_tax_policies (which has DE)
    let err = service
        .update_region(
            tenant_id,
            created.id,
            UpdateRegionInput {
                countries: Some(vec!["fr".to_string()]),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("region countries cannot exclude existing country tax policy for `DE`"));
}

#[tokio::test]
async fn region_read_port_contract() {
    use rustok_api::{PortActor, PortContext};
    use rustok_region::ports::{RegionListRequest, RegionReadPort, RegionReadRequest, RegionReadSelector};
    use std::time::Duration;

    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let created = service
        .create_region(tenant_id, create_region_input())
        .await
        .expect("created");

    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "corr-region-test",
    )
    .with_deadline(Duration::from_secs(5));

    let read = service
        .read_region(
            context.clone(),
            RegionReadRequest {
                selector: RegionReadSelector::Id(created.id),
                requested_locale: Some("en".to_string()),
                tenant_default_locale: Some("en".to_string()),
            },
        )
        .await
        .expect("port read should succeed")
        .expect("projection found");
    assert_eq!(read.region.id, created.id);

    let listed = service
        .list_regions_for_tenant(
            context.clone(),
            RegionListRequest {
                requested_locale: Some("en".to_string()),
                tenant_default_locale: Some("en".to_string()),
            },
        )
        .await
        .expect("port list should succeed");
    assert_eq!(listed.len(), 1);

    // Nil tenant ID in context
    let nil_context = PortContext::new(
        Uuid::nil().to_string(),
        PortActor::system(),
        "en",
        "corr-region-test",
    )
    .with_deadline(Duration::from_secs(5));
    let err = service
        .list_regions_for_tenant(
            nil_context,
            RegionListRequest {
                requested_locale: None,
                tenant_default_locale: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, "region.tenant_id_invalid");
}
