use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{
    Permission, PortError, SharedStaticModuleSettingsReader, StaticModuleSettingsReader,
    StaticModuleSettingsSnapshot,
};
use rustok_blog::dto::{
    CreateCommentInput, CreatePostInput, ListCommentsFilter, ModerateCommentInput,
    ModerateCommentStatus, UpdateCommentInput,
};
use rustok_blog::{BlogError, CommentService, PostService};
use rustok_comments::in_process_comments_thread_port;
use rustok_core::{MemoryTransport, SecurityContext, UserRole};
use rustok_outbox::TransactionalEventBus;
use uuid::Uuid;

use super::fixtures::{
    TestResult, drain_event_types, ensure_blog_schema, richtext, seed_channel, seed_tenant,
    setup_blog_test_db,
};

struct BlogSettingsReader {
    comments_mode: &'static str,
}

#[async_trait]
impl StaticModuleSettingsReader for BlogSettingsReader {
    async fn settings(
        &self,
        _tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        Ok(
            (module_slug == "blog").then(|| StaticModuleSettingsSnapshot {
                enabled: Some(true),
                settings: serde_json::json!({ "comments_mode": self.comments_mode }),
            }),
        )
    }
}

fn blog_settings_reader(comments_mode: &'static str) -> SharedStaticModuleSettingsReader {
    SharedStaticModuleSettingsReader(Arc::new(BlogSettingsReader { comments_mode }))
}

#[tokio::test]
async fn test_blog_comment_surface_policy_preserves_comment_data() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let _receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus.clone()).with_taxonomy_port(
        rustok_content_orchestration::in_process_blog_taxonomy_port(db.clone()),
    );
    let open_comments = CommentService::from_runtime_capabilities(
        db.clone(),
        Some(in_process_comments_thread_port(
            db.clone(),
            event_bus.clone(),
        )),
        Some(blog_settings_reader("open")),
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
                title: "Comment policy".to_string(),
                content: richtext("Post body"),
                excerpt: None,
                slug: Some("comment-policy".to_string()),
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
    let comment = open_comments
        .create_public_comment(
            tenant_id,
            admin.clone(),
            post_id,
            None,
            CreateCommentInput {
                command_id: Uuid::new_v4(),
                locale: "en".to_string(),
                content: richtext("Retained comment"),
                parent_comment_id: None,
            },
        )
        .await?;
    open_comments
        .moderate_comment(
            tenant_id,
            comment.id,
            admin.clone(),
            ModerateCommentInput {
                command_id: Uuid::new_v4(),
                status: ModerateCommentStatus::Approved,
                locale: Some("en".to_string()),
            },
            Some("en"),
        )
        .await?;

    let read_only = CommentService::from_runtime_capabilities(
        db.clone(),
        Some(in_process_comments_thread_port(
            db.clone(),
            event_bus.clone(),
        )),
        Some(blog_settings_reader("read_only")),
    );
    let read_only_public = rustok_blog::list_public_comments_with_snapshot(
        &read_only, None, tenant_id, post_id, "en", None, None, 1, 20,
    )
    .await?;
    assert_eq!(
        read_only_public.availability,
        rustok_blog::PublicCommentsAvailability::ReadOnly
    );
    assert_eq!(read_only_public.total, 1);
    assert!(matches!(
        read_only
            .create_public_comment(
                tenant_id,
                admin.clone(),
                post_id,
                None,
                CreateCommentInput {
                    command_id: Uuid::new_v4(),
                    locale: "en".to_string(),
                    content: richtext("Blocked reply"),
                    parent_comment_id: None,
                },
            )
            .await,
        Err(BlogError::CommentsReadOnly)
    ));

    // No Comments provider is supplied here: the disabled Blog policy must
    // hide the surface before any provider call, while data remains owned by
    // and retained in Comments.
    let disabled = CommentService::from_runtime_capabilities(
        db.clone(),
        None,
        Some(blog_settings_reader("disabled")),
    );
    let disabled_public = rustok_blog::list_public_comments_with_snapshot(
        &disabled, None, tenant_id, post_id, "en", None, None, 1, 20,
    )
    .await?;
    assert_eq!(
        disabled_public.availability,
        rustok_blog::PublicCommentsAvailability::Disabled
    );
    assert_eq!(disabled_public.total, 0);
    assert!(matches!(
        disabled
            .create_public_comment(
                tenant_id,
                admin.clone(),
                post_id,
                None,
                CreateCommentInput {
                    command_id: Uuid::new_v4(),
                    locale: "en".to_string(),
                    content: richtext("Blocked comment"),
                    parent_comment_id: None,
                },
            )
            .await,
        Err(BlogError::CommentsDisabled)
    ));

    let (retained, total) = open_comments
        .list_for_post(
            tenant_id,
            SecurityContext::system(),
            post_id,
            ListCommentsFilter {
                locale: Some("en".to_string()),
                page: 1,
                per_page: 20,
            },
        )
        .await?;
    assert_eq!(total, 1);
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].id, comment.id);

    Ok(())
}

