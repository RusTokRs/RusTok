//! Integration tests for the Blog module
//!
//! These tests require a database connection and are marked with #[ignore]
//! to prevent running in CI without proper test infrastructure.

use std::sync::Arc;

use async_trait::async_trait;
use rustok_blog::dto::CreateCommentInput;
use rustok_blog::dto::{
    CreateCategoryInput, CreatePostInput, CreateTagInput, ListCategoriesFilter, ListCommentsFilter,
    ListTagsFilter, PostListQuery, UpdateCommentInput,
};
use rustok_blog::state_machine::{BlogPost, BlogPostStatus, CommentStatus, ToBlogPostStatus};
use rustok_blog::{BlogError, BlogModule};
use rustok_blog::{CategoryService, CommentService, PostService, TagService};
use rustok_comments::{CommentsError, CommentsModule};
use rustok_core::{
    DomainEvent, EventTransport, MemoryTransport, MigrationSource, ReliabilityLevel,
    SecurityContext, UserRole,
};
use rustok_events::EventEnvelope;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use tokio::sync::broadcast;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn test_post_lifecycle() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let mut receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus.clone());

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let post_id = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Test Post".to_string(),
                body: "Hello, Blog!".to_string(),
                body_format: "markdown".to_string(),
                content_json: None,
                excerpt: Some("Short excerpt".to_string()),
                slug: None,
                publish: false,
                tags: vec!["rust".to_string()],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
            },
        )
        .await?;

    let post = post_service
        .get_post(tenant_id, admin.clone(), post_id, "en")
        .await?;
    assert_eq!(post.title, "Test Post");
    assert_eq!(post.status, BlogPostStatus::Draft);
    assert_eq!(post.tags, vec!["rust"]);

    post_service
        .publish_post(tenant_id, post_id, admin.clone())
        .await?;
    let published = post_service
        .get_post(tenant_id, admin.clone(), post_id, "en")
        .await?;
    assert_eq!(published.status, BlogPostStatus::Published);
    assert!(published.published_at.is_some());

    post_service
        .archive_post(
            tenant_id,
            post_id,
            admin.clone(),
            Some("outdated".to_string()),
        )
        .await?;
    let archived = post_service
        .get_post(tenant_id, admin.clone(), post_id, "en")
        .await?;
    assert_eq!(archived.status, BlogPostStatus::Archived);

    let event_types = drain_event_types(&mut receiver);
    assert!(event_types.iter().any(|e| e == "blog.post.created"));
    assert!(event_types.iter().any(|e| e == "blog.post.published"));
    assert!(event_types.iter().any(|e| e == "blog.post.archived"));

    Ok(())
}

#[tokio::test]
async fn test_create_and_publish_post() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let _receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus.clone());

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let post_id = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Draft Post".to_string(),
                body: "Content".to_string(),
                body_format: "markdown".to_string(),
                content_json: None,
                excerpt: None,
                slug: Some("draft-post".to_string()),
                publish: false,
                tags: vec![],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
            },
        )
        .await?;

    let draft = post_service
        .get_post(tenant_id, admin.clone(), post_id, "en")
        .await?;
    assert_eq!(draft.status, BlogPostStatus::Draft);
    assert!(draft.published_at.is_none());

    post_service
        .publish_post(tenant_id, post_id, admin.clone())
        .await?;

    let published = post_service
        .get_post(tenant_id, admin.clone(), post_id, "en")
        .await?;
    assert_eq!(published.status, BlogPostStatus::Published);
    assert!(published.published_at.is_some());
    assert_eq!(published.slug, "draft-post");

    Ok(())
}

#[tokio::test]
async fn test_post_read_paths_normalize_requested_and_fallback_locale() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let _receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus.clone());

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let post_id = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                locale: "en-us".to_string(),
                title: "Localized Post".to_string(),
                body: "Body".to_string(),
                body_format: "markdown".to_string(),
                content_json: None,
                excerpt: None,
                slug: Some("localized-post".to_string()),
                publish: true,
                tags: vec![],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
            },
        )
        .await?;

    let direct = post_service
        .get_post(tenant_id, admin.clone(), post_id, "EN_us")
        .await?;
    assert_eq!(direct.requested_locale, "en-us");
    assert_eq!(direct.locale, "en-us");
    assert_eq!(direct.effective_locale, "en-us");

    let by_fallback = post_service
        .get_post_with_locale_fallback(tenant_id, admin.clone(), post_id, "FR_fr", Some("EN_us"))
        .await?;
    assert_eq!(by_fallback.requested_locale, "fr-fr");
    assert_eq!(by_fallback.locale, "fr-fr");
    assert_eq!(by_fallback.effective_locale, "en-us");

    let listed = post_service
        .list_posts(
            tenant_id,
            SecurityContext::system(),
            PostListQuery {
                locale: Some("EN_us".to_string()),
                page: Some(1),
                per_page: Some(10),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.items[0].locale, "en-us");
    assert_eq!(listed.items[0].effective_locale, "en-us");

    Ok(())
}

