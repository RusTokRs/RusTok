use rust_decimal::Decimal;
use rustok_region::dto::{
    CreateRegionInput, RegionCountryTaxPolicyInput, RegionTranslationInput, UpdateRegionInput,
};
use rustok_region::{RegionError, RegionService, RegionTranslationService};
use rustok_test_utils::db::{
    assert_postgres_url, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, setup_test_db,
    unique_postgres_database_name,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, TransactionTrait};
use std::str::FromStr;
use uuid::Uuid;

mod support;

async fn setup() -> RegionService {
    let db = setup_test_db().await;
    support::apply_region_migrations(&db).await;
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
    let db = setup_test_db().await;
    support::apply_region_migrations(&db).await;
    assert_region_create_and_read(db).await;
}

#[tokio::test]
#[ignore = "requires RUSTOK_REGION_TEST_POSTGRES_URL with permission to create a disposable database"]
async fn postgres_create_and_resolve_region_by_country() {
    let url = std::env::var("RUSTOK_REGION_TEST_POSTGRES_URL")
        .expect("set RUSTOK_REGION_TEST_POSTGRES_URL to a PostgreSQL test instance");
    assert_postgres_url(&url);
    let admin = connect_postgres(&url)
        .await
        .expect("connect to test instance");
    let database_name = unique_postgres_database_name("region_create_read");
    create_postgres_database(&admin, &database_name)
        .await
        .expect("create disposable database");
    let database_url = postgres_database_url(&url, &database_name);
    let result = tokio::spawn(async move {
        let db = connect_postgres(&database_url)
            .await
            .expect("connect to disposable database");
        support::apply_region_migrations(&db).await;
        assert_region_create_and_read(db).await;
    })
    .await;
    drop_postgres_database_if_exists(&admin, &database_name)
        .await
        .expect("remove disposable database");
    if let Err(error) = result {
        std::panic::resume_unwind(error.into_panic());
    }
}

