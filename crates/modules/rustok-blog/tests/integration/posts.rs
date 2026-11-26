use std::sync::Arc;

use rustok_api::Patch;
use rustok_blog::dto::{CreatePostInput, PostListQuery, UpdatePostInput};
use rustok_blog::state_machine::BlogPostStatus;
use rustok_blog::{BlogError, PostService};
use rustok_core::{MemoryTransport, SecurityContext, UserRole};
use rustok_outbox::TransactionalEventBus;
use uuid::Uuid;

use super::fixtures::{
    TestResult, drain_event_types, ensure_blog_schema, richtext, setup_blog_test_db,
};

#[tokio::test]
async fn test_post_lifecycle() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let mut receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus.clone()).with_taxonomy_port(
        rustok_content_orchestration::in_process_blog_taxonomy_port(db.clone()),
    );

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let post_id = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                command_id: uuid::Uuid::new_v4(),
                locale: "en".to_string(),
                title: "Test Post".to_string(),
                content: rustok_blog::richtext::article_document_from_plain_text("Hello, Blog!"),
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
                scheduled_at: None,
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
    let post_service = PostService::new(db.clone(), event_bus.clone()).with_taxonomy_port(
        rustok_content_orchestration::in_process_blog_taxonomy_port(db.clone()),
    );

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let post_id = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                command_id: uuid::Uuid::new_v4(),
                locale: "en".to_string(),
                title: "Draft Post".to_string(),
                content: rustok_blog::richtext::article_document_from_plain_text("Content"),
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
                scheduled_at: None,
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
    let post_service = PostService::new(db.clone(), event_bus.clone()).with_taxonomy_port(
        rustok_content_orchestration::in_process_blog_taxonomy_port(db.clone()),
    );

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let post_id = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                command_id: uuid::Uuid::new_v4(),
                locale: "en-us".to_string(),
                title: "Localized Post".to_string(),
                content: rustok_blog::richtext::article_document_from_plain_text("Body"),
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
                scheduled_at: None,
            },
        )
        .await?;

    let direct = post_service
        .get_post(tenant_id, admin.clone(), post_id, "EN_us")
        .await?;
    assert_eq!(direct.requested_locale, "en-US");
    assert_eq!(direct.locale, "en-US");
    assert_eq!(direct.effective_locale, "en-US");

    let by_fallback = post_service
        .get_post_with_locale_fallback(tenant_id, admin.clone(), post_id, "FR_fr", Some("EN_us"))
        .await?;
    assert_eq!(by_fallback.requested_locale, "fr-FR");
    assert_eq!(by_fallback.locale, "fr-FR");
    assert_eq!(by_fallback.effective_locale, "en-US");
    assert_eq!(by_fallback.available_locales, ["en-US"]);

    let listed = post_service
        .list_posts(
            tenant_id,
            SecurityContext::system(),
            PostListQuery {
                locale: Some("EN_us".to_string()),
                per_page: Some(10),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.items[0].locale, "en-US");
    assert_eq!(listed.items[0].effective_locale, "en-US");

    let localized_update = |title, content| UpdatePostInput {
        locale: Some(by_fallback.requested_locale.clone()),
        title,
        content,
        excerpt: Patch::Keep,
        slug: None,
        tags: None,
        category_id: Patch::Keep,
        featured_image_url: Patch::Keep,
        seo_title: Patch::Keep,
        seo_description: Patch::Keep,
        channel_slugs: None,
        metadata: None,
        version: by_fallback.version,
        scheduled_at: Patch::Keep,
    };
    for (title, content, reason) in [
        (Some("Article français".to_string()), None, "Content"),
        (None, Some(richtext("Texte français")), "Title"),
    ] {
        let error = post_service
            .update_post(
                tenant_id,
                post_id,
                admin.clone(),
                localized_update(title, content),
            )
            .await
            .expect_err("a fallback read must not supply required localized fields");
        assert!(matches!(error, BlogError::Validation(message)
            if message == format!("{reason} is required for a new locale")));
        let unchanged = post_service
            .get_post_with_locale_fallback(
                tenant_id,
                admin.clone(),
                post_id,
                "fr-FR",
                Some("en-US"),
            )
            .await?;
        assert_eq!(unchanged.version, by_fallback.version);
        assert_eq!(unchanged.effective_locale, "en-US");
        assert_eq!(unchanged.available_locales, ["en-US"]);
        assert_eq!(unchanged.title, direct.title);
        assert_eq!(unchanged.content_plain_text, direct.content_plain_text);
    }

    post_service
        .update_post(
            tenant_id,
            post_id,
            admin.clone(),
            localized_update(
                Some("Article français".to_string()),
                Some(richtext("Texte français")),
            ),
        )
        .await?;
    let french = post_service
        .get_post_with_locale_fallback(tenant_id, admin.clone(), post_id, "fr-FR", Some("en-US"))
        .await?;
    assert_eq!(french.requested_locale, "fr-FR");
    assert_eq!(french.effective_locale, "fr-FR");
    assert_eq!(french.available_locales, ["en-US", "fr-FR"]);
    assert_eq!(french.version, by_fallback.version + 1);
    assert_eq!(french.title, "Article français");
    assert_eq!(french.content_plain_text, "Texte français");
    let english = post_service
        .get_post(tenant_id, admin, post_id, "en-US")
        .await?;
    assert_eq!(english.title, direct.title);
    assert_eq!(english.content_plain_text, direct.content_plain_text);

    Ok(())
}