#[tokio::test]
async fn test_list_posts_with_pagination() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let _receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus.clone());

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    for i in 0..5 {
        post_service
            .create_post(
                tenant_id,
                admin.clone(),
                CreatePostInput {
                    locale: "en".to_string(),
                    title: format!("Post {i}"),
                    body: "Body".to_string(),
                    body_format: "markdown".to_string(),
                    content_json: None,
                    excerpt: None,
                    slug: None,
                    publish: false,
                    tags: vec![],
                    category_id: None,
                    featured_image_url: None,
                    seo_title: None,
                    seo_description: None,
                    channel_slugs: None,
                    metadata: None,
                },
            )
            .await?;
    }

    let page1 = post_service
        .list_posts(
            tenant_id,
            admin.clone(),
            PostListQuery {
                page: Some(1),
                per_page: Some(2),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(page1.total, 5);
    assert_eq!(page1.items.len(), 2);
    assert_eq!(page1.total_pages, 3);

    let page3 = post_service
        .list_posts(
            tenant_id,
            admin.clone(),
            PostListQuery {
                page: Some(3),
                per_page: Some(2),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(page3.items.len(), 1);

    Ok(())
}

#[tokio::test]
async fn test_filter_posts_by_tag() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let _receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus.clone());

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Rust Post".to_string(),
                body: "Body".to_string(),
                body_format: "markdown".to_string(),
                content_json: None,
                excerpt: None,
                slug: None,
                publish: false,
                tags: vec!["rust".to_string()],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
            },
        )
        .await?;

    post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Go Post".to_string(),
                body: "Body".to_string(),
                body_format: "markdown".to_string(),
                content_json: None,
                excerpt: None,
                slug: None,
                publish: false,
                tags: vec!["go".to_string()],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
            },
        )
        .await?;

    let rust_posts = post_service
        .get_posts_by_tag(tenant_id, admin.clone(), "rust".to_string(), 1, 10)
        .await?;

    assert_eq!(rust_posts.total, 1);
    assert_eq!(rust_posts.items[0].tags, vec!["rust"]);

    Ok(())
}

#[tokio::test]
async fn test_cannot_delete_published_post() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let _receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus.clone());

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let post_id = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Published Post".to_string(),
                body: "Content".to_string(),
                body_format: "markdown".to_string(),
                content_json: None,
                excerpt: None,
                slug: None,
                publish: true,
                tags: vec![],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
            },
        )
        .await?;

    let err = post_service
        .delete_post(tenant_id, post_id, admin.clone())
        .await
        .expect_err("deleting a published post must fail");

    assert!(
        matches!(err, BlogError::CannotDeletePublished),
        "expected CannotDeletePublished, got: {err}"
    );

    Ok(())
}

#[tokio::test]
async fn test_category_crud() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let category_service = CategoryService::new(db.clone());

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
    let tag_service = TagService::new(db.clone());

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

    let category_service = CategoryService::new(db.clone());
    let tag_service = TagService::new(db);

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

async fn setup_blog_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:blog_integration_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);

    Database::connect(opts)
        .await
        .expect("failed to connect blog test sqlite database")
}

async fn ensure_blog_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
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
}

fn drain_event_types(receiver: &mut broadcast::Receiver<EventEnvelope>) -> Vec<String> {
    let mut types = Vec::new();
    loop {
        match receiver.try_recv() {
            Ok(envelope) => types.push(envelope.event.event_type().to_string()),
            Err(broadcast::error::TryRecvError::Empty) => break,
            Err(broadcast::error::TryRecvError::Closed) => break,
            Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
        }
    }
    types
}

