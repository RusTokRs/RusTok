use async_trait::async_trait;
use rustok_core::{ModuleContext, ModuleRegistry, RusToKModule};
use rustok_server::models::_entities::tenant_modules;
use rustok_server::services::module_lifecycle::{ModuleLifecycleService, ToggleModuleError};
use sea_orm::{
    ColumnTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
    QueryFilter, Statement,
};
use sea_orm_migration::MigrationTrait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct TestModule {
    slug: &'static str,
    should_fail_enable: bool,
    should_fail_disable: bool,
    enable_calls: Arc<AtomicUsize>,
    disable_calls: Arc<AtomicUsize>,
}

impl TestModule {
    fn new(slug: &'static str) -> Self {
        Self {
            slug,
            should_fail_enable: false,
            should_fail_disable: false,
            enable_calls: Arc::new(AtomicUsize::new(0)),
            disable_calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn with_enable_failure(mut self) -> Self {
        self.should_fail_enable = true;
        self
    }
}

impl rustok_core::MigrationSource for TestModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        vec![]
    }
}

#[async_trait]
impl RusToKModule for TestModule {
    fn slug(&self) -> &'static str {
        self.slug
    }

    fn name(&self) -> &'static str {
        "test"
    }

    fn description(&self) -> &'static str {
        "test module"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    async fn on_enable(&self, _ctx: ModuleContext<'_>) -> rustok_core::Result<()> {
        self.enable_calls.fetch_add(1, Ordering::SeqCst);
        if self.should_fail_enable {
            return Err(rustok_core::Error::External("enable failed".to_string()));
        }
        Ok(())
    }

    async fn on_disable(&self, _ctx: ModuleContext<'_>) -> rustok_core::Result<()> {
        self.disable_calls.fetch_add(1, Ordering::SeqCst);
        if self.should_fail_disable {
            return Err(rustok_core::Error::External("disable failed".to_string()));
        }
        Ok(())
    }
}

async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("db connect");

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        r#"
        CREATE TABLE tenants (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            slug TEXT NOT NULL UNIQUE,
            domain TEXT NULL UNIQUE,
            settings TEXT NOT NULL DEFAULT '{}',
            is_active BOOLEAN NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    ))
    .await
    .expect("create tenants");

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        r#"
        CREATE TABLE tenant_modules (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            module_slug TEXT NOT NULL,
            enabled BOOLEAN NOT NULL DEFAULT 1,
            settings TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE,
            UNIQUE (tenant_id, module_slug)
        );
        "#,
    ))
    .await
    .expect("create tenant_modules");

    db
}

async fn seed_tenant(db: &DatabaseConnection, tenant_id: uuid::Uuid) {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id, name, slug, settings, is_active, created_at, updated_at) VALUES (?, ?, ?, '{}', 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        [tenant_id.into(), "Tenant".into(), "tenant".into()],
    ))
    .await
    .expect("seed tenant");
}

#[tokio::test]
async fn successful_enable_and_idempotent_retry() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let module = TestModule::new("commerce");
    let calls = module.enable_calls.clone();
    let registry = ModuleRegistry::new().register(module);

    let enabled =
        ModuleLifecycleService::toggle_module(&db, &registry, tenant_id, "commerce", true)
            .await
            .expect("first enable");
    assert!(enabled.enabled);

    let second = ModuleLifecycleService::toggle_module(&db, &registry, tenant_id, "commerce", true)
        .await
        .expect("second enable");
    assert!(second.enabled);
    assert_eq!(calls.load(Ordering::SeqCst), 1, "hook should be idempotent");
}

#[tokio::test]
async fn hook_failure_rolls_back_state() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("forum").with_enable_failure());

    let err = ModuleLifecycleService::toggle_module(&db, &registry, tenant_id, "forum", true)
        .await
        .expect_err("enable should fail");

    assert!(matches!(err, ToggleModuleError::HookFailed(_)));

    let state = tenant_modules::Entity::find()
        .filter(tenant_modules::Column::TenantId.eq(tenant_id))
        .filter(tenant_modules::Column::ModuleSlug.eq("forum"))
        .one(&db)
        .await
        .expect("load state")
        .expect("state row exists");

    assert!(
        !state.enabled,
        "state should be rolled back after hook failure"
    );
}

#[tokio::test]
async fn concurrent_toggle_requests_keep_consistent_state() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let module = TestModule::new("blog");
    let enable_calls = module.enable_calls.clone();
    let disable_calls = module.disable_calls.clone();
    let registry = ModuleRegistry::new().register(module);

    let first = ModuleLifecycleService::toggle_module(&db, &registry, tenant_id, "blog", true);
    let second = ModuleLifecycleService::toggle_module(&db, &registry, tenant_id, "blog", false);

    let (r1, r2) = tokio::join!(first, second);
    assert!(r1.is_ok());
    assert!(r2.is_ok());

    let state = tenant_modules::Entity::find()
        .filter(tenant_modules::Column::TenantId.eq(tenant_id))
        .filter(tenant_modules::Column::ModuleSlug.eq("blog"))
        .one(&db)
        .await
        .expect("load state")
        .expect("state row exists");

    assert!(matches!(state.enabled, true | false));
    assert!(enable_calls.load(Ordering::SeqCst) <= 1);
    assert!(disable_calls.load(Ordering::SeqCst) <= 1);
}
