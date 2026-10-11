use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use rustok_content::{
    ContentOrchestrationBridge, ContentOrchestrationService, ContentResult, DemotePostToTopicInput,
    DemotePostToTopicOutput, MergeTopicsInput, MergeTopicsOutput, PromoteTopicToPostInput,
    PromoteTopicToPostOutput, SplitTopicInput, SplitTopicOutput,
};
use rustok_core::{DomainEvent, MemoryTransport, SecurityContext, UserRole};
use rustok_events::EventEnvelope;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    EntityTrait, QueryFilter, Statement,
};
use uuid::Uuid;

use rustok_content::entities::{orchestration_audit_log, orchestration_operation};

#[derive(Default)]
struct MockBridge {
    promote_calls: AtomicUsize,
    demote_calls: AtomicUsize,
    split_calls: AtomicUsize,
    merge_calls: AtomicUsize,
    promoted_post_id: Uuid,
    demoted_topic_id: Uuid,
    split_topic_id: Uuid,
}

impl MockBridge {
    fn new() -> Self {
        Self {
            promote_calls: AtomicUsize::new(0),
            demote_calls: AtomicUsize::new(0),
            split_calls: AtomicUsize::new(0),
            merge_calls: AtomicUsize::new(0),
            promoted_post_id: Uuid::new_v4(),
            demoted_topic_id: Uuid::new_v4(),
            split_topic_id: Uuid::new_v4(),
        }
    }
}

#[async_trait]
impl ContentOrchestrationBridge for MockBridge {
    async fn promote_topic_to_post(
        &self,
        _txn: &sea_orm::DatabaseTransaction,
        _tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        input: &PromoteTopicToPostInput,
    ) -> ContentResult<PromoteTopicToPostOutput> {
        self.promote_calls.fetch_add(1, Ordering::Relaxed);
        Ok(PromoteTopicToPostOutput {
            topic_id: input.topic_id,
            post_id: self.promoted_post_id,
            moved_comments: 2,
            effective_locale: input.locale.to_ascii_lowercase(),
        })
    }

    async fn demote_post_to_topic(
        &self,
        _txn: &sea_orm::DatabaseTransaction,
        _tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        input: &DemotePostToTopicInput,
    ) -> ContentResult<DemotePostToTopicOutput> {
        self.demote_calls.fetch_add(1, Ordering::Relaxed);
        Ok(DemotePostToTopicOutput {
            post_id: input.post_id,
            topic_id: self.demoted_topic_id,
            moved_comments: 3,
            effective_locale: input.locale.to_ascii_lowercase(),
        })
    }

    async fn split_topic(
        &self,
        _txn: &sea_orm::DatabaseTransaction,
        _tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        input: &SplitTopicInput,
    ) -> ContentResult<SplitTopicOutput> {
        self.split_calls.fetch_add(1, Ordering::Relaxed);
        Ok(SplitTopicOutput {
            source_topic_id: input.topic_id,
            target_topic_id: self.split_topic_id,
            moved_reply_ids: input.reply_ids.clone(),
            moved_comments: input.reply_ids.len() as u64,
        })
    }

    async fn merge_topics(
        &self,
        _txn: &sea_orm::DatabaseTransaction,
        _tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        input: &MergeTopicsInput,
    ) -> ContentResult<MergeTopicsOutput> {
        self.merge_calls.fetch_add(1, Ordering::Relaxed);
        Ok(MergeTopicsOutput {
            target_topic_id: input.target_topic_id,
            source_topic_ids: input.source_topic_ids.clone(),
            moved_comments: 5,
        })
    }
}