#[tokio::test]
async fn test_create_comment_succeeds_with_required_translation() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let _receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));

    let post_service = PostService::new(db.clone(), event_bus.clone());
    let comment_service = CommentService::new(db.clone(), event_bus);

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let security = SecurityContext::new(UserRole::Admin, Some(actor_id));

    let post = post_service
        .create_post(
            tenant_id,
            security.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Post for comments".to_string(),
                body: "Post body".to_string(),
                body_format: "markdown".to_string(),
                content_json: None,
                excerpt: None,
                slug: None,
                publish: false,
                tags: vec![],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
            },
        )
        .await
        .expect("post should be created for comment test");

    let result = comment_service
        .create_comment(
            tenant_id,
            security,
            post,
            CreateCommentInput {
                locale: "en".to_string(),
                content: "This comment should be persisted".to_string(),
                content_format: "markdown".to_string(),
                content_json: None,
                parent_comment_id: None,
            },
        )
        .await;

    match result {
        Ok(comment) => {
            assert_eq!(comment.post_id, post);
            assert_eq!(comment.content, "This comment should be persisted");
            assert_eq!(comment.locale, "en");
        }
        Err(err) => {
            let message = err.to_string();
            assert!(
                !message.contains("At least one translation is required"),
                "comment creation must not fail due to missing translation: {message}"
            );
            assert!(
                !message.contains("invalid_kind"),
                "comment creation must not fail due to kind validation: {message}"
            );
            panic!("comment creation failed unexpectedly: {message}");
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_comment_threaded_locale_fallback_update_delete_and_list() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let mut receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));

    let post_service = PostService::new(db.clone(), event_bus.clone());
    let comment_service = CommentService::new(db.clone(), event_bus.clone());

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let outsider = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    let post_id = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Post with comments".to_string(),
                body: "Body".to_string(),
                body_format: "markdown".to_string(),
                content_json: None,
                excerpt: None,
                slug: None,
                publish: false,
                tags: vec![],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
            },
        )
        .await?;

    let parent = comment_service
        .create_comment(
            tenant_id,
            admin.clone(),
            post_id,
            CreateCommentInput {
                locale: "en".to_string(),
                content: "Parent comment".to_string(),
                content_format: "markdown".to_string(),
                content_json: None,
                parent_comment_id: None,
            },
        )
        .await?;

    let child = comment_service
        .create_comment(
            tenant_id,
            admin.clone(),
            post_id,
            CreateCommentInput {
                locale: "fr".to_string(),
                content: "Réponse imbriquée".to_string(),
                content_format: "markdown".to_string(),
                content_json: None,
                parent_comment_id: Some(parent.id),
            },
        )
        .await?;
    assert_eq!(child.parent_comment_id, Some(parent.id));

    let fallback_en = comment_service
        .get_comment(tenant_id, child.id, "en")
        .await?;
    assert_eq!(fallback_en.content, "Réponse imbriquée");
    assert_eq!(fallback_en.effective_locale, "fr");

    let fallback_first = comment_service
        .get_comment(tenant_id, child.id, "de")
        .await?;
    assert_eq!(fallback_first.content, "Réponse imbriquée");
    assert_eq!(fallback_first.effective_locale, "fr");

    let updated = comment_service
        .update_comment(
            tenant_id,
            parent.id,
            admin.clone(),
            UpdateCommentInput {
                locale: "en".to_string(),
                content: Some("Parent updated".to_string()),
                content_format: None,
                content_json: None,
            },
        )
        .await?;
    assert_eq!(updated.content, "Parent updated");

    let forbidden = comment_service
        .update_comment(
            tenant_id,
            parent.id,
            outsider.clone(),
            UpdateCommentInput {
                locale: "en".to_string(),
                content: Some("Should fail".to_string()),
                content_format: None,
                content_json: None,
            },
        )
        .await
        .expect_err("customer should not update чужой комментарий");
    assert!(matches!(
        forbidden,
        BlogError::Comments(CommentsError::Forbidden(_))
    ));

    let not_found_update = comment_service
        .update_comment(
            tenant_id,
            Uuid::new_v4(),
            admin.clone(),
            UpdateCommentInput {
                locale: "en".to_string(),
                content: Some("missing".to_string()),
                content_format: None,
                content_json: None,
            },
        )
        .await
        .expect_err("must return not found");
    assert!(matches!(
        not_found_update,
        BlogError::Comments(CommentsError::CommentNotFound(_))
    ));

    comment_service
        .delete_comment(tenant_id, child.id, admin.clone())
        .await?;

    let not_found_delete = comment_service
        .delete_comment(tenant_id, Uuid::new_v4(), admin)
        .await
        .expect_err("must return not found on delete");
    assert!(matches!(
        not_found_delete,
        BlogError::Comments(CommentsError::CommentNotFound(_))
    ));

    let (page_one, total) = comment_service
        .list_for_post(
            tenant_id,
            SecurityContext::system(),
            post_id,
            ListCommentsFilter {
                locale: Some("en".to_string()),
                page: 1,
                per_page: 1,
            },
        )
        .await?;
    assert_eq!(total, 1);
    assert_eq!(page_one.len(), 1);

    let (page_two, _) = comment_service
        .list_for_post(
            tenant_id,
            SecurityContext::system(),
            post_id,
            ListCommentsFilter {
                locale: Some("en".to_string()),
                page: 2,
                per_page: 1,
            },
        )
        .await?;
    assert_eq!(page_two.len(), 0);
    let statuses: Vec<String> = page_one
        .iter()
        .chain(page_two.iter())
        .map(|c| c.status.clone())
        .collect();
    assert!(statuses.iter().any(|s| s == "pending"));
    assert!(!statuses.iter().any(|s| s == "trash"));

    let event_types = drain_event_types(&mut receiver);
    assert!(event_types.iter().any(|et| et == "blog.post.created"));
    assert!(event_types.iter().any(|et| et == "blog.post.updated"));

    Ok(())
}

