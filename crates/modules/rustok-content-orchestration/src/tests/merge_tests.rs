use std::sync::Arc;

use rustok_api::RichTextDocument;
use rustok_content::{CanonicalRouteResolver, ContentOrchestrationService, MergeTopicsInput};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ReplyService,
    TopicService, TopicStatus, forum_category, forum_reply, forum_topic,
};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::OwnerCanonicalRouteResolver;
use crate::ServerContentOrchestrationBridge;
use crate::tests::helpers::{
    admin_security, ensure_conversion_schema, insert_test_actor, setup_conversion_test_db,
    zero_cooldown_providers,
};

#[tokio::test]
async fn merge_topics_moves_replies_and_updates_category_counters() {
    let db = setup_conversion_test_db().await;
    ensure_conversion_schema(&db).await;

    let events = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let security = admin_security();
    let tenant_id = Uuid::new_v4();
    insert_test_actor(&db, tenant_id, &security).await;

    let category_a = CategoryService::new(db.clone())
        .create(
            tenant_id,
            security.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Category A".to_string(),
                slug: "cat-a".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("category A created");

    let category_b = CategoryService::new(db.clone())
        .create(
            tenant_id,
            security.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Category B".to_string(),
                slug: "cat-b".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(1),
                moderated: false,
            },
        )
        .await
        .expect("category B created");

    let topic_service = TopicService::new(db.clone(), events.clone())
        .with_settings_providers(zero_cooldown_providers());
    let topic_a = topic_service
        .create(
            tenant_id,
            security.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category_a.id,
                title: "Target thread".to_string(),
                slug: Some("target-thread".to_string()),
                body: RichTextDocument::single_paragraph("Target body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic A created");

    let topic_b = topic_service
        .create(
            tenant_id,
            security.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category_b.id,
                title: "Source thread".to_string(),
                slug: Some("source-thread".to_string()),
                body: RichTextDocument::single_paragraph("Source body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic B created");

    let reply_service = ReplyService::new(db.clone(), events.clone())
        .with_settings_providers(zero_cooldown_providers());
    let _reply_a = reply_service
        .create(
            tenant_id,
            security.clone(),
            topic_a.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: RichTextDocument::single_paragraph("Target reply"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply A created");

    let _reply_b = reply_service
        .create(
            tenant_id,
            security.clone(),
            topic_b.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: RichTextDocument::single_paragraph("Source reply"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply B created");

    let orchestration = ContentOrchestrationService::new(
        db.clone(),
        events.clone(),
        Arc::new(ServerContentOrchestrationBridge::new(
            db.clone(),
            events.clone(),
        )),
    );

    let merge_result = orchestration
        .merge_topics(
            tenant_id,
            security.clone(),
            MergeTopicsInput {
                target_topic_id: topic_a.id,
                source_topic_ids: vec![topic_b.id],
                reason: Some("merging duplicate topic".to_string()),
                idempotency_key: "merge-topic-test-1".to_string(),
            },
        )
        .await
        .expect("merge should succeed");

    assert_eq!(merge_result.target_id, topic_a.id);
    assert_eq!(merge_result.moved_comments, 1);

    let source_topic_row = forum_topic::Entity::find_by_id(topic_b.id)
        .filter(forum_topic::Column::TenantId.eq(tenant_id))
        .one(&db)
        .await
        .expect("query source topic")
        .expect("source topic row should exist with soft-deleted state");
    assert_eq!(source_topic_row.status, TopicStatus::Archived);
    assert!(source_topic_row.is_locked);
    assert_eq!(source_topic_row.reply_count, 0);

    let target_replies = forum_reply::Entity::find()
        .filter(forum_reply::Column::TopicId.eq(topic_a.id))
        .order_by_asc(forum_reply::Column::Position)
        .all(&db)
        .await
        .expect("query target replies");
    assert_eq!(target_replies.len(), 2);
    assert_eq!(target_replies[0].position, 1);
    assert_eq!(target_replies[1].position, 2);

    let cat_b = forum_category::Entity::find_by_id(category_b.id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(&db)
        .await
        .expect("query category B")
        .expect("category B exists");
    assert_eq!(cat_b.topic_count, 0);

    let canonical = OwnerCanonicalRouteResolver::new(db.clone());
    let alias = canonical
        .resolve_route(
            tenant_id,
            "en",
            format!("/modules/forum?topic={}", topic_b.id).as_str(),
        )
        .await
        .expect("resolve alias")
        .expect("alias exists");
    assert!(alias.redirect_required);
    assert_eq!(alias.target_id, topic_a.id);
}
