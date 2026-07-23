use std::time::Duration;

use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_core::MigrationSource;
use rustok_social_graph::{
    SetSocialRelationCommand, SocialGraphCommandPort, SocialGraphModule, SocialGraphPairRequest,
    SocialGraphPrivacyReadPort, SocialGraphService, SocialRelationKind,
};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[tokio::test]
async fn block_and_mute_state_is_tenant_scoped_and_replay_safe() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let recipient_id = Uuid::new_v4();
    let foreign_user_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    insert_tenant(&db, other_tenant_id).await;
    insert_user(&db, tenant_id, actor_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    insert_user(&db, other_tenant_id, foreign_user_id).await;

    let service = SocialGraphService::new(db.clone());
    let block = SocialGraphCommandPort::set_relation(
        &service,
        write_context(tenant_id, actor_id, "block-create"),
        SetSocialRelationCommand {
            source_user_id: actor_id,
            target_user_id: recipient_id,
            relation_kind: SocialRelationKind::Block,
            active: true,
            expected_revision: None,
        },
    )
    .await
    .expect("block relation should be created");
    assert_eq!(block.revision, 1);
    assert!(block.active);

    let replay = SocialGraphCommandPort::set_relation(
        &service,
        write_context(tenant_id, actor_id, "block-replay"),
        SetSocialRelationCommand {
            source_user_id: actor_id,
            target_user_id: recipient_id,
            relation_kind: SocialRelationKind::Block,
            active: true,
            expected_revision: Some(1),
        },
    )
    .await
    .expect("same semantic block state should replay");
    assert_eq!(replay.id, block.id);
    assert_eq!(replay.revision, 1);

    let blocked = SocialGraphPrivacyReadPort::blocks_between(
        &service,
        read_context(tenant_id),
        SocialGraphPairRequest {
            source_user_id: recipient_id,
            target_user_id: actor_id,
        },
    )
    .await
    .expect("reverse block lookup should succeed");
    assert!(blocked, "block privacy is strict in either direction");

    let unblocked = SocialGraphCommandPort::set_relation(
        &service,
        write_context(tenant_id, actor_id, "block-remove"),
        SetSocialRelationCommand {
            source_user_id: actor_id,
            target_user_id: recipient_id,
            relation_kind: SocialRelationKind::Block,
            active: false,
            expected_revision: Some(1),
        },
    )
    .await
    .expect("block state should be deactivated");
    assert_eq!(unblocked.revision, 2);
    assert!(!unblocked.active);

    SocialGraphCommandPort::set_relation(
        &service,
        write_context(tenant_id, recipient_id, "mute-create"),
        SetSocialRelationCommand {
            source_user_id: recipient_id,
            target_user_id: actor_id,
            relation_kind: SocialRelationKind::Mute,
            active: true,
            expected_revision: None,
        },
    )
    .await
    .expect("mute relation should be created");
    assert!(
        SocialGraphPrivacyReadPort::source_mutes_target(
            &service,
            read_context(tenant_id),
            SocialGraphPairRequest {
                source_user_id: recipient_id,
                target_user_id: actor_id,
            },
        )
        .await
        .expect("mute lookup should succeed")
    );
    assert!(
        !SocialGraphPrivacyReadPort::source_mutes_target(
            &service,
            read_context(tenant_id),
            SocialGraphPairRequest {
                source_user_id: actor_id,
                target_user_id: recipient_id,
            },
        )
        .await
        .expect("opposite mute lookup should succeed"),
        "mute remains directional"
    );

    let cross_tenant = SocialGraphCommandPort::set_relation(
        &service,
        write_context(tenant_id, actor_id, "cross-tenant"),
        SetSocialRelationCommand {
            source_user_id: actor_id,
            target_user_id: foreign_user_id,
            relation_kind: SocialRelationKind::Block,
            active: true,
            expected_revision: None,
        },
    )
    .await
    .expect_err("tenant-composite foreign key must reject foreign users");
    assert_eq!(cross_tenant.kind, PortErrorKind::Unavailable);
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

    let module = SocialGraphModule;
    let manager = SchemaManager::new(&db);
    for migration in module.migrations() {
        migration
            .up(&manager)
            .await
            .expect("social graph migration should apply");
    }
    db
}

async fn insert_tenant(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id) VALUES (?)",
        [tenant_id.into()],
    ))
    .await
    .expect("tenant fixture should insert");
}

async fn insert_user(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid) {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO users (id, tenant_id) VALUES (?, ?)",
        [user_id.into(), tenant_id.into()],
    ))
    .await
    .expect("user fixture should insert");
}

fn read_context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("social-graph-test"),
        "en",
        Uuid::new_v4().to_string(),
    )
    .with_deadline(Duration::from_secs(1))
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