async fn setup_content_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:content_orchestration_{}?mode=memory&cache=shared",
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

    db.execute_raw(Statement::from_string(
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

    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_content_orchestration_ops_idempotency
            ON content_orchestration_operations(tenant_id, operation, idempotency_key)"
            .to_string(),
    ))
    .await
    .expect("failed to create idx_content_orchestration_ops_idempotency");

    db.execute_raw(Statement::from_string(
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
async fn test_promote_topic_to_post_is_idempotent_and_publishes_single_event() {
    let db = setup_content_test_db().await;
    ensure_content_schema(&db).await;

    let transport = MemoryTransport::new();
    let mut receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let bridge = Arc::new(MockBridge::new());
    let orchestration = ContentOrchestrationService::new(db.clone(), event_bus, bridge.clone());

    let tenant_id = Uuid::new_v4();
    let security = orchestration_security();
    let topic_id = Uuid::new_v4();
    let input = PromoteTopicToPostInput {
        topic_id,
        locale: "EN_us".to_string(),
        blog_category_id: None,
        reason: Some("promote".to_string()),
        idempotency_key: "promote-topic-1".to_string(),
    };

    let first = orchestration
        .promote_topic_to_post(tenant_id, security.clone(), input.clone())
        .await
        .expect("first orchestration call should succeed");
    let second = orchestration
        .promote_topic_to_post(tenant_id, security, input)
        .await
        .expect("second orchestration call should reuse idempotent result");

    assert_eq!(first, second);
    assert_eq!(bridge.promote_calls.load(Ordering::Relaxed), 1);

    let operations = orchestration_operation::Entity::find()
        .filter(orchestration_operation::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("operations query should succeed");
    let audits = orchestration_audit_log::Entity::find()
        .filter(orchestration_audit_log::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("audit query should succeed");

    assert_eq!(operations.len(), 1);
    assert_eq!(audits.len(), 1);
    assert_eq!(operations[0].source_id, topic_id);
    assert_eq!(operations[0].target_id, bridge.promoted_post_id);

    let envelopes = drain_event_envelopes(&mut receiver);
    assert_eq!(envelopes.len(), 1);
    assert!(matches!(
        &envelopes[0].event,
        DomainEvent::TopicPromotedToPost { topic_id: event_topic_id, post_id, moved_comments, locale, reason }
            if *event_topic_id == topic_id
                && *post_id == bridge.promoted_post_id
                && *moved_comments == 2
                && locale == "en_us"
                && reason.as_deref() == Some("promote")
    ));
}

#[tokio::test]
async fn test_demote_split_and_merge_use_bridge_and_persist_audit_records() {
    let db = setup_content_test_db().await;
    ensure_content_schema(&db).await;

    let transport = MemoryTransport::new();
    let mut receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let bridge = Arc::new(MockBridge::new());
    let orchestration = ContentOrchestrationService::new(db.clone(), event_bus, bridge.clone());

    let tenant_id = Uuid::new_v4();
    let security = orchestration_security();
    let post_id = Uuid::new_v4();
    let source_topic_id = Uuid::new_v4();
    let target_topic_id = Uuid::new_v4();
    let moved_reply_ids = vec![Uuid::new_v4(), Uuid::new_v4()];

    let demoted = orchestration
        .demote_post_to_topic(
            tenant_id,
            security.clone(),
            DemotePostToTopicInput {
                post_id,
                locale: "ru".to_string(),
                forum_category_id: Uuid::new_v4(),
                reason: Some("demote".to_string()),
                idempotency_key: "demote-post-1".to_string(),
            },
        )
        .await
        .expect("demote must succeed");
    assert_eq!(demoted.source_id, post_id);
    assert_eq!(demoted.target_id, bridge.demoted_topic_id);

    let split = orchestration
        .split_topic(
            tenant_id,
            security.clone(),
            SplitTopicInput {
                topic_id: source_topic_id,
                locale: "ru".to_string(),
                reply_ids: moved_reply_ids.clone(),
                new_title: "Split target".to_string(),
                reason: Some("split".to_string()),
                idempotency_key: "split-topic-1".to_string(),
            },
        )
        .await
        .expect("split must succeed");
    assert_eq!(split.source_id, source_topic_id);
    assert_eq!(split.target_id, bridge.split_topic_id);
    assert_eq!(split.moved_comments, moved_reply_ids.len() as u64);

    let merged = orchestration
        .merge_topics(
            tenant_id,
            security,
            MergeTopicsInput {
                target_topic_id,
                source_topic_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
                reason: Some("merge".to_string()),
                idempotency_key: "merge-topic-1".to_string(),
            },
        )
        .await
        .expect("merge must succeed");
    assert_eq!(merged.source_id, target_topic_id);
    assert_eq!(merged.target_id, target_topic_id);
    assert_eq!(merged.moved_comments, 5);

    assert_eq!(bridge.demote_calls.load(Ordering::Relaxed), 1);
    assert_eq!(bridge.split_calls.load(Ordering::Relaxed), 1);
    assert_eq!(bridge.merge_calls.load(Ordering::Relaxed), 1);

    let operations = orchestration_operation::Entity::find()
        .filter(orchestration_operation::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("operations query should succeed");
    let audits = orchestration_audit_log::Entity::find()
        .filter(orchestration_audit_log::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("audit query should succeed");
    assert_eq!(operations.len(), 3);
    assert_eq!(audits.len(), 3);

    let envelopes = drain_event_envelopes(&mut receiver);
    assert_eq!(envelopes.len(), 3);
    assert!(envelopes.iter().any(|envelope| matches!(
        &envelope.event,
        DomainEvent::PostDemotedToTopic { post_id: event_post_id, topic_id, moved_comments, locale, reason }
            if *event_post_id == post_id
                && *topic_id == bridge.demoted_topic_id
                && *moved_comments == 3
                && locale == "ru"
                && reason.as_deref() == Some("demote")
    )));
    assert!(envelopes.iter().any(|envelope| matches!(
        &envelope.event,
        DomainEvent::TopicSplit { source_topic_id: event_source, target_topic_id: event_target, moved_comment_ids, moved_comments, reason }
            if *event_source == source_topic_id
                && *event_target == bridge.split_topic_id
                && moved_comment_ids == &moved_reply_ids
                && *moved_comments == moved_reply_ids.len() as u64
                && reason.as_deref() == Some("split")
    )));
    assert!(envelopes.iter().any(|envelope| matches!(
        &envelope.event,
        DomainEvent::TopicsMerged { target_topic_id: event_target, moved_comments, reason }
            if *event_target == target_topic_id
                && *moved_comments == 5
                && reason.as_deref() == Some("merge")
    )));
}

#[tokio::test]
async fn test_split_topic_rejects_empty_reply_list_before_bridge_call() {
    let db = setup_content_test_db().await;
    ensure_content_schema(&db).await;

    let transport = MemoryTransport::new();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let bridge = Arc::new(MockBridge::new());
    let orchestration = ContentOrchestrationService::new(db, event_bus, bridge.clone());

    let err = orchestration
        .split_topic(
            Uuid::new_v4(),
            orchestration_security(),
            SplitTopicInput {
                topic_id: Uuid::new_v4(),
                locale: "ru".to_string(),
                reply_ids: Vec::new(),
                new_title: "Bad split".to_string(),
                reason: None,
                idempotency_key: "split-empty".to_string(),
            },
        )
        .await
        .expect_err("empty split must fail");

    assert!(matches!(err, rustok_content::ContentError::Validation(_)));
    assert_eq!(bridge.split_calls.load(Ordering::Relaxed), 0);
}
