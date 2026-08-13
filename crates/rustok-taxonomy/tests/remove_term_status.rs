use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, TaxonomyModule, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
    entities::translation_change,
};
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use sea_orm_migration::prelude::SchemaManager;
use uuid::Uuid;

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

#[tokio::test]
async fn status_removal_migration_promotes_legacy_terms_and_drops_soft_lifecycle() {
    let db = setup_test_db().await;
    let schema_manager = SchemaManager::new(&db);
    let mut migrations = TaxonomyModule.migrations();
    let status_removal = migrations
        .pop()
        .expect("status removal must remain the final retained taxonomy migration");

    for migration in migrations {
        migration
            .up(&schema_manager)
            .await
            .expect("pre-cleanup taxonomy migration should succeed");
    }

    assert!(
        schema_manager
            .has_column("taxonomy_terms", "status")
            .await
            .expect("status column inspection should succeed")
    );

    let tenant_id = Uuid::new_v4();
    let service = TaxonomyService::new(db.clone());
    let term_id = service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                scope_type: TaxonomyScopeType::Module,
                scope_value: Some("blog".to_string()),
                locale: "en".to_string(),
                name: "Legacy".to_string(),
                slug: Some("legacy".to_string()),
                canonical_key: Some("legacy".to_string()),
                description: None,
                aliases: Vec::new(),
            },
        )
        .await
        .expect("legacy term should be created before status removal");

    db.execute_unprepared("UPDATE taxonomy_terms SET status = 'deprecated'")
        .await
        .expect("legacy term should be marked deprecated");
    db.execute_unprepared(
        "UPDATE taxonomy_translation_changes SET lifecycle = 'archived' WHERE operation <> 'delete'",
    )
    .await
    .expect("legacy translation evidence should be marked archived");

    status_removal
        .up(&schema_manager)
        .await
        .expect("status removal migration should succeed");

    assert!(
        !schema_manager
            .has_column("taxonomy_terms", "status")
            .await
            .expect("status column inspection should succeed")
    );

    let evidence = translation_change::Entity::find()
        .filter(translation_change::Column::TenantId.eq(tenant_id))
        .filter(translation_change::Column::TermId.eq(term_id))
        .all(&db)
        .await
        .expect("translation change evidence should remain readable");
    assert!(!evidence.is_empty());
    assert!(
        evidence
            .iter()
            .filter(|change| change.operation != "delete")
            .all(|change| change.lifecycle == "active")
    );

    let term = service
        .get_term(tenant_id, admin(), term_id, "en", None)
        .await
        .expect("surviving term should remain readable after status removal");
    assert_eq!(term.id, term_id);
    assert_eq!(term.slug, "legacy");
}
