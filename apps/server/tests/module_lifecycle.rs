use async_trait::async_trait;
use rustok_core::{ModuleContext, ModuleKind, ModuleRegistry, RusToKModule};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_modules::{
    ModuleCommandContext, ModuleControlPlane, ModuleOperationIssue, ModuleOperationRecoveryAction,
    ModuleOperationStatus,
};
use rustok_outbox::SysEventsMigration;
use rustok_server::models::_entities::{module_operations, tenant_modules};
use rustok_server::modules::ModulesManifest;
use rustok_server::services::module_lifecycle::{
    ModuleLifecycleService as OwnerModuleLifecycleService, ModuleLifecycleStateSnapshot,
    ModuleOperationRecoveryError, ToggleModuleError,
};
use rustok_server::services::platform_composition::PlatformCompositionService;
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    EntityTrait, QueryFilter, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid::Uuid;

fn command_context(tenant_id: Uuid, actor_id: Uuid, idempotency_key: Uuid) -> ModuleCommandContext {
    ModuleCommandContext {
        actor_id,
        tenant_id: Some(tenant_id),
        trace_id: format!("test:static-lifecycle:{idempotency_key}"),
        correlation_id: idempotency_key,
        idempotency_key,
    }
}

async fn toggle_with_actor(
    db: &DatabaseConnection,
    registry: &ModuleRegistry,
    tenant_id: Uuid,
    module_slug: &str,
    enabled: bool,
    actor_id: Uuid,
) -> Result<ModuleLifecycleStateSnapshot, ToggleModuleError> {
    let expected_revision = static_lifecycle_revision(db, tenant_id, module_slug).await;
    OwnerModuleLifecycleService::toggle_module(
        db,
        registry,
        tenant_id,
        module_slug,
        enabled,
        command_context(tenant_id, actor_id, Uuid::new_v4()),
        expected_revision,
    )
    .await
}

async fn static_lifecycle_revision(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    module_slug: &str,
) -> u64 {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "SELECT revision FROM module_static_tenant_lifecycle WHERE tenant_id = ?1 AND module_slug = ?2 LIMIT 1",
        vec![tenant_id.into(), module_slug.into()],
    ))
    .await
    .expect("read static lifecycle revision")
    .map(|row| row.try_get::<i64>("", "revision").expect("valid static lifecycle revision"))
    .map(|revision| u64::try_from(revision).expect("non-negative static lifecycle revision"))
    .unwrap_or(0)
}

async fn toggle_with_fresh_command(
    db: &DatabaseConnection,
    registry: &ModuleRegistry,
    tenant_id: Uuid,
    module_slug: &str,
    enabled: bool,
) -> Result<ModuleLifecycleStateSnapshot, ToggleModuleError> {
    toggle_with_actor(
        db,
        registry,
        tenant_id,
        module_slug,
        enabled,
        Uuid::new_v4(),
    )
    .await
}

struct TestModule {
    slug: &'static str,
    should_fail_enable: bool,
    should_fail_disable: bool,
    should_fail_post_enable: bool,
    should_fail_post_disable: bool,
    enable_calls: Arc<AtomicUsize>,
    disable_calls: Arc<AtomicUsize>,
    post_enable_calls: Arc<AtomicUsize>,
    post_disable_calls: Arc<AtomicUsize>,
}

struct FlakyPostHookModule {
    slug: &'static str,
    post_enable_calls: Arc<AtomicUsize>,
}

impl FlakyPostHookModule {
    fn new(slug: &'static str) -> Self {
        Self {
            slug,
            post_enable_calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl rustok_core::MigrationSource for FlakyPostHookModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        vec![]
    }
}

#[async_trait]
impl RusToKModule for FlakyPostHookModule {
    fn slug(&self) -> &'static str {
        self.slug
    }

