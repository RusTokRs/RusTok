use rustok_core::{ModuleRegistry, UserRole};
use rustok_installer::{
    AdminBootstrap, DatabaseConfig, DatabaseEngine, InstallEnvironment, InstallPlan, InstallProfile,
    InstallTopology, InstallTopologyMode, ModuleSelection, SecretMode, SecretValue,
    SeedPrincipalPort, SeedProfile, SeedUserRequest, TenantBootstrap,
};
use rustok_installer_persistence::SeaOrmInstallerBootstrapPorts;
use rustok_migrations::Migrator;
use rustok_test_utils::db::setup_test_db_with_migrations;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use uuid::Uuid;

fn test_plan(email: &str) -> InstallPlan {
    InstallPlan {
        environment: InstallEnvironment::Test,
        profile: InstallProfile::Monolith,
        database: DatabaseConfig {
            engine: DatabaseEngine::Sqlite,
            url: SecretValue::Plaintext {
                value: "sqlite::memory:".to_string(),
            },
            create_if_missing: false,
        },
        tenant: TenantBootstrap {
            slug: "installer-atomicity".to_string(),
            name: "Installer Atomicity".to_string(),
        },
        admin: AdminBootstrap {
            email: email.to_string(),
            password: SecretValue::Plaintext {
                value: "unused-plan-password".to_string(),
            },
        },
        modules: ModuleSelection::default(),
        topology: InstallTopology::for_mode(InstallTopologyMode::Monolith)
            .bind_composition("test-distribution@1".to_string(), "a".repeat(64)),
        seed_profile: SeedProfile::Minimal,
        secrets_mode: SecretMode::Env,
    }
}

async fn insert_tenant(db: &sea_orm::DatabaseConnection, tenant_id: Uuid) {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active) VALUES (?1, ?2, ?3, NULL, ?4, ?5, TRUE)",
        vec![
            tenant_id.into(),
            "Installer Atomicity".into(),
            "installer-atomicity".into(),
            serde_json::json!({}).into(),
            "en".into(),
        ],
    ))
    .await
    .expect("insert tenant");
}

async fn insert_reserved_slug_collision(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    slug: &str,
) {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO roles (id, tenant_id, name, slug, description, is_system) VALUES (?1, ?2, ?3, ?4, NULL, FALSE)",
        vec![
            Uuid::new_v4().into(),
            tenant_id.into(),
            "User-defined collision".into(),
            slug.into(),
        ],
    ))
    .await
    .expect("insert reserved role slug collision");
}

async fn bootstrap_user_count(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    email: &str,
) -> i64 {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "SELECT COUNT(*) AS total FROM users WHERE tenant_id = ?1 AND email = ?2",
        vec![tenant_id.into(), email.into()],
    ))
    .await
    .expect("query bootstrap users")
    .expect("count row")
    .try_get("", "total")
    .expect("read user count")
}

#[tokio::test]
async fn role_failure_rolls_back_new_admin_identity() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let tenant_id = Uuid::new_v4();
    let admin_email = "atomic-admin@example.com";
    insert_tenant(&db, tenant_id).await;
    insert_reserved_slug_collision(&db, tenant_id, "super_admin").await;

    let registry = ModuleRegistry::new();
    let ports = SeaOrmInstallerBootstrapPorts::new(db.clone(), &registry, Vec::new());
    let error = ports
        .provision_admin(
            &test_plan(admin_email),
            tenant_id,
            "strong-test-password-12345",
        )
        .await
        .expect_err("reserved slug collision must reject admin provisioning");

    assert!(error.to_string().contains("super_admin"));
    assert_eq!(bootstrap_user_count(&db, tenant_id, admin_email).await, 0);
}

#[tokio::test]
async fn seed_role_failure_rolls_back_new_customer_identity() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let tenant_id = Uuid::new_v4();
    let customer_email = "atomic-customer@example.com";
    insert_tenant(&db, tenant_id).await;
    insert_reserved_slug_collision(&db, tenant_id, "customer").await;

    let registry = ModuleRegistry::new();
    let ports = SeaOrmInstallerBootstrapPorts::new(db.clone(), &registry, Vec::new());
    let error = ports
        .ensure_seed_principal(
            SeedUserRequest {
                tenant_id,
                email: customer_email.to_string(),
                name: "Atomic Customer".to_string(),
                password: "strong-test-password-12345".to_string(),
            },
            UserRole::Customer,
        )
        .await
        .expect_err("reserved customer slug collision must reject seed principal provisioning");

    assert!(error.to_string().contains("customer"));
    assert_eq!(bootstrap_user_count(&db, tenant_id, customer_email).await, 0);
}

#[tokio::test]
async fn successful_provisioning_commits_identity_and_role_together() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let tenant_id = Uuid::new_v4();
    let admin_email = "committed-admin@example.com";
    insert_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new();
    let ports = SeaOrmInstallerBootstrapPorts::new(db.clone(), &registry, Vec::new());
    let outcome = ports
        .provision_admin(
            &test_plan(admin_email),
            tenant_id,
            "strong-test-password-12345",
        )
        .await
        .expect("admin provisioning should commit");

    assert!(outcome.created);
    assert_eq!(outcome.email, admin_email);
    assert_eq!(bootstrap_user_count(&db, tenant_id, admin_email).await, 1);

    let role_links: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS total FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = ?1 AND r.tenant_id = ?2 AND r.slug = ?3 AND r.is_system = TRUE",
            vec![outcome.user_id.into(), tenant_id.into(), "super_admin".into()],
        ))
        .await
        .expect("query superadmin assignment")
        .expect("role count row")
        .try_get("", "total")
        .expect("read role count");
    assert_eq!(role_links, 1);
}
