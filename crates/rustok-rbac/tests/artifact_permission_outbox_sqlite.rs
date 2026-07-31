use std::sync::Arc;

use rustok_outbox::{OutboxTransport, SysEvents, SysEventsMigration, TransactionalEventBus};
use rustok_rbac::{
    ArtifactRolePermissionAssignmentCommand, RbacArtifactPermissionAssignmentService,
};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, EntityTrait, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

async fn setup_database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect sqlite");
    for statement in [
        "CREATE TABLE roles (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL)",
        "CREATE TABLE rbac_artifact_permission_catalog (id TEXT PRIMARY KEY, scope_key TEXT NOT NULL, installation_id TEXT NOT NULL, module_slug TEXT NOT NULL, release_digest TEXT NOT NULL, permission_key TEXT NOT NULL, locale TEXT NOT NULL, label TEXT NOT NULL, description TEXT NOT NULL, registered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (scope_key, installation_id, permission_key, locale))",
        "CREATE TABLE rbac_artifact_role_permissions (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, role_id TEXT NOT NULL, installation_id TEXT NOT NULL, permission_key TEXT NOT NULL, granted_by_actor_id TEXT NOT NULL, granted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (tenant_id, role_id, installation_id, permission_key))",
        "CREATE TABLE rbac_artifact_role_permission_operations (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, role_id TEXT NOT NULL, installation_id TEXT NOT NULL, permission_key TEXT NOT NULL, actor_id TEXT NOT NULL, granted BOOLEAN NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (tenant_id, idempotency_key))",
    ] {
        db.execute_unprepared(statement)
            .await
            .expect("create RBAC fixture table");
    }
    SysEventsMigration
        .up(&SchemaManager::new(&db))
        .await
        .expect("create outbox table");
    db
}

async fn insert_scope(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    role_id: Uuid,
    installation_id: Uuid,
    permission_key: &str,
) {
    db.execute(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO roles (id, tenant_id) VALUES (?1, ?2)",
        vec![role_id.into(), tenant_id.into()],
    ))
    .await
    .expect("insert role");
    db.execute(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO rbac_artifact_permission_catalog (id, scope_key, installation_id, module_slug, release_digest, permission_key, locale, label, description) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        vec![
            Uuid::new_v4().into(),
            format!("tenant:{tenant_id}").into(),
            installation_id.into(),
            "sample".into(),
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            permission_key.into(),
            "en".into(),
            "Handle events".into(),
            "Allows the role to handle sample events".into(),
        ],
    ))
    .await
    .expect("insert artifact permission catalog row");
}

fn command(
    tenant_id: Uuid,
    role_id: Uuid,
    installation_id: Uuid,
    actor_id: Uuid,
    permission_key: &str,
    granted: bool,
    idempotency_key: &str,
) -> ArtifactRolePermissionAssignmentCommand {
    ArtifactRolePermissionAssignmentCommand {
        tenant_id,
        role_id,
        installation_id,
        permission_key: permission_key.to_string(),
        actor_id,
        granted,
        idempotency_key: idempotency_key.to_string(),
    }
}

#[tokio::test]
async fn grant_retry_and_revoke_publish_exactly_once_per_applied_operation() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let installation_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let permission_key = "sample.events.handle";
    insert_scope(&db, tenant_id, role_id, installation_id, permission_key).await;

    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = RbacArtifactPermissionAssignmentService::new(db.clone(), event_bus);
    let grant = command(
        tenant_id,
        role_id,
        installation_id,
        actor_id,
        permission_key,
        true,
        "grant-1",
    );

    assert!(service.assign(grant.clone()).await.expect("grant").applied);
    assert!(!service.assign(grant).await.expect("exact retry").applied);

    let after_retry = SysEvents::find().all(&db).await.expect("load outbox");
    assert_eq!(after_retry.len(), 1, "exact retry must not emit twice");
    assert_eq!(
        after_retry[0].event_type,
        "rbac.artifact_role_permission.assignment_changed"
    );
    assert_eq!(after_retry[0].schema_version, 1);
    assert_eq!(
        after_retry[0].payload["tenant_id"],
        serde_json::json!(tenant_id)
    );
    assert_eq!(
        after_retry[0].payload["actor_id"],
        serde_json::json!(actor_id)
    );
    assert_eq!(
        after_retry[0].payload["event"]["event"]["data"]["granted"],
        serde_json::json!(true)
    );

    let revoke = command(
        tenant_id,
        role_id,
        installation_id,
        actor_id,
        permission_key,
        false,
        "revoke-1",
    );
    assert!(service.assign(revoke).await.expect("revoke").applied);

    let after_revoke = SysEvents::find().all(&db).await.expect("load outbox");
    assert_eq!(after_revoke.len(), 2);
    assert_eq!(
        after_revoke[1].payload["event"]["event"]["data"]["granted"],
        serde_json::json!(false)
    );
}