    fn name(&self) -> &'static str {
        "flaky-post-hook-module"
    }

    fn description(&self) -> &'static str {
        "test module with retryable post-hook"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    async fn post_enable(&self, _ctx: ModuleContext<'_>) -> rustok_core::Result<()> {
        let previous_calls = self.post_enable_calls.fetch_add(1, Ordering::SeqCst);
        if previous_calls == 0 {
            return Err(rustok_core::Error::External(
                "transient post enable failure".to_string(),
            ));
        }
        Ok(())
    }
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
            should_fail_post_enable: false,
            should_fail_post_disable: false,
            enable_calls: Arc::new(AtomicUsize::new(0)),
            disable_calls: Arc::new(AtomicUsize::new(0)),
            post_enable_calls: Arc::new(AtomicUsize::new(0)),
            post_disable_calls: Arc::new(AtomicUsize::new(0)),
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

    fn with_post_enable_failure(mut self) -> Self {
        self.should_fail_post_enable = true;
        self
    }

    fn with_post_disable_failure(mut self) -> Self {
        self.should_fail_post_disable = true;
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

    async fn post_enable(&self, _ctx: ModuleContext<'_>) -> rustok_core::Result<()> {
        self.post_enable_calls.fetch_add(1, Ordering::SeqCst);
        if self.should_fail_post_enable {
            return Err(rustok_core::Error::External(
                "post enable failed".to_string(),
            ));
        }
        Ok(())
    }

    async fn post_disable(&self, _ctx: ModuleContext<'_>) -> rustok_core::Result<()> {
        self.post_disable_calls.fetch_add(1, Ordering::SeqCst);
        if self.should_fail_post_disable {
            return Err(rustok_core::Error::External(
                "post disable failed".to_string(),
            ));
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

    fn dependencies(&self) -> &[&'static str] {
        std::slice::from_ref(&self.dependency)
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
    let url = format!(
        "sqlite:file:module_lifecycle_{}?mode=memory&cache=shared",
        uuid::Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.expect("db connect");

    SysEventsMigration
        .up(&SchemaManager::new(&db))
        .await
        .expect("create sys_events");

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
        CREATE TABLE platform_state (
            id TEXT PRIMARY KEY,
            revision INTEGER NOT NULL,
            manifest_json TEXT NOT NULL,
            manifest_hash TEXT NOT NULL,
            updated_by TEXT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    ))
    .await
    .expect("create platform_state");

    let manifest = PlatformCompositionService::manifest_snapshot_json(&ModulesManifest::default())
        .expect("serialize isolated test composition");
    ModuleControlPlane::new(db.clone())
        .composition()
        .ensure_active_snapshot(&manifest, "test-bootstrap")
        .await
        .expect("seed isolated test composition");

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        r#"
        CREATE TABLE module_policy_revision_cursors (
            tenant_id TEXT NOT NULL,
            consumer_key TEXT NOT NULL CHECK (length(trim(consumer_key)) BETWEEN 1 AND 128),
            current_revision TEXT NULL CHECK (current_revision IS NULL OR length(current_revision) = 71),
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (tenant_id, consumer_key)
        );
        "#,
    ))
    .await
    .expect("create module_policy_revision_cursors");

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
            trace_id TEXT NULL,
            correlation_id TEXT NULL,
            idempotency_key TEXT NULL,
            expected_revision INTEGER NULL,
            error_message TEXT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
        );
        "#,
    ))
    .await
    .expect("create module_operations");

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        r#"
        CREATE TABLE module_static_tenant_lifecycle (
            tenant_id TEXT NOT NULL,
            module_slug TEXT NOT NULL,
            revision INTEGER NOT NULL DEFAULT 0,
            active_idempotency_key TEXT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (tenant_id, module_slug),
            FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
        );
        "#,
    ))
    .await
    .expect("create module_static_tenant_lifecycle");

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        r#"
        CREATE TABLE module_operation_override_states (
            operation_id TEXT PRIMARY KEY,
            previous_override_enabled BOOLEAN NULL,
            requested_override_enabled BOOLEAN NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (operation_id) REFERENCES module_operations(id) ON DELETE CASCADE
        );
        "#,
    ))
    .await
    .expect("create module_operation_override_states");

    db
}

