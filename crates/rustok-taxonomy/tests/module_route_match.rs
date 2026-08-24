use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, TaxonomyError, TaxonomyModule, TaxonomyScopeType, TaxonomyService,
    TaxonomyTermKind, entities::taxonomy_term_route_key,
};
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection};
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

async fn create_tag(
    service: &TaxonomyService,
    tenant_id: Uuid,
    scope_type: TaxonomyScopeType,
    scope_value: Option<&str>,
    name: &str,
    slug: &str,
    canonical_key: &str,
    aliases: &[&str],
) -> Uuid {
    service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                scope_type,
                scope_value: scope_value.map(str::to_string),
                locale: "en".to_string(),
                name: name.to_string(),
                slug: Some(slug.to_string()),
                canonical_key: Some(canonical_key.to_string()),
                description: None,
                aliases: aliases.iter().map(|alias| (*alias).to_string()).collect(),
            },
        )
        .await
        .expect("term should be created")
}

#[tokio::test]
async fn module_route_match_exposes_scope_fallback_locale_and_alias_identity() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let global_id = create_tag(
        &service,
        tenant_id,
        TaxonomyScopeType::Global,
        None,
        "Global Rust",
        "rust",
        "global-rust",
        &["ferris"],
    )
    .await;
    let module_id = create_tag(
        &service,
        tenant_id,
        TaxonomyScopeType::Module,
        Some("blog"),
        "Blog Rust",
        "rust",
        "blog-rust",
        &["ferris"],
    )
    .await;

    let canonical = service
        .resolve_term_route_for_module(
            tenant_id,
            TaxonomyTermKind::Tag,
            " Blog ",
            "fr-CA",
            Some("en"),
            " RUST ",
        )
        .await
        .expect("canonical route lookup should succeed")
        .expect("canonical route should resolve");
    assert_eq!(canonical.term_id, module_id);
    assert_ne!(canonical.term_id, global_id);
    assert_eq!(canonical.kind, TaxonomyTermKind::Tag);
    assert_eq!(canonical.scope_type, TaxonomyScopeType::Module);
    assert_eq!(canonical.scope_value.as_deref(), Some("blog"));
    assert_eq!(canonical.matched_locale, "en");
    assert_eq!(canonical.route_key, "rust");
    assert_eq!(canonical.alias_id, None);

    let alias = service
        .resolve_term_route_for_module(
            tenant_id,
            TaxonomyTermKind::Tag,
            "blog",
            "fr-CA",
            Some("en"),
            " FERRIS ",
        )
        .await
        .expect("alias route lookup should succeed")
        .expect("alias route should resolve");
    assert_eq!(alias.term_id, module_id);
    assert_eq!(alias.scope_type, TaxonomyScopeType::Module);
    assert_eq!(alias.scope_value.as_deref(), Some("blog"));
    assert_eq!(alias.matched_locale, "en");
    assert_eq!(alias.route_key, "ferris");
    assert!(alias.alias_id.is_some());
}

#[tokio::test]
async fn module_route_match_fails_closed_when_registry_source_is_missing() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let term_id = create_tag(
        &service,
        tenant_id,
        TaxonomyScopeType::Module,
        Some("blog"),
        "Rust",
        "rust",
        "rust",
        &[],
    )
    .await;

    taxonomy_term_route_key::ActiveModel {
        tenant_id: Set(tenant_id),
        kind: Set(TaxonomyTermKind::Tag),
        scope_type: Set(TaxonomyScopeType::Module),
        scope_value: Set("blog".to_string()),
        locale: Set("en".to_string()),
        route_key: Set("ghost-route".to_string()),
        term_id: Set(term_id),
    }
    .insert(&db)
    .await
    .expect("drift fixture should insert directly into the registry");

    let error = service
        .resolve_term_route_for_module(
            tenant_id,
            TaxonomyTermKind::Tag,
            "blog",
            "en",
            None,
            "ghost-route",
        )
        .await
        .expect_err("registry/source drift must fail closed");
    assert!(matches!(error, TaxonomyError::Conflict(_)));
}
