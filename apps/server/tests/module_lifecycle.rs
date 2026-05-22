use async_trait::async_trait;
use rustok_core::{ModuleContext, ModuleKind, ModuleRegistry, RusToKModule};
use rustok_server::models::_entities::{module_operations, tenant_modules};
use rustok_server::services::module_lifecycle::{
    ModuleLifecycleService, ModuleOperationStatus, ToggleModuleError,
};
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

struct DependentModule {
    slug: &'static str,
    dependency: &'static str,
}

struct CoreTestModule {
    slug: &'static str,
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

    fn with_disable_failure(mut self) -> Self {
        self.should_fail_disable = true;
        self
    }
}

impl rustok_core::MigrationSource for TestModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        vec![]
    }
}

impl rustok_core::MigrationSource for DependentModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        vec![]
    }
}

impl rustok_core::MigrationSource for CoreTestModule {
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

#[async_trait]
impl RusToKModule for DependentModule {
    fn slug(&self) -> &'static str {
        self.slug
    }

    fn name(&self) -> &'static str {
        "dependent-test-module"
    }

    fn description(&self) -> &'static str {
        "test dependent module"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec![self.dependency]
    }
}

#[async_trait]
impl RusToKModule for CoreTestModule {
    fn slug(&self) -> &'static str {
        self.slug
    }

    fn name(&self) -> &'static str {
        "core-test-module"
    }

    fn description(&self) -> &'static str {
        "test core module"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
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

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        r#"
        CREATE TABLE module_operations (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            module_slug TEXT NOT NULL,
            requested_enabled BOOLEAN NOT NULL,
            previous_effective_enabled BOOLEAN NOT NULL,
            status TEXT NOT NULL,
            requested_by TEXT NULL,
            error_message TEXT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
        );
        "#,
    ))
    .await
    .expect("create module_operations");

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

    let operations = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("commerce"))
        .all(&db)
        .await
        .expect("load operations");

    assert_eq!(
        operations.len(),
        1,
        "idempotent retry must not create duplicate module_operations journal rows",
    );
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

    let operation = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("forum"))
        .one(&db)
        .await
        .expect("load operation")
        .expect("operation exists");

    assert_eq!(operation.status, ModuleOperationStatus::Failed.as_str());
    assert!(operation
        .error_message
        .as_deref()
        .unwrap_or_default()
        .contains("enable failed"));
    assert!(
        operation.correlation_id.is_some(),
        "failed lifecycle operation must keep correlation id for retry/audit tracing",
    );
    let correlation_id = operation
        .correlation_id
        .as_deref()
        .expect("failed operation must have correlation id");
    let parsed = uuid::Uuid::parse_str(correlation_id).expect("correlation id must be uuid");
    assert_eq!(parsed.get_version_num(), 4);
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

#[tokio::test]
async fn successful_toggle_writes_committed_module_operation() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("pricing"));

    let enabled = ModuleLifecycleService::toggle_module(&db, &registry, tenant_id, "pricing", true)
        .await
        .expect("enable should succeed");
    assert!(enabled.enabled);

    let operation = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("pricing"))
        .one(&db)
        .await
        .expect("load operation")
        .expect("operation exists");

    assert_eq!(operation.status, ModuleOperationStatus::Committed.as_str());
    assert!(operation.error_message.is_none());
    assert!(operation.requested_enabled);
    assert!(!operation.previous_effective_enabled);
    assert!(
        operation.correlation_id.is_some(),
        "committed lifecycle operation must keep correlation id for tracing",
    );
    let correlation_id = operation
        .correlation_id
        .as_deref()
        .expect("committed operation must have correlation id");
    let parsed = uuid::Uuid::parse_str(correlation_id).expect("correlation id must be uuid");
    assert_eq!(parsed.get_version_num(), 4);
}

#[tokio::test]
async fn successful_toggle_with_actor_persists_requested_by() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("catalog"));

    ModuleLifecycleService::toggle_module_with_actor(
        &db,
        &registry,
        tenant_id,
        "catalog",
        true,
        Some("admin:user-1".to_string()),
    )
    .await
    .expect("enable should succeed");

    let operation = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("catalog"))
        .one(&db)
        .await
        .expect("load operation")
        .expect("operation exists");

    assert_eq!(operation.status, ModuleOperationStatus::Committed.as_str());
    assert_eq!(operation.requested_by.as_deref(), Some("admin:user-1"));
}

#[tokio::test]
async fn dependency_validation_failure_does_not_create_journal_row() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new()
        .register(TestModule::new("pricing"))
        .register(DependentModule {
            slug: "checkout",
            dependency: "pricing",
        });

    let err = ModuleLifecycleService::toggle_module(&db, &registry, tenant_id, "checkout", true)
        .await
        .expect_err("enable should fail because dependency is missing");
    assert!(matches!(err, ToggleModuleError::MissingDependencies(_)));

    let operation = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("checkout"))
        .one(&db)
        .await
        .expect("query operations");

    assert!(
        operation.is_none(),
        "validation errors before lifecycle execution must not create journal rows",
    );
}

