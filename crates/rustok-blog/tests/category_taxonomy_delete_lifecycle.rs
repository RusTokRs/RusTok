use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use rustok_blog::{
    BlogCategoryTaxonomyBindingEntity, BlogModule, CategoryService, CreateCategoryInput,
    entities::blog_category,
};
use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::{
    TaxonomyCategoryDeleteCleanupPort, TaxonomyError, TaxonomyModule, TaxonomyResult,
    entities::{taxonomy_category_hierarchy, taxonomy_term},
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

struct RecordingCleanup {
    calls: Arc<AtomicUsize>,
    fail: bool,
}

#[async_trait]
impl TaxonomyCategoryDeleteCleanupPort for RecordingCleanup {
    async fn cleanup_in_tx(
        &self,
        _txn: &sea_orm::DatabaseTransaction,
        _tenant_id: Uuid,
        _category_id: Uuid,
    ) -> TaxonomyResult<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(TaxonomyError::validation("forced cleanup failure"));
        }
        Ok(())
    }
}

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

fn service(
    db: &DatabaseConnection,
    cleanup: Arc<dyn TaxonomyCategoryDeleteCleanupPort>,
) -> CategoryService {
    CategoryService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(MemoryTransport::new())),
    )
    .with_category_delete_cleanup(cleanup)
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

fn create_input(name: &str, position: i32) -> CreateCategoryInput {
    CreateCategoryInput {
        locale: "en".to_string(),
        name: name.to_string(),
        slug: Some(name.to_ascii_lowercase()),
        description: None,
        parent_id: None,
        position: Some(position),
        settings: serde_json::json!({}),
    }
}

#[tokio::test]
async fn delete_removes_blog_binding_and_taxonomy_owner_and_replays_sibling_position() {
    let db = setup().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let service = service(
        &db,
        Arc::new(RecordingCleanup {
            calls: calls.clone(),
            fail: false,
        }),
    );
    let tenant_id = Uuid::new_v4();

    let first = service
        .create(tenant_id, admin(), create_input("First", 0))
        .await
        .expect("first Blog Category should be created");
    let second = service
        .create(tenant_id, admin(), create_input("Second", 1))
        .await
        .expect("second Blog Category should be created");

    service
        .delete(tenant_id, first, admin())
        .await
        .expect("Blog delete should use Taxonomy owner lifecycle");

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(
        blog_category::Entity::find_by_id(first)
            .one(&db)
            .await
            .expect("Blog Category lookup should succeed")
            .is_none()
    );
    assert!(
        BlogCategoryTaxonomyBindingEntity::find_by_id((tenant_id, first))
            .one(&db)
            .await
            .expect("binding lookup should succeed")
            .is_none()
    );
    assert!(
        taxonomy_term::Entity::find_by_id(first)
            .one(&db)
            .await
            .expect("Taxonomy Category lookup should succeed")
            .is_none()
    );

    let second_blog = blog_category::Entity::find_by_id(second)
        .filter(blog_category::Column::TenantId.eq(tenant_id))
        .one(&db)
        .await
        .expect("remaining Blog Category lookup should succeed")
        .expect("remaining Blog Category should exist");
    assert_eq!(second_blog.position, 0);
    let second_taxonomy = taxonomy_category_hierarchy::Entity::find_by_id((tenant_id, second))
        .one(&db)
        .await
        .expect("Taxonomy hierarchy lookup should succeed")
        .expect("remaining Taxonomy hierarchy row should exist");
    assert_eq!(second_taxonomy.position, 0);
}

#[tokio::test]
async fn host_cleanup_failure_rolls_back_blog_and_taxonomy_deletion() {
    let db = setup().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let service = service(
        &db,
        Arc::new(RecordingCleanup {
            calls: calls.clone(),
            fail: true,
        }),
    );
    let tenant_id = Uuid::new_v4();
    let category_id = service
        .create(tenant_id, admin(), create_input("Rollback", 0))
        .await
        .expect("Blog Category should be created");

    let error = service
        .delete(tenant_id, category_id, admin())
        .await
        .expect_err("host cleanup failure must abort the owner transaction");
    assert!(error.to_string().contains("forced cleanup failure"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    assert!(
        blog_category::Entity::find_by_id(category_id)
            .one(&db)
            .await
            .expect("Blog Category lookup should succeed")
            .is_some()
    );
    assert!(
        BlogCategoryTaxonomyBindingEntity::find_by_id((tenant_id, category_id))
            .one(&db)
            .await
            .expect("binding lookup should succeed")
            .is_some()
    );
    assert!(
        taxonomy_term::Entity::find_by_id(category_id)
            .one(&db)
            .await
            .expect("Taxonomy Category lookup should succeed")
            .is_some()
    );
}