#[tokio::test]
async fn test_create_comment_succeeds_with_required_translation() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let _receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));

    let post_service = PostService::new(db.clone(), event_bus.clone());
    let comment_service = CommentService::from_runtime_capabilities(
        db.clone(),
        Some(in_process_comments_thread_port(db.clone(), event_bus)),
        Some(blog_settings_reader("open")),
    );

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let security = SecurityContext::new(UserRole::Admin, Some(actor_id));

    let post = post_service
        .create_post(
            tenant_id,
            security.clone(),
            CreatePostInput {
                command_id: uuid::Uuid::new_v4(),
                locale: "en".to_string(),
                title: "Post for comments".to_string(),
                content: rustok_blog::richtext::article_document_from_plain_text("Post body"),
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
        .await
        .expect("post should be created for comment test");

    let result = comment_service
        .create_public_comment(
            tenant_id,
            security,
            post,
            None,
            CreateCommentInput {
                command_id: Uuid::new_v4(),
                locale: "en".to_string(),
                content: richtext("This comment should be persisted"),
                parent_comment_id: None,
            },
        )
        .await;

    match result {
        Ok(comment) => {
            assert_eq!(comment.post_id, post);
            assert_eq!(comment.content_text, "This comment should be persisted");
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
async fn test_public_comment_create_rejects_draft_and_hidden_channel() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let _receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus.clone()).with_taxonomy_port(
        rustok_content_orchestration::in_process_blog_taxonomy_port(db.clone()),
    );
    let comment_service = CommentService::from_runtime_capabilities(
        db.clone(),
        Some(in_process_comments_thread_port(db.clone(), event_bus)),
        Some(blog_settings_reader("open")),
    )
    .with_channel_availability(rustok_blog::BlogChannelAvailability::from_port(
        rustok_channel::in_process_public_module_read_port(db.clone()),
    ));
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;
    seed_channel(&db, tenant_id, "web").await;
    seed_channel(&db, tenant_id, "mobile").await;
    let author = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let post_id = post_service
        .create_post(
            tenant_id,
            author.clone(),
            CreatePostInput {
                command_id: uuid::Uuid::new_v4(),
                locale: "en".to_string(),
                title: "Public comment target".to_string(),
                content: richtext("Post body"),
                excerpt: None,
                slug: Some("public-comment-target".to_string()),
                publish: false,
                tags: vec![],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: Some(vec!["web".to_string()]),
                metadata: None,
                scheduled_at: None,
            },
        )
        .await?;
    let input = || CreateCommentInput {
        command_id: Uuid::new_v4(),
        locale: "en".to_string(),
        content: richtext("Visible comment"),
        parent_comment_id: None,
    };

    let draft_error = comment_service
        .create_public_comment(tenant_id, author.clone(), post_id, Some("web"), input())
        .await
        .expect_err("draft posts must not accept public comments");
    assert!(matches!(draft_error, BlogError::PostNotFound(id) if id == post_id));

    post_service
        .publish_post(tenant_id, post_id, author.clone())
        .await?;
    let channel_error = comment_service
        .create_public_comment(tenant_id, author.clone(), post_id, Some("mobile"), input())
        .await
        .expect_err("a hidden channel must not accept public comments");
    assert!(matches!(channel_error, BlogError::PostNotFound(id) if id == post_id));

    let comment = comment_service
        .create_public_comment(tenant_id, author, post_id, Some("web"), input())
        .await?;
    assert_eq!(comment.post_id, post_id);
    assert_eq!(comment.status, "pending");

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
    let comment_service = CommentService::from_runtime_capabilities(
        db.clone(),
        Some(in_process_comments_thread_port(
            db.clone(),
            event_bus.clone(),
        )),
        Some(blog_settings_reader("open")),
    );

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let outsider = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    let post_id = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                command_id: uuid::Uuid::new_v4(),
                locale: "en".to_string(),
                title: "Post with comments".to_string(),
                content: rustok_blog::richtext::article_document_from_plain_text("Body"),
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

    let parent = comment_service
        .create_public_comment(
            tenant_id,
            admin.clone(),
            post_id,
            None,
            CreateCommentInput {
                command_id: Uuid::new_v4(),
                locale: "en".to_string(),
                content: richtext("Parent comment"),
                parent_comment_id: None,
            },
        )
        .await?;

    let child = comment_service
        .create_public_comment(
            tenant_id,
            admin.clone(),
            post_id,
            None,
            CreateCommentInput {
                command_id: Uuid::new_v4(),
                locale: "fr".to_string(),
                content: richtext("Réponse imbriquée"),
                parent_comment_id: Some(parent.id),
            },
        )
        .await?;
    assert_eq!(child.parent_comment_id, Some(parent.id));

    let fallback_en = comment_service
        .get_comment(tenant_id, admin.clone(), child.id, "en")
        .await?;
    assert_eq!(fallback_en.content_text, "Réponse imbriquée");
    assert_eq!(fallback_en.effective_locale, "fr");

    let fallback_first = comment_service
        .get_comment(tenant_id, admin.clone(), child.id, "de")
        .await?;
    assert_eq!(fallback_first.content_text, "Réponse imbriquée");
    assert_eq!(fallback_first.effective_locale, "fr");

    let updated = comment_service
        .update_comment(
            tenant_id,
            parent.id,
            admin.clone(),
            UpdateCommentInput {
                command_id: Uuid::new_v4(),
                locale: "en".to_string(),
                content: Some(richtext("Parent updated")),
            },
        )
        .await?;
    assert_eq!(updated.content_text, "Parent updated");

    let forbidden = comment_service
        .update_comment(
            tenant_id,
            parent.id,
            outsider.clone(),
            UpdateCommentInput {
                command_id: Uuid::new_v4(),
                locale: "en".to_string(),
                content: Some(richtext("Should fail")),
            },
        )
        .await
        .expect_err("customer should not update чужой комментарий");
    assert!(matches!(
        forbidden,
        BlogError::Forbidden(_) | BlogError::Rich(_)
    ));

    let not_found_update = comment_service
        .update_comment(
            tenant_id,
            Uuid::new_v4(),
            admin.clone(),
            UpdateCommentInput {
                command_id: Uuid::new_v4(),
                locale: "en".to_string(),
                content: Some(richtext("missing")),
            },
        )
        .await
        .expect_err("must return not found");
    assert!(matches!(
        not_found_update,
        BlogError::CommentNotFound(_) | BlogError::Rich(_)
    ));

    comment_service
        .delete_comment(tenant_id, child.id, admin.clone(), Uuid::new_v4())
        .await?;

    let not_found_delete = comment_service
        .delete_comment(tenant_id, Uuid::new_v4(), admin, Uuid::new_v4())
        .await
        .expect_err("must return not found on delete");
    assert!(matches!(
        not_found_delete,
        BlogError::CommentNotFound(_) | BlogError::Rich(_)
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
    assert!(event_types.iter().any(|et| et == "comment.created"));
    assert!(event_types.iter().any(|et| et == "comment.deleted"));

    Ok(())
}

#[tokio::test]
async fn test_moderate_comment_requires_comments_moderate_permission() -> TestResult<()> {
    let db = setup_blog_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = MemoryTransport::new();
    let _receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));

    let post_service = PostService::new(db.clone(), event_bus.clone());
    let comment_service = CommentService::from_runtime_capabilities(
        db.clone(),
        Some(in_process_comments_thread_port(db.clone(), event_bus)),
        Some(blog_settings_reader("open")),
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
                title: "Post for moderation".to_string(),
                content: rustok_blog::richtext::article_document_from_plain_text("Body"),
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

    let comment = comment_service
        .create_public_comment(
            tenant_id,
            admin,
            post_id,
            None,
            CreateCommentInput {
                command_id: Uuid::new_v4(),
                locale: "en".to_string(),
                content: richtext("Needs moderation"),
                parent_comment_id: None,
            },
        )
        .await?;

    // Blog reads and moderates comments through the Comments owner, so the
    // authority to moderate is the Comments grant — not `blog_posts:manage`.
    let blog_manager = SecurityContext::from_permissions(
        UserRole::Manager,
        Some(Uuid::new_v4()),
        [Permission::BLOG_POSTS_MANAGE],
    );
    let forbidden = comment_service
        .moderate_comment(
            tenant_id,
            comment.id,
            blog_manager,
            ModerateCommentInput {
                command_id: Uuid::new_v4(),
                status: ModerateCommentStatus::Approved,
                locale: Some("en".to_string()),
            },
            Some("en"),
        )
        .await
        .expect_err("blog_posts:manage is not a comment-moderation grant");
    assert!(matches!(forbidden, BlogError::Forbidden(_)));

    let comment_moderator = SecurityContext::from_permissions(
        UserRole::Manager,
        Some(Uuid::new_v4()),
        [Permission::COMMENTS_MODERATE],
    );
    let moderated = comment_service
        .moderate_comment(
            tenant_id,
            comment.id,
            comment_moderator,
            ModerateCommentInput {
                command_id: Uuid::new_v4(),
                status: ModerateCommentStatus::Approved,
                locale: Some("en".to_string()),
            },
            Some("en"),
        )
        .await?;
    assert_eq!(moderated.status, "approved");

    let without_grant = SecurityContext::from_permissions(
        UserRole::Manager,
        Some(Uuid::new_v4()),
        std::iter::empty::<Permission>(),
    );
    let forbidden = comment_service
        .moderate_comment(
            tenant_id,
            comment.id,
            without_grant,
            ModerateCommentInput {
                command_id: Uuid::new_v4(),
                status: ModerateCommentStatus::Spam,
                locale: Some("en".to_string()),
            },
            Some("en"),
        )
        .await
        .expect_err("moderation requires comments:moderate");
    assert!(matches!(forbidden, BlogError::Forbidden(_)));

    Ok(())
}
