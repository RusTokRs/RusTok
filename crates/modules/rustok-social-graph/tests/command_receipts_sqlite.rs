mod support;

use std::time::Duration;

use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_core::MigrationSource;
use rustok_social_graph::{
    SetSocialRelationCommand, SocialGraphCommandPort, SocialGraphModule, SocialRelationKind,
};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[tokio::test]
async fn receipt_replays_original_result_and_rejects_payload_reuse() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let source_user_id = Uuid::new_v4();
    let target_user_id = Uuid::new_v4();
    let other_target_user_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, source_user_id).await;
    insert_user(&db, tenant_id, target_user_id).await;
    insert_user(&db, tenant_id, other_target_user_id).await;

    let service = support::write_service(db.clone());
    let follow = command(source_user_id, target_user_id, true, None);
    let first = SocialGraphCommandPort::set_relation(
        &service,
        write_context(tenant_id, source_user_id, "follow-command-1"),
        follow,
    )
    .await
    .expect("first follow should commit");
    assert_eq!(first.revision, 1);
    assert!(first.active);

    let unfollowed = SocialGraphCommandPort::set_relation(
        &service,
        write_context(tenant_id, source_user_id, "follow-command-2"),
        command(source_user_id, target_user_id, false, Some(1)),
    )
    .await
    .expect("unfollow should advance the live relation");
    assert_eq!(unfollowed.revision, 2);
    assert!(!unfollowed.active);

    let replay = SocialGraphCommandPort::set_relation(
        &service,
        write_context(tenant_id, source_user_id, "follow-command-1"),
        follow,
    )
    .await
    .expect("same idempotency identity should replay");
    assert_eq!(replay.id, first.id);
    assert_eq!(replay.revision, 1);
    assert!(
        replay.active,
        "receipt must retain the original command response"
    );

    let current = service
        .relation_state(
            tenant_id,
            source_user_id,
            target_user_id,
            SocialRelationKind::Follow,
        )
        .await
        .expect("current relation should load")
        .expect("relation should exist");
    assert_eq!(current.revision, 2);
    assert!(!current.active, "receipt replay must not rewind live state");

    let conflict = SocialGraphCommandPort::set_relation(
        &service,
        write_context(tenant_id, source_user_id, "follow-command-1"),
        command(source_user_id, other_target_user_id, true, None),
    )
    .await
    .expect_err("one idempotency key must not identify another command");
    assert_eq!(conflict.kind, PortErrorKind::Conflict);
    assert_eq!(conflict.code, "social_graph.idempotency_conflict");

    let receipt_count = db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM social_graph_command_receipts".to_string(),
        ))
        .await
        .expect("receipt count should query")
        .expect("receipt count row should exist")
        .try_get::<i64>("", "count")
        .expect("receipt count should decode");
    assert_eq!(receipt_count, 2);
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

async fn setup() -> DatabaseConnection {
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

    support::migrate_outbox(&db).await;
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
