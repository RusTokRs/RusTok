use std::sync::Arc;

use rustok_blog::dto::{CreateCategoryInput, MoveCategoryInput};
use rustok_blog::entities::blog_category;
use rustok_blog::services::{CategoryCommandService, CategoryService};
use rustok_blog::{BlogError, BlogModule};
use rustok_comments::CommentsModule;
use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
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
    for migration in CommentsModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("comments migration should apply");
    }
    db
}

fn services(db: &DatabaseConnection) -> (CategoryService, CategoryCommandService) {
    let event_bus = TransactionalEventBus::new(Arc::new(MemoryTransport::new()));
    (
        CategoryService::new(db.clone(), event_bus.clone()),
        CategoryCommandService::new(db.clone(), event_bus),
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
        .expect("category should be created")
}

async fn load_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
) -> blog_category::Model {
    blog_category::Entity::find_by_id(category_id)
        .filter(blog_category::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .expect("category read should succeed")
        .expect("category should exist")
}

#[tokio::test]
async fn move_reparents_subtree_and_failed_moves_leave_tree_unchanged() {
    let db = setup().await;
    let (category_service, command_service) = services(&db);
    let tenant_id = Uuid::new_v4();

    let root_a = create_category(&category_service, tenant_id, "Root A", None, 0).await;
    let root_b = create_category(&category_service, tenant_id, "Root B", None, 1).await;
    let child = create_category(&category_service, tenant_id, "Child", Some(root_a), 0).await;
    let grandchild =
        create_category(&category_service, tenant_id, "Grandchild", Some(child), 0).await;

    assert_eq!(load_category(&db, tenant_id, child).await.depth, 1);
    assert_eq!(load_category(&db, tenant_id, grandchild).await.depth, 2);

    let moved = command_service
        .move_category(
            tenant_id,
            child,
            admin(),
            MoveCategoryInput {
                parent_id: Some(root_b),
                position: 0,
            },
        )
        .await
        .expect("child should move under the second root");
    assert_eq!(moved.moved.parent_id, Some(root_b));
    assert_eq!(moved.moved.depth, 1);
    assert!(
        moved
            .updated
            .iter()
            .any(|placement| placement.id == grandchild && placement.depth == 2)
    );

    let child_after_reparent = load_category(&db, tenant_id, child).await;
    let grandchild_after_reparent = load_category(&db, tenant_id, grandchild).await;
    assert_eq!(child_after_reparent.parent_id, Some(root_b));
    assert_eq!(child_after_reparent.depth, 1);
    assert_eq!(grandchild_after_reparent.parent_id, Some(child));
    assert_eq!(grandchild_after_reparent.depth, 2);

    let moved_to_root = command_service
        .move_category(
            tenant_id,
            child,
            admin(),
            MoveCategoryInput {
                parent_id: None,
                position: 2,
            },
        )
        .await
        .expect("child should move to the root level");
    assert_eq!(moved_to_root.moved.parent_id, None);
    assert_eq!(moved_to_root.moved.depth, 0);
    assert_eq!(load_category(&db, tenant_id, grandchild).await.depth, 1);

    let self_parent = command_service
        .move_category(
            tenant_id,
            child,
            admin(),
            MoveCategoryInput {
                parent_id: Some(child),
                position: 0,
            },
        )
        .await
        .expect_err("a category cannot become its own parent");
    assert!(matches!(self_parent, BlogError::Validation(_)));

    let descendant_parent = command_service
        .move_category(
            tenant_id,
            child,
            admin(),
            MoveCategoryInput {
                parent_id: Some(grandchild),
                position: 0,
            },
        )
        .await
        .expect_err("a category cannot move beneath its own descendant");
    assert!(matches!(descendant_parent, BlogError::Validation(_)));

    let other_tenant = Uuid::new_v4();
    let foreign_parent =
        create_category(&category_service, other_tenant, "Foreign Root", None, 0).await;
    let foreign_parent_error = command_service
        .move_category(
            tenant_id,
            child,
            admin(),
            MoveCategoryInput {
                parent_id: Some(foreign_parent),
                position: 0,
            },
        )
        .await
        .expect_err("cross-tenant parent must be rejected");
    assert!(matches!(foreign_parent_error, BlogError::Validation(_)));

    let unchanged_child = load_category(&db, tenant_id, child).await;
    let unchanged_grandchild = load_category(&db, tenant_id, grandchild).await;
    assert_eq!(unchanged_child.parent_id, None);
    assert_eq!(unchanged_child.depth, 0);
    assert_eq!(unchanged_grandchild.parent_id, Some(child));
    assert_eq!(unchanged_grandchild.depth, 1);
}
