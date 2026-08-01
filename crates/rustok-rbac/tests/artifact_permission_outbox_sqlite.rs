use std::sync::Arc;

use async_trait::async_trait;
use rustok_events::RbacArtifactPermissionEvent;
use rustok_rbac::{
    ArtifactPermissionAssignmentError, ArtifactPermissionEventPublisher,
    ArtifactRolePermissionAssignmentCommand, RbacArtifactPermissionAssignmentService,
};
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, Statement,
};
use uuid::Uuid;

#[derive(Clone)]
struct SqliteArtifactPermissionEventPublisher {
    fail: bool,
}

#[async_trait]
impl ArtifactPermissionEventPublisher for SqliteArtifactPermissionEventPublisher {
    async fn publish_assignment_changed(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        event: RbacArtifactPermissionEvent,
    ) -> Result<(), ArtifactPermissionAssignmentError> {
        if self.fail {
            return Err(ArtifactPermissionAssignmentError::Database(
                "test publisher rejected the event".to_string(),
            ));
        }

        let event_type = event.event_type();
        let schema_version = event.schema_version();
        let RbacArtifactPermissionEvent::AssignmentChanged {
            operation_id,
            role_id,
            installation_id,
            permission_key,
            granted,
        } = event;
        transaction
            .execute(Statement::from_sql_and_values(
                transaction.get_database_backend(),
                "INSERT INTO rbac_artifact_permission_events (operation_id, tenant_id, actor_id, role_id, installation_id, permission_key, granted, event_type, schema_version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                vec![
                    operation_id.into(),
                    tenant_id.into(),
                    actor_id.into(),
                    role_id.into(),
                    installation_id.into(),
                    permission_key.into(),
                    granted.into(),
                    event_type.into(),
                    i32::from(schema_version).into(),
                ],
            ))
            .await
            .map_err(|error| ArtifactPermissionAssignmentError::Database(error.to_string()))?;
        Ok(())
    }
}

async fn setup_database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect sqlite");
    for statement in [
        "CREATE TABLE roles (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL)",
        "CREATE TABLE rbac_artifact_permission_catalog (id TEXT PRIMARY KEY, scope_key TEXT NOT NULL, installation_id TEXT NOT NULL, module_slug TEXT NOT NULL, release_digest TEXT NOT NULL, permission_key TEXT NOT NULL, locale TEXT NOT NULL, label TEXT NOT NULL, description TEXT NOT NULL, registered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (scope_key, installation_id, permission_key, locale))",
        "CREATE TABLE rbac_artifact_role_permissions (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, role_id TEXT NOT NULL, installation_id TEXT NOT NULL, permission_key TEXT NOT NULL, granted_by_actor_id TEXT NOT NULL, granted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (tenant_id, role_id, installation_id, permission_key))",
        "CREATE TABLE rbac_artifact_role_permission_operations (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, role_id TEXT NOT NULL, installation_id TEXT NOT NULL, permission_key TEXT NOT NULL, actor_id TEXT NOT NULL, granted BOOLEAN NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (tenant_id, idempotency_key))",
        "CREATE TABLE rbac_artifact_permission_events (operation_id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, actor_id TEXT NOT NULL, role_id TEXT NOT NULL, installation_id TEXT NOT NULL, permission_key TEXT NOT NULL, granted BOOLEAN NOT NULL, event_type TEXT NOT NULL, schema_version INTEGER NOT NULL)",
    ] {
        db.execute_unprepared(statement)
            .await
            .expect("create RBAC fixture table");
    }
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

async fn table_count(db: &DatabaseConnection, table: &str) -> i64 {
    db.query_one(Statement::from_string(
        db.get_database_backend(),
        format!("SELECT COUNT(*) AS count FROM {table}"),
    ))
    .await
    .expect("count table")
    .expect("count row")
    .try_get("", "count")
    .expect("decode count")
}

async fn event_grants(db: &DatabaseConnection) -> Vec<bool> {
    db.query_all(Statement::from_string(
        db.get_database_backend(),
        "SELECT granted FROM rbac_artifact_permission_events ORDER BY rowid".to_string(),
    ))
    .await
    .expect("load published events")
    .into_iter()
    .map(|row| row.try_get("", "granted").expect("decode granted"))
    .collect()
}

#[tokio::test]
async fn only_state_changes_publish_artifact_permission_events() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let installation_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let permission_key = "sample.events.handle";
    insert_scope(&db, tenant_id, role_id, installation_id, permission_key).await;

    let service = RbacArtifactPermissionAssignmentService::new(
        db.clone(),
        Arc::new(SqliteArtifactPermissionEventPublisher { fail: false }),
    );
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
    assert!(
        service
            .assign(command(
                tenant_id,
                role_id,
                installation_id,
                actor_id,
                permission_key,
                true,
                "grant-confirmation",
            ))
            .await
            .expect("grant state confirmation")
            .applied
    );
    assert_eq!(event_grants(&db).await, vec![true]);

    assert!(
        service
            .assign(command(
                tenant_id,
                role_id,
                installation_id,
                actor_id,
                permission_key,
                false,
                "revoke-1",
            ))
            .await
            .expect("revoke")
            .applied
    );
    assert!(
        service
            .assign(command(
                tenant_id,
                role_id,
                installation_id,
                actor_id,
                permission_key,
                false,
                "revoke-confirmation",
            ))
            .await
            .expect("revoke state confirmation")
            .applied
    );
    assert_eq!(event_grants(&db).await, vec![true, false]);
}

#[tokio::test]
async fn publication_failure_rolls_back_grant_and_idempotency_receipt() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let installation_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let permission_key = "sample.events.handle";
    insert_scope(&db, tenant_id, role_id, installation_id, permission_key).await;

    let service = RbacArtifactPermissionAssignmentService::new(
        db.clone(),
        Arc::new(SqliteArtifactPermissionEventPublisher { fail: true }),
    );
    let error = service
        .assign(command(
            tenant_id,
            role_id,
            installation_id,
            actor_id,
            permission_key,
            true,
            "grant-without-publication",
        ))
        .await
        .expect_err("publisher failure must fail closed");

    assert!(matches!(
        error,
        ArtifactPermissionAssignmentError::Database(_)
    ));
    assert_eq!(table_count(&db, "rbac_artifact_role_permissions").await, 0);
    assert_eq!(
        table_count(&db, "rbac_artifact_role_permission_operations").await,
        0
    );
    assert_eq!(table_count(&db, "rbac_artifact_permission_events").await, 0);
}
