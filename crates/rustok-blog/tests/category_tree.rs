use std::sync::Arc;

use rustok_blog::dto::CreateCategoryInput;
use rustok_blog::entities::blog_category;
use rustok_blog::services::{CategoryService, CategoryTreeService};
use rustok_blog::BlogModule;
use rustok_comments::CommentsModule;
use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait};
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

#[tokio::test]
async fn tree_read_is_ordered_localized_and_rejects_materialized_depth_drift() {
    let db = setup().await;
    let category_service = CategoryService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(MemoryTransport::new())),
    );
    let tree_service = CategoryTreeService::new(db.clone());
    let tenant_id = Uuid::new_v4();

    let root_a = create_category(&category_service, tenant_id, "Root A", None, 0).await;
    let root_b = create_category(&category_service, tenant_id, "Root B", None, 1).await;
    let child = create_category(&category_service, tenant_id, "Child", Some(root_a), 0).await;

    let tree = tree_service
        .read(tenant_id, admin(), Some("FR"))
        .await
        .expect("valid tree should be readable");
    assert_eq!(tree.total_nodes, 3);
    assert_eq!(tree.max_depth, 1);
    assert_eq!(tree.roots.len(), 2);
    assert_eq!(tree.roots[0].id, root_a);
    assert_eq!(tree.roots[1].id, root_b);
    assert_eq!(tree.roots[0].requested_locale, "fr");
    assert_eq!(tree.roots[0].effective_locale, "en");
    assert_eq!(tree.roots[0].children.len(), 1);
    assert_eq!(tree.roots[0].children[0].id, child);
    assert_eq!(tree.roots[0].children[0].depth, 1);

    let child_model = blog_category::Entity::find_by_id(child)
        .one(&db)
        .await
        .expect("child read should succeed")
        .expect("child should exist");
    let mut active: blog_category::ActiveModel = child_model.into();
    active.depth = Set(9);
    active
        .update(&db)
        .await
        .expect("fixture should corrupt materialized depth");

    let error = tree_service
        .read(tenant_id, admin(), Some("fr"))
        .await
        .expect_err("depth drift must fail closed");
    assert!(error.to_string().contains("materialized depth"));
}
