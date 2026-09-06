use chrono::Utc;
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, ListTaxonomyTermsFilter, ResolveTaxonomyTermInput, TaxonomyModule,
    TaxonomyScopeType, TaxonomyService, TaxonomyTermKind, entities::taxonomy_term_alias,
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

async fn create_term_with_canonical_key(
    service: &TaxonomyService,
    tenant_id: Uuid,
    scope_type: TaxonomyScopeType,
    scope_value: Option<&str>,
    locale: &str,
    name: &str,
    slug: &str,
    canonical_key: &str,
) -> Uuid {
    service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                scope_type,
                scope_value: scope_value.map(str::to_string),
                locale: locale.to_string(),
                name: name.to_string(),
                slug: Some(slug.to_string()),
                canonical_key: Some(canonical_key.to_string()),
                description: None,
                aliases: vec![],
            },
        )
        .await
        .expect("term should be created")
}

async fn create_term(
    service: &TaxonomyService,
    tenant_id: Uuid,
    scope_type: TaxonomyScopeType,
    scope_value: Option<&str>,
    locale: &str,
    name: &str,
    slug: &str,
) -> Uuid {
    create_term_with_canonical_key(
        service,
        tenant_id,
        scope_type,
        scope_value,
        locale,
        name,
        slug,
        slug,
    )
    .await
}

async fn create_module_term(
    service: &TaxonomyService,
    tenant_id: Uuid,
    name: &str,
    slug: &str,
) -> Uuid {
    create_term(
        service,
        tenant_id,
        TaxonomyScopeType::Module,
        Some("blog"),
        "en",
        name,
        slug,
    )
    .await
}

async fn inject_unregistered_legacy_alias(
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
    .expect("legacy alias fixture should bypass route-registry admission");
}

#[tokio::test]
async fn public_route_lookup_uses_registry_authority_over_unregistered_legacy_alias() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let translation_owner = create_module_term(&service, tenant_id, "Systems", "systems").await;
    let alias_owner = create_module_term(&service, tenant_id, "Zig", "zig").await;
    inject_unregistered_legacy_alias(&db, tenant_id, alias_owner, "systems").await;

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
        .expect("registry-authority lookup should remain resolvable")
        .expect("registered route owner should resolve");

    assert_eq!(resolved.id, translation_owner);
    assert_ne!(resolved.id, alias_owner);
}

#[tokio::test]
async fn owner_transaction_lookup_uses_registry_authority_over_unregistered_legacy_alias() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let translation_owner = create_module_term(&service, tenant_id, "Systems", "systems").await;
    let alias_owner = create_module_term(&service, tenant_id, "Zig", "zig").await;
    inject_unregistered_legacy_alias(&db, tenant_id, alias_owner, "systems").await;

    let txn = db.begin().await.expect("transaction should start");
    let term_ids = service
        .ensure_terms_for_module_in_tx(
            &txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            "blog",
            "en",
            &["systems".to_string()],
        )
        .await
        .expect("owner lookup should use the registered route owner");
    txn.commit().await.expect("transaction should commit");

    assert_eq!(term_ids, vec![translation_owner]);
    assert_ne!(term_ids[0], alias_owner);
}

#[tokio::test]
async fn owner_batch_collapses_equivalent_labels_and_normalizes_scope_and_locale() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let txn = db.begin().await.expect("transaction should start");

    let term_ids = service
        .ensure_terms_for_module_in_tx(
            &txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            " Blog ",
            "en_us",
            &[
                " Rust ".to_string(),
                "rust".to_string(),
                "RUST".to_string(),
                "   ".to_string(),
            ],
        )
        .await
        .expect("equivalent owner labels should resolve in one transaction");
    txn.commit().await.expect("transaction should commit");

    assert_eq!(
        term_ids.len(),
        1,
        "one route identity must yield one term id"
    );
    let term = service
        .get_term(tenant_id, admin(), term_ids[0], "en-US", None)
        .await
        .expect("created term should load through the canonical locale");
    assert_eq!(term.scope_type, TaxonomyScopeType::Module);
    assert_eq!(term.scope_value.as_deref(), Some("blog"));
    assert_eq!(term.requested_locale, "en-US");
    assert_eq!(term.effective_locale, "en-US");
    assert_eq!(term.canonical_key, "rust");
    assert_eq!(term.name, "Rust");

    let (terms, total) = service
        .list_terms(
            tenant_id,
            admin(),
            ListTaxonomyTermsFilter {
                kind: Some(TaxonomyTermKind::Tag),
                scope_type: Some(TaxonomyScopeType::Module),
                scope_value: Some(" BLOG ".to_string()),
                locale: Some("en_us".to_string()),
                page: Some(1),
                per_page: Some(10),
            },
            None,
        )
        .await
        .expect("normalized module term list should load");
    assert_eq!(total, 1);
    assert_eq!(terms.len(), 1);
    assert_eq!(terms[0].id, term_ids[0]);
}

#[tokio::test]
async fn owner_batch_prefers_module_term_before_global_across_locale_fallback() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let global_term_id = create_term(
        &service,
        tenant_id,
        TaxonomyScopeType::Global,
        None,
        "en",
        "Rust",
        "rust",
    )
    .await;
    let module_term_id = create_module_term(&service, tenant_id, "Rust", "rust").await;

    let txn = db.begin().await.expect("transaction should start");
    let term_ids = service
        .ensure_terms_for_module_in_tx(
            &txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            "blog",
            "fr-FR",
            &["RUST".to_string()],
        )
        .await
        .expect("owner lookup should resolve through the platform fallback locale");
    txn.commit().await.expect("transaction should commit");

    assert_eq!(term_ids, vec![module_term_id]);
    assert_ne!(term_ids[0], global_term_id);
}