#[allow(dead_code)]
async fn next_event(
    receiver: &mut broadcast::Receiver<EventEnvelope>,
) -> TestResult<EventEnvelope> {
    let envelope = tokio::time::timeout(std::time::Duration::from_secs(5), receiver.recv())
        .await
        .map_err(|_| "timed out waiting for event")??;
    Ok(envelope)
}

#[derive(Clone)]
struct FailingTransport {
    sender: broadcast::Sender<EventEnvelope>,
}

impl FailingTransport {
    fn new() -> Self {
        let (sender, _) = broadcast::channel(16);
        Self { sender }
    }

    fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.sender.subscribe()
    }
}

#[async_trait]
impl EventTransport for FailingTransport {
    async fn publish(&self, _envelope: EventEnvelope) -> rustok_core::Result<()> {
        Err(rustok_core::Error::External(
            "simulated transport failure".to_string(),
        ))
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::InMemory
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

mod unit_tests {
    use super::*;

    #[test]
    fn test_blog_post_state_machine() {
        let id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let author_id = Uuid::new_v4();

        let post = BlogPost::new_draft(
            id,
            tenant_id,
            author_id,
            "Test Post".to_string(),
            "test-post".to_string(),
            "en".to_string(),
        );
        assert_eq!(post.to_status(), BlogPostStatus::Draft);

        let post = post.publish();
        assert_eq!(post.to_status(), BlogPostStatus::Published);

        let post = post.archive("Outdated".to_string());
        assert_eq!(post.to_status(), BlogPostStatus::Archived);

        let post = post.restore_to_draft();
        assert_eq!(post.to_status(), BlogPostStatus::Draft);
    }

    #[test]
    fn test_comment_status_transitions() {
        assert_eq!(CommentStatus::Pending.approve(), CommentStatus::Approved);
        assert_eq!(CommentStatus::Approved.mark_spam(), CommentStatus::Spam);
        assert_eq!(CommentStatus::Spam.approve(), CommentStatus::Approved);
        assert_eq!(CommentStatus::Pending.trash(), CommentStatus::Trash);
    }

    #[test]
    fn test_error_conversions() {
        let id = Uuid::new_v4();
        let err = BlogError::post_not_found(id);
        assert!(matches!(err, BlogError::PostNotFound(_)));

        let err = BlogError::duplicate_slug("test-slug", "en");
        assert!(matches!(err, BlogError::DuplicateSlug { .. }));
    }

    #[test]
    fn test_post_list_query() {
        let query = PostListQuery {
            page: Some(2),
            per_page: Some(25),
            status: Some(BlogPostStatus::Published),
            tag: Some("rust".to_string()),
            ..Default::default()
        };

        assert_eq!(query.page(), 2);
        assert_eq!(query.per_page(), 25);
        assert_eq!(query.offset(), 25);
    }

    #[test]
    fn test_blog_events_exist() {
        let post_id = Uuid::new_v4();

        let created = DomainEvent::BlogPostCreated {
            post_id,
            author_id: None,
            locale: "en".to_string(),
        };
        assert_eq!(created.event_type(), "blog.post.created");
        assert_eq!(created.schema_version(), 1);
        assert!(created.affects_index());

        let published = DomainEvent::BlogPostPublished {
            post_id,
            author_id: None,
        };
        assert_eq!(published.event_type(), "blog.post.published");
        assert!(published.affects_index());

        let unpublished = DomainEvent::BlogPostUnpublished { post_id };
        assert_eq!(unpublished.event_type(), "blog.post.unpublished");
        assert!(unpublished.affects_index());

        let updated = DomainEvent::BlogPostUpdated {
            post_id,
            locale: "ru".to_string(),
        };
        assert_eq!(updated.event_type(), "blog.post.updated");
        assert!(updated.affects_index());

        let archived = DomainEvent::BlogPostArchived {
            post_id,
            reason: Some("outdated".to_string()),
        };
        assert_eq!(archived.event_type(), "blog.post.archived");
        assert!(archived.affects_index());

        let deleted = DomainEvent::BlogPostDeleted { post_id };
        assert_eq!(deleted.event_type(), "blog.post.deleted");
        assert!(deleted.affects_index());
    }

    #[test]
    fn test_create_post_input_fields() {
        let input = CreatePostInput {
            locale: "ru".to_string(),
            title: "Заголовок".to_string(),
            body: "Тело поста".to_string(),
            body_format: "markdown".to_string(),
            content_json: None,
            excerpt: Some("Краткое содержание".to_string()),
            slug: Some("zagolovok".to_string()),
            publish: false,
            tags: vec!["rust".to_string(), "cms".to_string()],
            category_id: None,
            featured_image_url: Some("https://example.com/image.jpg".to_string()),
            seo_title: Some("SEO заголовок".to_string()),
            seo_description: Some("SEO описание для поисковиков".to_string()),
            channel_slugs: None,
            metadata: None,
        };

        assert_eq!(input.locale, "ru");
        assert_eq!(input.tags.len(), 2);
        assert!(input.featured_image_url.is_some());
        assert!(input.seo_title.is_some());
        assert!(input.seo_description.is_some());
        assert!(!input.publish);
    }

    #[tokio::test]
    async fn test_publish_returns_error_and_does_not_deliver_event_with_failing_transport() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let failing_transport = FailingTransport::new();
        let mut receiver = failing_transport.subscribe();
        let event_bus = TransactionalEventBus::new(Arc::new(failing_transport));

        let result = event_bus
            .publish(
                tenant_id,
                Some(actor_id),
                DomainEvent::BlogPostCreated {
                    post_id: Uuid::new_v4(),
                    author_id: Some(actor_id),
                    locale: "en".to_string(),
                },
            )
            .await;

        assert!(result.is_err());
        assert!(matches!(
            receiver.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), receiver.recv())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_publish_delivers_blog_event_with_memory_transport() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let transport = MemoryTransport::new();
        let mut receiver = transport.subscribe();
        let event_bus = TransactionalEventBus::new(Arc::new(transport));

        event_bus
            .publish(
                tenant_id,
                Some(actor_id),
                DomainEvent::BlogPostCreated {
                    post_id: Uuid::new_v4(),
                    author_id: Some(actor_id),
                    locale: "en".to_string(),
                },
            )
            .await
            .expect("memory transport should accept publish");

        let envelope = receiver.recv().await.expect("event should be published");
        assert_eq!(envelope.tenant_id, tenant_id);
        assert_eq!(envelope.actor_id, Some(actor_id));
        assert!(matches!(
            envelope.event,
            DomainEvent::BlogPostCreated {
                author_id: Some(id),
                locale,
                ..
            } if id == actor_id && locale == "en"
        ));
    }

    #[test]
    fn test_blog_security_context_uses_explicit_actor() {
        let actor_id = Uuid::new_v4();
        let security = SecurityContext::new(UserRole::Admin, Some(actor_id));

        assert_eq!(security.user_id, Some(actor_id));
        assert_eq!(security.role, UserRole::Admin);
    }
}
