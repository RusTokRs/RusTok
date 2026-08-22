use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, SetTaxonomyCategoryPlacementInput, TaxonomyError, TaxonomyModule,
    TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
};
use rustok_test_utils::db::setup_test_db;
use sea_orm::DatabaseConnection;
use sea_orm_migration::prelude::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, TaxonomyService) {
    let db = setup_test_db().await;
    let manager = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("taxonomy migration should apply");
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
    kind: TaxonomyTermKind,
    scope_type: TaxonomyScopeType,
    scope_value: Option<&str>,
    name: &str,
) -> Uuid {
    service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind,
                scope_type,
                scope_value: scope_value.map(str::to_owned),
                locale: "en".to_string(),
                name: name.to_string(),
                slug: None,
                canonical_key: Some(format!("{}-{}", name.to_ascii_lowercase(), Uuid::new_v4())),
                description: None,
                aliases: Vec::new(),
            },
        )
        .await
        .expect("term should be created")
}

#[tokio::test]
async fn category_kind_uses_existing_term_and_route_contract() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let category_id = create_term(
        &service,
        tenant_id,
        TaxonomyTermKind::Category,
        TaxonomyScopeType::Global,
        None,
        "Engineering",
    )
    .await;

    let term = service
        .get_term(tenant_id, admin(), category_id, "en", None)
        .await
        .expect("category should load through generic Taxonomy read");
    assert_eq!(term.kind, TaxonomyTermKind::Category);
    assert_eq!(term.name, "Engineering");
    assert_eq!(term.effective_locale, "en");

    let placement = service
        .get_category_placement(tenant_id, admin(), category_id)
        .await
        .expect("new category should have an effective root placement");
    assert_eq!(placement.term_id, category_id);
    assert_eq!(placement.parent_id, None);
    assert_eq!(placement.position, 0);
}

#[tokio::test]
async fn category_placement_accepts_same_scope_and_rejects_cycle() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let root = create_term(
        &service,
        tenant_id,
        TaxonomyTermKind::Category,
        TaxonomyScopeType::Global,
        None,
        "Root",
    )
    .await;
    let child = create_term(
        &service,
        tenant_id,
        TaxonomyTermKind::Category,
        TaxonomyScopeType::Global,
        None,
        "Child",
    )
    .await;

    let placement = service
        .set_category_placement(
            tenant_id,
            admin(),
            child,
            SetTaxonomyCategoryPlacementInput {
                parent_id: Some(root),
                position: 7,
            },
        )
        .await
        .expect("same-scope parent should be accepted");
    assert_eq!(placement.parent_id, Some(root));
    assert_eq!(placement.position, 7);

    let err = service
        .set_category_placement(
            tenant_id,
            admin(),
            root,
            SetTaxonomyCategoryPlacementInput {
                parent_id: Some(child),
                position: 0,
            },
        )
        .await
        .expect_err("moving a root under its descendant must fail");
    assert!(err.to_string().contains("cycle"));
}

#[tokio::test]
async fn category_placement_rejects_cross_scope_parent_and_non_category_child() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let global = create_term(
        &service,
        tenant_id,
        TaxonomyTermKind::Category,
        TaxonomyScopeType::Global,
        None,
        "Global",
    )
    .await;
    let module = create_term(
        &service,
        tenant_id,
        TaxonomyTermKind::Category,
        TaxonomyScopeType::Module,
        Some("forum"),
        "Forum",
    )
    .await;

    let err = service
        .set_category_placement(
            tenant_id,
            admin(),
            module,
            SetTaxonomyCategoryPlacementInput {
                parent_id: Some(global),
                position: 0,
            },
        )
        .await
        .expect_err("cross-scope hierarchy must fail");
    assert!(err.to_string().contains("same Taxonomy scope"));

    let tag = create_term(
        &service,
        tenant_id,
        TaxonomyTermKind::Tag,
        TaxonomyScopeType::Global,
        None,
        "Tag",
    )
    .await;
    let err = service
        .set_category_placement(
            tenant_id,
            admin(),
            tag,
            SetTaxonomyCategoryPlacementInput {
                parent_id: None,
                position: 0,
            },
        )
        .await
        .expect_err("Tag must not enter Category hierarchy");
    assert!(matches!(err, TaxonomyError::Validation(_)));
}

#[tokio::test]
async fn category_placement_rejects_parent_from_another_tenant() {
    let (_db, service) = setup().await;
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let child = create_term(
        &service,
        tenant_a,
        TaxonomyTermKind::Category,
        TaxonomyScopeType::Global,
        None,
        "Tenant A",
    )
    .await;
    let foreign_parent = create_term(
        &service,
        tenant_b,
        TaxonomyTermKind::Category,
        TaxonomyScopeType::Global,
        None,
        "Tenant B",
    )
    .await;

    let err = service
        .set_category_placement(
            tenant_a,
            admin(),
            child,
            SetTaxonomyCategoryPlacementInput {
                parent_id: Some(foreign_parent),
                position: 0,
            },
        )
        .await
        .expect_err("cross-tenant parent must fail closed");
    assert!(matches!(err, TaxonomyError::TermNotFound(id) if id == foreign_parent));
}
