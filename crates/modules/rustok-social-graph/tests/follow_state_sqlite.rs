mod support;

use std::time::Duration;

use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_core::MigrationSource;
use rustok_social_graph::{
    SetSocialRelationCommand, SocialGraphCommandPort, SocialGraphFollowReadPort, SocialGraphModule,
    SocialGraphPairRequest, SocialRelationKind,
};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[tokio::test]
async fn follow_state_read_retains_revision_for_conflict_recovery() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let target_id = Uuid::new_v4();
    let other_id = Uuid::new_v4();
    insert_identity(&db, tenant_id, actor_id).await;
    insert_identity(&db, tenant_id, target_id).await;
    insert_identity(&db, tenant_id, other_id).await;

    let service = support::write_service(db);
    let initial = SocialGraphFollowReadPort::source_follow_state(
        &service,
        user_context(tenant_id, actor_id, "follow-state-initial"),
        SocialGraphPairRequest {
            source_user_id: actor_id,
            target_user_id: target_id,
        },
    )
    .await
    .expect("missing relation should resolve as an inactive state");
    assert!(!initial.following);
    assert_eq!(initial.revision, None);

    let followed = SocialGraphCommandPort::set_relation(
        &service,
        user_context(tenant_id, actor_id, "follow-state-create"),
        SetSocialRelationCommand {
            source_user_id: actor_id,
            target_user_id: target_id,
            relation_kind: SocialRelationKind::Follow,
            active: true,
            expected_revision: None,
        },
    )
    .await
    .expect("follow relation should be created");
    assert_eq!(followed.revision, 1);

    let active = SocialGraphFollowReadPort::source_follow_state(
        &service,
        user_context(tenant_id, actor_id, "follow-state-active"),
        SocialGraphPairRequest {
            source_user_id: actor_id,
            target_user_id: target_id,
        },
    )
    .await
    .expect("active state should resolve");
    assert!(active.following);
    assert_eq!(active.revision, Some(1));

    SocialGraphCommandPort::set_relation(
        &service,
        user_context(tenant_id, actor_id, "follow-state-deactivate"),
        SetSocialRelationCommand {
            source_user_id: actor_id,
            target_user_id: target_id,
            relation_kind: SocialRelationKind::Follow,
            active: false,
            expected_revision: active.revision,
        },
    )
    .await
    .expect("unfollow should accept the recovered revision");

    let inactive = SocialGraphFollowReadPort::source_follow_state(
        &service,
        user_context(tenant_id, actor_id, "follow-state-inactive"),
        SocialGraphPairRequest {
            source_user_id: actor_id,
            target_user_id: target_id,
        },
    )
    .await
    .expect("inactive relation should retain its revision");
    assert!(!inactive.following);
    assert_eq!(inactive.revision, Some(2));

    let mismatch = SocialGraphFollowReadPort::source_follow_state(
        &service,
        user_context(tenant_id, other_id, "follow-state-mismatch"),
        SocialGraphPairRequest {
            source_user_id: actor_id,
            target_user_id: target_id,
        },
    )
    .await
    .expect_err("user actors must own the follow source");
    assert_eq!(mismatch.kind, PortErrorKind::Forbidden);
}

async fn setup() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("SQLite should connect");
    db.execute_unprepared(
        "PRAGMA foreign_keys = ON; CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL); CREATE TABLE users (id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL, FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE);",
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

async fn insert_identity(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid) {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT OR IGNORE INTO tenants (id) VALUES (?)",
        [tenant_id.into()],
    ))
    .await
    .expect("tenant should insert");
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO users (id, tenant_id) VALUES (?, ?)",
        [user_id.into(), tenant_id.into()],
    ))
    .await
    .expect("user should insert");
}

fn user_context(tenant_id: Uuid, actor_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        Uuid::new_v4().to_string(),
    )
    .with_deadline(Duration::from_secs(1))
    .with_idempotency_key(key)
}
