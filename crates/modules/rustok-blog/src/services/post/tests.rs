use super::*;
use std::sync::Arc;

use rustok_core::MigrationSource;
use rustok_core::{SecurityContext, UserRole};
use rustok_outbox::{OutboxTransport, SysEventsMigration};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::{MigrationTrait, SchemaManager};

async fn setup_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:blog_service_post_{}?mode=memory&cache=shared",
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
    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migration should apply");
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in crate::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("blog migration should apply");
    }
}

#[test]
fn post_list_query_defaults() {
    let query = PostListQuery::default();
    assert_eq!(query.page(), 1);
    assert_eq!(query.per_page(), 20);
    assert_eq!(query.offset(), 0);
}

#[test]
fn post_list_query_clamps_bounds() {
    let query = PostListQuery {
        page: Some(0),
        per_page: Some(200),
        ..Default::default()
    };
    assert_eq!(query.page(), 1);
    assert_eq!(query.per_page(), 100);
}

#[test]
fn channel_visibility_normalizes_and_filters_blog_channel_lists() {
    let channel_slugs =
        normalize_channel_slugs(&[" Web ".to_string(), "mobile".to_string(), "web".to_string()]);

    assert_eq!(channel_slugs, vec!["mobile".to_string(), "web".to_string()]);
    assert!(is_post_visible_for_channel(&channel_slugs, Some("web")));
    assert!(!is_post_visible_for_channel(
        &channel_slugs,
        Some("storefront")
    ));
    assert!(!is_post_visible_for_channel(&channel_slugs, None));
}

#[test]
fn slug_normalization_is_stable() {
    assert_eq!(normalize_slug("Hello, World!"), "hello-world");
    assert_eq!(normalize_slug("  many   spaces  "), "many-spaces");
}

#[tokio::test]
async fn post_lifecycle_uses_blog_owned_tables() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = OutboxTransport::new(db.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus);

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let post_id = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Draft Post".to_string(),
                content: crate::richtext::article_document_from_plain_text("Content"),
                excerpt: None,
                slug: Some("draft-post".to_string()),
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
        .await
        .expect("post should be created");

    let draft = post_service
        .get_post(tenant_id, admin.clone(), post_id, "en")
        .await
        .expect("draft should be readable");
    assert_eq!(draft.status, BlogPostStatus::Draft);
    assert_eq!(draft.tags, vec!["rust"]);

    post_service
        .publish_post(tenant_id, post_id, admin.clone())
        .await
        .expect("post should publish");

    let published = post_service
        .get_post(tenant_id, admin.clone(), post_id, "en")
        .await
        .expect("published should be readable");
    assert_eq!(published.status, BlogPostStatus::Published);
    assert_eq!(published.slug, "draft-post");
    assert!(published.published_at.is_some());
}

#[tokio::test]
async fn customer_cannot_create_or_read_draft_posts() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = OutboxTransport::new(db.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus);

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    let denied_create = post_service
        .create_post(
            tenant_id,
            customer.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Customer draft".to_string(),
                content: crate::richtext::article_document_from_plain_text("Body"),
                excerpt: None,
                slug: Some("customer-draft".to_string()),
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
        .expect_err("customer should not create posts");
    assert!(matches!(denied_create, BlogError::Forbidden(_)));

    let post_id = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Admin draft".to_string(),
                content: crate::richtext::article_document_from_plain_text("Body"),
                excerpt: None,
                slug: Some("admin-draft".to_string()),
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
        .expect("admin draft should be created");

    let denied_read = post_service
        .get_post(tenant_id, customer.clone(), post_id, "en")
        .await
        .expect_err("customer should not read drafts");
    assert!(matches!(denied_read, BlogError::Forbidden(_)));

    let listed = post_service
        .list_posts(
            tenant_id,
            customer,
            PostListQuery {
                page: Some(1),
                per_page: Some(10),
                ..Default::default()
            },
        )
        .await
        .expect("customer listing should succeed");
    assert!(listed.items.is_empty());
    assert_eq!(listed.total, 0);
}

#[tokio::test]
async fn create_and_update_post_store_channel_visibility_in_typed_relation() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = OutboxTransport::new(db.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus);

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let post_id = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Visible post".to_string(),
                content: crate::richtext::article_document_from_plain_text("Body"),
                excerpt: None,
                slug: Some("visible-post".to_string()),
                publish: true,
                tags: vec![],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: Some(vec![" Web ".to_string(), "mobile".to_string()]),
                metadata: Some(serde_json::json!({"custom": "value"})),
            },
        )
        .await
        .expect("post should be created");

    let created = post_service
        .get_post(tenant_id, admin.clone(), post_id, "en")
        .await
        .expect("post should load");
    assert_eq!(
        created.channel_slugs,
        vec!["mobile".to_string(), "web".to_string()]
    );
    assert_eq!(created.metadata["custom"], "value");

    post_service
        .update_post(
            tenant_id,
            post_id,
            admin.clone(),
            UpdatePostInput {
                locale: Some("en".to_string()),
                title: None,
                content: None,
                excerpt: Patch::Keep,
                slug: None,
                tags: None,
                category_id: Patch::Keep,
                featured_image_url: Patch::Keep,
                seo_title: Patch::Keep,
                seo_description: Patch::Keep,
                channel_slugs: Some(vec!["storefront".to_string()]),
                metadata: Some(serde_json::json!({"custom": "next"})),
                version: created.version,
            },
        )
        .await
        .expect("post should update");

    let updated = post_service
        .get_post(tenant_id, admin, post_id, "en")
        .await
        .expect("updated post should load");
    assert_eq!(updated.channel_slugs, vec!["storefront".to_string()]);
    assert_eq!(updated.metadata["custom"], "next");
}

#[tokio::test]
async fn public_visible_listing_filters_by_typed_channel_relation() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = OutboxTransport::new(db.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus);

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    for (slug, title, channel_slugs) in [
        ("web-visible", "Web Visible", Some(vec!["web".to_string()])),
        (
            "mobile-only",
            "Mobile Only",
            Some(vec!["mobile".to_string()]),
        ),
        ("global", "Global", None),
    ] {
        post_service
            .create_post(
                tenant_id,
                admin.clone(),
                CreatePostInput {
                    locale: "en".to_string(),
                    title: title.to_string(),
                    content: crate::richtext::article_document_from_plain_text("Body"),
                    excerpt: None,
                    slug: Some(slug.to_string()),
                    publish: true,
                    tags: vec![],
                    category_id: None,
                    featured_image_url: None,
                    seo_title: None,
                    seo_description: None,
                    channel_slugs,
                    metadata: None,
                },
            )
            .await
            .expect("post should be created");
    }

    let visible = post_service
        .list_public_visible_with_locale_fallback(
            tenant_id,
            PostListQuery {
                status: Some(BlogPostStatus::Published),
                locale: Some("en".to_string()),
                page: Some(1),
                per_page: Some(10),
                sort_by: Some(PostSortField::PublishedAt),
                sort_order: Some(PostSortOrder::Desc),
                ..Default::default()
            },
            Some("en"),
            Some("web"),
        )
        .await
        .expect("public visible list should succeed");

    assert_eq!(visible.total, 2);
    let slugs = visible
        .items
        .into_iter()
        .map(|item| item.slug)
        .collect::<Vec<_>>();
    assert!(slugs.contains(&"web-visible".to_string()));
    assert!(slugs.contains(&"global".to_string()));
    assert!(!slugs.contains(&"mobile-only".to_string()));
}
