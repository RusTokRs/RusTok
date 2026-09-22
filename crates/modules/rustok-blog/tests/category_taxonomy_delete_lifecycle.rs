use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use rustok_blog::{BlogError, BlogModule, CategoryService, CreateCategoryInput};
use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_events::EventEnvelope;
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::{
    TaxonomyCategoryDeleteCleanupPort, TaxonomyError, TaxonomyModule, TaxonomyResult,
    entities::{taxonomy_category_hierarchy, taxonomy_term},
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, Statement,
};
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
) -> (
    CategoryService,
    tokio::sync::broadcast::Receiver<EventEnvelope>,
) {
    let transport = MemoryTransport::new();
    let receiver = transport.subscribe();
    let service = CategoryService::new(db.clone(), TransactionalEventBus::new(Arc::new(transport)))
        .with_category_delete_cleanup(cleanup);
    (service, receiver)
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

fn create_child_input(name: &str, parent_id: Uuid, position: i32) -> CreateCategoryInput {
    CreateCategoryInput {
        locale: "en".to_string(),
        name: name.to_string(),
        slug: Some(name.to_ascii_lowercase()),
        description: None,
        parent_id: Some(parent_id),
        position: Some(position),
        settings: serde_json::json!({}),
    }
}

#[tokio::test]
async fn delete_category_detaches_posts_without_dangling_reference() {
    let db = setup().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let (service, _events) = service(
        &db,
        Arc::new(RecordingCleanup {
            calls: calls.clone(),
            fail: false,
        }),
    );
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let category_id = service
        .create(tenant_id, admin(), create_input("Posts", 0))
        .await
        .expect("Blog Category should be created");

    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
        INSERT INTO blog_posts (
            id, tenant_id, author_id, category_id, status, slug, metadata,
            published_at, created_at, updated_at, archived_at,
            comment_count, view_count, version
        ) VALUES (
            ?, ?, ?, ?, 'draft', ?, '{}', NULL,
            CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, NULL, 0, 0, 1
        )
        "#,
        [
            post_id.into(),
            tenant_id.into(),
            author_id.into(),
            category_id.into(),
            "category-delete-post".to_string().into(),
        ],
    ))
    .await
    .expect("post should reference the Blog Category");

    service
        .delete(tenant_id, category_id, admin())
        .await
        .expect("Blog Category delete should detach assigned posts safely");

    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            db.get_database_backend(),
            "SELECT category_id, version FROM blog_posts WHERE tenant_id = ? AND id = ?",
            [tenant_id.into(), post_id.into()],
        ))
        .await
        .expect("post lookup should succeed")
        .expect("post should remain after category deletion");
    let category_id: Option<Uuid> = row
        .try_get("", "category_id")
        .expect("post category_id should be readable");
    let version: i32 = row
        .try_get("", "version")
        .expect("post version should be readable");
    assert!(category_id.is_none());
    assert_eq!(version, 2);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn delete_removes_blog_binding_and_taxonomy_owner_and_replays_sibling_position() {
    let db = setup().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let (service, _events) = service(
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
    assert!(matches!(
        service
            .get(tenant_id, admin(), first, "en")
            .await
            .expect_err("deleted Blog Category must disappear from owner reads"),
        BlogError::CategoryNotFound(_)
    ));
    assert!(
        taxonomy_term::Entity::find_by_id(first)
            .one(&db)
            .await
            .expect("Taxonomy Category lookup should succeed")
            .is_none()
    );

    service
        .get(tenant_id, admin(), second, "en")
        .await
        .expect("remaining Blog Category should exist through owner reads");
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
    let (service, _events) = service(
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

    service
        .get(tenant_id, admin(), category_id, "en")
        .await
        .expect("rolled-back Blog Category must remain visible through owner reads");
    assert!(
        taxonomy_term::Entity::find_by_id(category_id)
            .one(&db)
            .await
            .expect("Taxonomy Category lookup should succeed")
            .is_some()
    );
}


#[tokio::test]
async fn delete_nested_category_replays_only_its_sibling_positions() {
    let db = setup().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let (service, _events) = service(
        &db,
        Arc::new(RecordingCleanup {
            calls: calls.clone(),
            fail: false,
        }),
    );
    let tenant_id = Uuid::new_v4();

    let root = service
        .create(tenant_id, admin(), create_input("Root", 0))
        .await
        .expect("root Blog Category should be created");
    let first_child = service
        .create(tenant_id, admin(), create_child_input("First Child", root, 0))
        .await
        .expect("first child Blog Category should be created");
    let second_child = service
        .create(tenant_id, admin(), create_child_input("Second Child", root, 1))
        .await
        .expect("second child Blog Category should be created");

    service
        .delete(tenant_id, first_child, admin())
        .await
        .expect("nested Blog Category delete should compact only destination siblings");

    let remaining = taxonomy_category_hierarchy::Entity::find()
        .filter(taxonomy_category_hierarchy::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_category_hierarchy::Column::TermId.eq(second_child))
        .one(&db)
        .await
        .expect("remaining sibling hierarchy lookup should succeed")
        .expect("remaining sibling hierarchy row should exist");

    assert_eq!(remaining.parent_term_id, Some(root));
    assert_eq!(remaining.position, 0);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}


#[tokio::test]
async fn create_rejects_preexisting_hierarchy_coverage_drift() {
    let db = setup().await;
    let (service, _events) = service(
        &db,
        Arc::new(RecordingCleanup {
            calls: Arc::new(AtomicUsize::new(0)),
            fail: false,
        }),
    );
    let tenant_id = Uuid::new_v4();

    let root = service
        .create(tenant_id, admin(), create_input("Existing", 0))
        .await
        .expect("existing Blog Category should be created");

    taxonomy_category_hierarchy::Entity::delete_by_id((tenant_id, root))
        .exec(&db)
        .await
        .expect("hierarchy corruption fixture should be created");

    let result = service
        .create(tenant_id, admin(), create_input("New", 0))
        .await;

    assert!(
        matches!(result, Err(BlogError::Invariant(_))),
        "category create must fail closed when existing Taxonomy hierarchy coverage is incomplete"
    );
}