async fn seed_tenant(db: &DatabaseConnection, tenant_id: uuid::Uuid) {
    let slug = format!("tenant-{}", tenant_id.simple());
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id, name, slug, settings, is_active, created_at, updated_at) VALUES (?, ?, ?, '{}', 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![tenant_id.into(), "Tenant".into(), slug.into()],
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
    let actor_id = Uuid::new_v4();
    let idempotency_key = Uuid::new_v4();

    let enabled = OwnerModuleLifecycleService::toggle_module(
        &db,
        &registry,
        tenant_id,
        "commerce",
        true,
        command_context(tenant_id, actor_id, idempotency_key),
        0,
    )
    .await
    .expect("first enable");
    assert!(enabled.enabled);

    let second = OwnerModuleLifecycleService::toggle_module(
        &db,
        &registry,
        tenant_id,
        "commerce",
        true,
        command_context(tenant_id, actor_id, idempotency_key),
        0,
    )
    .await
    .expect("exact retry");
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
    assert_eq!(
        operations[0].trace_id.as_deref(),
        Some(format!("test:static-lifecycle:{idempotency_key}").as_str()),
        "the journal must retain the command trace identity",
    );
}

#[tokio::test]
async fn pre_enable_failure_keeps_state_uncommitted() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("forum").with_enable_failure());

    let err = toggle_with_fresh_command(&db, &registry, tenant_id, "forum", true)
        .await
        .expect_err("enable should fail");

    assert!(matches!(err, ToggleModuleError::PreHookFailed(_)));

    let state = tenant_modules::Entity::find()
        .filter(tenant_modules::Column::TenantId.eq(tenant_id))
        .filter(tenant_modules::Column::ModuleSlug.eq("forum"))
        .one(&db)
        .await
        .expect("load state");

    assert!(
        state.is_none(),
        "pre-enable hook failure must not persist an explicit tenant override",
    );

    let operation = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("forum"))
        .one(&db)
        .await
        .expect("load operation")
        .expect("operation exists");

    assert_eq!(operation.status, ModuleOperationStatus::Failed.as_str());
    assert!(
        operation
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("enable failed")
    );
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

    let first = toggle_with_fresh_command(&db, &registry, tenant_id, "blog", true);
    let second = toggle_with_fresh_command(&db, &registry, tenant_id, "blog", false);

    let (r1, r2) = tokio::join!(first, second);
    assert!(
        r1.is_ok() ^ r2.is_ok(),
        "concurrent distinct transitions must commit exactly one policy successor: first={r1:?}, second={r2:?}",
    );
    let rejected = r1
        .as_ref()
        .err()
        .or(r2.as_ref().err())
        .expect("one concurrent transition must be rejected");
    assert!(
        matches!(rejected, ToggleModuleError::OperationInProgress)
            || matches!(rejected, ToggleModuleError::Policy(message) if message.contains("durable cursor: Stale")),
        "the competing transition must fail closed at the aggregate or durable predecessor gate: {rejected:?}",
    );

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

    let enabled = toggle_with_fresh_command(&db, &registry, tenant_id, "pricing", true)
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

    let policy_event = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT payload FROM sys_events WHERE event_type = ?1".to_string(),
            vec!["module.effective_policy_revision_changed".into()],
        ))
        .await
        .expect("load effective-policy event")
        .expect("effective-policy transition event exists");
    let payload: serde_json::Value = policy_event
        .try_get("", "payload")
        .expect("effective-policy event payload decodes");
    let envelope: EventEnvelope =
        serde_json::from_value(payload).expect("effective-policy event envelope decodes");
    assert_eq!(envelope.tenant_id, tenant_id);
    assert!(matches!(
        envelope.event,
        DomainEvent::ModuleEffectivePolicyRevisionChanged {
            ref consumer_key,
            previous_revision: None,
            ref next_revision,
        } if consumer_key == "module.lifecycle" && next_revision.starts_with("sha256:")
    ));
}