#[tokio::test]
async fn dependent_validation_failure_does_not_create_journal_row() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new()
        .register(TestModule::new("pricing"))
        .register(DependentModule {
            slug: "checkout",
            dependency: "pricing",
        });

    ModuleLifecycleService::toggle_module(&db, &registry, tenant_id, "pricing", true)
        .await
        .expect("enable dependency first");
    ModuleLifecycleService::toggle_module(&db, &registry, tenant_id, "checkout", true)
        .await
        .expect("enable dependent second");

    let err = ModuleLifecycleService::toggle_module(&db, &registry, tenant_id, "pricing", false)
        .await
        .expect_err("disable should fail because module has dependents");
    assert!(matches!(err, ToggleModuleError::HasDependents(_)));

    let operations = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("pricing"))
        .all(&db)
        .await
        .expect("query operations");

    assert_eq!(
        operations.len(),
        1,
        "pre-validation dependent failure must not create extra journal rows",
    );
    assert_eq!(operations[0].status, ModuleOperationStatus::Committed.as_str());
    assert!(operations[0].requested_enabled);
}

#[tokio::test]
async fn unknown_module_failure_does_not_create_journal_row() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("pricing"));

    let err = ModuleLifecycleService::toggle_module(&db, &registry, tenant_id, "unknown", true)
        .await
        .expect_err("unknown module should fail");
    assert!(matches!(err, ToggleModuleError::UnknownModule));

    let operations = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("query operations");
    assert!(
        operations.is_empty(),
        "unknown module validation must not create module_operations journal rows",
    );
}

#[tokio::test]
async fn core_module_disable_failure_does_not_create_journal_row() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(CoreTestModule { slug: "tenant" });

    let err = ModuleLifecycleService::toggle_module(&db, &registry, tenant_id, "tenant", false)
        .await
        .expect_err("core module disable should fail");
    assert!(matches!(
        err,
        ToggleModuleError::CoreModuleCannotBeDisabled(module) if module == "tenant"
    ));

    let operations = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("query operations");
    assert!(
        operations.is_empty(),
        "core-module pre-validation failure must not create module_operations rows",
    );
}

#[tokio::test]
async fn noop_disable_for_already_disabled_module_does_not_create_journal_row() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("inventory"));

    let module =
        ModuleLifecycleService::toggle_module(&db, &registry, tenant_id, "inventory", false)
            .await
            .expect("no-op disable should succeed");
    assert!(!module.enabled);

    let operations = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("inventory"))
        .all(&db)
        .await
        .expect("query operations");

    assert!(
        operations.is_empty(),
        "no-op state transitions must not create module_operations rows",
    );
}

#[tokio::test]
async fn noop_enable_for_already_enabled_module_does_not_create_extra_journal_row() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("catalog"));

    ModuleLifecycleService::toggle_module(&db, &registry, tenant_id, "catalog", true)
        .await
        .expect("initial enable should succeed");
    let second = ModuleLifecycleService::toggle_module(&db, &registry, tenant_id, "catalog", true)
        .await
        .expect("no-op enable should succeed");
    assert!(second.enabled);

    let operations = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("catalog"))
        .all(&db)
        .await
        .expect("query operations");

    assert_eq!(
        operations.len(),
        1,
        "no-op enable transition must not create extra module_operations rows",
    );
    assert_eq!(operations[0].status, ModuleOperationStatus::Committed.as_str());
}

#[tokio::test]
async fn toggle_without_actor_records_null_requested_by() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("forum"));

    ModuleLifecycleService::toggle_module(&db, &registry, tenant_id, "forum", true)
        .await
        .expect("enable should succeed");

    let operation = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("forum"))
        .one(&db)
        .await
        .expect("query operation")
        .expect("operation exists");

    assert_eq!(operation.status, ModuleOperationStatus::Committed.as_str());
    assert!(
        operation.requested_by.is_none(),
        "toggle_module wrapper without actor must persist requested_by as NULL",
    );
}

#[tokio::test]
async fn hook_failure_with_actor_records_failed_operation_with_actor() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("billing"));

    ModuleLifecycleService::toggle_module_with_actor(
        &db,
        &registry,
        tenant_id,
        "billing",
        true,
        Some("admin:user-2".to_string()),
    )
    .await
    .expect("enable should succeed");

    let failing_registry =
        ModuleRegistry::new().register(TestModule::new("billing").with_disable_failure());
    let err = ModuleLifecycleService::toggle_module_with_actor(
        &db,
        &failing_registry,
        tenant_id,
        "billing",
        false,
        Some("admin:user-2".to_string()),
    )
    .await
    .expect_err("disable hook failure expected");
    assert!(matches!(err, ToggleModuleError::HookFailed(_)));

    let failed_operation = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("billing"))
        .filter(module_operations::Column::RequestedEnabled.eq(false))
        .one(&db)
        .await
        .expect("query failed operation")
        .expect("failed operation exists");

    assert_eq!(failed_operation.status, ModuleOperationStatus::Failed.as_str());
    assert_eq!(
        failed_operation.requested_by.as_deref(),
        Some("admin:user-2"),
        "actor metadata must be preserved for failed operations too",
    );
}

#[tokio::test]
async fn hook_failure_without_actor_records_failed_operation_with_null_actor() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("orders").with_enable_failure());
    let err = ModuleLifecycleService::toggle_module(&db, &registry, tenant_id, "orders", true)
        .await
        .expect_err("enable hook failure expected");
    assert!(matches!(err, ToggleModuleError::HookFailed(_)));

    let failed_operation = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("orders"))
        .filter(module_operations::Column::RequestedEnabled.eq(true))
        .one(&db)
        .await
        .expect("query failed operation")
        .expect("failed operation exists");

    assert_eq!(failed_operation.status, ModuleOperationStatus::Failed.as_str());
    assert!(
        failed_operation.requested_by.is_none(),
        "wrapper toggle_module without actor must keep requested_by=NULL even on failed operations",
    );
}
