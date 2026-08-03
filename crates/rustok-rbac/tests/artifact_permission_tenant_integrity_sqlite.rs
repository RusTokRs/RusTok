use rustok_core::module::MigrationSource;
use rustok_rbac::RbacModule;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect SQLite");
    db.execute_unprepared(
        r#"
        PRAGMA foreign_keys = ON;
        CREATE TABLE users (id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL);
        CREATE TABLE roles (id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL);
        CREATE TABLE permissions (id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL);
        CREATE TABLE user_roles (user_id TEXT NOT NULL, role_id TEXT NOT NULL);
        CREATE TABLE role_permissions (role_id TEXT NOT NULL, permission_id TEXT NOT NULL);
        "#,
    )
    .await
    .expect("create RBAC parent tables");

    let manager = SchemaManager::new(&db);
    let migrations = RbacModule.migrations();
    assert_eq!(
        migrations.len(),
        4,
        "artifact integrity must remain consolidated in canonical migrations"
    );
    for migration in migrations {
        migration.up(&manager).await.expect("apply RBAC migration");
    }
    db
}

async fn execute(db: &DatabaseConnection, sql: impl Into<String>) -> Result<(), String> {
    db.execute(Statement::from_string(DbBackend::Sqlite, sql.into()))
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

async fn count(db: &DatabaseConnection, table: &str) -> i64 {
    db.query_one(Statement::from_string(
        DbBackend::Sqlite,
        format!("SELECT COUNT(*) AS count FROM {table}"),
    ))
    .await
    .expect("count query")
    .expect("count row")
    .try_get("", "count")
    .expect("count value")
}

fn quoted(value: Uuid) -> String {
    format!("'{value}'")
}

#[tokio::test]
async fn database_rejects_cross_tenant_and_orphan_artifact_state() {
    let db = database().await;

    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let role_a = Uuid::new_v4();
    let role_b = Uuid::new_v4();
    let actor_a = Uuid::new_v4();
    let actor_b = Uuid::new_v4();
    let installation_id = Uuid::new_v4();
    let artifact_permission_id = Uuid::new_v4();

    for sql in [
        format!(
            "INSERT INTO roles (id, tenant_id) VALUES ({}, {})",
            quoted(role_a),
            quoted(tenant_a)
        ),
        format!(
            "INSERT INTO roles (id, tenant_id) VALUES ({}, {})",
            quoted(role_b),
            quoted(tenant_b)
        ),
        format!(
            "INSERT INTO users (id, tenant_id) VALUES ({}, {})",
            quoted(actor_a),
            quoted(tenant_a)
        ),
        format!(
            "INSERT INTO users (id, tenant_id) VALUES ({}, {})",
            quoted(actor_b),
            quoted(tenant_b)
        ),
        format!(
            "INSERT INTO rbac_artifact_permission_definitions (id, scope_key, installation_id, module_slug, release_digest, permission_key) VALUES ({}, 'tenant:{}', {}, 'sample', 'sha256:test', 'sample.events.handle')",
            quoted(artifact_permission_id),
            tenant_a,
            quoted(installation_id)
        ),
        format!(
            "INSERT INTO rbac_artifact_permission_translations (id, artifact_permission_id, locale, label, description) VALUES ({}, {}, 'en', 'Handle events', 'Allows handling events')",
            quoted(Uuid::new_v4()),
            quoted(artifact_permission_id)
        ),
    ] {
        execute(&db, sql).await.expect("seed valid parent state");
    }

    execute(
        &db,
        format!(
            "INSERT INTO rbac_artifact_role_permissions (id, tenant_id, role_id, artifact_permission_id, granted_by_actor_id) VALUES ({}, {}, {}, {}, {})",
            quoted(Uuid::new_v4()),
            quoted(tenant_a),
            quoted(role_a),
            quoted(artifact_permission_id),
            quoted(actor_a),
        ),
    )
    .await
    .expect("valid artifact grant");
    execute(
        &db,
        format!(
            "INSERT INTO rbac_artifact_role_permission_operations (id, tenant_id, idempotency_key, role_id, artifact_permission_id, actor_id, granted) VALUES ({}, {}, 'valid-op', {}, {}, {}, 1)",
            quoted(Uuid::new_v4()),
            quoted(tenant_a),
            quoted(role_a),
            quoted(artifact_permission_id),
            quoted(actor_a),
        ),
    )
    .await
    .expect("valid artifact receipt");

    execute(
        &db,
        format!(
            "UPDATE rbac_artifact_permission_translations SET label = 'Updated label', description = 'Updated description' WHERE artifact_permission_id = {}",
            quoted(artifact_permission_id)
        ),
    )
    .await
    .expect("localized metadata update must remain allowed");

    let rejected = [
        format!(
            "INSERT INTO rbac_artifact_role_permissions (id, tenant_id, role_id, artifact_permission_id, granted_by_actor_id) VALUES ({}, {}, {}, {}, {})",
            quoted(Uuid::new_v4()), quoted(tenant_a), quoted(role_b), quoted(artifact_permission_id), quoted(actor_a)
        ),
        format!(
            "INSERT INTO rbac_artifact_role_permissions (id, tenant_id, role_id, artifact_permission_id, granted_by_actor_id) VALUES ({}, {}, {}, {}, {})",
            quoted(Uuid::new_v4()), quoted(tenant_a), quoted(role_a), quoted(artifact_permission_id), quoted(actor_b)
        ),
        format!(
            "INSERT INTO rbac_artifact_role_permissions (id, tenant_id, role_id, artifact_permission_id, granted_by_actor_id) VALUES ({}, {}, {}, {}, {})",
            quoted(Uuid::new_v4()), quoted(tenant_a), quoted(role_a), quoted(Uuid::new_v4()), quoted(actor_a)
        ),
        format!(
            "INSERT INTO rbac_artifact_role_permission_operations (id, tenant_id, idempotency_key, role_id, artifact_permission_id, actor_id, granted) VALUES ({}, {}, 'foreign-op', {}, {}, {}, 1)",
            quoted(Uuid::new_v4()), quoted(tenant_a), quoted(role_b), quoted(artifact_permission_id), quoted(actor_a)
        ),
        format!(
            "UPDATE roles SET tenant_id = {} WHERE id = {}",
            quoted(tenant_b), quoted(role_a)
        ),
        format!(
            "UPDATE users SET tenant_id = {} WHERE id = {}",
            quoted(tenant_b), quoted(actor_a)
        ),
        format!("DELETE FROM roles WHERE id = {}", quoted(role_a)),
        format!("DELETE FROM users WHERE id = {}", quoted(actor_a)),
        format!(
            "UPDATE rbac_artifact_permission_definitions SET permission_key = 'sample.changed' WHERE id = {}",
            quoted(artifact_permission_id)
        ),
        format!(
            "DELETE FROM rbac_artifact_permission_definitions WHERE id = {}",
            quoted(artifact_permission_id)
        ),
    ];

    for sql in rejected {
        assert!(
            execute(&db, sql).await.is_err(),
            "database must reject malformed artifact authorization state"
        );
    }

    assert_eq!(count(&db, "rbac_artifact_role_permissions").await, 1);
    assert_eq!(
        count(&db, "rbac_artifact_role_permission_operations").await,
        1
    );
    assert_eq!(count(&db, "rbac_artifact_permission_definitions").await, 1);
    assert_eq!(
        count(&db, "rbac_artifact_permission_translations").await,
        1
    );
}
