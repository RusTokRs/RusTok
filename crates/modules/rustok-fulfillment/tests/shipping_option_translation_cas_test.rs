use rust_decimal::Decimal;
use rustok_fulfillment::{
    CreateShippingOptionInput, FulfillmentService, ShippingOptionTranslationExactLocaleApply,
    ShippingOptionTranslationExactLocaleError, ShippingOptionTranslationInput,
    ShippingOptionTranslationService, UpdateShippingOptionInput,
};
use rustok_test_utils::db::setup_test_db;
use std::str::FromStr;
use uuid::Uuid;

mod support;

async fn setup() -> (FulfillmentService, ShippingOptionTranslationService) {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;
    (
        FulfillmentService::new(db.clone()),
        ShippingOptionTranslationService::new(db),
    )
}

fn create_input() -> CreateShippingOptionInput {
    CreateShippingOptionInput {
        translations: vec![
            ShippingOptionTranslationInput {
                locale: "en".to_string(),
                name: "Express".to_string(),
            },
            ShippingOptionTranslationInput {
                locale: "fr".to_string(),
                name: "Express FR".to_string(),
            },
        ],
        currency_code: "usd".to_string(),
        amount: Decimal::from_str("12.50").expect("valid amount"),
        provider_id: None,
        allowed_shipping_profile_slugs: None,
        metadata: serde_json::json!({"source": "translation-cas-test"}),
    }
}

#[tokio::test]
async fn exact_locale_apply_updates_only_target_and_preserves_other_locales() {
    let (owner, translations) = setup().await;
    let tenant_id = Uuid::new_v4();
    let option = owner
        .create_shipping_option(tenant_id, create_input())
        .await
        .expect("shipping option should be created");

    let before = translations
        .read_exact_locale(tenant_id, option.id, "en", "de")
        .await
        .expect("source snapshot should load");
    assert_eq!(before.exact_locales, vec!["en".to_string(), "fr".to_string()]);
    assert!(before.target.is_none());

    let applied = translations
        .apply_exact_locale(
            tenant_id,
            option.id,
            ShippingOptionTranslationExactLocaleApply {
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                name: "Express DE".to_string(),
                expected_resource_revision: before.resource_revision.clone(),
                expected_source_revision: before.source_revision.clone(),
                expected_target_revision: None,
            },
        )
        .await
        .expect("exact target locale should be applied");
    assert_eq!(applied.target.locale, "de");
    assert_eq!(applied.target.name, "Express DE");

    let after = translations
        .read_exact_locale(tenant_id, option.id, "en", "de")
        .await
        .expect("updated snapshot should load");
    assert_eq!(
        after.exact_locales,
        vec!["de".to_string(), "en".to_string(), "fr".to_string()]
    );
    assert_eq!(after.source.name, "Express");
    assert_eq!(after.target.as_ref().map(|value| value.name.as_str()), Some("Express DE"));
    assert_ne!(after.resource_revision, before.resource_revision);
}

#[tokio::test]
async fn exact_locale_apply_rejects_stale_resource_revision() {
    let (owner, translations) = setup().await;
    let tenant_id = Uuid::new_v4();
    let option = owner
        .create_shipping_option(tenant_id, create_input())
        .await
        .expect("shipping option should be created");

    let before = translations
        .read_exact_locale(tenant_id, option.id, "en", "fr")
        .await
        .expect("initial snapshot should load");
    let target_revision = before.target_revision.clone();

    translations
        .apply_exact_locale(
            tenant_id,
            option.id,
            ShippingOptionTranslationExactLocaleApply {
                source_locale: "en".to_string(),
                target_locale: "fr".to_string(),
                name: "Rapide".to_string(),
                expected_resource_revision: before.resource_revision.clone(),
                expected_source_revision: before.source_revision.clone(),
                expected_target_revision: target_revision.clone(),
            },
        )
        .await
        .expect("first compare-and-swap should succeed");

    let error = translations
        .apply_exact_locale(
            tenant_id,
            option.id,
            ShippingOptionTranslationExactLocaleApply {
                source_locale: "en".to_string(),
                target_locale: "fr".to_string(),
                name: "Encore".to_string(),
                expected_resource_revision: before.resource_revision,
                expected_source_revision: before.source_revision,
                expected_target_revision: target_revision,
            },
        )
        .await
        .expect_err("stale resource revision must be rejected");

    assert!(matches!(
        error,
        ShippingOptionTranslationExactLocaleError::RevisionConflict {
            revision: "resource"
        }
    ));
}

#[tokio::test]
async fn operational_shipping_option_state_does_not_change_translation_revision() {
    let (owner, translations) = setup().await;
    let tenant_id = Uuid::new_v4();
    let option = owner
        .create_shipping_option(tenant_id, create_input())
        .await
        .expect("shipping option should be created");

    let before = translations
        .read_exact_locale(tenant_id, option.id, "en", "fr")
        .await
        .expect("initial snapshot should load");

    owner
        .update_shipping_option(
            tenant_id,
            option.id,
            UpdateShippingOptionInput {
                translations: None,
                currency_code: Some("eur".to_string()),
                amount: Some(Decimal::from_str("19.00").expect("valid amount")),
                provider_id: Some("manual".to_string()),
                allowed_shipping_profile_slugs: None,
                metadata: Some(serde_json::json!({"operational": true})),
            },
        )
        .await
        .expect("operational update should succeed");
    owner
        .deactivate_shipping_option(tenant_id, option.id)
        .await
        .expect("deactivation should succeed");

    let after = translations
        .read_exact_locale(tenant_id, option.id, "en", "fr")
        .await
        .expect("snapshot after operational changes should load");

    assert_eq!(after.resource_revision, before.resource_revision);
    assert_eq!(after.source_revision, before.source_revision);
    assert_eq!(after.target_revision, before.target_revision);
    assert!(!after.active);
}