#[tokio::test]
async fn owner_batch_reuses_global_term_when_module_term_is_absent() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let global_term_id = create_term(
        &service,
        tenant_id,
        TaxonomyScopeType::Global,
        None,
        "en",
        "Rust",
        "rust",
    )
    .await;

    let txn = db.begin().await.expect("transaction should start");
    let term_ids = service
        .ensure_terms_for_module_in_tx(
            &txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            "blog",
            "fr-FR",
            &[" rust ".to_string()],
        )
        .await
        .expect("global route should satisfy owner lookup when no module route exists");
    txn.commit().await.expect("transaction should commit");

    assert_eq!(term_ids, vec![global_term_id]);

    let (_, module_total) = service
        .list_terms(
            tenant_id,
            admin(),
            ListTaxonomyTermsFilter {
                kind: Some(TaxonomyTermKind::Tag),
                scope_type: Some(TaxonomyScopeType::Module),
                scope_value: Some("blog".to_string()),
                locale: Some("fr-FR".to_string()),
                page: Some(1),
                per_page: Some(10),
            },
            None,
        )
        .await
        .expect("module term list should load");
    assert_eq!(
        module_total, 0,
        "global reuse must not create a shadow module term"
    );
}

#[tokio::test]
async fn owner_batch_prefers_module_canonical_key_before_global_route() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let global_term_id = create_term_with_canonical_key(
        &service,
        tenant_id,
        TaxonomyScopeType::Global,
        None,
        "en",
        "Global Rust",
        "rust",
        "global-rust",
    )
    .await;
    let module_term_id = create_term_with_canonical_key(
        &service,
        tenant_id,
        TaxonomyScopeType::Module,
        Some("blog"),
        "en",
        "Ferris",
        "ferris",
        "rust",
    )
    .await;

    let txn = db.begin().await.expect("transaction should start");
    let term_ids = service
        .ensure_terms_for_module_in_tx(
            &txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            "blog",
            "en",
            &["RUST".to_string()],
        )
        .await
        .expect("module canonical key should resolve before global route lookup");
    txn.commit().await.expect("transaction should commit");

    assert_eq!(term_ids, vec![module_term_id]);
    assert_ne!(term_ids[0], global_term_id);
}

#[tokio::test]
async fn owner_batch_reuses_global_canonical_key_without_shadow_module_term() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let global_term_id = create_term_with_canonical_key(
        &service,
        tenant_id,
        TaxonomyScopeType::Global,
        None,
        "en",
        "Ferris",
        "ferris",
        "rust",
    )
    .await;

    let txn = db.begin().await.expect("transaction should start");
    let term_ids = service
        .ensure_terms_for_module_in_tx(
            &txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            "blog",
            "en",
            &["rust".to_string()],
        )
        .await
        .expect("global canonical key should satisfy owner lookup");
    txn.commit().await.expect("transaction should commit");

    assert_eq!(term_ids, vec![global_term_id]);

    let (_, module_total) = service
        .list_terms(
            tenant_id,
            admin(),
            ListTaxonomyTermsFilter {
                kind: Some(TaxonomyTermKind::Tag),
                scope_type: Some(TaxonomyScopeType::Module),
                scope_value: Some("blog".to_string()),
                locale: Some("en".to_string()),
                page: Some(1),
                per_page: Some(10),
            },
            None,
        )
        .await
        .expect("module term list should load");
    assert_eq!(
        module_total, 0,
        "global canonical reuse must not create a shadow module term"
    );
}

#[tokio::test]
async fn owner_batch_canonical_key_lookup_is_tenant_isolated() {
    let (db, service) = setup().await;
    let first_tenant_id = Uuid::new_v4();
    let second_tenant_id = Uuid::new_v4();
    let first_term_id = create_term_with_canonical_key(
        &service,
        first_tenant_id,
        TaxonomyScopeType::Module,
        Some("blog"),
        "en",
        "First Ferris",
        "ferris-first",
        "rust",
    )
    .await;
    let second_term_id = create_term_with_canonical_key(
        &service,
        second_tenant_id,
        TaxonomyScopeType::Module,
        Some("blog"),
        "en",
        "Second Ferris",
        "ferris-second",
        "rust",
    )
    .await;

    let first_txn = db.begin().await.expect("first transaction should start");
    let first_ids = service
        .ensure_terms_for_module_in_tx(
            &first_txn,
            first_tenant_id,
            TaxonomyTermKind::Tag,
            "blog",
            "en",
            &["rust".to_string()],
        )
        .await
        .expect("first tenant canonical lookup should succeed");
    first_txn
        .commit()
        .await
        .expect("first transaction should commit");

    let second_txn = db.begin().await.expect("second transaction should start");
    let second_ids = service
        .ensure_terms_for_module_in_tx(
            &second_txn,
            second_tenant_id,
            TaxonomyTermKind::Tag,
            "blog",
            "en",
            &["rust".to_string()],
        )
        .await
        .expect("second tenant canonical lookup should succeed");
    second_txn
        .commit()
        .await
        .expect("second transaction should commit");

    assert_eq!(first_ids, vec![first_term_id]);
    assert_eq!(second_ids, vec![second_term_id]);
    assert_ne!(first_ids[0], second_ids[0]);
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
