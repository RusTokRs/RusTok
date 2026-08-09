//! Verifies RBAC repair against its minimal platform and module-owned schema.

use rustok_api::Permission;
use rustok_core::{MigrationSource, UserRole};
use rustok_rbac::{
    apply_system_role_repair_in_transaction, plan_system_role_repair,
    read_permission_invalidation_generation, reserve_permission_invalidation_generation,
    RbacRoleAssignmentDbWriter, RbacSystemRoleRepairError,
};
use rustok_test_utils::db::setup_test_db_with_migrations;
use sea_orm_migration::sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};
use sea_orm_migration::{MigrationTrait, MigratorTrait};
use uuid::Uuid;

struct RbacSystemRoleTestMigrator;

#[async_trait::async_trait]
impl MigratorTrait for RbacSystemRoleTestMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        let mut migrations: Vec<Box<dyn MigrationTrait>> = vec![
            Box::new(super::m20250101_000001_create_tenants::Migration),
            Box::new(super::m20250101_000002_create_users::Migration),
            Box::new(super::m20250101_000005_create_roles_and_permissions::Migration),
        ];
        migrations.extend(MigrationSource::migrations(&rustok_rbac::RbacModule));
        migrations.sort_by_key(|migration| migration.name().to_string());
        migrations
    }
}

async fn insert_tenant(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id, name, slug, settings, default_locale, is_active) VALUES (?1, ?2, ?3, ?4, ?5, TRUE)",
        vec![
            tenant_id.into(),
            format!("Tenant {slug}").into(),
            slug.into(),
            serde_json::json!({}).into(),
            "en".into(),
        ],
    ))
    .await
    .expect("insert tenant");
}

async fn insert_user(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid, email: &str) {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO users (id, tenant_id, email, password_hash) VALUES (?1, ?2, ?3, ?4)",
        vec![
            user_id.into(),
            tenant_id.into(),
            email.into(),
            "hash".into(),
        ],
    ))
    .await
    .expect("insert user");
}

async fn insert_non_system_role(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    role_id: Uuid,
    slug: &str,
) {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO roles (id, tenant_id, name, slug, is_system) VALUES (?1, ?2, ?3, ?4, FALSE)",
        vec![role_id.into(), tenant_id.into(), slug.into(), slug.into()],
    ))
    .await
    .expect("insert role");
}

async fn role_id(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) -> Uuid {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "SELECT id FROM roles WHERE tenant_id = ?1 AND slug = ?2",
        vec![tenant_id.into(), slug.into()],
    ))
    .await
    .expect("query role")
    .expect("role exists")
    .try_get("", "id")
    .expect("role id")
}

async fn insert_stale_role_permission(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    role_id: Uuid,
) -> Uuid {
    let permission_id = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO permissions (id, tenant_id, resource, action) VALUES (?1, ?2, ?3, ?4)",
        vec![
            permission_id.into(),
            tenant_id.into(),
            Permission::SETTINGS_MANAGE.resource.to_string().into(),
            Permission::SETTINGS_MANAGE.action.to_string().into(),
        ],
    ))
    .await
    .expect("insert stale permission");
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO role_permissions (id, role_id, permission_id) VALUES (?1, ?2, ?3)",
        vec![Uuid::new_v4().into(), role_id.into(), permission_id.into()],
    ))
    .await
    .expect("insert stale role permission");
    permission_id
}

async fn role_permission_exists(
    db: &DatabaseConnection,
    role_id: Uuid,
    permission_id: Uuid,
) -> bool {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "SELECT id FROM role_permissions WHERE role_id = ?1 AND permission_id = ?2",
        vec![role_id.into(), permission_id.into()],
    ))
    .await
    .expect("query role permission")
    .is_some()
}

async fn role_count(db: &DatabaseConnection, tenant_id: Uuid) -> i64 {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "SELECT COUNT(*) AS total FROM roles WHERE tenant_id = ?1",
        vec![tenant_id.into()],
    ))
    .await
    .expect("query role count")
    .expect("role count row")
    .try_get("", "total")
    .expect("role count")
}

#[tokio::test]
async fn dry_run_is_read_only_and_apply_repairs_permission_drift() {
    let db = setup_test_db_with_migrations::<RbacSystemRoleTestMigrator>().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id, "repair-dry-run").await;
    insert_user(&db, tenant_id, user_id, "repair@example.com").await;

    RbacRoleAssignmentDbWriter::new(db.clone())
        .assign_role_permissions(tenant_id, user_id, UserRole::Manager)
        .await
        .expect("create manager role");
    let manager_role_id = role_id(&db, tenant_id, "manager").await;
    let stale_permission_id = insert_stale_role_permission(&db, tenant_id, manager_role_id).await;

    let plan = plan_system_role_repair(&db, Some(tenant_id))
        .await
        .expect("build repair plan");

    assert!(!plan.applied);
    assert!(plan.role_permission_links_removed >= 1);
    assert!(plan
        .affected_users
        .iter()
        .any(|affected| affected.tenant_id == tenant_id && affected.user_id == user_id));
    assert!(role_permission_exists(&db, manager_role_id, stale_permission_id).await);
    assert_eq!(
        read_permission_invalidation_generation(&db).await.unwrap(),
        0
    );

    let tx = db.begin().await.unwrap();
    let mut applied = apply_system_role_repair_in_transaction(&tx, Some(tenant_id))
        .await
        .expect("apply repair");
    let generation = reserve_permission_invalidation_generation(&tx)
        .await
        .expect("reserve invalidation generation");
    tx.commit().await.unwrap();
    applied.applied = true;
    applied.runtime_restart_required = false;

    assert!(applied.applied);
    assert!(!applied.runtime_restart_required);
    assert_eq!(generation, 1);
    assert_eq!(
        read_permission_invalidation_generation(&db).await.unwrap(),
        1
    );
    assert!(!role_permission_exists(&db, manager_role_id, stale_permission_id).await);
}

#[tokio::test]
async fn global_apply_rolls_back_when_any_tenant_has_reserved_slug_collision() {
    let db = setup_test_db_with_migrations::<RbacSystemRoleTestMigrator>().await;
    let clean_tenant_id = Uuid::from_u128(1);
    let collision_tenant_id = Uuid::from_u128(2);
    insert_tenant(&db, clean_tenant_id, "repair-clean").await;
    insert_tenant(&db, collision_tenant_id, "repair-collision").await;
    insert_non_system_role(&db, collision_tenant_id, Uuid::new_v4(), "admin").await;

    let tx = db.begin().await.unwrap();
    let error = apply_system_role_repair_in_transaction(&tx, None)
        .await
        .expect_err("reserved slug collision must reject global repair");
    tx.rollback().await.unwrap();

    assert!(matches!(
        error,
        RbacSystemRoleRepairError::BuiltInRoleSlugCollision { tenant_id, ref slug }
            if tenant_id == collision_tenant_id && slug == "admin"
    ));
    assert_eq!(
        read_permission_invalidation_generation(&db).await.unwrap(),
        0
    );
    assert_eq!(role_count(&db, clean_tenant_id).await, 0);
    assert_eq!(role_count(&db, collision_tenant_id).await, 1);
}