#[tokio::test]
async fn successful_toggle_with_actor_persists_requested_by() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("catalog"));
    let actor_id = Uuid::new_v4();

    toggle_with_actor(&db, &registry, tenant_id, "catalog", true, actor_id)
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
    assert_eq!(operation.requested_by, Some(actor_id.to_string()));
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

    let err = toggle_with_fresh_command(&db, &registry, tenant_id, "checkout", true)
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

    toggle_with_fresh_command(&db, &registry, tenant_id, "pricing", true)
        .await
        .expect("enable dependency first");
    toggle_with_fresh_command(&db, &registry, tenant_id, "checkout", true)
        .await
        .expect("enable dependent second");

    let err = toggle_with_fresh_command(&db, &registry, tenant_id, "pricing", false)
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
    assert_eq!(
        operations[0].status,
        ModuleOperationStatus::Committed.as_str()
    );
    assert!(operations[0].requested_enabled);
}

#[tokio::test]
async fn unknown_module_failure_does_not_create_journal_row() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("pricing"));

    let err = toggle_with_fresh_command(&db, &registry, tenant_id, "unknown", true)
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

    let err = toggle_with_fresh_command(&db, &registry, tenant_id, "tenant", false)
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
async fn repeated_explicit_disable_records_a_distinct_no_op_receipt() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("inventory"));

    let module = toggle_with_fresh_command(&db, &registry, tenant_id, "inventory", false)
        .await
        .expect("explicit disable should succeed");
    assert!(!module.enabled);

    let repeated = toggle_with_fresh_command(&db, &registry, tenant_id, "inventory", false)
        .await
        .expect("repeated explicit disable should succeed");
    assert!(!repeated.enabled);

    let operations = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("inventory"))
        .all(&db)
        .await
        .expect("query operations");

    assert_eq!(
        operations.len(),
        2,
        "a repeated explicit disable is a distinct no-op command with its own receipt",
    );
    assert_eq!(
        operations[0].status,
        ModuleOperationStatus::Committed.as_str()
    );
}

#[tokio::test]
async fn noop_enable_for_already_enabled_module_records_a_receipt() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("catalog"));

    toggle_with_fresh_command(&db, &registry, tenant_id, "catalog", true)
        .await
        .expect("initial enable should succeed");
    let second = toggle_with_fresh_command(&db, &registry, tenant_id, "catalog", true)
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
        2,
        "a distinct no-op command must retain its own committed lifecycle receipt",
    );
    assert_eq!(
        operations[0].status,
        ModuleOperationStatus::Committed.as_str()
    );
}

#[tokio::test]
async fn toggle_records_authenticated_actor_identity() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("forum"));
    let actor_id = Uuid::new_v4();

    toggle_with_actor(&db, &registry, tenant_id, "forum", true, actor_id)
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
    assert_eq!(operation.requested_by, Some(actor_id.to_string()));
}

#[tokio::test]
async fn hook_failure_with_actor_records_failed_operation_with_actor() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("billing"));
    let actor_id = Uuid::new_v4();

    toggle_with_actor(&db, &registry, tenant_id, "billing", true, actor_id)
        .await
        .expect("enable should succeed");

    let failing_registry =
        ModuleRegistry::new().register(TestModule::new("billing").with_disable_failure());
    let err = toggle_with_actor(
        &db,
        &failing_registry,
        tenant_id,
        "billing",
        false,
        actor_id,
    )
    .await
    .expect_err("disable hook failure expected");
    assert!(matches!(err, ToggleModuleError::PreHookFailed(_)));

    let state = tenant_modules::Entity::find()
        .filter(tenant_modules::Column::TenantId.eq(tenant_id))
        .filter(tenant_modules::Column::ModuleSlug.eq("billing"))
        .one(&db)
        .await
        .expect("load billing state")
        .expect("billing state exists");
    assert!(
        state.enabled,
        "pre-disable hook failure must keep previous committed state",
    );

    let failed_operation = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("billing"))
        .filter(module_operations::Column::RequestedEnabled.eq(false))
        .one(&db)
        .await
        .expect("query failed operation")
        .expect("failed operation exists");

    assert_eq!(
        failed_operation.status,
        ModuleOperationStatus::Failed.as_str()
    );
    assert!(
        failed_operation.previous_effective_enabled,
        "pre-disable failure must retain the previously effective enabled state",
    );
    assert_eq!(
        failed_operation.requested_by,
        Some(actor_id.to_string()),
        "actor metadata must be preserved for failed operations too",
    );
    assert!(
        failed_operation.correlation_id.is_some(),
        "failed pre-disable operation must keep correlation id for retry/audit tracing",
    );
    let correlation_id = failed_operation
        .correlation_id
        .as_deref()
        .expect("failed operation must have correlation id");
    let parsed = uuid::Uuid::parse_str(correlation_id).expect("correlation id must be uuid");
    assert_eq!(parsed.get_version_num(), 4);

    let operations = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("billing"))
        .all(&db)
        .await
        .expect("load billing operations");
    assert_eq!(
        operations.len(),
        2,
        "enable + pre-disable failure must produce exactly two lifecycle journal rows",
    );
}

