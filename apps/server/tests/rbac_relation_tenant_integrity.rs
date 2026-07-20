use rustok_migrations::Migrator;
use rustok_test_utils::db::setup_test_db_with_migrations;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;
use uuid::Uuid;

const MIGRATION_NAME: &str = "m20260714_900001_enforce_rbac_relation_tenant_integrity";

async fn execute(
    db: &sea_orm::DatabaseConnection,
    sql: &str,
    values: Vec<sea_orm::Value>,
) -> Result<(), sea_orm::DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        sql,
        values,
    ))
    .await
    .map(|_| ())
}

async fn count(db: &sea_orm::DatabaseConnection, sql: &str) -> i64 {
    db.query_one(Statement::from_string(DbBackend::Sqlite, sql.to_string()))
        .await
        .expect("query count")
        .expect("count row")
        .try_get("", "total")
        .expect("read count")
}

async fn insert_tenant(db: &sea_orm::DatabaseConnection, id: Uuid, slug: &str) {
    execute(
        db,
        "INSERT INTO tenants (id, name, slug) VALUES (?1, ?2, ?3)",
        vec![id.into(), slug.into(), slug.into()],
    )
    .await
    .expect("insert tenant");
}

async fn insert_user(db: &sea_orm::DatabaseConnection, id: Uuid, tenant_id: Uuid, email: &str) {
    execute(
        db,
        "INSERT INTO users (id, tenant_id, email, password_hash) VALUES (?1, ?2, ?3, ?4)",
        vec![id.into(), tenant_id.into(), email.into(), "hash".into()],
    )
    .await
    .expect("insert user");
}

async fn insert_role(db: &sea_orm::DatabaseConnection, id: Uuid, tenant_id: Uuid, slug: &str) {
    execute(
        db,
        "INSERT INTO roles (id, tenant_id, name, slug, is_system) VALUES (?1, ?2, ?3, ?4, TRUE)",
        vec![id.into(), tenant_id.into(), slug.into(), slug.into()],
    )
    .await
    .expect("insert role");
}

async fn insert_permission(
    db: &sea_orm::DatabaseConnection,
    id: Uuid,
    tenant_id: Uuid,
    resource: &str,
) {
    execute(
        db,
        "INSERT INTO permissions (id, tenant_id, resource, action) VALUES (?1, ?2, ?3, ?4)",
        vec![id.into(), tenant_id.into(), resource.into(), "read".into()],
    )
    .await
    .expect("insert permission");
}

