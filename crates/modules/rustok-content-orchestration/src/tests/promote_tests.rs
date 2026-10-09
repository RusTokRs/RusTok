use std::sync::Arc;

use rustok_api::RichTextDocument;
use rustok_blog::{blog_post, blog_post_tag, blog_post_translation};
use rustok_comments::{comment, comment_thread};
use rustok_content::{
    CanonicalRouteResolver, ContentOrchestrationService, PromoteTopicToPostInput,
};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ReplyService,
    TopicService, forum_category, forum_topic,
};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::OwnerCanonicalRouteResolver;
use crate::ServerContentOrchestrationBridge;
use crate::tests::helpers::{
    admin_security, ensure_conversion_schema, insert_test_actor, setup_conversion_test_db,
};

#[tokio::test]
async fn promote_topic_to_post_moves_replies_and_registers_redirects() {
    let db = setup_conversion_test_db().await;
    ensure_conversion_schema(&db).await;

    let events = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let security = admin_security();
    let tenant_id = Uuid::new_v4();
    insert_test_actor(&db, tenant_id, &security).await;

    let category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            security.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "General".to_string(),
                slug: "general".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("forum category should be created");

    let topic = TopicService::new(db.clone(), events.clone())
        .create(
            tenant_id,
            security.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Forum thread".to_string(),
                slug: Some("forum-thread".to_string()),
                body: RichTextDocument::single_paragraph("Original forum body"),
                metadata: serde_json::json!({}),
                tags: vec!["release".to_string(), "notes".to_string()],
                channel_slugs: None,
            },
        )
        .await
        .expect("forum topic should be created");

    let reply_service = ReplyService::new(db.clone(), events.clone());
    reply_service
        .create(
            tenant_id,
            security.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: RichTextDocument::single_paragraph("First reply"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("first reply should be created");
    reply_service
        .create(
            tenant_id,
            security.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: RichTextDocument::single_paragraph("Second reply"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("second reply should be created");

    let orchestration = ContentOrchestrationService::new(
        db.clone(),
        events.clone(),
        Arc::new(ServerContentOrchestrationBridge::new(db.clone())),
    );
    let promoted = match orchestration
        .promote_topic_to_post(
            tenant_id,
            security.clone(),
            PromoteTopicToPostInput {
                topic_id: topic.id,
                locale: "en".to_string(),
                blog_category_id: None,
                reason: Some("promote discussion".to_string()),
                idempotency_key: "topic-to-post-e2e".to_string(),
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
                forum_topic::Entity::find_by_id(topic.id)
                    .one(&db)
                    .await
                    .expect("forum topic lookup should succeed")
                    .is_some()
            );
            return;
        }
    };

    assert!(
        forum_topic::Entity::find_by_id(topic.id)
            .one(&db)
            .await
            .expect("forum topic lookup should succeed")
            .is_none(),
        "source forum topic should be deleted after promotion"
    );

    let post = blog_post::Entity::find_by_id(promoted.target_id)
        .one(&db)
        .await
        .expect("blog post lookup should succeed")
        .expect("promoted blog post should exist");
    assert_eq!(post.slug, "forum-thread");
    assert_eq!(post.comment_count, 2);

    let post_translation = blog_post_translation::Entity::find()
        .filter(blog_post_translation::Column::PostId.eq(promoted.target_id))
        .filter(blog_post_translation::Column::Locale.eq("en"))
        .one(&db)
        .await
        .expect("blog post translation lookup should succeed")
        .expect("promoted blog post translation should exist");
    assert_eq!(post_translation.title, "Forum thread");
    assert_eq!(post_translation.body, "Original forum body");

    let thread = comment_thread::Entity::find()
        .filter(comment_thread::Column::TargetType.eq("blog_post"))
        .filter(comment_thread::Column::TargetId.eq(promoted.target_id))
        .one(&db)
        .await
        .expect("comment thread lookup should succeed")
        .expect("destination comment thread should exist");
    let comments = comment::Entity::find()
        .filter(comment::Column::ThreadId.eq(thread.id))
        .all(&db)
        .await
        .expect("comments lookup should succeed");
    assert_eq!(comments.len(), 2);
    let comment_bodies = rustok_comments::comment_body::Entity::find()
        .filter(
            rustok_comments::comment_body::Column::CommentId
                .is_in(comments.iter().map(|c| c.id).collect::<Vec<_>>()),
        )
        .all(&db)
        .await
        .expect("comment bodies lookup should succeed");
    assert_eq!(comment_bodies.len(), 2);
    assert!(comment_bodies.iter().any(|b| b.body == "First reply"));
    assert!(comment_bodies.iter().any(|b| b.body == "Second reply"));

    let post_tags = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::PostId.eq(promoted.target_id))
        .all(&db)
        .await
        .expect("blog post tags lookup should succeed");
    assert_eq!(post_tags.len(), 2);

    let category = forum_category::Entity::find_by_id(category.id)
        .one(&db)
        .await
        .expect("forum category lookup should succeed")
        .expect("forum category should exist");
    assert_eq!(category.topic_count, 0);
    assert_eq!(category.reply_count, 0);

    let canonical = OwnerCanonicalRouteResolver::new(db.clone());
    let alias_resolution = canonical
        .resolve_route(
            tenant_id,
            "en",
            format!("/modules/forum?topic={}", topic.id).as_str(),
        )
        .await
        .expect("legacy forum route should resolve")
        .expect("legacy forum route should exist as alias");
    assert!(alias_resolution.redirect_required);
    assert_eq!(alias_resolution.target_kind, "blog_post");
    assert_eq!(alias_resolution.target_id, promoted.target_id);
    assert_eq!(
        alias_resolution.canonical_url,
        "/modules/blog?slug=forum-thread"
    );

    let canonical_resolution = canonical
        .resolve_route(tenant_id, "en", "/modules/blog?slug=forum-thread")
        .await
        .expect("canonical blog route should resolve")
        .expect("canonical blog route should exist");
    assert!(!canonical_resolution.redirect_required);
    assert_eq!(canonical_resolution.target_kind, "blog_post");
    assert_eq!(canonical_resolution.target_id, promoted.target_id);

    let promoted_retry = orchestration
        .promote_topic_to_post(
            tenant_id,
            security,
            PromoteTopicToPostInput {
                topic_id: topic.id,
                locale: "EN".to_string(),
                blog_category_id: None,
                reason: Some("promote discussion".to_string()),
                idempotency_key: "topic-to-post-e2e".to_string(),
            },
        )
        .await
        .expect("idempotent promotion retry should succeed");
    assert_eq!(promoted_retry.target_id, promoted.target_id);
    assert_eq!(promoted_retry.moved_comments, promoted.moved_comments);

    let posts_after_retry = blog_post::Entity::find()
        .filter(blog_post::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("blog posts after retry query should succeed");
    let comments_after_retry = comment::Entity::find()
        .all(&db)
        .await
        .expect("stored comments after retry query should succeed");
    let post_tags_after_retry = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::PostId.eq(promoted.target_id))
        .all(&db)
        .await
        .expect("blog post tags after retry query should succeed");
    assert_eq!(posts_after_retry.len(), 1);
    assert_eq!(comments_after_retry.len(), 2);
    assert_eq!(post_tags_after_retry.len(), 2);
}