#[tokio::test]
async fn hook_failure_records_failed_operation_with_authenticated_actor() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("orders").with_enable_failure());
    let actor_id = Uuid::new_v4();
    let err = toggle_with_actor(&db, &registry, tenant_id, "orders", true, actor_id)
        .await
        .expect_err("enable hook failure expected");
    assert!(matches!(err, ToggleModuleError::PreHookFailed(_)));

    let failed_operation = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("orders"))
        .filter(module_operations::Column::RequestedEnabled.eq(true))
        .one(&db)
        .await
        .expect("query failed operation")
        .expect("failed operation exists");

    assert_eq!(
        failed_operation.status,
        ModuleOperationStatus::Failed.as_str()
    );
    assert!(
        !failed_operation.previous_effective_enabled,
        "pre-enable failure must retain the previously effective disabled state",
    );
    assert_eq!(failed_operation.requested_by, Some(actor_id.to_string()));
    assert!(
        failed_operation.correlation_id.is_some(),
        "failed pre-enable operation must keep correlation id for retry/audit tracing",
    );
    let correlation_id = failed_operation
        .correlation_id
        .as_deref()
        .expect("failed operation must have correlation id");
    let parsed = uuid::Uuid::parse_str(correlation_id).expect("correlation id must be uuid");
    assert_eq!(parsed.get_version_num(), 4);

    let operations = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("orders"))
        .all(&db)
        .await
        .expect("load order operations");
    assert_eq!(
        operations.len(),
        1,
        "single pre-enable failure attempt must produce exactly one lifecycle journal row",
    );
}

#[tokio::test]
async fn post_enable_failure_keeps_committed_state_and_marks_failed_operation() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let failing_module = TestModule::new("search").with_post_enable_failure();
    let post_enable_calls = failing_module.post_enable_calls.clone();
    let registry = ModuleRegistry::new().register(failing_module);
    let actor_id = Uuid::new_v4();
    let err = toggle_with_actor(&db, &registry, tenant_id, "search", true, actor_id)
        .await
        .expect_err("post-enable failure expected");
    assert!(matches!(err, ToggleModuleError::PostHookFailed(_)));

    let state = tenant_modules::Entity::find()
        .filter(tenant_modules::Column::TenantId.eq(tenant_id))
        .filter(tenant_modules::Column::ModuleSlug.eq("search"))
        .one(&db)
        .await
        .expect("load state")
        .expect("state row exists");
    assert!(
        state.enabled,
        "post-hook failure must keep committed enabled state",
    );

    let failed_operation = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("search"))
        .filter(module_operations::Column::RequestedEnabled.eq(true))
        .one(&db)
        .await
        .expect("query failed operation")
        .expect("failed operation exists");
    assert_eq!(
        failed_operation.status,
        ModuleOperationStatus::Failed.as_str()
    );
    assert!(
        failed_operation
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("post-hook")
    );
    assert_eq!(
        failed_operation.requested_by,
        Some(actor_id.to_string()),
        "post-hook failed operation must keep actor metadata for retry/audit attribution",
    );
    assert!(
        failed_operation.correlation_id.is_some(),
        "post-hook failure operation must keep correlation id for retry/audit tracing",
    );
    let correlation_id = failed_operation
        .correlation_id
        .as_deref()
        .expect("failed operation must have correlation id");
    let parsed = uuid::Uuid::parse_str(correlation_id).expect("correlation id must be uuid");
    assert_eq!(parsed.get_version_num(), 4);

    let operations = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("search"))
        .all(&db)
        .await
        .expect("load search operations");
    assert_eq!(
        operations.len(),
        1,
        "single post-enable failure attempt must produce exactly one journal row",
    );

    let retry = toggle_with_fresh_command(&db, &registry, tenant_id, "search", true)
        .await
        .expect("retry enable after committed post-hook failure should be a no-op");
    assert!(retry.enabled);

    let operations_after_retry = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("search"))
        .all(&db)
        .await
        .expect("load search operations after retry");
    assert_eq!(
        operations_after_retry.len(),
        2,
        "a new no-op command after a committed post-enable failure records its receipt",
    );
    assert_eq!(
        post_enable_calls.load(Ordering::SeqCst),
        1,
        "a no-op command after committed post-enable failure must not invoke post-enable hook again",
    );
}

