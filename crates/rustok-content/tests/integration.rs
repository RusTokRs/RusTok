use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rustok_content::{
    BodyInput, ContentOrchestrationService, CreateNodeInput, DemotePostToTopicInput,
    MergeTopicsInput, NodeService, NodeTranslationInput, PromoteTopicToPostInput, SplitTopicInput,
};
use rustok_core::events::{
    DomainEvent, EventDispatcher, EventEnvelope, EventHandler, HandlerResult,
};
use rustok_core::{EventBus, MemoryTransport, SecurityContext, UserRole};
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use uuid::Uuid;

#[derive(Clone, Default)]
struct ContentIndexProjection {
    documents: Arc<Mutex<HashMap<Uuid, String>>>,
}

impl ContentIndexProjection {
    fn upsert(&self, node_id: Uuid, kind: &str) {
        self.documents
            .lock()
            .expect("content projection lock poisoned")
            .insert(node_id, kind.to_string());
    }

    fn get(&self, node_id: Uuid) -> Option<String> {
        self.documents
            .lock()
            .expect("content projection lock poisoned")
            .get(&node_id)
            .cloned()
    }

    fn len(&self) -> usize {
        self.documents
            .lock()
            .expect("content projection lock poisoned")
            .len()
    }
}

#[derive(Clone)]
struct NodeCreatedIndexHandler {
    projection: ContentIndexProjection,
    processed_count: Arc<AtomicUsize>,
}

impl NodeCreatedIndexHandler {
    fn new(projection: ContentIndexProjection, processed_count: Arc<AtomicUsize>) -> Self {
        Self {
            projection,
            processed_count,
        }
    }
}

#[async_trait]
impl EventHandler for NodeCreatedIndexHandler {
    fn name(&self) -> &'static str {
        "node_created_index_handler"
    }

    fn handles(&self, event: &DomainEvent) -> bool {
        matches!(event, DomainEvent::NodeCreated { .. })
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        if let DomainEvent::NodeCreated { node_id, kind, .. } = &envelope.event {
            self.projection.upsert(*node_id, kind);
            self.processed_count.fetch_add(1, Ordering::Relaxed);
        }

        Ok(())
    }
}

#[tokio::test]
async fn test_node_created_event_updates_index_projection() {
    let tenant_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();

    let bus = EventBus::new();
    let mut event_stream = bus.subscribe();

    let projection = ContentIndexProjection::default();
    let processed_count = Arc::new(AtomicUsize::new(0));

    let mut dispatcher = EventDispatcher::new(bus.clone());
    dispatcher.register(NodeCreatedIndexHandler::new(
        projection.clone(),
        Arc::clone(&processed_count),
    ));
    let running_dispatcher = dispatcher.start();

    bus.publish(
        tenant_id,
        None,
        DomainEvent::NodeCreated {
            node_id,
            kind: "post".to_string(),
            author_id: None,
        },
    )
    .expect("must publish NodeCreated event");

    let envelope = tokio::time::timeout(std::time::Duration::from_secs(1), event_stream.recv())
        .await
        .expect("must receive published event")
        .expect("event stream should stay open");

    assert!(matches!(
        envelope.event,
        DomainEvent::NodeCreated { node_id: event_node_id, .. } if event_node_id == node_id
    ));

    wait_until(|| processed_count.load(Ordering::Relaxed) == 1).await;

    assert_eq!(processed_count.load(Ordering::Relaxed), 1);
    assert_eq!(projection.get(node_id).as_deref(), Some("post"));
    assert_eq!(projection.len(), 1);

    running_dispatcher.stop();
}

#[tokio::test]
async fn test_node_created_event_repeat_is_idempotent_for_index_projection() {
    let tenant_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();

    let bus = EventBus::new();
    let projection = ContentIndexProjection::default();
    let processed_count = Arc::new(AtomicUsize::new(0));

    let mut dispatcher = EventDispatcher::new(bus.clone());
    dispatcher.register(NodeCreatedIndexHandler::new(
        projection.clone(),
        Arc::clone(&processed_count),
    ));
    let running_dispatcher = dispatcher.start();

    for _ in 0..2 {
        bus.publish(
            tenant_id,
            None,
            DomainEvent::NodeCreated {
                node_id,
                kind: "post".to_string(),
                author_id: None,
            },
        )
        .expect("NodeCreated publish must succeed");
    }

    wait_until(|| processed_count.load(Ordering::Relaxed) >= 2).await;

    assert_eq!(processed_count.load(Ordering::Relaxed), 2);
    assert_eq!(projection.get(node_id).as_deref(), Some("post"));
    assert_eq!(projection.len(), 1, "projection must stay deduplicated");

    running_dispatcher.stop();
}

