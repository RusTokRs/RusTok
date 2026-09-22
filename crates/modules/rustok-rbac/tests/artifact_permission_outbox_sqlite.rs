use std::sync::Arc;

use async_trait::async_trait;
use rustok_events::RbacArtifactPermissionEvent;
use rustok_rbac::{
    ArtifactPermissionAssignmentError, ArtifactPermissionAssignmentScope,
    ArtifactPermissionEventPublisher, ArtifactRolePermissionAssignmentCommand,
    RbacArtifactPermissionAssignmentService,
};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, Statement};
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
            artifact_permission_id,
            role_id,
            installation_id,
            permission_key,
            granted,
        } = event;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                transaction.get_database_backend(),
                "INSERT INTO rbac_artifact_permission_events (operation_id, tenant_id, actor_id, artifact_permission_id, role_id, installation_id, permission_key, granted, event_type, schema_version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                vec![
                    operation_id.into(),
                    tenant_id.into(),
                    actor_id.into(),
                    artifact_permission_id.into(),
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
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("enable SQLite foreign keys");
    for statement in [
        "CREATE TABLE roles (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, UNIQUE (tenant_id, id))",
        "CREATE TABLE users (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, UNIQUE (tenant_id, id))",
        "CREATE TABLE rbac_artifact_permission_installations (installation_id TEXT PRIMARY KEY, scope_key TEXT NOT NULL, module_slug TEXT NOT NULL, release_digest TEXT NOT NULL, registered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (installation_id, scope_key, module_slug, release_digest))",
        "CREATE TABLE rbac_artifact_permission_definitions (id TEXT PRIMARY KEY, scope_key TEXT NOT NULL, installation_id TEXT NOT NULL, module_slug TEXT NOT NULL, release_digest TEXT NOT NULL, permission_key TEXT NOT NULL, registered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, FOREIGN KEY (installation_id, scope_key, module_slug, release_digest) REFERENCES rbac_artifact_permission_installations (installation_id, scope_key, module_slug, release_digest) ON UPDATE RESTRICT ON DELETE RESTRICT, UNIQUE (id, scope_key), UNIQUE (scope_key, installation_id, permission_key))",
        "CREATE TABLE rbac_artifact_role_permissions (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, role_id TEXT NOT NULL, artifact_permission_id TEXT NOT NULL, permission_scope_key TEXT NOT NULL, granted_by_actor_id TEXT NOT NULL, granted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, CHECK (permission_scope_key = 'platform' OR permission_scope_key = 'tenant:' || tenant_id), FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, FOREIGN KEY (tenant_id, granted_by_actor_id) REFERENCES users (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, FOREIGN KEY (artifact_permission_id, permission_scope_key) REFERENCES rbac_artifact_permission_definitions (id, scope_key) ON UPDATE RESTRICT ON DELETE RESTRICT, UNIQUE (tenant_id, role_id, artifact_permission_id))",
        "CREATE TABLE rbac_artifact_role_permission_operations (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, role_id TEXT NOT NULL, artifact_permission_id TEXT NOT NULL, permission_scope_key TEXT NOT NULL, actor_id TEXT NOT NULL, granted BOOLEAN NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, CHECK (permission_scope_key = 'platform' OR permission_scope_key = 'tenant:' || tenant_id), FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, FOREIGN KEY (tenant_id, actor_id) REFERENCES users (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, FOREIGN KEY (artifact_permission_id, permission_scope_key) REFERENCES rbac_artifact_permission_definitions (id, scope_key) ON UPDATE RESTRICT ON DELETE RESTRICT, UNIQUE (tenant_id, idempotency_key))",
        "CREATE TABLE rbac_artifact_permission_events (operation_id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, actor_id TEXT NOT NULL, artifact_permission_id TEXT NOT NULL, role_id TEXT NOT NULL, installation_id TEXT NOT NULL, permission_key TEXT NOT NULL, granted BOOLEAN NOT NULL, event_type TEXT NOT NULL, schema_version INTEGER NOT NULL)",
    ] {
        db.execute_unprepared(statement)
            .await
            .expect("create RBAC fixture table");
    }
    db
}

