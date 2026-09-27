use std::sync::Arc;

use rustok_blog::{
    CommentService as BlogCommentService, CreateCommentInput as BlogCreateCommentInput,
    CreatePostInput, PostService, blog_post,
};
use rustok_comments::{CommentsService, ListCommentsFilter, comment};
use rustok_content::{CanonicalUrlService, ContentOrchestrationService, DemotePostToTopicInput};
use rustok_core::SecurityContext;
use rustok_forum::{
    CategoryService, CreateCategoryInput, ListRepliesFilter, ReplyService, ReplyStatus,
    forum_reply, forum_reply_body, forum_topic, forum_topic_translation,
};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::ServerContentOrchestrationBridge;
use crate::tests::helpers::{
    admin_security, blog_settings_reader, ensure_conversion_schema, insert_test_actor, richtext,
    setup_conversion_test_db,
};

#[tokio::test]
async fn demote_post_to_topic_moves_comments_and_registers_redirects() {
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
                locale: "en".to_string(),
                title: "Legacy post".to_string(),
                content: richtext("Original blog body"),
                excerpt: None,
                slug: Some("legacy-post".to_string()),
                publish: true,
                tags: vec!["alpha".to_string(), "beta".to_string()],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
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
        Arc::new(ServerContentOrchestrationBridge::new(db.clone())),
    );
    let demoted = match orchestration
        .demote_post_to_topic(
            tenant_id,
            security.clone(),
            DemotePostToTopicInput {
                post_id,
                locale: "en".to_string(),
                forum_category_id: forum_category.id,
                reason: Some("demote post".to_string()),
                idempotency_key: "post-to-topic-e2e".to_string(),
            },
        )
        .await
    {
        Ok(output) => output,
        Err(error) => {
            assert!(
                error
                    .to_string()
                    .contains("until Forum uses the canonical richtext contract")
            );
            assert!(
                blog_post::Entity::find_by_id(post_id)
                    .one(&db)
                    .await
                    .expect("blog post lookup should succeed")
                    .is_some()
            );
            return;
        }
    };

    assert!(
        blog_post::Entity::find_by_id(post_id)
            .one(&db)
            .await
            .expect("blog post lookup should succeed")
            .is_none(),
        "source blog post should be deleted after demotion"
    );

    let topic = forum_topic::Entity::find_by_id(demoted.target_id)
        .one(&db)
        .await
        .expect("forum topic lookup should succeed")
        .expect("demoted forum topic should exist");
    assert_eq!(topic.category_id, forum_category.id);
    assert_eq!(topic.reply_count, 2);

    let topic_translation = forum_topic_translation::Entity::find()
        .filter(forum_topic_translation::Column::TopicId.eq(demoted.target_id))
        .filter(forum_topic_translation::Column::Locale.eq("en"))
        .one(&db)
        .await
        .expect("forum topic translation lookup should succeed")
        .expect("demoted forum topic translation should exist");
    assert_eq!(topic_translation.title, "Legacy post");
    assert_eq!(topic_translation.body, "Original blog body");

    let reply_service = ReplyService::new(db.clone(), events.clone());
    let (replies, total) = reply_service
        .list_response_for_topic_with_locale_fallback(
            tenant_id,
            SecurityContext::system(),
            demoted.target_id,
            ListRepliesFilter {
                locale: Some("en".to_string()),
                page: 1,
                per_page: 20,
            },
            None,
        )
        .await
        .expect("replies for demoted topic should list");
    assert_eq!(total, 2);
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[0].content_plain_text, "First blog comment");
    assert_eq!(replies[1].content_plain_text, "Second blog comment");
    assert!(
        replies
            .iter()
            .all(|reply| reply.status == ReplyStatus::Pending.as_str())
    );

    let stored_replies = forum_reply::Entity::find()
        .filter(forum_reply::Column::TopicId.eq(demoted.target_id))
        .all(&db)
        .await
        .expect("stored replies query should succeed");
    let stored_reply_bodies = forum_reply_body::Entity::find()
        .all(&db)
        .await
        .expect("stored reply bodies query should succeed");
    assert_eq!(stored_replies.len(), 2);
    assert_eq!(stored_reply_bodies.len(), 2);

    let comments_service = CommentsService::new(db.clone());
    let (_, remaining_comments) = comments_service
        .list_comments_for_target(
            tenant_id,
            SecurityContext::system(),
            "blog_post",
            post_id,
            ListCommentsFilter {
                locale: "en".to_string(),
                page: 1,
                per_page: 20,
            },
            None,
        )
        .await
        .expect("legacy blog comments should be queryable");
    assert_eq!(remaining_comments, 0);

    let canonical = CanonicalUrlService::new(db.clone());
    let alias_resolution = canonical
        .resolve_route(tenant_id, "en", "/modules/blog?slug=legacy-post")
        .await
        .expect("legacy blog route should resolve")
        .expect("legacy blog route should exist as alias");
    assert!(alias_resolution.redirect_required);
    assert_eq!(alias_resolution.target_kind, "forum_topic");
    assert_eq!(alias_resolution.target_id, demoted.target_id);
    assert_eq!(
        alias_resolution.canonical_url,
        format!("/modules/forum?topic={}", demoted.target_id)
    );

    let canonical_resolution = canonical
        .resolve_route(
            tenant_id,
            "en",
            format!("/modules/forum?topic={}", demoted.target_id).as_str(),
        )
        .await
        .expect("canonical forum route should resolve")
        .expect("canonical forum route should exist");
    assert!(!canonical_resolution.redirect_required);
    assert_eq!(canonical_resolution.target_kind, "forum_topic");
    assert_eq!(canonical_resolution.target_id, demoted.target_id);

    let demoted_retry = orchestration
        .demote_post_to_topic(
            tenant_id,
            security,
            DemotePostToTopicInput {
                post_id,
                locale: "EN".to_string(),
                forum_category_id: forum_category.id,
                reason: Some("demote post".to_string()),
                idempotency_key: "post-to-topic-e2e".to_string(),
            },
        )
        .await
        .expect("idempotent demotion retry should succeed");
    assert_eq!(demoted_retry.target_id, demoted.target_id);
    assert_eq!(demoted_retry.moved_comments, demoted.moved_comments);

    let topics_after_retry = forum_topic::Entity::find()
        .filter(forum_topic::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("forum topics after retry query should succeed");
    let replies_after_retry = forum_reply::Entity::find()
        .filter(forum_reply::Column::TopicId.eq(demoted.target_id))
        .all(&db)
        .await
        .expect("forum replies after retry query should succeed");
    let residual_comments = comment::Entity::find()
        .all(&db)
        .await
        .expect("residual comments query should succeed");
    assert_eq!(topics_after_retry.len(), 1);
    assert_eq!(replies_after_retry.len(), 2);
    assert_eq!(residual_comments.len(), 0);
}
