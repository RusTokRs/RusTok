use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_core::MigrationSource;
use rustok_core::events::EventTransport;
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_social_graph::entities::relation;
use rustok_social_graph::{
    SocialGraphModule, SocialGraphRelationEventMaintenancePort,
    SocialGraphRelationEventMaintenanceService, SocialGraphRelationEventReplayCommand,
    SocialRelationKind,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    Statement,
};
use sea_orm_migration::SchemaManager;
use serde_json::Value;
use uuid::Uuid;

#[tokio::test]
async fn bounded_replay_is_tenant_scoped_cursor_driven_and_dry_run_safe() {
    let db = setup(true).await;
    let tenant_id = id(100);
    let other_tenant_id = id(200);
    insert_tenant(&db, tenant_id).await;
    insert_tenant(&db, other_tenant_id).await;
    for user_id in [id(1), id(2), id(3), id(4)] {
        insert_user(&db, tenant_id, user_id).await;
    }
    for user_id in [id(5), id(6)] {
        insert_user(&db, other_tenant_id, user_id).await;
    }

    insert_relation(
        &db,
        id(10),
        tenant_id,
        id(1),
        id(2),
        SocialRelationKind::Follow,
        true,
        1,
    )
    .await;
    insert_relation(
        &db,
        id(20),
        tenant_id,
        id(2),
        id(3),
        SocialRelationKind::Mute,
        false,
        2,
    )
    .await;
    insert_relation(
        &db,
        id(30),
        tenant_id,
        id(3),
        id(4),
        SocialRelationKind::Block,
        true,
        3,
    )
    .await;
    insert_relation(
        &db,
        id(15),
        other_tenant_id,
        id(5),
        id(6),
        SocialRelationKind::Follow,
        true,
        1,
    )
    .await;

    let service = replay_service(db.clone());
    let dry_run = SocialGraphRelationEventMaintenancePort::replay_relation_state_events(
        &service,
        maintenance_context(tenant_id, "relation-event-replay-dry-run"),
        SocialGraphRelationEventReplayCommand {
            after_relation_id: None,
            limit: 2,
            dry_run: true,
        },
    )
    .await
    .expect("dry-run should select the first bounded page");
    assert_eq!(dry_run.selected_relations, 2);
    assert_eq!(dry_run.published_events, 0);
    assert_eq!(dry_run.next_after_relation_id, Some(id(20)));
    assert_eq!(event_count(&db).await, 0);

    let first = SocialGraphRelationEventMaintenancePort::replay_relation_state_events(
        &service,
        maintenance_context(tenant_id, "relation-event-replay-first"),
        SocialGraphRelationEventReplayCommand {
            after_relation_id: None,
            limit: 2,
            dry_run: false,
        },
    )
    .await
    .expect("first replay page should publish atomically");
    assert_eq!(first.selected_relations, 2);
    assert_eq!(first.published_events, 2);
    assert_eq!(first.next_after_relation_id, Some(id(20)));

    let second = SocialGraphRelationEventMaintenancePort::replay_relation_state_events(
        &service,
        maintenance_context(tenant_id, "relation-event-replay-second"),
        SocialGraphRelationEventReplayCommand {
            after_relation_id: first.next_after_relation_id,
            limit: 2,
            dry_run: false,
        },
    )
    .await
    .expect("second replay page should publish the remaining relation");
    assert_eq!(second.selected_relations, 1);
    assert_eq!(second.published_events, 1);
    assert_eq!(second.next_after_relation_id, Some(id(30)));

    let payloads = event_payloads(&db).await;
    assert_eq!(payloads.len(), 3);
    let mut relation_ids = payloads
        .iter()
        .map(|payload| {
            assert_eq!(payload["event_type"], "social_graph.relation.state_changed");
            assert_eq!(payload["tenant_id"], tenant_id.to_string());
            assert!(payload["actor_id"].is_null());
            Uuid::parse_str(
                payload["event"]["event"]["data"]["relation_id"]
                    .as_str()
                    .expect("relation id should be encoded"),
            )
            .expect("relation id should decode")
        })
        .collect::<Vec<_>>();
    relation_ids.sort_unstable();
    assert_eq!(relation_ids, vec![id(10), id(20), id(30)]);
}

#[tokio::test]
async fn replay_rejects_user_actor_and_invalid_limit_without_events() {
    let db = setup(true).await;
    let tenant_id = id(100);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, id(1)).await;
    insert_user(&db, tenant_id, id(2)).await;
    insert_relation(
        &db,
        id(10),
        tenant_id,
        id(1),
        id(2),
        SocialRelationKind::Follow,
        true,
        1,
    )
    .await;
    let service = replay_service(db.clone());

    let user_error = SocialGraphRelationEventMaintenancePort::replay_relation_state_events(
        &service,
        PortContext::new(
            tenant_id.to_string(),
            PortActor::user(id(1).to_string()),
            "en",
            Uuid::new_v4().to_string(),
        )
        .with_deadline(Duration::from_secs(1))
        .with_idempotency_key("relation-event-replay-user"),
        SocialGraphRelationEventReplayCommand {
            after_relation_id: None,
            limit: 1,
            dry_run: false,
        },
    )
    .await
    .expect_err("user actor must not replay owner events");
    assert_eq!(user_error.kind, PortErrorKind::Forbidden);
    assert_eq!(
        user_error.code,
        "social_graph.relation_event_replay_forbidden"
    );

    let limit_error = SocialGraphRelationEventMaintenancePort::replay_relation_state_events(
        &service,
        maintenance_context(tenant_id, "relation-event-replay-limit"),
        SocialGraphRelationEventReplayCommand {
            after_relation_id: None,
            limit: 0,
            dry_run: false,
        },
    )
    .await
    .expect_err("zero replay limit must be rejected");
    assert_eq!(limit_error.kind, PortErrorKind::Validation);
    assert_eq!(
        limit_error.code,
        "social_graph.relation_event_replay_limit_invalid"
    );
    assert_eq!(event_count(&db).await, 0);
}

