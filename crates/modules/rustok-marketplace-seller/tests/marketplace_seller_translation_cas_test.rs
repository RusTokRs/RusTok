use chrono::Utc;
use rustok_marketplace_seller::entities::{seller, seller_translation};
use rustok_marketplace_seller::{
    MarketplaceSellerTranslationExactLocaleApply, MarketplaceSellerTranslationExactLocaleError,
    MarketplaceSellerTranslationService,
};
use sea_orm::{ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, Set};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

async fn setup() -> (MarketplaceSellerTranslationService, DatabaseConnection) {
    let url = format!(
        "sqlite:file:marketplace_seller_translation_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    let manager = SchemaManager::new(&db);
    rustok_outbox::SysEventsMigration
        .up(&manager)
        .await
        .unwrap();
    for migration in rustok_marketplace_seller::migrations::migrations() {
        migration.up(&manager).await.unwrap();
    }
    (MarketplaceSellerTranslationService::new(db.clone()), db)
}

async fn insert_test_seller(db: &DatabaseConnection, tenant_id: Uuid, display_name: &str) -> Uuid {
    let seller_id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();
    seller::ActiveModel {
        id: Set(seller_id),
        tenant_id: Set(tenant_id),
        handle: Set(format!("seller-{seller_id}")),
        legal_name: Set(Some("Acme Corporation LLC".to_string())),
        status: Set("active".to_string()),
        onboarding_status: Set("approved".to_string()),
        metadata: Set(serde_json::json!({"tier": "gold"})),
        created_at: Set(now),
        updated_at: Set(now),
        activated_at: Set(Some(now)),
        suspended_at: Set(None),
    }
    .insert(db)
    .await
    .unwrap();

    seller_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        seller_id: Set(seller_id),
        locale: Set("en".to_string()),
        display_name: Set(display_name.to_string()),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await
    .unwrap();

    seller_id
}

#[tokio::test]
async fn exact_locale_apply_updates_only_target_and_preserves_other_locales() {
    let (translations, db) = setup().await;
    let tenant_id = Uuid::new_v4();
    let seller_id = insert_test_seller(&db, tenant_id, "Acme Store").await;

    let before = translations
        .read_exact_locale(tenant_id, seller_id, "en", "fr")
        .await
        .expect("initial snapshot should load");

    assert_eq!(before.seller_id, seller_id);
    assert_eq!(before.source_locale, "en");
    assert_eq!(before.target_locale, "fr");
    assert_eq!(before.exact_locales, vec!["en".to_string()]);
    assert_eq!(before.source.display_name, "Acme Store");
    assert!(before.target.is_none());
    assert!(before.target_revision.is_none());
    assert!(before.resource_revision.starts_with("sha256:"));
    assert!(before.source_revision.starts_with("sha256:"));

    let applied = translations
        .apply_exact_locale(
            tenant_id,
            seller_id,
            MarketplaceSellerTranslationExactLocaleApply {
                source_locale: "en".to_string(),
                target_locale: "fr".to_string(),
                display_name: "Boutique Acme".to_string(),
                expected_resource_revision: before.resource_revision.clone(),
                expected_source_revision: before.source_revision.clone(),
                expected_target_revision: None,
            },
        )
        .await
        .expect("exact target locale should be applied");

    assert_eq!(applied.seller_id, seller_id);
    assert_eq!(applied.target.locale, "fr");
    assert_eq!(applied.target.display_name, "Boutique Acme");
    assert_ne!(applied.resource_revision, before.resource_revision);
    assert!(applied.target_revision.starts_with("sha256:"));

    let after = translations
        .read_exact_locale(tenant_id, seller_id, "en", "fr")
        .await
        .expect("snapshot after apply should load");

    assert_eq!(
        after.exact_locales,
        vec!["en".to_string(), "fr".to_string()]
    );
    assert_eq!(after.source.display_name, "Acme Store");
    assert_eq!(
        after.target.as_ref().map(|t| t.display_name.as_str()),
        Some("Boutique Acme")
    );
    assert_eq!(after.resource_revision, applied.resource_revision);
    assert_eq!(
        after.target_revision.as_deref(),
        Some(applied.target_revision.as_str())
    );
}

#[tokio::test]
async fn exact_locale_apply_updates_existing_target() {
    let (translations, db) = setup().await;
    let tenant_id = Uuid::new_v4();
    let seller_id = insert_test_seller(&db, tenant_id, "Acme Store").await;

    let first_snapshot = translations
        .read_exact_locale(tenant_id, seller_id, "en", "de")
        .await
        .expect("snapshot should load");

    let first_applied = translations
        .apply_exact_locale(
            tenant_id,
            seller_id,
            MarketplaceSellerTranslationExactLocaleApply {
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                display_name: "Acme Laden".to_string(),
                expected_resource_revision: first_snapshot.resource_revision,
                expected_source_revision: first_snapshot.source_revision,
                expected_target_revision: None,
            },
        )
        .await
        .expect("first apply should succeed");

    let second_snapshot = translations
        .read_exact_locale(tenant_id, seller_id, "en", "de")
        .await
        .expect("snapshot after first apply should load");

    let second_applied = translations
        .apply_exact_locale(
            tenant_id,
            seller_id,
            MarketplaceSellerTranslationExactLocaleApply {
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                display_name: "Acme Geschäft".to_string(),
                expected_resource_revision: second_snapshot.resource_revision,
                expected_source_revision: second_snapshot.source_revision,
                expected_target_revision: Some(first_applied.target_revision),
            },
        )
        .await
        .expect("second apply should update existing target");

    assert_eq!(second_applied.target.display_name, "Acme Geschäft");

    let final_snapshot = translations
        .read_exact_locale(tenant_id, seller_id, "en", "de")
        .await
        .expect("final snapshot should load");

    assert_eq!(
        final_snapshot
            .target
            .as_ref()
            .map(|t| t.display_name.as_str()),
        Some("Acme Geschäft")
    );
}

#[tokio::test]
async fn exact_locale_apply_rejects_stale_resource_revision() {
    let (translations, db) = setup().await;
    let tenant_id = Uuid::new_v4();
    let seller_id = insert_test_seller(&db, tenant_id, "Acme Store").await;

    let before = translations
        .read_exact_locale(tenant_id, seller_id, "en", "fr")
        .await
        .expect("snapshot should load");

    translations
        .apply_exact_locale(
            tenant_id,
            seller_id,
            MarketplaceSellerTranslationExactLocaleApply {
                source_locale: "en".to_string(),
                target_locale: "fr".to_string(),
                display_name: "Boutique Acme".to_string(),
                expected_resource_revision: before.resource_revision.clone(),
                expected_source_revision: before.source_revision.clone(),
                expected_target_revision: None,
            },
        )
        .await
        .expect("first apply should succeed");

    let stale_error = translations
        .apply_exact_locale(
            tenant_id,
            seller_id,
            MarketplaceSellerTranslationExactLocaleApply {
                source_locale: "en".to_string(),
                target_locale: "es".to_string(),
                display_name: "Tienda Acme".to_string(),
                expected_resource_revision: before.resource_revision,
                expected_source_revision: before.source_revision,
                expected_target_revision: None,
            },
        )
        .await
        .expect_err("stale resource revision should be rejected");

    assert!(matches!(
        stale_error,
        MarketplaceSellerTranslationExactLocaleError::RevisionConflict {
            revision: "resource"
        }
    ));
}

#[tokio::test]
async fn exact_locale_apply_rejects_stale_source_and_target_revisions() {
    let (translations, db) = setup().await;
    let tenant_id = Uuid::new_v4();
    let seller_id = insert_test_seller(&db, tenant_id, "Acme Store").await;

    let before = translations
        .read_exact_locale(tenant_id, seller_id, "en", "fr")
        .await
        .expect("snapshot should load");

    let bad_source_err = translations
        .apply_exact_locale(
            tenant_id,
            seller_id,
            MarketplaceSellerTranslationExactLocaleApply {
                source_locale: "en".to_string(),
                target_locale: "fr".to_string(),
                display_name: "Boutique Acme".to_string(),
                expected_resource_revision: before.resource_revision.clone(),
                expected_source_revision:
                    "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                        .to_string(),
                expected_target_revision: None,
            },
        )
        .await
        .expect_err("bad source revision must be rejected");

    assert!(matches!(
        bad_source_err,
        MarketplaceSellerTranslationExactLocaleError::RevisionConflict { revision: "source" }
    ));

    let bad_target_err = translations
        .apply_exact_locale(
            tenant_id,
            seller_id,
            MarketplaceSellerTranslationExactLocaleApply {
                source_locale: "en".to_string(),
                target_locale: "fr".to_string(),
                display_name: "Boutique Acme".to_string(),
                expected_resource_revision: before.resource_revision,
                expected_source_revision: before.source_revision,
                expected_target_revision: Some("sha256:unexpected_target_revision".to_string()),
            },
        )
        .await
        .expect_err("non-matching target revision must be rejected");

    assert!(matches!(
        bad_target_err,
        MarketplaceSellerTranslationExactLocaleError::RevisionConflict { revision: "target" }
    ));
}

#[tokio::test]
async fn read_exact_locale_rejects_missing_source_locale() {
    let (translations, db) = setup().await;
    let tenant_id = Uuid::new_v4();
    let seller_id = insert_test_seller(&db, tenant_id, "Acme Store").await;

    let err = translations
        .read_exact_locale(tenant_id, seller_id, "de", "fr")
        .await
        .expect_err("missing source locale should error");

    assert!(matches!(
        err,
        MarketplaceSellerTranslationExactLocaleError::SourceLocaleNotFound {
            seller_id: id,
            locale
        } if id == seller_id && locale == "de"
    ));
}

#[tokio::test]
async fn translation_service_validation_rules() {
    let (translations, db) = setup().await;
    let tenant_id = Uuid::new_v4();
    let seller_id = insert_test_seller(&db, tenant_id, "Acme Store").await;

    let nil_err = translations
        .read_exact_locale(Uuid::nil(), seller_id, "en", "fr")
        .await
        .expect_err("nil tenant must fail");
    assert!(matches!(
        nil_err,
        MarketplaceSellerTranslationExactLocaleError::Validation(_)
    ));

    let same_locale_err = translations
        .read_exact_locale(tenant_id, seller_id, "en", "en")
        .await
        .expect_err("same source and target must fail");
    assert!(matches!(
        same_locale_err,
        MarketplaceSellerTranslationExactLocaleError::Validation(_)
    ));

    let empty_name_err = translations
        .apply_exact_locale(
            tenant_id,
            seller_id,
            MarketplaceSellerTranslationExactLocaleApply {
                source_locale: "en".to_string(),
                target_locale: "fr".to_string(),
                display_name: "   ".to_string(),
                expected_resource_revision: "any".to_string(),
                expected_source_revision: "any".to_string(),
                expected_target_revision: None,
            },
        )
        .await
        .expect_err("empty display_name must fail");
    assert!(matches!(
        empty_name_err,
        MarketplaceSellerTranslationExactLocaleError::Validation(_)
    ));

    let long_name = "a".repeat(161);
    let oversized_err = translations
        .apply_exact_locale(
            tenant_id,
            seller_id,
            MarketplaceSellerTranslationExactLocaleApply {
                source_locale: "en".to_string(),
                target_locale: "fr".to_string(),
                display_name: long_name,
                expected_resource_revision: "any".to_string(),
                expected_source_revision: "any".to_string(),
                expected_target_revision: None,
            },
        )
        .await
        .expect_err("oversized display_name must fail");
    assert!(matches!(
        oversized_err,
        MarketplaceSellerTranslationExactLocaleError::Validation(_)
    ));
}
