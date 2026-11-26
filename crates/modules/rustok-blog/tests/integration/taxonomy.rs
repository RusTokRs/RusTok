use std::sync::Arc;

use async_trait::async_trait;
use rustok_blog::dto::{CreateCategoryInput, CreateTagInput, ListCategoriesFilter, ListTagsFilter};
use rustok_blog::{BlogError, CategoryService, TagService};
use rustok_core::{MemoryTransport, SecurityContext, UserRole};
use rustok_outbox::TransactionalEventBus;
use uuid::Uuid;

use super::fixtures::{TestResult, ensure_blog_schema, setup_blog_test_db};

struct NoopCategoryDeleteCleanup;

#[async_trait]
impl rustok_blog::BlogCategoryDeleteCleanupPort for NoopCategoryDeleteCleanup {
    async fn cleanup_in_tx(
        &self,
        _txn: &sea_orm::DatabaseTransaction,
        _tenant_id: Uuid,
        _category_id: Uuid,
    ) -> Result<(), rustok_blog::BlogError> {
        Ok(())
    }
}

#[tokio::test]
async fn test_category_crud() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let _receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let category_service = CategoryService::new(db.clone(), event_bus)
        .with_taxonomy_port(rustok_content_orchestration::in_process_blog_taxonomy_port(
            db.clone(),
        ))
        .with_category_delete_cleanup(Arc::new(NoopCategoryDeleteCleanup));

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let category_id = category_service
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Technology".to_string(),
                slug: None,
                description: Some("Tech articles".to_string()),
                parent_id: None,
                position: None,
                settings: serde_json::json!({}),
            },
        )
        .await?;

    let category = category_service
        .get(tenant_id, admin.clone(), category_id, "en")
        .await?;
    assert_eq!(category.name, "Technology");
    assert_eq!(category.slug, "technology");
    assert_eq!(category.description.as_deref(), Some("Tech articles"));

    let (list, total) = category_service
        .list(
            tenant_id,
            admin.clone(),
            ListCategoriesFilter {
                locale: Some("en".to_string()),
                page: 1,
                per_page: 20,
            },
        )
        .await?;
    assert_eq!(total, 1);
    assert_eq!(list[0].name, "Technology");

    category_service
        .delete(tenant_id, category_id, admin.clone())
        .await?;

    let not_found = category_service
        .get(tenant_id, admin.clone(), category_id, "en")
        .await
        .expect_err("deleted category should not be found");
    assert!(matches!(not_found, BlogError::CategoryNotFound(_)));

    Ok(())
}

#[tokio::test]
async fn test_tag_crud_and_slug_normalization() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let _receiver = transport.subscribe();
    let tag_service = TagService::new(db.clone()).with_taxonomy_port(
        rustok_content_orchestration::in_process_blog_taxonomy_port(db.clone()),
    );

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let tag_id = tag_service
        .create_tag(
            tenant_id,
            admin.clone(),
            CreateTagInput {
                locale: "en".to_string(),
                name: "Rust Language".to_string(),
                slug: None,
            },
        )
        .await?;

    let tag = tag_service
        .get_tag(tenant_id, admin.clone(), tag_id, "en")
        .await?;
    assert_eq!(tag.name, "Rust Language");
    assert_eq!(tag.slug, "rust-language");

    let tag_id2 = tag_service
        .create_tag(
            tenant_id,
            admin.clone(),
            CreateTagInput {
                locale: "en".to_string(),
                name: "Web".to_string(),
                slug: Some("web-custom".to_string()),
            },
        )
        .await?;

    let (list, total) = tag_service
        .list_tags(
            tenant_id,
            admin.clone(),
            ListTagsFilter {
                locale: Some("en".to_string()),
                page: 1,
                per_page: 20,
            },
        )
        .await?;
    assert_eq!(total, 2);

    tag_service
        .delete_tag(tenant_id, tag_id2, admin.clone())
        .await?;

    let (list_after, total_after) = tag_service
        .list_tags(
            tenant_id,
            admin.clone(),
            ListTagsFilter {
                locale: Some("en".to_string()),
                page: 1,
                per_page: 20,
            },
        )
        .await?;
    assert_eq!(total_after, 1);
    assert_eq!(list_after[0].slug, "rust-language");

    let _ = list;

    Ok(())
}

#[tokio::test]
async fn test_taxonomy_services_enforce_rbac() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let _receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let taxonomy_port = rustok_content_orchestration::in_process_blog_taxonomy_port(db.clone());
    let category_service =
        CategoryService::new(db.clone(), event_bus).with_taxonomy_port(taxonomy_port.clone());
    let tag_service = TagService::new(db).with_taxonomy_port(taxonomy_port);

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    let denied_category = category_service
        .create(
            tenant_id,
            customer.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Denied".to_string(),
                slug: None,
                description: None,
                parent_id: None,
                position: None,
                settings: serde_json::json!({}),
            },
        )
        .await
        .expect_err("customer should not create categories");
    assert!(matches!(denied_category, BlogError::Forbidden(_)));

    let denied_tag = tag_service
        .create_tag(
            tenant_id,
            customer,
            CreateTagInput {
                locale: "en".to_string(),
                name: "Denied".to_string(),
                slug: None,
            },
        )
        .await
        .expect_err("customer should not create tags");
    assert!(matches!(denied_tag, BlogError::Forbidden(_)));

    let category_id = category_service
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Allowed".to_string(),
                slug: None,
                description: None,
                parent_id: None,
                position: None,
                settings: serde_json::json!({}),
            },
        )
        .await?;
    let tag_id = tag_service
        .create_tag(
            tenant_id,
            admin.clone(),
            CreateTagInput {
                locale: "en".to_string(),
                name: "Allowed".to_string(),
                slug: None,
            },
        )
        .await?;

    let category = category_service
        .get(tenant_id, admin.clone(), category_id, "en")
        .await?;
    let tag = tag_service.get_tag(tenant_id, admin, tag_id, "en").await?;
    assert_eq!(category.name, "Allowed");
    assert_eq!(tag.name, "Allowed");

    Ok(())
}