async fn insert_role_and_actor(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    role_id: Uuid,
    actor_id: Uuid,
) {
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO roles (id, tenant_id) VALUES (?1, ?2)",
        vec![role_id.to_string().into(), tenant_id.to_string().into()],
    ))
    .await
    .expect("insert role");
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO users (id, tenant_id) VALUES (?1, ?2)",
        vec![actor_id.to_string().into(), tenant_id.to_string().into()],
    ))
    .await
    .expect("insert actor");
}

async fn insert_definition(
    db: &DatabaseConnection,
    scope_key: &str,
    installation_id: Uuid,
    permission_key: &str,
) -> Uuid {
    let artifact_permission_id = Uuid::new_v4();
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO rbac_artifact_permission_installations (installation_id, scope_key, module_slug, release_digest) VALUES (?1, ?2, ?3, ?4) ON CONFLICT (installation_id) DO NOTHING",
        vec![
            installation_id.to_string().into(),
            scope_key.into(),
            "sample".into(),
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
        ],
    ))
    .await
    .expect("insert artifact permission installation identity");
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO rbac_artifact_permission_definitions (id, scope_key, installation_id, module_slug, release_digest, permission_key) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        vec![
            artifact_permission_id.to_string().into(),
            scope_key.into(),
            installation_id.to_string().into(),
            "sample".into(),
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            permission_key.into(),
        ],
    ))
    .await
    .expect("insert artifact permission definition");
    artifact_permission_id
}

async fn insert_corrupt_parallel_definition(
    db: &DatabaseConnection,
    scope_key: &str,
    installation_id: Uuid,
    permission_key: &str,
) -> Uuid {
    db.execute_unprepared("PRAGMA foreign_keys = OFF")
        .await
        .expect("disable foreign keys for corruption fixture");
    let artifact_permission_id = Uuid::new_v4();
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO rbac_artifact_permission_definitions (id, scope_key, installation_id, module_slug, release_digest, permission_key) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        vec![
            artifact_permission_id.to_string().into(),
            scope_key.into(),
            installation_id.to_string().into(),
            "sample".into(),
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            permission_key.into(),
        ],
    ))
    .await
    .expect("insert corrupt parallel definition");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("restore foreign keys after corruption fixture");
    artifact_permission_id
}

#[allow(clippy::too_many_arguments)]
fn command(
    tenant_id: Uuid,
    role_id: Uuid,
    scope: ArtifactPermissionAssignmentScope,
    installation_id: Uuid,
    permission_key: &str,
    actor_id: Uuid,
    granted: bool,
    idempotency_key: &str,
) -> ArtifactRolePermissionAssignmentCommand {
    ArtifactRolePermissionAssignmentCommand {
        tenant_id,
        role_id,
        scope,
        installation_id,
        permission_key: permission_key.to_string(),
        actor_id,
        granted,
        idempotency_key: idempotency_key.to_string(),
    }
}

