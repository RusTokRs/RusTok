mod support;

use std::time::Duration;

use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_core::MigrationSource;
use rustok_social_graph::{
    MAX_SOCIAL_GRAPH_FOLLOW_TARGETS, SetSocialRelationCommand, SocialGraphCommandPort,
    SocialGraphFollowBatchRequest, SocialGraphModule, SocialGraphPairRequest,
    SocialGraphPrivacyReadPort, SocialRelationKind,
};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[tokio::test]
async fn follow_reads_are_directional_batched_and_actor_bound() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let followed_id = Uuid::new_v4();
    let other_id = Uuid::new_v4();
    insert_identity(&db, tenant_id, actor_id).await;
    insert_identity(&db, tenant_id, followed_id).await;
    insert_identity(&db, tenant_id, other_id).await;

    let service = support::write_service(db);
    SocialGraphCommandPort::set_relation(
        &service,
        user_context(tenant_id, actor_id, "follow-create"),
        SetSocialRelationCommand {
            source_user_id: actor_id,
            target_user_id: followed_id,
            relation_kind: SocialRelationKind::Follow,
            active: true,
            expected_revision: None,
        },
    )
    .await
    .expect("follow relation should be created");

    assert!(
        SocialGraphPrivacyReadPort::source_follows_target(
            &service,
            user_context(tenant_id, actor_id, "follow-read"),
            SocialGraphPairRequest {
                source_user_id: actor_id,
                target_user_id: followed_id,
            },
        )
        .await
        .expect("owned follow read should succeed")
    );
    assert!(
        !SocialGraphPrivacyReadPort::source_follows_target(
            &service,
            service_context(tenant_id),
            SocialGraphPairRequest {
                source_user_id: followed_id,
                target_user_id: actor_id,
            },
        )
        .await
        .expect("reverse follow read should succeed")
    );

    let batch = SocialGraphPrivacyReadPort::source_follows_targets(
        &service,
        user_context(tenant_id, actor_id, "follow-batch"),
        SocialGraphFollowBatchRequest {
            source_user_id: actor_id,
            target_user_ids: vec![other_id, followed_id, followed_id],
        },
    )
    .await
    .expect("bounded follow batch should succeed");
    assert_eq!(batch.followed_target_user_ids, vec![followed_id]);

    let mismatch = SocialGraphPrivacyReadPort::source_follows_targets(
        &service,
        user_context(tenant_id, other_id, "follow-mismatch"),
        SocialGraphFollowBatchRequest {
            source_user_id: actor_id,
            target_user_ids: vec![followed_id],
        },
    )
    .await
    .expect_err("user actor must own the follow source");
    assert_eq!(mismatch.kind, PortErrorKind::Forbidden);

    let oversized = SocialGraphPrivacyReadPort::source_follows_targets(
        &service,
        user_context(tenant_id, actor_id, "follow-oversized"),
        SocialGraphFollowBatchRequest {
            source_user_id: actor_id,
            target_user_ids: (0..=MAX_SOCIAL_GRAPH_FOLLOW_TARGETS)
                .map(|_| Uuid::new_v4())
                .collect(),
        },
    )
    .await
    .expect_err("follow batch must enforce its target cap");
    assert_eq!(oversized.kind, PortErrorKind::Validation);
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

fn service_context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("follow-test"),
        "en",
        Uuid::new_v4().to_string(),
    )
    .with_deadline(Duration::from_secs(1))
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
