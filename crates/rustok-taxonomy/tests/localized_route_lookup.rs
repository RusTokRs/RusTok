use chrono::Utc;
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, ResolveTaxonomyTermInput, TaxonomyError, TaxonomyModule,
    TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
    entities::taxonomy_term_alias,
};
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, TransactionTrait};
use sea_orm_migration::prelude::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, TaxonomyService) {
    let db = setup_test_db().await;
    let schema_manager = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema_manager)
            .await
            .expect("failed to run taxonomy migration");
    }
    let service = TaxonomyService::new(db.clone());
    (db, service)
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn create_module_term(
    service: &TaxonomyService,
    tenant_id: Uuid,
    name: &str,
    slug: &str,
) -> Uuid {
    service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                scope_type: TaxonomyScopeType::Module,
                scope_value: Some("blog".to_string()),
                locale: "en".to_string(),
                name: name.to_string(),
                slug: Some(slug.to_string()),
                canonical_key: Some(slug.to_string()),
                description: None,
                aliases: vec![],
            },
        )
        .await
        .expect("module term should be created")
}

async fn inject_legacy_alias(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    term_id: Uuid,
    slug: &str,
) {
    taxonomy_term_alias::ActiveModel {
        id: Set(Uuid::new_v4()),
        term_id: Set(term_id),
        tenant_id: Set(tenant_id),
        locale: Set("en".to_string()),
        name: Set(slug.to_string()),
        slug: Set(slug.to_string()),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await
    .expect("legacy alias fixture should bypass service admission checks");
}

#[tokio::test]
async fn public_route_lookup_fails_closed_on_cross_table_ambiguity() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let _translation_owner = create_module_term(&service, tenant_id, "Systems", "systems").await;
    let alias_owner = create_module_term(&service, tenant_id, "Zig", "zig").await;
    inject_legacy_alias(&db, tenant_id, alias_owner, "systems").await;

    let error = service
        .resolve_term_for_module(
            tenant_id,
            admin(),
            ResolveTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                module_slug: "blog".to_string(),
                locale: "en".to_string(),
                slug_or_alias: "systems".to_string(),
                fallback_locale: Some("en".to_string()),
            },
        )
        .await
        .expect_err("ambiguous localized route key must fail closed");

    assert!(
        matches!(error, TaxonomyError::Conflict(message) if message.contains("ambiguous localized taxonomy route key"))
    );
}

#[tokio::test]
async fn owner_transaction_lookup_fails_closed_on_cross_table_ambiguity() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let _translation_owner = create_module_term(&service, tenant_id, "Systems", "systems").await;
    let alias_owner = create_module_term(&service, tenant_id, "Zig", "zig").await;
    inject_legacy_alias(&db, tenant_id, alias_owner, "systems").await;

    let txn = db.begin().await.expect("transaction should start");
    let error = service
        .ensure_terms_for_module_in_tx(
            &txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            "blog",
            "en",
            &["systems".to_string()],
        )
        .await
        .expect_err("ambiguous owner route key must fail closed");
    txn.rollback().await.expect("transaction should roll back");

    assert!(
        matches!(error, TaxonomyError::Conflict(message) if message.contains("ambiguous localized taxonomy route key"))
    );
}

#[tokio::test]
async fn same_term_translation_and_alias_are_not_ambiguous() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let term_id = service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                scope_type: TaxonomyScopeType::Module,
                scope_value: Some("blog".to_string()),
                locale: "en".to_string(),
                name: "Systems".to_string(),
                slug: Some("systems".to_string()),
                canonical_key: Some("systems".to_string()),
                description: None,
                aliases: vec!["systems".to_string()],
            },
        )
        .await
        .expect("same-term alias may share its canonical localized route key");

    let resolved = service
        .resolve_term_for_module(
            tenant_id,
            admin(),
            ResolveTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                module_slug: "blog".to_string(),
                locale: "en".to_string(),
                slug_or_alias: "systems".to_string(),
                fallback_locale: Some("en".to_string()),
            },
        )
        .await
        .expect("same-term duplicate route representation must remain resolvable")
        .expect("term should resolve");

    assert_eq!(resolved.id, term_id);
}
