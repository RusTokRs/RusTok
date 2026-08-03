use rustok_core::module::MigrationSource;
use rustok_rbac::RbacModule;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn base_database() -> DatabaseConnection {
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
    db
}

async fn apply_migrations(db: &DatabaseConnection, count: usize) {
    let manager = SchemaManager::new(db);
    let migrations = RbacModule.migrations();
    assert_eq!(migrations.len(), 5, "RBAC migration inventory changed");
    for migration in migrations.into_iter().take(count) {
        migration.up(&manager).await.expect("apply RBAC migration");
    }
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
async fn migration_cleans_legacy_malformed_artifact_rows() {
    let db = base_database().await;
    apply_migrations(&db, 4).await;

    let tenant_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let installation_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    execute(
        &db,
        format!(
            "INSERT INTO rbac_artifact_role_permissions (id, tenant_id, role_id, installation_id, permission_key, granted_by_actor_id) VALUES ({}, {}, {}, {}, 'sample.events.handle', {})",
            quoted(Uuid::new_v4()),
            quoted(tenant_id),
            quoted(role_id),
            quoted(installation_id),
            quoted(actor_id),
        ),
    )
    .await
    .expect("seed malformed grant");
    execute(
        &db,
        format!(
            "INSERT INTO rbac_artifact_role_permission_operations (id, tenant_id, idempotency_key, role_id, installation_id, permission_key, actor_id, granted) VALUES ({}, {}, 'legacy-op', {}, {}, 'sample.events.handle', {}, 1)",
            quoted(Uuid::new_v4()),
            quoted(tenant_id),
            quoted(role_id),
            quoted(installation_id),
            quoted(actor_id),
        ),
    )
    .await
    .expect("seed malformed receipt");

    let manager = SchemaManager::new(&db);
    RbacModule
        .migrations()
        .into_iter()
        .nth(4)
        .expect("artifact integrity migration")
        .up(&manager)
        .await
        .expect("apply artifact integrity migration");

    assert_eq!(count(&db, "rbac_artifact_role_permissions").await, 0);
    assert_eq!(
        count(&db, "rbac_artifact_role_permission_operations").await,
        0
    );
}

#[tokio::test]
async fn database_rejects_cross_tenant_and_orphan_artifact_state() {
    let db = base_database().await;
    apply_migrations(&db, 5).await;

    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let role_a = Uuid::new_v4();
    let role_b = Uuid::new_v4();
    let actor_a = Uuid::new_v4();
    let actor_b = Uuid::new_v4();
    let installation_id = Uuid::new_v4();
    let catalog_id = Uuid::new_v4();

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
            "INSERT INTO rbac_artifact_permission_catalog (id, scope_key, installation_id, module_slug, release_digest, permission_key, locale, label, description, registered_at) VALUES ({}, 'tenant:{}', {}, 'sample', 'sha256:test', 'sample.events.handle', 'en', 'Handle events', 'Allows handling events', '2026-08-01T00:00:00Z')",
            quoted(catalog_id),
            tenant_a,
            quoted(installation_id)
        ),
    ] {
        execute(&db, sql).await.expect("seed valid parent state");
    }

    execute(
        &db,
        format!(
            "INSERT INTO rbac_artifact_role_permissions (id, tenant_id, role_id, installation_id, permission_key, granted_by_actor_id) VALUES ({}, {}, {}, {}, 'sample.events.handle', {})",
            quoted(Uuid::new_v4()),
            quoted(tenant_a),
            quoted(role_a),
            quoted(installation_id),
            quoted(actor_a),
        ),
    )
    .await
    .expect("valid artifact grant");
    execute(
        &db,
        format!(
            "INSERT INTO rbac_artifact_role_permission_operations (id, tenant_id, idempotency_key, role_id, installation_id, permission_key, actor_id, granted) VALUES ({}, {}, 'valid-op', {}, {}, 'sample.events.handle', {}, 1)",
            quoted(Uuid::new_v4()),
            quoted(tenant_a),
            quoted(role_a),
            quoted(installation_id),
            quoted(actor_a),
        ),
    )
    .await
    .expect("valid artifact receipt");

    execute(
        &db,
        format!(
            "UPDATE rbac_artifact_permission_catalog SET label = 'Updated label', description = 'Updated description' WHERE id = {}",
            quoted(catalog_id)
        ),
    )
    .await
    .expect("localized metadata update must remain allowed");

    let rejected = [
        format!(
            "INSERT INTO rbac_artifact_role_permissions (id, tenant_id, role_id, installation_id, permission_key, granted_by_actor_id) VALUES ({}, {}, {}, {}, 'sample.events.handle', {})",
            quoted(Uuid::new_v4()), quoted(tenant_a), quoted(role_b), quoted(installation_id), quoted(actor_a)
        ),
        format!(
            "INSERT INTO rbac_artifact_role_permissions (id, tenant_id, role_id, installation_id, permission_key, granted_by_actor_id) VALUES ({}, {}, {}, {}, 'sample.events.handle', {})",
            quoted(Uuid::new_v4()), quoted(tenant_a), quoted(role_a), quoted(installation_id), quoted(actor_b)
        ),
        format!(
            "INSERT INTO rbac_artifact_role_permissions (id, tenant_id, role_id, installation_id, permission_key, granted_by_actor_id) VALUES ({}, {}, {}, {}, 'sample.unknown', {})",
            quoted(Uuid::new_v4()), quoted(tenant_a), quoted(role_a), quoted(installation_id), quoted(actor_a)
        ),
        format!(
            "INSERT INTO rbac_artifact_role_permission_operations (id, tenant_id, idempotency_key, role_id, installation_id, permission_key, actor_id, granted) VALUES ({}, {}, 'foreign-op', {}, {}, 'sample.events.handle', {}, 1)",
            quoted(Uuid::new_v4()), quoted(tenant_a), quoted(role_b), quoted(installation_id), quoted(actor_a)
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
            "UPDATE rbac_artifact_permission_catalog SET permission_key = 'sample.changed' WHERE id = {}",
            quoted(catalog_id)
        ),
        format!(
            "DELETE FROM rbac_artifact_permission_catalog WHERE id = {}",
            quoted(catalog_id)
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
}
