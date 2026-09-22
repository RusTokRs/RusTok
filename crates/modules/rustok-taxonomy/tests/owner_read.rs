use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, TaxonomyModule, TaxonomyOwnerReader, TaxonomyScopeType,
    TaxonomyService, TaxonomyTermKind,
};
use rustok_test_utils::db::setup_test_db;
use sea_orm::TransactionTrait;
use sea_orm_migration::prelude::SchemaManager;
use uuid::Uuid;

async fn setup() -> (sea_orm::DatabaseConnection, TaxonomyService) {
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

async fn create_term(
    service: &TaxonomyService,
    tenant_id: Uuid,
    scope_type: TaxonomyScopeType,
    scope_value: Option<&str>,
    name: &str,
    slug: &str,
) -> Uuid {
    service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                scope_type,
                scope_value: scope_value.map(ToOwned::to_owned),
                locale: "en".to_owned(),
                name: name.to_owned(),
                slug: Some(slug.to_owned()),
                canonical_key: Some(slug.to_owned()),
                description: None,
                aliases: vec![],
            },
        )
        .await
        .expect("taxonomy term should be created")
}

#[tokio::test]
async fn owner_reader_enforces_scope_ids_and_locale_fallback() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let module_term_id = create_term(
        &service,
        tenant_id,
        TaxonomyScopeType::Module,
        Some("blog"),
        "Rust",
        "rust",
    )
    .await;
    let global_term_id = create_term(
        &service,
        tenant_id,
        TaxonomyScopeType::Global,
        None,
        "Systems",
        "systems",
    )
    .await;
    let reader = TaxonomyOwnerReader::new(db);

    let module_terms = reader
        .load_scoped_terms(
            tenant_id,
            TaxonomyTermKind::Tag,
            TaxonomyScopeType::Module,
            Some("blog"),
            Some(&[module_term_id, global_term_id]),
            "fr",
            Some("en"),
        )
        .await
        .expect("module-scoped owner read should succeed");

    assert_eq!(module_terms.len(), 1);
    assert_eq!(module_terms[0].id, module_term_id);
    assert_eq!(module_terms[0].requested_locale, "fr");
    assert_eq!(module_terms[0].effective_locale, "en");
    assert_eq!(module_terms[0].name, "Rust");
    assert_eq!(module_terms[0].slug, "rust");
    assert_eq!(module_terms[0].scope_value.as_deref(), Some("blog"));

    let global_terms = reader
        .load_scoped_terms(
            tenant_id,
            TaxonomyTermKind::Tag,
            TaxonomyScopeType::Global,
            None,
            Some(&[module_term_id, global_term_id]),
            "en",
            None,
        )
        .await
        .expect("global owner read should succeed");

    assert_eq!(global_terms.len(), 1);
    assert_eq!(global_terms[0].id, global_term_id);
    assert_eq!(global_terms[0].scope_value, None);
}

#[tokio::test]
async fn owner_reader_treats_empty_identity_filter_as_empty_result() {
    let (db, _service) = setup().await;
    let reader = TaxonomyOwnerReader::new(db);

    let terms = reader
        .load_scoped_terms(
            Uuid::new_v4(),
            TaxonomyTermKind::Tag,
            TaxonomyScopeType::Module,
            Some("blog"),
            Some(&[]),
            "en",
            None,
        )
        .await
        .expect("empty identity page should be accepted");

    assert!(terms.is_empty());
}

#[tokio::test]
async fn transaction_owner_reader_preserves_mixed_scope_and_rejects_foreign_tenant_ids() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let module_term_id = create_term(
        &service,
        tenant_id,
        TaxonomyScopeType::Module,
        Some("blog"),
        "Rust",
        "rust",
    )
    .await;
    let global_term_id = create_term(
        &service,
        tenant_id,
        TaxonomyScopeType::Global,
        None,
        "Systems",
        "systems",
    )
    .await;
    let foreign_term_id = create_term(
        &service,
        foreign_tenant_id,
        TaxonomyScopeType::Module,
        Some("blog"),
        "Foreign",
        "foreign",
    )
    .await;

    let txn = db
        .begin()
        .await
        .expect("owner read transaction should start");
    let terms = TaxonomyOwnerReader::load_terms_by_ids_in_tx(
        &txn,
        tenant_id,
        TaxonomyTermKind::Tag,
        &[foreign_term_id, global_term_id, module_term_id],
        "fr",
        Some("en"),
    )
    .await
    .expect("transaction owner read should succeed");
    txn.rollback()
        .await
        .expect("owner read transaction should roll back");

    assert_eq!(terms.len(), 2);
    assert!(terms.iter().any(|term| term.id == module_term_id));
    assert!(terms.iter().any(|term| term.id == global_term_id));
    assert!(!terms.iter().any(|term| term.id == foreign_term_id));
    assert!(
        terms
            .iter()
            .all(|term| term.requested_locale == "fr" && term.effective_locale == "en")
    );
}
