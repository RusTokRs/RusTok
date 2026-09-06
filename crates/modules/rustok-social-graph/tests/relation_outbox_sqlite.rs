use std::sync::Arc;
use std::time::Duration;

use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_core::MigrationSource;
use rustok_core::events::EventTransport;
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_social_graph::{
    SetSocialRelationCommand, SocialGraphCommandPort, SocialGraphModule, SocialGraphService,
    SocialRelationKind,
};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use serde_json::Value;
use uuid::Uuid;

#[tokio::test]
async fn relation_changes_publish_once_while_noop_and_replay_do_not() {
    let db = setup(true).await;
    let tenant_id = Uuid::new_v4();
    let source_user_id = Uuid::new_v4();
    let target_user_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, source_user_id).await;
    insert_user(&db, tenant_id, target_user_id).await;
    let service = write_service(db.clone());

    let first = SocialGraphCommandPort::set_relation(
        &service,
        write_context(tenant_id, source_user_id, "relation-event-follow"),
        command(source_user_id, target_user_id, true, None),
    )
    .await
    .expect("follow should commit with an outbox event");
    assert_eq!(first.revision, 1);
    assert_eq!(event_count(&db).await, 1);

    let noop = SocialGraphCommandPort::set_relation(
        &service,
        write_context(tenant_id, source_user_id, "relation-event-noop"),
        command(source_user_id, target_user_id, true, Some(1)),
    )
    .await
    .expect("exact state no-op should complete its receipt");
    assert_eq!(noop.revision, 1);
    assert_eq!(event_count(&db).await, 1, "no-op must not emit an event");

    let second = SocialGraphCommandPort::set_relation(
        &service,
        write_context(tenant_id, source_user_id, "relation-event-unfollow"),
        command(source_user_id, target_user_id, false, Some(1)),
    )
    .await
    .expect("unfollow should commit with a second event");
    assert_eq!(second.revision, 2);
    assert_eq!(event_count(&db).await, 2);

    let replay = SocialGraphCommandPort::set_relation(
        &service,
        write_context(tenant_id, source_user_id, "relation-event-follow"),
        command(source_user_id, target_user_id, true, None),
    )
    .await
    .expect("receipt replay should return the original response");
    assert_eq!(replay.revision, 1);
    assert_eq!(event_count(&db).await, 2, "replay must not emit an event");

    let events = event_payloads(&db).await;
    assert_eq!(events.len(), 2);
    assert_event(
        &events[0],
        tenant_id,
        source_user_id,
        target_user_id,
        true,
        1,
    );
    assert_event(
        &events[1],
        tenant_id,
        source_user_id,
        target_user_id,
        false,
        2,
    );
}

#[tokio::test]
async fn missing_transactional_outbox_rolls_back_relation_and_receipt() {
    let db = setup(false).await;
    let tenant_id = Uuid::new_v4();
    let source_user_id = Uuid::new_v4();
    let target_user_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, source_user_id).await;
    insert_user(&db, tenant_id, target_user_id).await;

    let error = SocialGraphCommandPort::set_relation(
        &write_service(db.clone()),
        write_context(tenant_id, source_user_id, "relation-event-missing-outbox"),
        command(source_user_id, target_user_id, true, None),
    )
    .await
    .expect_err("missing sys_events storage must fail closed");
    assert_eq!(error.kind, PortErrorKind::Unavailable);
    assert_eq!(error.code, "social_graph.event_publication_unavailable");
    assert_eq!(table_count(&db, "social_graph_relations").await, 0);
    assert_eq!(table_count(&db, "social_graph_command_receipts").await, 0);
}

fn write_service(db: DatabaseConnection) -> SocialGraphService {
    let transport: Arc<dyn EventTransport> = Arc::new(OutboxTransport::new(db.clone()));
    SocialGraphService::with_event_bus(db, TransactionalEventBus::new(transport))
}

fn command(
    source_user_id: Uuid,
    target_user_id: Uuid,
    active: bool,
    expected_revision: Option<i64>,
) -> SetSocialRelationCommand {
    SetSocialRelationCommand {
        source_user_id,
        target_user_id,
        relation_kind: SocialRelationKind::Follow,
        active,
        expected_revision,
    }
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
    let manager = SchemaManager::new(&db);
    if include_outbox {
        for migration in OutboxModule.migrations() {
            migration
                .up(&manager)
                .await
                .expect("outbox migration should apply");
        }
    }
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

fn write_context(tenant_id: Uuid, source_user_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(source_user_id.to_string()),
        "en",
        Uuid::new_v4().to_string(),
    )
    .with_deadline(Duration::from_secs(1))
    .with_idempotency_key(key)
}

async fn event_count(db: &DatabaseConnection) -> i64 {
    table_count(db, "sys_events").await
}

async fn table_count(db: &DatabaseConnection, table: &str) -> i64 {
    db.query_one_raw(Statement::from_string(
        DbBackend::Sqlite,
        format!("SELECT COUNT(*) AS count FROM {table}"),
    ))
    .await
    .expect("count should query")
    .expect("count row should exist")
    .try_get::<i64>("", "count")
    .expect("count should decode")
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

fn assert_event(
    payload: &Value,
    tenant_id: Uuid,
    source_user_id: Uuid,
    target_user_id: Uuid,
    active: bool,
    revision: i64,
) {
    assert_eq!(payload["event_type"], "social_graph.relation.state_changed");
    assert_eq!(payload["schema_version"], 1);
    assert_eq!(payload["tenant_id"], tenant_id.to_string());
    assert_eq!(payload["actor_id"], source_user_id.to_string());
    assert_eq!(payload["event"]["family"], "social_graph_relation");
    assert_eq!(payload["event"]["event"]["type"], "RelationStateChanged");
    let data = &payload["event"]["event"]["data"];
    assert_eq!(data["source_user_id"], source_user_id.to_string());
    assert_eq!(data["target_user_id"], target_user_id.to_string());
    assert_eq!(data["relation_kind"], "follow");
    assert_eq!(data["active"], active);
    assert_eq!(data["revision"], revision);

    let encoded = payload.to_string();
    for forbidden in [
        "idempotency_key",
        "expected_revision",
        "request_json",
        "response_json",
        "claims",
        "roles",
        "locale",
        "channel",
    ] {
        assert!(
            !encoded.contains(forbidden),
            "outbox payload leaked {forbidden}"
        );
    }
}
