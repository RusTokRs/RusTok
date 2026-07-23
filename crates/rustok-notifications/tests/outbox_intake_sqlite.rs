use chrono::Utc;
use rustok_core::MigrationSource;
use rustok_events::{ContractEventEnvelope, DomainEvent, EventEnvelope, ForumMentionEvent};
use rustok_notifications::entities::source_inbox;
use rustok_notifications::{
    MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE, NotificationOutboxIntakeWorker,
    NotificationsModule,
};
use rustok_outbox::entity::{self as outbox_event, SysEventStatus};
use rustok_outbox::{OutboxModule, OutboxTransport};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[tokio::test]
async fn committed_root_and_contract_envelopes_enter_source_inbox_once() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    let transport = OutboxTransport::new(db.clone());

    let mut topic_events = Vec::new();
    for _ in 0..33 {
        let topic_id = Uuid::new_v4();
        let envelope = EventEnvelope::new(
            tenant_id,
            Some(Uuid::new_v4()),
            DomainEvent::ForumTopicCreated {
                topic_id,
                category_id: Uuid::new_v4(),
                author_id: Some(Uuid::new_v4()),
                locale: "en".to_string(),
            },
        );
        let outbox_event_id = envelope.id;
        transport
            .write_to_outbox(&db, envelope)
            .await
            .expect("root envelope should persist in outbox");
        mark_dispatched(&db, outbox_event_id).await;
        topic_events.push((outbox_event_id, topic_id));
    }

    let mention_revision = 7_i64;
    let mention = ContractEventEnvelope::new(
        tenant_id,
        Some(Uuid::new_v4()),
        ForumMentionEvent::UserMentionAdded {
            source_kind: "topic".to_string(),
            source_id: Uuid::new_v4(),
            source_revision_id: mention_revision,
            source_locale: "en".to_string(),
            mentioned_user_id: Uuid::new_v4(),
        },
    )
    .expect("mention contract should validate");
    let mention_outbox_event_id = mention.id();
    transport
        .write_contract_to_outbox(&db, mention)
        .await
        .expect("contract envelope should persist in pending outbox state");

    let worker = NotificationOutboxIntakeWorker::new(db.clone(), 32)
        .expect("bounded intake worker should compose");
    let first_ids = worker
        .pending_outbox_event_ids()
        .await
        .expect("first intake page should select");
    assert_eq!(first_ids.len(), 32);
    let first = worker
        .process_next_batch()
        .await
        .expect("first intake page should process");
    assert_eq!(first.selected, 32);
    assert_eq!(first.accepted, 32);
    assert!(first.failures.is_empty());

    let second = worker
        .process_next_batch()
        .await
        .expect("second intake page should process");
    assert_eq!(second.selected, 2);
    assert_eq!(second.accepted, 2);
    assert!(second.failures.is_empty());
    assert!(worker
        .pending_outbox_event_ids()
        .await
        .expect("completed intake should be excluded by receipt")
        .is_empty());

    assert_eq!(
        source_inbox::Entity::find()
            .filter(source_inbox::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .expect("source inbox count should succeed"),
        34
    );

    let (topic_outbox_event_id, topic_id) = topic_events[0];
    let topic_row = source_inbox::Entity::find()
        .filter(source_inbox::Column::TenantId.eq(tenant_id))
        .filter(source_inbox::Column::SourceEventId.eq(topic_id))
        .one(&db)
        .await
        .expect("topic source inbox query should succeed")
        .expect("topic source inbox row should exist");
    assert_eq!(topic_row.event_type, "forum.topic.created");
    assert_eq!(topic_row.source_revision, 1);
    assert_ne!(topic_outbox_event_id, topic_row.source_event_id);

    let mention_row = source_inbox::Entity::find()
        .filter(source_inbox::Column::TenantId.eq(tenant_id))
        .filter(source_inbox::Column::SourceEventId.eq(mention_outbox_event_id))
        .one(&db)
        .await
        .expect("mention source inbox query should succeed")
        .expect("mention source inbox row should exist");
    assert_eq!(mention_row.event_type, "forum.mention.user_added");
    assert_eq!(mention_row.source_revision, mention_revision);

    let mention_outbox_row = outbox_event::Entity::find_by_id(mention_outbox_event_id)
        .one(&db)
        .await
        .expect("mention outbox row query should succeed")
        .expect("mention outbox row should remain available");
    assert_eq!(mention_outbox_row.status, SysEventStatus::Pending);
    assert!(mention_outbox_row.dispatched_at.is_none());

    let replay = worker
        .process_outbox_event(topic_outbox_event_id)
        .await
        .expect("accepted outbox event should replay its receipt");
    assert!(replay.replayed);
    assert_eq!(replay.source_inbox_id, topic_row.id);

    let oversized = NotificationOutboxIntakeWorker::new(
        db,
        MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE + 1,
    )
    .expect_err("intake batches above the hard maximum must fail");
    assert_eq!(oversized.stable_code(), "NOTIFICATION_VALIDATION_ERROR");
}

async fn mark_dispatched(db: &DatabaseConnection, event_id: Uuid) {
    let row = outbox_event::Entity::find_by_id(event_id)
        .one(db)
        .await
        .expect("outbox event query should succeed")
        .expect("outbox event should exist");
    let mut active: outbox_event::ActiveModel = row.into();
    active.status = Set(SysEventStatus::Dispatched);
    active.dispatched_at = Set(Some(Utc::now()));
    active
        .update(db)
        .await
        .expect("outbox event should become dispatched");
}

async fn setup() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:notification_outbox_intake_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("outbox intake sqlite database should connect");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("foreign keys should enable");
    db.execute_unprepared(
        r#"
        CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL);
        CREATE TABLE users (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
        );
        "#,
    )
    .await
    .expect("platform identity fixture should apply");
    let manager = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("outbox migration should apply");
    }
    for migration in NotificationsModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("notification migration should apply");
    }
    db
}

async fn insert_tenant(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute_unprepared(&format!("INSERT INTO tenants (id) VALUES ('{tenant_id}')"))
        .await
        .expect("tenant fixture should persist");
}