async fn setup_content_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:content_integration_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);

    Database::connect(opts)
        .await
        .expect("failed to connect content test sqlite database")
}

async fn ensure_content_schema(db: &DatabaseConnection) {
    if db.get_database_backend() != DbBackend::Sqlite {
        return;
    }

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE IF NOT EXISTS nodes (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            parent_id TEXT NULL,
            author_id TEXT NULL,
            kind TEXT NOT NULL,
            category_id TEXT NULL,
            status TEXT NOT NULL,
            position INTEGER NOT NULL,
            depth INTEGER NOT NULL,
            reply_count INTEGER NOT NULL,
            metadata TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            published_at TEXT NULL,
            deleted_at TEXT NULL,
            version INTEGER NOT NULL DEFAULT 1
        )"
        .to_string(),
    ))
    .await
    .expect("failed to create nodes table");

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE IF NOT EXISTS node_translations (
            id TEXT PRIMARY KEY,
            node_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            title TEXT NULL,
            slug TEXT NULL,
            excerpt TEXT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(node_id) REFERENCES nodes(id)
        )"
        .to_string(),
    ))
    .await
    .expect("failed to create node_translations table");

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE IF NOT EXISTS bodies (
            id TEXT PRIMARY KEY,
            node_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            body TEXT NULL,
            format TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(node_id) REFERENCES nodes(id)
        )"
        .to_string(),
    ))
    .await
    .expect("failed to create bodies table");

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE IF NOT EXISTS content_orchestration_operations (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            operation TEXT NOT NULL,
            idempotency_key TEXT NOT NULL,
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            moved_comments INTEGER NOT NULL,
            created_at TEXT NOT NULL
        )"
        .to_string(),
    ))
    .await
    .expect("failed to create content_orchestration_operations table");

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_content_orchestration_ops_idempotency
            ON content_orchestration_operations(tenant_id, operation, idempotency_key)"
            .to_string(),
    ))
    .await
    .expect("failed to create idx_content_orchestration_ops_idempotency");

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE IF NOT EXISTS content_orchestration_audit_logs (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            operation TEXT NOT NULL,
            idempotency_key TEXT NOT NULL,
            actor_id TEXT NULL,
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            payload TEXT NOT NULL,
            created_at TEXT NOT NULL
        )"
        .to_string(),
    ))
    .await
    .expect("failed to create content_orchestration_audit_logs table");
}

fn drain_event_envelopes(
    receiver: &mut tokio::sync::broadcast::Receiver<EventEnvelope>,
) -> Vec<EventEnvelope> {
    let mut envelopes = Vec::new();
    loop {
        match receiver.try_recv() {
            Ok(envelope) => envelopes.push(envelope),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
            Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => continue,
        }
    }
    envelopes
}

fn orchestration_security() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