#[tokio::test]
async fn sqlite_database_enforces_rbac_relation_tenant_integrity() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    insert_tenant(&db, tenant_a, "rbac-integrity-a").await;
    insert_tenant(&db, tenant_b, "rbac-integrity-b").await;

    let user_a = Uuid::new_v4();
    let role_a = Uuid::new_v4();
    let role_b = Uuid::new_v4();
    let permission_a = Uuid::new_v4();
    let permission_b = Uuid::new_v4();
    insert_user(&db, user_a, tenant_a, "integrity-a@example.com").await;
    insert_role(&db, role_a, tenant_a, "integrity_role_a").await;
    insert_role(&db, role_b, tenant_b, "integrity_role_b").await;
    insert_permission(&db, permission_a, tenant_a, "integrity_a").await;
    insert_permission(&db, permission_b, tenant_b, "integrity_b").await;

    execute(
        &db,
        "INSERT INTO user_roles (id, user_id, role_id) VALUES (?1, ?2, ?3)",
        vec![Uuid::new_v4().into(), user_a.into(), role_a.into()],
    )
    .await
    .expect("same-tenant user role must be accepted");
    assert!(
        execute(
            &db,
            "INSERT INTO user_roles (id, user_id, role_id) VALUES (?1, ?2, ?3)",
            vec![Uuid::new_v4().into(), user_a.into(), role_b.into()],
        )
        .await
        .is_err()
    );

    execute(
        &db,
        "INSERT INTO role_permissions (id, role_id, permission_id) VALUES (?1, ?2, ?3)",
        vec![Uuid::new_v4().into(), role_a.into(), permission_a.into()],
    )
    .await
    .expect("same-tenant role permission must be accepted");
    assert!(
        execute(
            &db,
            "INSERT INTO role_permissions (id, role_id, permission_id) VALUES (?1, ?2, ?3)",
            vec![Uuid::new_v4().into(), role_a.into(), permission_b.into()],
        )
        .await
        .is_err()
    );

    assert!(
        execute(
            &db,
            "UPDATE users SET tenant_id = ?1 WHERE id = ?2",
            vec![tenant_b.into(), user_a.into()],
        )
        .await
        .is_err()
    );
    assert!(
        execute(
            &db,
            "UPDATE roles SET tenant_id = ?1 WHERE id = ?2",
            vec![tenant_b.into(), role_a.into()],
        )
        .await
        .is_err()
    );
    assert!(
        execute(
            &db,
            "UPDATE permissions SET tenant_id = ?1 WHERE id = ?2",
            vec![tenant_b.into(), permission_a.into()],
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn migration_removes_historical_cross_tenant_links_before_enforcing_triggers() {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect sqlite");
    let migration_index = Migrator::migrations()
        .iter()
        .position(|migration| migration.name() == MIGRATION_NAME)
        .expect("RBAC tenant integrity migration must be registered");
    assert!(migration_index > 0);
    Migrator::up(&db, Some(migration_index as u32))
        .await
        .expect("apply migrations preceding RBAC tenant integrity");

    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    insert_tenant(&db, tenant_a, "rbac-upgrade-a").await;
    insert_tenant(&db, tenant_b, "rbac-upgrade-b").await;
    let user_a = Uuid::new_v4();
    let role_b = Uuid::new_v4();
    let permission_a = Uuid::new_v4();
    insert_user(&db, user_a, tenant_a, "upgrade-a@example.com").await;
    insert_role(&db, role_b, tenant_b, "upgrade_role_b").await;
    insert_permission(&db, permission_a, tenant_a, "upgrade_a").await;

    execute(
        &db,
        "INSERT INTO user_roles (id, user_id, role_id) VALUES (?1, ?2, ?3)",
        vec![Uuid::new_v4().into(), user_a.into(), role_b.into()],
    )
    .await
    .expect("legacy schema permits cross-tenant user role");
    execute(
        &db,
        "INSERT INTO role_permissions (id, role_id, permission_id) VALUES (?1, ?2, ?3)",
        vec![Uuid::new_v4().into(), role_b.into(), permission_a.into()],
    )
    .await
    .expect("legacy schema permits cross-tenant role permission");
    assert_eq!(
        count(&db, "SELECT COUNT(*) AS total FROM user_roles").await,
        1
    );
    assert_eq!(
        count(&db, "SELECT COUNT(*) AS total FROM role_permissions").await,
        1
    );

    Migrator::up(&db, None)
        .await
        .expect("apply RBAC tenant integrity migration");

    assert_eq!(
        count(&db, "SELECT COUNT(*) AS total FROM user_roles").await,
        0
    );
    assert_eq!(
        count(&db, "SELECT COUNT(*) AS total FROM role_permissions").await,
        0
    );
    assert!(
        execute(
            &db,
            "INSERT INTO user_roles (id, user_id, role_id) VALUES (?1, ?2, ?3)",
            vec![Uuid::new_v4().into(), user_a.into(), role_b.into()],
        )
        .await
        .is_err()
    );
    assert!(
        execute(
            &db,
            "INSERT INTO role_permissions (id, role_id, permission_id) VALUES (?1, ?2, ?3)",
            vec![Uuid::new_v4().into(), role_b.into(), permission_a.into()],
        )
        .await
        .is_err()
    );
}
