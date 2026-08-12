use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, ModuleTermUpdateInput, TaxonomyError, TaxonomyModule,
    TaxonomyScopeType, TaxonomyService, TaxonomyTermKind, UpdateTaxonomyTermInput,
    entities::taxonomy_term_route_key, update_module_term_in_tx,
};
use rustok_test_utils::db::setup_test_db;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    TransactionTrait,
};
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
    aliases: Vec<String>,
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
                aliases,
            },
        )
        .await
        .expect("module term should be created")
}

async fn route_keys(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    term_id: Uuid,
) -> Vec<String> {
    let mut keys = taxonomy_term_route_key::Entity::find()
        .filter(taxonomy_term_route_key::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_route_key::Column::TermId.eq(term_id))
        .all(db)
        .await
        .expect("route-key registry should be readable")
        .into_iter()
        .map(|route| route.route_key)
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

#[tokio::test]
async fn same_term_translation_and_alias_share_one_route_reservation() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let term_id = create_module_term(
        &service,
        tenant_id,
        "Systems",
        "systems",
        vec!["systems".to_string()],
    )
    .await;

    assert_eq!(route_keys(&db, tenant_id, term_id).await, vec!["systems"]);
}

#[tokio::test]
async fn update_reserves_new_route_keys_and_releases_stale_keys() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let term_id = create_module_term(
        &service,
        tenant_id,
        "Rust",
        "rust",
        vec!["systems".to_string()],
    )
    .await;

    service
        .update_term(
            tenant_id,
            term_id,
            admin(),
            UpdateTaxonomyTermInput {
                locale: "en".to_string(),
                name: None,
                slug: Some("ferris".to_string()),
                description: None,
                status: None,
                aliases: Some(vec!["ecosystem".to_string()]),
            },
        )
        .await
        .expect("localized route update should succeed");

    assert_eq!(
        route_keys(&db, tenant_id, term_id).await,
        vec!["ecosystem".to_string(), "ferris".to_string()]
    );
}

#[tokio::test]
async fn module_owner_update_rejects_route_reserved_by_an_alias() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let _alias_owner = create_module_term(
        &service,
        tenant_id,
        "Rust",
        "rust",
        vec!["systems".to_string()],
    )
    .await;
    let target = create_module_term(&service, tenant_id, "Zig", "zig", vec![]).await;

    let txn = db.begin().await.expect("transaction should start");
    let error = update_module_term_in_tx(
        &txn,
        tenant_id,
        target,
        &admin(),
        TaxonomyTermKind::Tag,
        "blog",
        ModuleTermUpdateInput {
            locale: "en".to_string(),
            name: None,
            slug: Some("systems".to_string()),
        },
    )
    .await
    .expect_err("module owner update must see alias route reservations");
    txn.rollback().await.expect("transaction should roll back");

    assert!(matches!(error, TaxonomyError::DuplicateSlug(slug) if slug == "systems"));
}

#[tokio::test]
async fn database_primary_key_rejects_second_route_owner() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let first = create_module_term(&service, tenant_id, "Rust", "systems", vec![]).await;
    let second = create_module_term(&service, tenant_id, "Zig", "zig", vec![]).await;

    assert_ne!(first, second);
    let error = taxonomy_term_route_key::ActiveModel {
        tenant_id: Set(tenant_id),
        kind: Set(TaxonomyTermKind::Tag),
        scope_type: Set(TaxonomyScopeType::Module),
        scope_value: Set("blog".to_string()),
        locale: Set("en".to_string()),
        route_key: Set("systems".to_string()),
        term_id: Set(second),
    }
    .insert(&db)
    .await
    .expect_err("database must reject a second owner of one route identity");

    assert!(matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    ));
}

#[tokio::test]
async fn deleting_term_cascades_route_reservations() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let term_id = create_module_term(
        &service,
        tenant_id,
        "Rust",
        "rust",
        vec!["systems".to_string()],
    )
    .await;

    service
        .delete_term(tenant_id, term_id, admin())
        .await
        .expect("term deletion should succeed");

    assert!(route_keys(&db, tenant_id, term_id).await.is_empty());
}
