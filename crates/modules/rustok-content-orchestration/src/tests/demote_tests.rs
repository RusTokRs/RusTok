use std::sync::Arc;

use rustok_blog::{
    CommentService as BlogCommentService, CreateCommentInput as BlogCreateCommentInput,
    CreatePostInput, PostService,
};
use rustok_comments::{CommentsService, ListCommentsFilter};
use rustok_content::{ContentError, ContentOrchestrationService, DemotePostToTopicInput};
use rustok_forum::{CategoryService, CreateCategoryInput};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use uuid::Uuid;

use crate::ServerContentOrchestrationBridge;
use crate::tests::helpers::{
    admin_security, blog_settings_reader, ensure_conversion_schema, insert_test_actor, richtext,
    setup_conversion_test_db,
};

#[tokio::test]
async fn demotion_rejects_posts_with_comments_and_preserves_the_source() {
    let db = setup_conversion_test_db().await;
    ensure_conversion_schema(&db).await;

    let events = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let security = admin_security();
    let tenant_id = Uuid::new_v4();
    insert_test_actor(&db, tenant_id, &security).await;

    let forum_category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            security.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Imported".to_string(),
                slug: "imported".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("forum destination category should be created");

    let post_id = PostService::new(db.clone(), events.clone())
        .create_post(
            tenant_id,
            security.clone(),
            CreatePostInput {
                command_id: Uuid::new_v4(),
                locale: "en".to_string(),
                title: "Legacy post".to_string(),
                content: richtext("Original blog body"),
                excerpt: None,
                slug: Some("legacy-post".to_string()),
                publish: true,
                tags: Vec::new(),
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
        .expect("blog post should be created");

    let blog_comment_service = BlogCommentService::from_runtime_capabilities(
        db.clone(),
        Some(rustok_comments::in_process_comments_thread_port(
            db.clone(),
            events.clone(),
        )),
        Some(blog_settings_reader("open")),
    );
    blog_comment_service
        .create_public_comment(
            tenant_id,
            security.clone(),
            post_id,
            None,
            BlogCreateCommentInput {
                command_id: Uuid::new_v4(),
                locale: "en".to_string(),
                content: richtext("First blog comment"),
                parent_comment_id: None,
            },
        )
        .await
        .expect("first blog comment should be created");
    blog_comment_service
        .create_public_comment(
            tenant_id,
            security.clone(),
            post_id,
            None,
            BlogCreateCommentInput {
                command_id: Uuid::new_v4(),
                locale: "en".to_string(),
                content: richtext("Second blog comment"),
                parent_comment_id: None,
            },
        )
        .await
        .expect("second blog comment should be created");

    let orchestration = ContentOrchestrationService::new(
        db.clone(),
        events.clone(),
        Arc::new(ServerContentOrchestrationBridge::new(
            db.clone(),
            events.clone(),
        )),
    );
    let error = orchestration
        .demote_post_to_topic(
            tenant_id,
            security.clone(),
            DemotePostToTopicInput {
                post_id,
                locale: "en".into(),
                forum_category_id: forum_category.id,
                reason: None,
                idempotency_key: "post-with-comments".into(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        ContentError::Validation(message)
            if message.contains("until Forum uses the canonical richtext contract")
    ));
    PostService::new(db.clone(), events)
        .get_post(tenant_id, security.clone(), post_id, "en")
        .await
        .unwrap();
    let (_, count) = CommentsService::new(db)
        .list_comments_for_target(
            tenant_id,
            security,
            "blog_post",
            post_id,
            ListCommentsFilter {
                locale: "en".into(),
                page: 1,
                per_page: 20,
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(count, 2);
}