async fn table_count(db: &DatabaseConnection, table: &str) -> i64 {
    db.query_one_raw(Statement::from_string(
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
    db.query_all_raw(Statement::from_string(
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
    insert_role_and_actor(&db, tenant_id, role_id, actor_id).await;
    let artifact_permission_id = insert_definition(
        &db,
        &format!("tenant:{tenant_id}"),
        installation_id,
        permission_key,
    )
    .await;

    let service = RbacArtifactPermissionAssignmentService::new(
        db.clone(),
        Arc::new(SqliteArtifactPermissionEventPublisher { fail: false }),
    );
    let grant = command(
        tenant_id,
        role_id,
        ArtifactPermissionAssignmentScope::Tenant,
        installation_id,
        permission_key,
        actor_id,
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
                ArtifactPermissionAssignmentScope::Tenant,
                installation_id,
                permission_key,
                actor_id,
                true,
                "grant-confirmation",
            ))
            .await
            .expect("grant state confirmation")
            .applied
    );
    assert_eq!(event_grants(&db).await, vec![true]);
    let persisted_scope: String = db
        .query_one_raw(Statement::from_string(
            db.get_database_backend(),
            "SELECT permission_scope_key FROM rbac_artifact_role_permissions LIMIT 1".to_string(),
        ))
        .await
        .expect("query grant")
        .expect("grant row")
        .try_get("", "permission_scope_key")
        .expect("decode permission scope");
    assert_eq!(persisted_scope, format!("tenant:{tenant_id}"));

    let event_permission_id: Uuid = db
        .query_one_raw(Statement::from_string(
            db.get_database_backend(),
            "SELECT artifact_permission_id FROM rbac_artifact_permission_events LIMIT 1"
                .to_string(),
        ))
        .await
        .expect("query event")
        .expect("event row")
        .try_get("", "artifact_permission_id")
        .expect("decode artifact permission identity");
    assert_eq!(event_permission_id, artifact_permission_id);

    assert!(
        service
            .assign(command(
                tenant_id,
                role_id,
                ArtifactPermissionAssignmentScope::Tenant,
                installation_id,
                permission_key,
                actor_id,
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
                ArtifactPermissionAssignmentScope::Tenant,
                installation_id,
                permission_key,
                actor_id,
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
async fn explicit_scope_mutation_does_not_shadow_platform_or_tenant_definition() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let installation_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let permission_key = "sample.events.handle";
    insert_role_and_actor(&db, tenant_id, role_id, actor_id).await;
    let platform_permission_id =
        insert_definition(&db, "platform", installation_id, permission_key).await;
    let tenant_permission_id = insert_corrupt_parallel_definition(
        &db,
        &format!("tenant:{tenant_id}"),
        installation_id,
        permission_key,
    )
    .await;
    let service = RbacArtifactPermissionAssignmentService::new(
        db.clone(),
        Arc::new(SqliteArtifactPermissionEventPublisher { fail: false }),
    );

    for (scope, idempotency_key) in [
        (
            ArtifactPermissionAssignmentScope::Platform,
            "grant-platform",
        ),
        (ArtifactPermissionAssignmentScope::Tenant, "grant-tenant"),
    ] {
        service
            .assign(command(
                tenant_id,
                role_id,
                scope,
                installation_id,
                permission_key,
                actor_id,
                true,
                idempotency_key,
            ))
            .await
            .expect("grant explicit permission scope");
    }
    assert_eq!(table_count(&db, "rbac_artifact_role_permissions").await, 2);

    service
        .assign(command(
            tenant_id,
            role_id,
            ArtifactPermissionAssignmentScope::Platform,
            installation_id,
            permission_key,
            actor_id,
            false,
            "revoke-platform",
        ))
        .await
        .expect("revoke explicit platform scope");
    let remaining_id_text: String = db
        .query_one_raw(Statement::from_string(
            db.get_database_backend(),
            "SELECT artifact_permission_id FROM rbac_artifact_role_permissions LIMIT 1".to_string(),
        ))
        .await
        .expect("query remaining grant")
        .expect("remaining grant")
        .try_get("", "artifact_permission_id")
        .expect("decode remaining identity text");
    let remaining_id = Uuid::parse_str(&remaining_id_text).expect("parse remaining identity");
    assert_eq!(remaining_id, tenant_permission_id);
    assert_ne!(remaining_id, platform_permission_id);

    service
        .assign(command(
            tenant_id,
            role_id,
            ArtifactPermissionAssignmentScope::Tenant,
            installation_id,
            permission_key,
            actor_id,
            false,
            "revoke-tenant",
        ))
        .await
        .expect("revoke explicit tenant scope");
    assert_eq!(table_count(&db, "rbac_artifact_role_permissions").await, 0);
    assert_eq!(event_grants(&db).await, vec![true, true, false, false]);
}

#[tokio::test]
async fn publication_failure_rolls_back_grant_and_idempotency_receipt() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let installation_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let permission_key = "sample.events.handle";
    insert_role_and_actor(&db, tenant_id, role_id, actor_id).await;
    insert_definition(
        &db,
        &format!("tenant:{tenant_id}"),
        installation_id,
        permission_key,
    )
    .await;

    let service = RbacArtifactPermissionAssignmentService::new(
        db.clone(),
        Arc::new(SqliteArtifactPermissionEventPublisher { fail: true }),
    );
    let error = service
        .assign(command(
            tenant_id,
            role_id,
            ArtifactPermissionAssignmentScope::Tenant,
            installation_id,
            permission_key,
            actor_id,
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