#[tokio::test]
async fn test_orchestration_topic_post_split_merge_and_idempotency_with_events() {
    let db = setup_content_test_db().await;
    ensure_content_schema(&db).await;

    let transport = MemoryTransport::new();
    let mut receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));

    let node_service = NodeService::new(db.clone(), event_bus.clone());
    let orchestration = ContentOrchestrationService::new(NodeService::new(db, event_bus));

    let tenant_id = Uuid::new_v4();
    let security = orchestration_security();

    let topic = node_service
        .create_node(
            tenant_id,
            security.clone(),
            CreateNodeInput {
                kind: "forum_topic".to_string(),
                translations: vec![
                    NodeTranslationInput {
                        locale: "en".to_string(),
                        title: Some("Topic A".to_string()),
                        slug: None,
                        excerpt: None,
                    },
                    NodeTranslationInput {
                        locale: "ru".to_string(),
                        title: Some("Тема уникальная".to_string()),
                        slug: None,
                        excerpt: None,
                    },
                ],
                bodies: vec![
                    BodyInput {
                        locale: "en".to_string(),
                        body: Some("Topic body".to_string()),
                        format: Some("markdown".to_string()),
                    },
                    BodyInput {
                        locale: "ru".to_string(),
                        body: Some("Тело темы".to_string()),
                        format: Some("markdown".to_string()),
                    },
                ],
                status: Some(rustok_content::entities::node::ContentStatus::Published),
                parent_id: None,
                author_id: security.user_id,
                category_id: None,
                position: Some(0),
                depth: Some(0),
                reply_count: Some(0),
                metadata: serde_json::json!({"forum_status": "open"}),
            },
        )
        .await
        .expect("topic should be created");

    node_service
        .db()
        .execute(Statement::from_string(
            DbBackend::Sqlite,
            format!(
                "UPDATE node_translations SET slug = NULL WHERE node_id = '{}'",
                topic.id
            ),
        ))
        .await
        .expect("clear source slugs for orchestration duplicate-slug workaround");

    let reply1 = node_service
        .create_node(
            tenant_id,
            security.clone(),
            CreateNodeInput {
                kind: "forum_reply".to_string(),
                translations: vec![NodeTranslationInput {
                    locale: "en".to_string(),
                    title: Some("r1".to_string()),
                    slug: None,
                    excerpt: None,
                }],
                bodies: vec![BodyInput {
                    locale: "en".to_string(),
                    body: Some("reply-1".to_string()),
                    format: Some("markdown".to_string()),
                }],
                status: Some(rustok_content::entities::node::ContentStatus::Published),
                parent_id: Some(topic.id),
                author_id: security.user_id,
                category_id: None,
                position: None,
                depth: None,
                reply_count: None,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("reply1 should be created");

    let reply2 = node_service
        .create_node(
            tenant_id,
            security.clone(),
            CreateNodeInput {
                kind: "forum_reply".to_string(),
                translations: vec![NodeTranslationInput {
                    locale: "en".to_string(),
                    title: Some("r2".to_string()),
                    slug: None,
                    excerpt: None,
                }],
                bodies: vec![BodyInput {
                    locale: "en".to_string(),
                    body: Some("reply-2".to_string()),
                    format: Some("markdown".to_string()),
                }],
                status: Some(rustok_content::entities::node::ContentStatus::Published),
                parent_id: Some(topic.id),
                author_id: security.user_id,
                category_id: None,
                position: None,
                depth: None,
                reply_count: None,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("reply2 should be created");

    let promoted = orchestration
        .promote_topic_to_post(
            tenant_id,
            security.clone(),
            PromoteTopicToPostInput {
                topic_id: topic.id,
                locale: "en".to_string(),
                reason: Some("editorial".to_string()),
                idempotency_key: "promote-key-1".to_string(),
            },
        )
        .await
        .expect("topic->post should succeed");
    assert_eq!(promoted.moved_comments, 2);

    let promoted_post = node_service
        .get_node(tenant_id, promoted.target_id)
        .await
        .expect("promoted post should exist");
    assert_eq!(promoted_post.kind, "blog_post");
    assert_eq!(
        promoted_post
            .metadata
            .get("canonical_node_id")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
        Some(topic.id.to_string())
    );

    let reply1_after = node_service.get_node(tenant_id, reply1.id).await.unwrap();
    assert_eq!(reply1_after.parent_id, Some(promoted.target_id));

    let demoted = orchestration
        .demote_post_to_topic(
            tenant_id,
            security.clone(),
            DemotePostToTopicInput {
                post_id: promoted.target_id,
                locale: "en".to_string(),
                reason: Some("revert".to_string()),
                idempotency_key: "demote-key-1".to_string(),
            },
        )
        .await
        .expect("post->topic should succeed");
    assert_eq!(demoted.moved_comments, 2);

    let demoted_topic = node_service
        .get_node(tenant_id, demoted.target_id)
        .await
        .unwrap();
    assert_eq!(demoted_topic.kind, "forum_topic");
    assert_eq!(
        demoted_topic
            .metadata
            .get("canonical_node_id")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
        Some(promoted.target_id.to_string())
    );

    node_service
        .update_node(
            tenant_id,
            demoted.target_id,
            security.clone(),
            rustok_content::UpdateNodeInput {
                translations: Some(vec![NodeTranslationInput {
                    locale: "en".to_string(),
                    title: Some("Topic B".to_string()),
                    slug: None,
                    excerpt: None,
                }]),
                ..Default::default()
            },
        )
        .await
        .expect("normalize demoted topic translations before split");

    let split = orchestration
        .split_topic(
            tenant_id,
            security.clone(),
            SplitTopicInput {
                topic_id: demoted.target_id,
                locale: "en".to_string(),
                reply_ids: vec![reply1.id],
                new_title: "Topic split".to_string(),
                reason: Some("cleanup".to_string()),
                idempotency_key: "split-key-1".to_string(),
            },
        )
        .await
        .expect("split should succeed");
    assert_eq!(split.moved_comments, 1);

    let split_repeat = orchestration
        .split_topic(
            tenant_id,
            security.clone(),
            SplitTopicInput {
                topic_id: demoted.target_id,
                locale: "en".to_string(),
                reply_ids: vec![reply1.id],
                new_title: "Topic split repeat".to_string(),
                reason: Some("retry".to_string()),
                idempotency_key: "split-key-1".to_string(),
            },
        )
        .await
        .expect("same split command should not corrupt data");
    assert_eq!(split_repeat.moved_comments, 0);

    let reply1_split = node_service.get_node(tenant_id, reply1.id).await.unwrap();
    let reply2_stays = node_service.get_node(tenant_id, reply2.id).await.unwrap();
    assert_eq!(reply1_split.parent_id, Some(split.target_id));
    assert_eq!(reply2_stays.parent_id, Some(demoted.target_id));

    let split_topic = node_service
        .get_node(tenant_id, split.target_id)
        .await
        .unwrap();
    assert_eq!(
        split_topic
            .metadata
            .get("canonical_node_id")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
        Some(demoted.target_id.to_string())
    );

    let merge = orchestration
        .merge_topics(
            tenant_id,
            security,
            MergeTopicsInput {
                target_topic_id: demoted.target_id,
                source_topic_ids: vec![split.target_id],
                reason: Some("merge-back".to_string()),
                idempotency_key: "merge-key-1".to_string(),
            },
        )
        .await
        .expect("merge should succeed");
    assert_eq!(merge.moved_comments, 1);

    let reply1_merged = node_service.get_node(tenant_id, reply1.id).await.unwrap();
    assert_eq!(reply1_merged.parent_id, Some(demoted.target_id));

    let merge_target = node_service
        .get_node(tenant_id, demoted.target_id)
        .await
        .unwrap();
    assert_eq!(
        merge_target
            .metadata
            .get("canonical_node_id")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
        Some(demoted.target_id.to_string())
    );

    let operations_rows = node_service
        .db()
        .query_all(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT id FROM content_orchestration_operations".to_string(),
        ))
        .await
        .expect("operations table should be queryable");
    assert_eq!(operations_rows.len(), 4);

    let audit_rows = node_service
        .db()
        .query_all(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT id FROM content_orchestration_audit_logs".to_string(),
        ))
        .await
        .expect("audit table should be queryable");
    assert_eq!(audit_rows.len(), 4);

    let events = drain_event_envelopes(&mut receiver);
    assert!(events.iter().any(|e| {
        matches!(
            &e.event,
            DomainEvent::TopicPromotedToPost {
                topic_id,
                post_id,
                moved_comments,
                ..
            } if *topic_id == topic.id && *post_id == promoted.target_id && *moved_comments == 2
        )
    }));
    assert!(events.iter().any(|e| {
        matches!(
            &e.event,
            DomainEvent::PostDemotedToTopic {
                post_id,
                topic_id,
                moved_comments,
                ..
            } if *post_id == promoted.target_id && *topic_id == demoted.target_id && *moved_comments == 2
        )
    }));
    assert!(events.iter().any(|e| {
        matches!(
            &e.event,
            DomainEvent::TopicSplit {
                source_topic_id,
                target_topic_id,
                moved_comment_ids,
                moved_comments,
                ..
            } if *source_topic_id == demoted.target_id
                && *target_topic_id == split.target_id
                && *moved_comments == 1
                && moved_comment_ids.contains(&reply1.id)
        )
    }));
    assert!(events.iter().any(|e| {
        matches!(
            &e.event,
            DomainEvent::TopicsMerged {
                target_topic_id,
                moved_comments,
                ..
            } if *target_topic_id == demoted.target_id && *moved_comments == 1
        )
    }));
}

async fn wait_until(condition: impl Fn() -> bool) {
    for _ in 0..40 {
        if condition() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    panic!("condition was not met within the expected time");
}