#[tokio::test]
async fn replay_rolls_back_the_whole_batch_when_one_outbox_insert_fails() {
    let db = setup(true).await;
    let tenant_id = id(100);
    insert_tenant(&db, tenant_id).await;
    for user_id in [id(1), id(2), id(3)] {
        insert_user(&db, tenant_id, user_id).await;
    }
    insert_relation(
        &db,
        id(10),
        tenant_id,
        id(1),
        id(2),
        SocialRelationKind::Follow,
        true,
        1,
    )
    .await;
    insert_relation(
        &db,
        id(20),
        tenant_id,
        id(2),
        id(3),
        SocialRelationKind::Mute,
        true,
        1,
    )
    .await;
    db.execute_unprepared(
        r#"
        CREATE TRIGGER fail_second_social_graph_replay_event
        BEFORE INSERT ON sys_events
        WHEN (SELECT COUNT(*) FROM sys_events) >= 1
        BEGIN
            SELECT RAISE(ABORT, 'fail second replay event');
        END;
        "#,
    )
    .await
    .expect("failure trigger should install");

    let error = SocialGraphRelationEventMaintenancePort::replay_relation_state_events(
        &replay_service(db.clone()),
        maintenance_context(tenant_id, "relation-event-replay-rollback"),
        SocialGraphRelationEventReplayCommand {
            after_relation_id: None,
            limit: 2,
            dry_run: false,
        },
    )
    .await
    .expect_err("one failed insert must roll the complete page back");
    assert_eq!(error.kind, PortErrorKind::Unavailable);
    assert_eq!(error.code, "social_graph.event_publication_unavailable");
    assert_eq!(event_count(&db).await, 0);
}

fn replay_service(db: DatabaseConnection) -> SocialGraphRelationEventMaintenanceService {
    let transport: Arc<dyn EventTransport> = Arc::new(OutboxTransport::new(db.clone()));
    SocialGraphRelationEventMaintenanceService::new(db, TransactionalEventBus::new(transport))
}

fn maintenance_context(tenant_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("social-graph-relation-event-maintenance"),
        "en",
        Uuid::new_v4().to_string(),
    )
    .with_deadline(Duration::from_secs(1))
    .with_idempotency_key(key)
}

async fn setup(include_outbox: bool) -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("SQLite should connect");
    db.execute_unprepared(
        r#"
        PRAGMA foreign_keys = ON;
        CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL);
        CREATE TABLE users (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
        );
        "#,
    )
    .await
    .expect("identity fixture should migrate");
    if include_outbox {
        let manager = SchemaManager::new(&db);
        for migration in OutboxModule.migrations() {
            migration
                .up(&manager)
                .await
                .expect("outbox migration should apply");
        }
    }
    let manager = SchemaManager::new(&db);
    for migration in SocialGraphModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("social graph migration should apply");
    }
    db
}

async fn insert_tenant(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id) VALUES (?)",
        [tenant_id.into()],
    ))
    .await
    .expect("tenant fixture should insert");
}

async fn insert_user(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid) {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO users (id, tenant_id) VALUES (?, ?)",
        [user_id.into(), tenant_id.into()],
    ))
    .await
    .expect("user fixture should insert");
}

async fn insert_relation(
    db: &DatabaseConnection,
    relation_id: Uuid,
    tenant_id: Uuid,
    source_user_id: Uuid,
    target_user_id: Uuid,
    relation_kind: SocialRelationKind,
    active: bool,
    revision: i64,
) {
    let now = Utc::now().fixed_offset();
    relation::ActiveModel {
        id: Set(relation_id),
        tenant_id: Set(tenant_id),
        source_user_id: Set(source_user_id),
        target_user_id: Set(target_user_id),
        relation_kind: Set(relation_kind),
        active: Set(active),
        revision: Set(revision),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await
    .expect("historical relation fixture should insert");
}

async fn event_count(db: &DatabaseConnection) -> i64 {
    db.query_one_raw(Statement::from_string(
        DbBackend::Sqlite,
        "SELECT COUNT(*) AS count FROM sys_events".to_string(),
    ))
    .await
    .expect("event count should query")
    .expect("event count row should exist")
    .try_get::<i64>("", "count")
    .expect("event count should decode")
}

async fn event_payloads(db: &DatabaseConnection) -> Vec<Value> {
    db.query_all_raw(Statement::from_string(
        DbBackend::Sqlite,
        "SELECT payload FROM sys_events ORDER BY created_at, id".to_string(),
    ))
    .await
    .expect("events should query")
    .into_iter()
    .map(|row| {
        row.try_get::<Value>("", "payload")
            .expect("payload should decode")
    })
    .collect()
}

fn id(value: u128) -> Uuid {
    Uuid::from_u128(value)
}