#[tokio::test]
async fn post_disable_failure_keeps_committed_state_and_marks_failed_operation() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("search"));
    toggle_with_fresh_command(&db, &registry, tenant_id, "search", true)
        .await
        .expect("enable should succeed");

    let failing_module = TestModule::new("search").with_post_disable_failure();
    let post_disable_calls = failing_module.post_disable_calls.clone();
    let failing_registry = ModuleRegistry::new().register(failing_module);
    let actor_id = Uuid::new_v4();
    let err = toggle_with_actor(&db, &failing_registry, tenant_id, "search", false, actor_id)
        .await
        .expect_err("post-disable failure expected");
    assert!(matches!(err, ToggleModuleError::PostHookFailed(_)));

    let state = tenant_modules::Entity::find()
        .filter(tenant_modules::Column::TenantId.eq(tenant_id))
        .filter(tenant_modules::Column::ModuleSlug.eq("search"))
        .one(&db)
        .await
        .expect("load state")
        .expect("state row exists");
    assert!(
        !state.enabled,
        "post-hook failure must keep committed disabled state",
    );

    let failed_operation = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("search"))
        .filter(module_operations::Column::RequestedEnabled.eq(false))
        .one(&db)
        .await
        .expect("query failed operation")
        .expect("failed operation exists");
    assert_eq!(
        failed_operation.status,
        ModuleOperationStatus::Failed.as_str()
    );
    assert!(
        failed_operation
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("post-hook")
    );
    assert_eq!(
        failed_operation.requested_by,
        Some(actor_id.to_string()),
        "post-hook failed operation must keep actor metadata for retry/audit attribution",
    );
    assert!(
        failed_operation.correlation_id.is_some(),
        "post-hook failure operation must keep correlation id for retry/audit tracing",
    );
    let correlation_id = failed_operation
        .correlation_id
        .as_deref()
        .expect("failed operation must have correlation id");
    let parsed = uuid::Uuid::parse_str(correlation_id).expect("correlation id must be uuid");
    assert_eq!(parsed.get_version_num(), 4);

    let operations = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("search"))
        .all(&db)
        .await
        .expect("load search operations");
    assert_eq!(
        operations.len(),
        2,
        "enable + failed disable must produce exactly two lifecycle journal rows",
    );

    let retry = toggle_with_fresh_command(&db, &failing_registry, tenant_id, "search", false)
        .await
        .expect("retry disable after committed post-hook failure should be a no-op");
    assert!(!retry.enabled);

    let operations_after_retry = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("search"))
        .all(&db)
        .await
        .expect("load search operations after retry");
    assert_eq!(
        operations_after_retry.len(),
        3,
        "a new no-op command after a committed post-disable failure records its receipt",
    );
    assert_eq!(
        post_disable_calls.load(Ordering::SeqCst),
        1,
        "a no-op command after committed post-disable failure must not invoke post-disable hook again",
    );
}

