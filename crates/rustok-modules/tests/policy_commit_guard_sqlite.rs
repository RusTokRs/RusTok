use async_trait::async_trait;
use rustok_core::{MigrationSource, ModuleRegistry, RusToKModule};
use rustok_modules::SeaOrmModulePolicyRevisionConsumer;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
    TransactionTrait,
};
use sea_orm_migration::MigrationTrait;
use uuid::Uuid;

struct NotificationsTestModule;

impl MigrationSource for NotificationsTestModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[async_trait]
impl RusToKModule for NotificationsTestModule {
    fn slug(&self) -> &'static str {
        "notifications"
    }

    fn name(&self) -> &'static str {
        "Notifications test"
    }

    fn description(&self) -> &'static str {
        "Transaction-bound effective policy test module"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }
}

#[tokio::test]
async fn static_policy_resolves_tenant_override_under_lifecycle_cursor_lock() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let registry = ModuleRegistry::new().register(NotificationsTestModule);
    let consumer = SeaOrmModulePolicyRevisionConsumer::new(db.clone());

    let enabled_txn = db.begin().await.expect("enabled policy transaction");
    let enabled = consumer
        .lock_and_resolve_static_policy_in_transaction(
            &enabled_txn,
            tenant_id,
            "module.lifecycle",
            &registry,
            vec!["notifications".to_string()],
        )
        .await
        .expect("default-enabled policy should resolve");
    assert!(enabled.contains("notifications"));
    let enabled_revision = enabled.policy_revision().to_string();
    enabled_txn.commit().await.expect("enabled policy commit");

    let disabled_txn = db.begin().await.expect("disabled policy transaction");
    disabled_txn
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenant_modules (tenant_id, module_slug, enabled) VALUES (?1, ?2, ?3)"
                .to_string(),
            vec![
                tenant_id.to_string().into(),
                "notifications".to_string().into(),
                false.into(),
            ],
        ))
        .await
        .expect("disabled tenant override should persist in transaction");
    let disabled = consumer
        .lock_and_resolve_static_policy_in_transaction(
            &disabled_txn,
            tenant_id,
            "module.lifecycle",
            &registry,
            vec!["notifications".to_string()],
        )
        .await
        .expect("transaction-bound disabled policy should resolve");
    assert!(!disabled.contains("notifications"));
    assert_ne!(disabled.policy_revision(), enabled_revision);
    disabled_txn.commit().await.expect("disabled policy commit");

    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT current_revision FROM module_policy_revision_cursors WHERE tenant_id = ?1 AND consumer_key = ?2"
                .to_string(),
            vec![
                tenant_id.to_string().into(),
                "module.lifecycle".to_string().into(),
            ],
        ))
        .await
        .expect("cursor query should succeed")
        .expect("cursor row should exist");
    let current_revision: Option<String> = row
        .try_get("", "current_revision")
        .expect("cursor revision should decode");
    assert!(
        current_revision.is_none(),
        "commit guard locks but never advances the lifecycle owner cursor"
    );
}

async fn setup() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:module_policy_commit_guard_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("policy guard sqlite database");
    db.execute_unprepared(
        r#"
        CREATE TABLE tenant_modules (
            tenant_id TEXT NOT NULL,
            module_slug TEXT NOT NULL,
            enabled BOOLEAN NOT NULL,
            PRIMARY KEY (tenant_id, module_slug)
        );
        CREATE TABLE module_policy_revision_cursors (
            tenant_id TEXT NOT NULL,
            consumer_key TEXT NOT NULL,
            current_revision TEXT NULL,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (tenant_id, consumer_key)
        );
        "#,
    )
    .await
    .expect("policy guard schema");
    db
}
