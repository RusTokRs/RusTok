use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, TaxonomyModule, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
    taxonomy_term_identity_exists,
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
    name: &str,
) -> Uuid {
    service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
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
async fn owner_identity_is_bounded_by_tenant_and_term_kind() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let category_id = create_term(
        &service,
        tenant_id,
        TaxonomyTermKind::Category,
        "Category identity fixture",
    )
    .await;
    let tag_id = create_term(
        &service,
        tenant_id,
        TaxonomyTermKind::Tag,
        "Tag identity fixture",
    )
    .await;

    assert!(
        taxonomy_term_identity_exists(&db, tenant_id, TaxonomyTermKind::Category, category_id)
            .await
            .expect("Category identity lookup should succeed")
    );
    assert!(
        !taxonomy_term_identity_exists(
            &db,
            other_tenant_id,
            TaxonomyTermKind::Category,
            category_id
        )
        .await
        .expect("foreign tenant lookup should succeed")
    );
    assert!(
        !taxonomy_term_identity_exists(&db, tenant_id, TaxonomyTermKind::Category, tag_id)
            .await
            .expect("wrong kind lookup should succeed")
    );
    assert!(
        !taxonomy_term_identity_exists(&db, tenant_id, TaxonomyTermKind::Category, Uuid::new_v4(),)
            .await
            .expect("missing identity lookup should succeed")
    );
}