#[tokio::test]
async fn retry_failed_post_hook_operation_records_committed_recovery_attempt() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let module = FlakyPostHookModule::new("analytics");
    let post_enable_calls = module.post_enable_calls.clone();
    let registry = ModuleRegistry::new().register(module);

    let err = toggle_with_fresh_command(&db, &registry, tenant_id, "analytics", true)
        .await
        .expect_err("first post-enable attempt should fail");
    assert!(matches!(err, ToggleModuleError::PostHookFailed(_)));

    let failed_operation = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("analytics"))
        .one(&db)
        .await
        .expect("query failed operation")
        .expect("failed operation exists");

    let plan = OwnerModuleLifecycleService::module_operation_recovery_plan(
        &db,
        &registry,
        tenant_id,
        failed_operation.id,
    )
    .await
    .expect("load recovery plan");
    assert_eq!(plan.issue, ModuleOperationIssue::PostHookFailed);
    assert!(plan.retryable);
    assert_eq!(
        plan.recommended_action,
        ModuleOperationRecoveryAction::RetryPostHook
    );
    assert!(plan.correlation_id.is_some());

    let foreign_tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, foreign_tenant_id).await;
    let foreign_plan = OwnerModuleLifecycleService::module_operation_recovery_plan(
        &db,
        &registry,
        foreign_tenant_id,
        failed_operation.id,
    )
    .await
    .expect_err("recovery plans must not cross the authenticated tenant boundary");
    assert!(matches!(
        foreign_plan,
        ModuleOperationRecoveryError::OperationNotFound
    ));
    let foreign_retry = OwnerModuleLifecycleService::retry_failed_post_hook_operation(
        &db,
        &registry,
        foreign_tenant_id,
        failed_operation.id,
        command_context(foreign_tenant_id, Uuid::new_v4(), uuid::Uuid::new_v4()),
        0,
    )
    .await
    .expect_err("recovery must not cross the authenticated tenant boundary");
    assert!(matches!(
        foreign_retry,
        ModuleOperationRecoveryError::OperationNotFound
    ));

    let retry_actor_id = Uuid::new_v4();
    let retry_idempotency_key = uuid::Uuid::new_v4();
    let retry_operation = OwnerModuleLifecycleService::retry_failed_post_hook_operation(
        &db,
        &registry,
        tenant_id,
        failed_operation.id,
        command_context(tenant_id, retry_actor_id, retry_idempotency_key),
        1,
    )
    .await
    .expect("post-hook retry should succeed");

    assert_eq!(retry_operation.status, ModuleOperationStatus::Committed);
    assert_eq!(
        retry_operation.requested_by,
        Some(retry_actor_id.to_string())
    );
    assert!(retry_operation.requested_enabled);
    assert!(
        !retry_operation.previous_effective_enabled,
        "the retry must preserve the original operation's historical availability",
    );
    assert!(retry_operation.error_message.is_none());
    assert_ne!(retry_operation.operation_id, failed_operation.id);
    assert_eq!(
        retry_operation.correlation_id,
        Some(retry_idempotency_key.to_string())
    );
    assert_eq!(
        retry_operation.trace_id.as_deref(),
        Some(format!("test:static-lifecycle:{retry_idempotency_key}").as_str())
    );
    assert_eq!(
        post_enable_calls.load(Ordering::SeqCst),
        2,
        "explicit post-hook retry should invoke post_enable once more"
    );

    let operations = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("analytics"))
        .all(&db)
        .await
        .expect("load analytics operations");
    assert_eq!(operations.len(), 2);

    let replayed_operation = OwnerModuleLifecycleService::retry_failed_post_hook_operation(
        &db,
        &registry,
        tenant_id,
        failed_operation.id,
        command_context(tenant_id, retry_actor_id, retry_idempotency_key),
        1,
    )
    .await
    .expect("same retry idempotency key should replay the journal operation");
    assert_eq!(
        replayed_operation.operation_id,
        retry_operation.operation_id
    );
    assert_eq!(post_enable_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn retry_failed_post_hook_operation_rejects_pre_hook_failures() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let registry = ModuleRegistry::new().register(TestModule::new("orders").with_enable_failure());
    let err = toggle_with_fresh_command(&db, &registry, tenant_id, "orders", true)
        .await
        .expect_err("pre-enable failure expected");
    assert!(matches!(err, ToggleModuleError::PreHookFailed(_)));

    let failed_operation = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("orders"))
        .one(&db)
        .await
        .expect("query failed operation")
        .expect("failed operation exists");

    let plan = OwnerModuleLifecycleService::module_operation_recovery_plan(
        &db,
        &registry,
        tenant_id,
        failed_operation.id,
    )
    .await
    .expect("load recovery plan");
    assert_eq!(plan.issue, ModuleOperationIssue::PreHookFailed);
    assert!(!plan.retryable);
    assert_eq!(
        plan.recommended_action,
        ModuleOperationRecoveryAction::RepeatToggle
    );

    let err = OwnerModuleLifecycleService::retry_failed_post_hook_operation(
        &db,
        &registry,
        tenant_id,
        failed_operation.id,
        command_context(tenant_id, Uuid::new_v4(), uuid::Uuid::new_v4()),
        0,
    )
    .await
    .expect_err("pre-hook failures are not post-hook retryable");
    assert!(matches!(err, ModuleOperationRecoveryError::NotRetryable(_)));
}

