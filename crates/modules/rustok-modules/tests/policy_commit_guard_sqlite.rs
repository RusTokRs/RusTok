use async_trait::async_trait;
use rustok_core::{MigrationSource, ModuleRegistry, RusToKModule};
use rustok_modules::{
    ModulePolicyRevisionApplyOutcome, ModulePolicyRevisionConsumerError,
    ModulePolicyRevisionCursorState, ModulePolicyRevisionTransition,
    SeaOrmModulePolicyRevisionConsumer,
};
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
        .execute_raw(Statement::from_sql_and_values(
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
        .query_one_raw(Statement::from_sql_and_values(
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

#[tokio::test]
async fn lifecycle_cursor_sequences_exact_successors_and_requires_explicit_resync() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let consumer = SeaOrmModulePolicyRevisionConsumer::new(db);
    let first_revision = format!("sha256:{}", "a".repeat(64));
    let second_revision = format!("sha256:{}", "b".repeat(64));
    let third_revision = format!("sha256:{}", "c".repeat(64));
    let fourth_revision = format!("sha256:{}", "d".repeat(64));

    let initial_state = consumer
        .cursor_state(tenant_id, "module.lifecycle")
        .await
        .expect("uninitialized cursor should resolve");
    assert_eq!(
        initial_state,
        ModulePolicyRevisionCursorState {
            current_revision: None,
            current_sequence: 0,
        }
    );

    let first = ModulePolicyRevisionTransition {
        previous_revision: None,
        next_revision: first_revision.clone(),
    };
    assert_eq!(
        consumer
            .apply(tenant_id, "module.lifecycle", &first)
            .await
            .expect("first successor should advance"),
        ModulePolicyRevisionApplyOutcome::Applied
    );
    assert_eq!(
        consumer
            .cursor_state(tenant_id, "module.lifecycle")
            .await
            .expect("first cursor state should resolve"),
        ModulePolicyRevisionCursorState {
            current_revision: Some(first_revision.clone()),
            current_sequence: 1,
        }
    );

    let second = ModulePolicyRevisionTransition {
        previous_revision: Some(first_revision.clone()),
        next_revision: second_revision.clone(),
    };
    assert_eq!(
        consumer
            .apply(tenant_id, "module.lifecycle", &second)
            .await
            .expect("matching successor should advance"),
        ModulePolicyRevisionApplyOutcome::Applied
    );
    let second_state = consumer
        .cursor_state(tenant_id, "module.lifecycle")
        .await
        .expect("second cursor state should resolve");
    assert_eq!(
        second_state,
        ModulePolicyRevisionCursorState {
            current_revision: Some(second_revision.clone()),
            current_sequence: 2,
        }
    );

    // Resyncing to the revision the cursor already holds validates the
    // exact expected state without advancing the sequence. The lifecycle
    // writer relies on this no-op form to prove that the cursor did not
    // move while it derived a policy snapshot.
    assert_eq!(
        consumer
            .resync_if_current(
                tenant_id,
                "module.lifecycle",
                &second_state,
                &second_revision,
            )
            .await
            .expect("same-revision resync should be a no-op"),
        second_state
    );

    let gap = ModulePolicyRevisionTransition {
        previous_revision: Some(first_revision),
        next_revision: third_revision.clone(),
    };
    assert_eq!(
        consumer
            .apply(tenant_id, "module.lifecycle", &gap)
            .await
            .expect("gap should be reported without storage failure"),
        ModulePolicyRevisionApplyOutcome::ResyncRequired
    );
    assert_eq!(
        consumer
            .cursor_state(tenant_id, "module.lifecycle")
            .await
            .expect("gap must leave cursor state unchanged"),
        second_state
    );

    let stale_resync = consumer
        .resync_if_current(
            tenant_id,
            "module.lifecycle",
            &initial_state,
            &third_revision,
        )
        .await;
    assert!(matches!(
        stale_resync,
        Err(ModulePolicyRevisionConsumerError::CursorStateConflict {
            expected_sequence: 0,
            current_sequence: 2,
        })
    ));

    let resynced_state = consumer
        .resync_if_current(
            tenant_id,
            "module.lifecycle",
            &second_state,
            &third_revision,
        )
        .await
        .expect("explicit full-snapshot resync should advance once");
    assert_eq!(
        resynced_state,
        ModulePolicyRevisionCursorState {
            current_revision: Some(third_revision.clone()),
            current_sequence: 3,
        }
    );

    let fourth = ModulePolicyRevisionTransition {
        previous_revision: Some(third_revision),
        next_revision: fourth_revision.clone(),
    };
    assert_eq!(
        consumer
            .apply(tenant_id, "module.lifecycle", &fourth)
            .await
            .expect("successor after explicit resync should advance"),
        ModulePolicyRevisionApplyOutcome::Applied
    );
    assert_eq!(
        consumer
            .cursor_state(tenant_id, "module.lifecycle")
            .await
            .expect("fourth cursor state should resolve"),
        ModulePolicyRevisionCursorState {
            current_revision: Some(fourth_revision),
            current_sequence: 4,
        }
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
            current_sequence INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (tenant_id, consumer_key)
        );
        "#,
    )
    .await
    .expect("policy guard schema");
    db
}