async fn assert_region_create_and_read(db: DatabaseConnection) {
    let service = RegionService::new(db.clone());
    let tenant_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    let mut input = create_region_input();
    input.metadata = serde_json::json!({
        "channel_tax_provider_ids": { channel_id.to_string(): { "provider_id": "region_default" } }
    });
    input
        .country_tax_policies
        .as_mut()
        .unwrap()
        .push(RegionCountryTaxPolicyInput {
            country_code: "FR".into(),
            tax_rate: Decimal::from(19),
            tax_included: false,
        });
    let created = service
        .create_region(tenant_id, input)
        .await
        .expect("region should be created");
    assert_eq!(created.currency_code, "EUR");
    assert_eq!(created.countries, vec!["DE".to_string(), "FR".to_string()]);
    assert_eq!(created.country_tax_policies.len(), 2);
    assert_eq!(created.country_tax_policies[0].country_code, "DE");
    assert_eq!(
        created.country_tax_policies[0].tax_rate,
        Decimal::from_str("7.00").expect("valid decimal")
    );

    let resolved = service
        .resolve_region_for_country(tenant_id, "fr", Some("fr"), Some("en"))
        .await
        .expect("region lookup should succeed")
        .expect("region should resolve");
    assert_eq!(resolved.id, created.id);
    assert_eq!(resolved.requested_locale.as_deref(), Some("fr"));
    assert_eq!(resolved.effective_locale.as_deref(), Some("en"));
    assert_eq!(resolved.available_locales, ["en"]);
    assert_eq!(resolved.name, "European Union");

    let other_tenant = Uuid::new_v4();
    assert!(matches!(
        service.get_region(other_tenant, created.id, None, None).await,
        Err(RegionError::RegionNotFound(id)) if id == created.id
    ));
    assert!(
        service
            .resolve_region_for_country(other_tenant, "FR", None, None)
            .await
            .unwrap()
            .is_none()
    );

    let tx = db.begin().await.unwrap();
    let policy = RegionService::read_tax_policy_in_tx(&tx, tenant_id, created.id, Some(channel_id))
        .await
        .unwrap()
        .expect("region policy");
    assert_eq!(policy.tax_rate, created.tax_rate);
    assert_eq!(policy.provider_id, created.tax_provider_id);
    assert_eq!(policy.tax_included, created.tax_included);
    assert_eq!(policy.country_rules, created.country_tax_policies);
    assert_eq!(
        policy.channel_provider_id.as_deref(),
        Some("region_default")
    );
    assert!(
        RegionService::read_tax_policy_in_tx(&tx, other_tenant, created.id, Some(channel_id))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        RegionService::read_tax_policy_in_tx(&tx, tenant_id, Uuid::new_v4(), None)
            .await
            .unwrap()
            .is_none()
    );
    let without_channel = RegionService::read_tax_policy_in_tx(&tx, tenant_id, created.id, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(without_channel.channel_provider_id, None);

    // The native owner entrypoint must observe the supplied transaction's
    // writes. A separately pooled read would miss this uncommitted change.
    let backend = tx.get_database_backend();
    let region_marker = if backend == sea_orm::DbBackend::Postgres {
        "$1"
    } else {
        "?"
    };
    tx.execute_raw(Statement::from_sql_and_values(
        backend,
        format!("UPDATE regions SET tax_rate = 12 WHERE id = {region_marker}"),
        [created.id.into()],
    ))
    .await
    .unwrap();
    assert_eq!(
        RegionService::read_tax_policy_in_tx(&tx, tenant_id, created.id, None)
            .await
            .unwrap()
            .unwrap()
            .tax_rate,
        Decimal::from(12)
    );
    tx.execute_unprepared("DELETE FROM region_country_tax_policies")
        .await
        .unwrap();
    assert!(
        RegionService::read_tax_policy_in_tx(&tx, tenant_id, created.id, None)
            .await
            .unwrap()
            .unwrap()
            .country_rules
            .is_empty()
    );
    tx.rollback().await.unwrap();

    let persisted = service
        .get_region(tenant_id, created.id, None, None)
        .await
        .unwrap();
    assert_eq!(persisted.tax_rate, created.tax_rate);
    assert_eq!(persisted.country_tax_policies, created.country_tax_policies);

    let tx = db.begin().await.unwrap();
    tx.execute_unprepared("DROP TABLE region_country_tax_policies")
        .await
        .unwrap();
    assert!(
        RegionService::read_tax_policy_in_tx(&tx, tenant_id, created.id, None)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn projection_failure_rolls_back_region_and_translation_journal() {
    let db = setup_test_db().await;
    support::apply_region_migrations(&db).await;
    let service = RegionService::new(db.clone());
    let translations = RegionTranslationService::new(db.clone());
    let tenant_id = Uuid::new_v4();
    let region = service
        .create_region(tenant_id, create_region_input())
        .await
        .unwrap();
    let highwater = translations
        .region_translation_change_highwater(tenant_id)
        .await
        .unwrap();
    assert!(highwater.is_some());

    // Corrupt the projection only after the journal insert to exercise the last
    // read inside the write transaction, after every intended write ran.
    db.execute_unprepared(
        "CREATE TRIGGER fail_region_projection AFTER INSERT ON region_translation_change_journal \
         BEGIN UPDATE regions SET metadata = 'invalid JSON' WHERE id = NEW.region_id; END;",
    )
    .await
    .unwrap();

    assert!(matches!(
        service
            .update_region(
                tenant_id,
                region.id,
                UpdateRegionInput {
                    currency_code: Some("USD".into()),
                    translations: Some(vec![RegionTranslationInput {
                        locale: "en".into(),
                        name: "Changed".into()
                    }]),
                    ..Default::default()
                }
            )
            .await,
        Err(RegionError::Database(_))
    ));

    let persisted = service
        .get_region(tenant_id, region.id, Some("en"), None)
        .await
        .unwrap();
    assert_eq!(persisted.currency_code, "EUR");
    assert_eq!(persisted.name, "European Union");
    assert_eq!(persisted.metadata, region.metadata);
    assert_eq!(
        translations
            .region_translation_change_highwater(tenant_id)
            .await
            .unwrap(),
        highwater
    );
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
    assert!(
        err.to_string()
            .contains("tax_rate must be between 0 and 100")
    );

    let mut input = create_region_input();
    input.tax_rate = Decimal::from_str("-1.00").unwrap();
    let err = service.create_region(tenant_id, input).await.unwrap_err();
    assert!(
        err.to_string()
            .contains("tax_rate must be between 0 and 100")
    );

    let mut input = create_region_input();
    input.country_tax_policies = Some(vec![RegionCountryTaxPolicyInput {
        country_code: "de".to_string(),
        tax_rate: Decimal::from_str("150.00").unwrap(),
        tax_included: true,
    }]);
    let err = service.create_region(tenant_id, input).await.unwrap_err();
    assert!(
        err.to_string()
            .contains("country tax policy tax_rate must be between 0 and 100")
    );
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
    assert!(
        err.to_string()
            .contains("country tax policy targets `IT` which is not in region countries")
    );
}

#[tokio::test]
async fn rejects_invalid_currency_code() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();

    let mut input = create_region_input();
    input.currency_code = "123".to_string();
    let err = service.create_region(tenant_id, input).await.unwrap_err();
    assert!(
        err.to_string()
            .contains("currency_code must be a 3-letter alphabetic code")
    );
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
    assert!(
        err.to_string()
            .contains("region countries cannot exclude existing country tax policy for `DE`")
    );
}

#[tokio::test]
async fn region_read_port_contract() {
    use rustok_api::{PortActor, PortContext};
    use rustok_region::ports::{
        RegionListRequest, RegionReadPort, RegionReadRequest, RegionReadSelector,
    };
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
