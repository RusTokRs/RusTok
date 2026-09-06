use std::sync::Arc;

use rustok_blog::BlogModule;
use rustok_blog::dto::{CreateCategoryInput, ListCategoriesFilter};
use rustok_blog::entities::blog_category;
use rustok_blog::services::CategoryService;
use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
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

fn service(db: &DatabaseConnection) -> CategoryService {
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
    settings: serde_json::Value,
) -> Uuid {
    service
        .create(
            tenant_id,
            admin(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: name.to_string(),
                slug: Some(name.to_ascii_lowercase().replace(' ', "-")),
                description: Some(format!("{name} description")),
                parent_id,
                position: Some(position),
                settings,
            },
        )
        .await
        .expect("Blog category should be created")
}

#[tokio::test]
async fn public_get_and_list_use_taxonomy_copy_and_placement_after_storage_retirement() {
    let db = setup().await;
    let manager = SchemaManager::new(&db);
    assert!(
        !manager
            .has_table("blog_category_translations")
            .await
            .expect("legacy translation table lookup should succeed")
    );

    let service = service(&db);
    let tenant_id = Uuid::new_v4();

    let root = create_category(
        &service,
        tenant_id,
        "Root",
        None,
        0,
        serde_json::json!({"layout": "root"}),
    )
    .await;
    let other_root = create_category(
        &service,
        tenant_id,
        "Other Root",
        None,
        1,
        serde_json::json!({}),
    )
    .await;
    let child = create_category(
        &service,
        tenant_id,
        "Child",
        Some(root),
        0,
        serde_json::json!({"layout": "blog-owned"}),
    )
    .await;

    let category = blog_category::Entity::find_by_id(child)
        .filter(blog_category::Column::TenantId.eq(tenant_id))
        .one(&db)
        .await
        .expect("Blog category read should succeed")
        .expect("Blog category should exist");
    let mut poisoned_category: blog_category::ActiveModel = category.into();
    poisoned_category.parent_id = Set(Some(other_root));
    poisoned_category.position = Set(77);
    poisoned_category
        .update(&db)
        .await
        .expect("legacy placement poison should persist");

    let read = service
        .get(tenant_id, admin(), child, "ar")
        .await
        .expect("Taxonomy-backed category get should succeed");
    assert_eq!(read.locale, "ar");
    assert_eq!(read.effective_locale, "en");
    assert_eq!(read.name, "Child");
    assert_eq!(read.slug, "child");
    assert_eq!(read.parent_id, Some(root));
    assert_eq!(read.position, 0);
    assert_eq!(read.settings, serde_json::json!({"layout": "blog-owned"}));

    let (items, total) = service
        .list(
            tenant_id,
            admin(),
            ListCategoriesFilter {
                locale: Some("ar".to_string()),
                page: 1,
                per_page: 100,
            },
        )
        .await
        .expect("Taxonomy-backed category list should succeed");
    assert_eq!(total, 3);
    let child_item = items
        .iter()
        .find(|item| item.id == child)
        .expect("child should remain in Blog membership list");
    assert_eq!(child_item.effective_locale, "en");
    assert_eq!(child_item.name, "Child");
    assert_eq!(child_item.slug, "child");
    assert_eq!(child_item.parent_id, Some(root));
    assert_eq!(child_item.position, 0);
    assert_eq!(
        child_item.settings,
        serde_json::json!({"layout": "blog-owned"})
    );
}
