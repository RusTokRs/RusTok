use std::sync::Arc;

use async_trait::async_trait;
use rustok_blog::{
    BlogCategoryDeleteCleanupPort, BlogError, BlogModule, CategoryService, CreateCategoryInput,
    CreateTagInput, ListTagsFilter, TagService, UpdateCategoryInput, UpdateTagInput,
};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_taxonomy::{ModuleTermCreateInput, TaxonomyModule, TaxonomyService, TaxonomyTermKind};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction,
    TransactionTrait,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

use super::in_process_blog_taxonomy_port;

async fn setup() -> DatabaseConnection {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1).sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE users (id TEXT NOT NULL PRIMARY KEY, tenant_id TEXT NOT NULL, UNIQUE (tenant_id, id))",
    )
    .await
    .unwrap();
    let manager = SchemaManager::new(&db);
    SysEventsMigration.up(&manager).await.unwrap();
    for migration in TaxonomyModule
        .migrations()
        .into_iter()
        .chain(BlogModule.migrations())
    {
        migration.up(&manager).await.unwrap();
    }
    db
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

struct RejectCleanup;

#[async_trait]
impl BlogCategoryDeleteCleanupPort for RejectCleanup {
    async fn cleanup_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _category_id: Uuid,
    ) -> Result<(), BlogError> {
        Err(BlogError::conflict("attached data refused cleanup"))
    }
}

#[tokio::test]
async fn category_copy_and_delete_use_taxonomy_and_preserve_typed_cleanup_failure() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let port = in_process_blog_taxonomy_port(db.clone());
    let bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = CategoryService::new(db.clone(), bus)
        .with_taxonomy_port(port.clone())
        .with_category_delete_cleanup(Arc::new(RejectCleanup));
    let category_id = service
        .create(
            tenant_id,
            admin(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "World".into(),
                slug: Some("world".into()),
                description: None,
                parent_id: None,
                position: Some(0),
                settings: serde_json::json!({}),
            },
        )
        .await
        .unwrap();
    let category = service
        .update(
            tenant_id,
            category_id,
            admin(),
            UpdateCategoryInput {
                locale: "en".into(),
                name: Some("News".into()),
                slug: Some("news".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(category.slug, "news");
    let taxonomy = TaxonomyService::new(db.clone());
    let historical = taxonomy
        .resolve_term_route_for_module(
            tenant_id,
            TaxonomyTermKind::Category,
            "blog",
            "en",
            None,
            "world",
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(historical.term_id, category_id);
    assert!(historical.alias_id.is_some());

    let error = service
        .delete(tenant_id, category_id, admin())
        .await
        .unwrap_err();
    assert!(
        matches!(error, BlogError::Conflict(message) if message == "attached data refused cleanup")
    );
    let category = service
        .get(tenant_id, admin(), category_id, "en")
        .await
        .unwrap();
    assert_eq!(category.slug, "news");

    let service = service
        .with_category_delete_cleanup(Arc::new(rustok_blog::NoopBlogCategoryDeleteCleanupPort));
    service
        .delete(tenant_id, category_id, admin())
        .await
        .unwrap();
    let result = taxonomy
        .resolve_term_route_for_module(
            tenant_id,
            TaxonomyTermKind::Category,
            "blog",
            "en",
            None,
            "world",
        )
        .await
        .unwrap();
    assert!(result.is_none());
    assert!(matches!(
        service.get(tenant_id, admin(), category_id, "en").await,
        Err(BlogError::CategoryNotFound(_))
    ));
}

#[tokio::test]
async fn tag_mutations_use_owner_scope_and_caller_transaction() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let port = in_process_blog_taxonomy_port(db.clone());
    let tags = TagService::new(db.clone()).with_taxonomy_port(port.clone());
    let id = tags
        .create_tag(
            tenant_id,
            admin(),
            CreateTagInput {
                locale: "en".into(),
                name: "Notes".into(),
                slug: None,
            },
        )
        .await
        .unwrap();
    let taxonomy = TaxonomyService::new(db.clone());
    let txn = db.begin().await.unwrap();
    let other_term = taxonomy
        .create_module_term_in_tx(
            &txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            "product",
            ModuleTermCreateInput {
                locale: "en".into(),
                name: "Different".into(),
                slug: None,
                canonical_key: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        port.find_all_valid_tag_ids((&txn).into(), tenant_id)
            .await
            .unwrap(),
        [id].into_iter().collect()
    );
    assert!(
        !port
            .load_tags_by_ids((&txn).into(), tenant_id, &[other_term], "en", None)
            .await
            .unwrap()
            .contains_key(&other_term)
    );
    assert!(
        port.find_all_valid_tag_ids((&txn).into(), Uuid::new_v4())
            .await
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        port.ensure_module_tags_in_tx(&txn, tenant_id, "en", &["Absent".into()], false)
            .await,
        Err(BlogError::Forbidden(_))
    ));
    let created = port
        .ensure_module_tags_in_tx(&txn, tenant_id, "en", &["Temporary".into()], true)
        .await
        .unwrap()[0];
    assert_eq!(
        port.resolve_tag_id_for_module((&txn).into(), tenant_id, "temporary", "en", None)
            .await
            .unwrap(),
        Some(created)
    );
    txn.rollback().await.unwrap();

    let updated = tags
        .update_tag(
            tenant_id,
            id,
            admin(),
            UpdateTagInput {
                locale: "en".into(),
                name: Some("Field notes".into()),
                slug: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.slug, "field-notes");
    let (items, count) = tags
        .list_tags(
            tenant_id,
            admin(),
            ListTagsFilter {
                locale: Some("en".into()),
                page: 1,
                per_page: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(items[0].id, id);
    tags.delete_tag(tenant_id, id, admin()).await.unwrap();
    assert!(
        port.find_all_valid_tag_ids((&db).into(), tenant_id)
            .await
            .unwrap()
            .is_empty()
    );
}
