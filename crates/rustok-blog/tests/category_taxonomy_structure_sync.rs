use std::sync::Arc;

use rustok_blog::BlogModule;
use rustok_blog::dto::{CreateCategoryInput, MoveCategoryInput};
use rustok_blog::services::{CategoryCommandService, CategoryService};
use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::{TaxonomyModule, entities::taxonomy_category_hierarchy};
use sea_orm::{DatabaseConnection, EntityTrait};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> DatabaseConnection {
    let db = rustok_test_utils::db::setup_test_db().await;
    let manager = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in BlogModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("blog migration should apply");
    }
    db
}

fn category_service(db: &DatabaseConnection) -> CategoryService {
    CategoryService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(MemoryTransport::new())),
    )
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn create_category(
    service: &CategoryService,
    tenant_id: Uuid,
    name: &str,
    parent_id: Option<Uuid>,
    position: i32,
) -> Uuid {
    service
        .create(
            tenant_id,
            admin(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: name.to_string(),
                slug: Some(name.to_ascii_lowercase().replace(' ', "-")),
                description: None,
                parent_id,
                position: Some(position),
                settings: serde_json::json!({}),
            },
        )
        .await
        .expect("Blog category should be created")
}

async fn taxonomy_placement(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
) -> taxonomy_category_hierarchy::Model {
    taxonomy_category_hierarchy::Entity::find_by_id((tenant_id, category_id))
        .one(db)
        .await
        .expect("Taxonomy hierarchy read should succeed")
        .expect("Taxonomy hierarchy placement should exist")
}

#[tokio::test]
async fn create_at_index_keeps_taxonomy_sibling_positions_dense() {
    let db = setup().await;
    let service = category_service(&db);
    let tenant_id = Uuid::new_v4();

    let first = create_category(&service, tenant_id, "First", None, 0).await;
    let last = create_category(&service, tenant_id, "Last", None, 1).await;
    let middle = create_category(&service, tenant_id, "Middle", None, 1).await;

    assert_eq!(taxonomy_placement(&db, tenant_id, first).await.position, 0);
    assert_eq!(taxonomy_placement(&db, tenant_id, middle).await.position, 1);
    assert_eq!(taxonomy_placement(&db, tenant_id, last).await.position, 2);
}

#[tokio::test]
async fn move_reparent_syncs_taxonomy_parent_and_both_sibling_sets() {
    let db = setup().await;
    let service = category_service(&db);
    let command = CategoryCommandService::new(db.clone());
    let tenant_id = Uuid::new_v4();

    let root_a = create_category(&service, tenant_id, "Root A", None, 0).await;
    let root_b = create_category(&service, tenant_id, "Root B", None, 1).await;
    let child_a = create_category(&service, tenant_id, "Child A", Some(root_a), 0).await;
    let child_b = create_category(&service, tenant_id, "Child B", Some(root_a), 1).await;
    let child_c = create_category(&service, tenant_id, "Child C", Some(root_b), 0).await;

    command
        .move_category(
            tenant_id,
            child_a,
            admin(),
            MoveCategoryInput {
                parent_id: Some(root_b),
                position: 0,
            },
        )
        .await
        .expect("Blog category move should synchronize Taxonomy hierarchy");

    let moved = taxonomy_placement(&db, tenant_id, child_a).await;
    assert_eq!(moved.parent_term_id, Some(root_b));
    assert_eq!(moved.position, 0);

    let source_remaining = taxonomy_placement(&db, tenant_id, child_b).await;
    assert_eq!(source_remaining.parent_term_id, Some(root_a));
    assert_eq!(source_remaining.position, 0);

    let target_shifted = taxonomy_placement(&db, tenant_id, child_c).await;
    assert_eq!(target_shifted.parent_term_id, Some(root_b));
    assert_eq!(target_shifted.position, 1);
}