#[tokio::test]
async fn compensation_replays_its_reverse_lifecycle_operation_for_the_same_key() {
    let db = setup_db().await;
    let tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let module = TestModule::new("billing").with_post_enable_failure();
    let post_disable_calls = module.post_disable_calls.clone();
    let registry = ModuleRegistry::new().register(module);

    let err = toggle_with_fresh_command(&db, &registry, tenant_id, "billing", true)
        .await
        .expect_err("post-enable failure should leave a compensable operation");
    assert!(matches!(err, ToggleModuleError::PostHookFailed(_)));

    let failed_operation = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::ModuleSlug.eq("billing"))
        .one(&db)
        .await
        .expect("query failed operation")
        .expect("failed operation exists");

    let foreign_tenant_id = uuid::Uuid::new_v4();
    seed_tenant(&db, foreign_tenant_id).await;
    let foreign_compensation = OwnerModuleLifecycleService::compensate_failed_operation(
        &db,
        &registry,
        foreign_tenant_id,
        failed_operation.id,
        command_context(foreign_tenant_id, Uuid::new_v4(), uuid::Uuid::new_v4()),
        0,
    )
    .await
    .expect_err("compensation must not cross the authenticated tenant boundary");
    assert!(matches!(
        foreign_compensation,
        ModuleOperationRecoveryError::OperationNotFound
    ));

    let actor_id = Uuid::new_v4();
    let idempotency_key = uuid::Uuid::new_v4();

    let compensated = OwnerModuleLifecycleService::compensate_failed_operation(
        &db,
        &registry,
        tenant_id,
        failed_operation.id,
        command_context(tenant_id, actor_id, idempotency_key),
        1,
    )
    .await
    .expect("compensation should disable the module");
    assert_eq!(compensated.module_slug, "billing");
    assert!(!compensated.enabled);

    let replayed = OwnerModuleLifecycleService::compensate_failed_operation(
        &db,
        &registry,
        tenant_id,
        failed_operation.id,
        command_context(tenant_id, actor_id, idempotency_key),
        1,
    )
    .await
    .expect("same compensation key should replay the reverse operation");
    assert_eq!(replayed.module_slug, "billing");
    assert_eq!(replayed.operation_id, compensated.operation_id);
    assert!(compensated.operation_id.is_some());
    assert_eq!(post_disable_calls.load(Ordering::SeqCst), 1);

    let compensation = module_operations::Entity::find()
        .filter(module_operations::Column::TenantId.eq(tenant_id))
        .filter(module_operations::Column::IdempotencyKey.eq(idempotency_key))
        .one(&db)
        .await
        .expect("query compensation operation")
        .expect("compensation operation exists");
    assert_eq!(compensated.operation_id, Some(compensation.id));
    assert_eq!(
        compensation.status,
        ModuleOperationStatus::Committed.as_str()
    );
    assert_eq!(
        compensation.correlation_id,
        Some(failed_operation.id.to_string())
    );
}