#[tokio::test]
async fn test_list_posts_with_pagination() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let _receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus.clone()).with_taxonomy_port(
        rustok_content_orchestration::in_process_blog_taxonomy_port(db.clone()),
    );

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    for i in 0..5 {
        post_service
            .create_post(
                tenant_id,
                admin.clone(),
                CreatePostInput {
                    command_id: uuid::Uuid::new_v4(),
                    locale: "en".to_string(),
                    title: format!("Post {i}"),
                    content: rustok_blog::richtext::article_document_from_plain_text("Body"),
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
                    scheduled_at: None,
                },
            )
            .await?;
    }

    // Default sort is created_at DESC. Walk the cursor: 2 + 2 + 1 items, then no cursor.
    let page1 = post_service
        .list_posts(
            tenant_id,
            admin.clone(),
            PostListQuery {
                per_page: Some(2),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(page1.items.len(), 2);
    let cursor1 = page1
        .next_cursor
        .clone()
        .expect("first page should have a cursor");

    let page2 = post_service
        .list_posts(
            tenant_id,
            admin.clone(),
            PostListQuery {
                after: Some(cursor1),
                per_page: Some(2),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(page2.items.len(), 2);
    let cursor2 = page2
        .next_cursor
        .clone()
        .expect("second page should have a cursor");

    let page3 = post_service
        .list_posts(
            tenant_id,
            admin.clone(),
            PostListQuery {
                after: Some(cursor2),
                per_page: Some(2),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(page3.items.len(), 1);
    assert!(page3.next_cursor.is_none());

    let seen: std::collections::HashSet<_> = page1
        .items
        .iter()
        .chain(page2.items.iter())
        .chain(page3.items.iter())
        .map(|item| item.id)
        .collect();
    assert_eq!(seen.len(), 5, "keyset pages must not repeat or skip posts");

    Ok(())
}

#[tokio::test]
async fn test_filter_posts_by_tag() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let _receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus.clone()).with_taxonomy_port(
        rustok_content_orchestration::in_process_blog_taxonomy_port(db.clone()),
    );

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                command_id: uuid::Uuid::new_v4(),
                locale: "en".to_string(),
                title: "Rust Post".to_string(),
                content: rustok_blog::richtext::article_document_from_plain_text("Body"),
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
                scheduled_at: None,
            },
        )
        .await?;

    post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                command_id: uuid::Uuid::new_v4(),
                locale: "en".to_string(),
                title: "Go Post".to_string(),
                content: rustok_blog::richtext::article_document_from_plain_text("Body"),
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
                scheduled_at: None,
            },
        )
        .await?;

    let rust_posts = post_service
        .get_posts_by_tag(tenant_id, admin.clone(), "rust".to_string(), None, 10)
        .await?;

    assert_eq!(rust_posts.items.len(), 1);
    assert!(rust_posts.next_cursor.is_none());
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
    let post_service = PostService::new(db.clone(), event_bus.clone()).with_taxonomy_port(
        rustok_content_orchestration::in_process_blog_taxonomy_port(db.clone()),
    );

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let post_id = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                command_id: uuid::Uuid::new_v4(),
                locale: "en".to_string(),
                title: "Published Post".to_string(),
                content: rustok_blog::richtext::article_document_from_plain_text("Content"),
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
                scheduled_at: None,
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
