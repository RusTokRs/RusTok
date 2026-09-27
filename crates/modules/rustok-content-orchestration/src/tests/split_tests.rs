use std::sync::Arc;

use rustok_api::RichTextDocument;
use rustok_content::{ContentOrchestrationService, SplitTopicInput};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ReplyService,
    TopicService, forum_category, forum_reply, forum_topic_translation,
};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::ServerContentOrchestrationBridge;
use crate::tests::helpers::{
    admin_security, ensure_conversion_schema, insert_test_actor, setup_conversion_test_db,
};

#[tokio::test]
async fn split_topic_moves_replies_and_updates_category_counter() {
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
                name: "Discussions".to_string(),
                slug: "discussions".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("category should be created");

    let topic = TopicService::new(db.clone(), events.clone())
        .create(
            tenant_id,
            security.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Main thread".to_string(),
                slug: Some("main-thread".to_string()),
                body: RichTextDocument::single_paragraph("Original main body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");

    let reply_service = ReplyService::new(db.clone(), events.clone());
    let reply_1 = reply_service
        .create(
            tenant_id,
            security.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: RichTextDocument::single_paragraph("Reply 1"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply 1 should be created");

    let reply_2 = reply_service
        .create(
            tenant_id,
            security.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: RichTextDocument::single_paragraph("Reply 2"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply 2 should be created");

    let orchestration = ContentOrchestrationService::new(
        db.clone(),
        events.clone(),
        Arc::new(ServerContentOrchestrationBridge::new(db.clone())),
    );

    let split_result = orchestration
        .split_topic(
            tenant_id,
            security.clone(),
            SplitTopicInput {
                topic_id: topic.id,
                locale: "en".to_string(),
                reply_ids: vec![reply_2.id],
                new_title: "Forked thread".to_string(),
                reason: Some("split off discussion".to_string()),
                idempotency_key: "split-topic-test-1".to_string(),
            },
        )
        .await
        .expect("split should succeed");

    assert_eq!(split_result.source_id, topic.id);
    assert_eq!(split_result.moved_comments, 1);

    let target_trans = forum_topic_translation::Entity::find()
        .filter(forum_topic_translation::Column::TopicId.eq(split_result.target_id))
        .filter(forum_topic_translation::Column::Locale.eq("en"))
        .one(&db)
        .await
        .expect("query target translation")
        .expect("target translation exists");
    assert_eq!(target_trans.title, "Forked thread");
    assert_eq!(target_trans.slug, Some("forked-thread".to_string()));

    let cat = forum_category::Entity::find_by_id(category.id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(&db)
        .await
        .expect("query category")
        .expect("category exists");
    assert_eq!(cat.topic_count, 2);

    let source_replies = forum_reply::Entity::find()
        .filter(forum_reply::Column::TopicId.eq(topic.id))
        .all(&db)
        .await
        .expect("query source replies");
    assert_eq!(source_replies.len(), 1);
    assert_eq!(source_replies[0].id, reply_1.id);
    assert_eq!(source_replies[0].position, 1);

    let target_replies = forum_reply::Entity::find()
        .filter(forum_reply::Column::TopicId.eq(split_result.target_id))
        .all(&db)
        .await
        .expect("query target replies");
    assert_eq!(target_replies.len(), 1);
    assert_eq!(target_replies[0].id, reply_2.id);
    assert_eq!(target_replies[0].position, 1);
}
