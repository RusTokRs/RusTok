extern crate rhai_full as rhai;

use async_trait::async_trait;
use rustok_core::{MigrationSource, Permission, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod api;
pub mod bridge;
pub mod context;
pub mod controllers;
pub mod engine;
pub mod error;
pub mod execution_log;
pub mod graphql;
pub mod integration;
pub mod migration;
pub mod migrations;
pub mod model;
pub mod runner;
pub mod runtime;
pub mod scheduler;
pub mod storage;
pub mod utils;

pub use api::{create_router, AppState};
pub use bridge::{Bridge, PhaseCapabilities};
pub use context::{ExecutionContext, ExecutionPhase};
pub use controllers::{routes, LOCO_EXECUTION_HISTORY_ROUTES};
pub use engine::{EngineConfig, ScriptEngine};
pub use error::{ScriptError, ScriptResult};
pub use execution_log::{
    ExecutionLogEntry, ExecutionLogSink, ScriptExecutionsMigration, SeaOrmExecutionLog,
};
pub use graphql::{AlloyMutation, AlloyQuery};
pub use integration::{BeforeHookResult, HookExecutor, ScriptableEntity};
pub use migration::ScriptsMigration;
pub use model::{
    register_entity_proxy, EntityProxy, EventType, HttpMethod, Script, ScriptId, ScriptStatus,
    ScriptTrigger,
};
pub use runner::{
    ExecutionOutcome, ExecutionResult, HookOutcome, ScriptExecutor, ScriptOrchestrator,
};
pub use runtime::{init, runtime_from_ctx, scoped_runtime, AlloyRuntime, SharedAlloyRuntime};
pub use scheduler::{ScheduledJob, Scheduler};
pub use storage::{InMemoryStorage, ScriptPage, ScriptQuery, ScriptRegistry, SeaOrmStorage};

pub struct AlloyModule;

pub fn create_default_engine() -> ScriptEngine {
    let config = EngineConfig::default();
    create_engine_with_config(config)
}

pub fn create_engine_with_config(config: engine::EngineConfig) -> ScriptEngine {
    let mut engine = ScriptEngine::new(config);

    bridge::register_utils(engine.engine_mut());
    register_entity_proxy(engine.engine_mut());

    engine
}

pub fn create_engine_for_phase(phase: context::ExecutionPhase) -> ScriptEngine {
    let config = EngineConfig::default();
    let mut engine = ScriptEngine::new(config);

    Bridge::register_for_phase(engine.engine_mut(), phase);
    register_entity_proxy(engine.engine_mut());

    engine
}

pub fn create_orchestrator<R: ScriptRegistry>(
    registry: std::sync::Arc<R>,
) -> ScriptOrchestrator<R> {
    let engine = create_default_engine();
    ScriptOrchestrator::new(std::sync::Arc::new(engine), registry)
}

pub fn create_orchestrator_with_engine<R: ScriptRegistry>(
    engine: std::sync::Arc<ScriptEngine>,
    registry: std::sync::Arc<R>,
) -> ScriptOrchestrator<R> {
    ScriptOrchestrator::new(engine, registry)
}

impl MigrationSource for AlloyModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[async_trait]
impl RusToKModule for AlloyModule {
    fn slug(&self) -> &'static str {
        "alloy"
    }

    fn name(&self) -> &'static str {
        "Alloy"
    }

    fn description(&self) -> &'static str {
        "Alloy runtime and scripting capability"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::SCRIPTS_CREATE,
            Permission::SCRIPTS_READ,
            Permission::SCRIPTS_UPDATE,
            Permission::SCRIPTS_DELETE,
            Permission::SCRIPTS_LIST,
            Permission::SCRIPTS_EXECUTE,
            Permission::SCRIPTS_MANAGE,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhai::Dynamic;
    use std::sync::Arc;

    #[derive(Default)]
    struct CapturingExecutionLog {
        entries: std::sync::Mutex<Vec<(ExecutionResult, ExecutionContext)>>,
    }

    #[async_trait::async_trait]
    impl ExecutionLogSink for CapturingExecutionLog {
        async fn record_result(
            &self,
            result: &ExecutionResult,
            ctx: &ExecutionContext,
        ) -> ScriptResult<()> {
            self.entries
                .lock()
                .expect("execution log lock should not be poisoned")
                .push((result.clone(), ctx.clone()));
            Ok(())
        }
    }

    impl CapturingExecutionLog {
        fn snapshot(&self) -> Vec<(ExecutionResult, ExecutionContext)> {
            self.entries
                .lock()
                .expect("execution log lock should not be poisoned")
                .clone()
        }
    }

    #[test]
    fn test_simple_script() {
        let engine = create_default_engine();
        let ctx = ExecutionContext::new(ExecutionPhase::Manual);

        let result = engine
            .execute(
                "test_hello",
                r#"
                log("Hello from script!");
                let x = 10 + 20;
                x
            "#,
                &ctx,
            )
            .unwrap();

        assert_eq!(result.as_int().unwrap(), 30);
    }

    #[test]
    fn test_abort() {
        let engine = create_default_engine();
        let ctx = ExecutionContext::new(ExecutionPhase::Before);

        let result = engine.execute("test_abort", r#"abort("Deal amount too small")"#, &ctx);

        assert!(matches!(result, Err(ScriptError::Aborted(_))));
    }

    #[test]
    fn test_entity_access() {
        let engine = create_default_engine();

        let mut deal: std::collections::HashMap<String, Dynamic> = std::collections::HashMap::new();
        deal.insert("amount".to_string(), Dynamic::from(50000_i64));
        deal.insert("name".to_string(), Dynamic::from("Big Deal"));

        let entity = EntityProxy::new("1", "deal", deal);
        let ctx = ExecutionContext::new(ExecutionPhase::Before).with_entity_proxy(entity);

        let result = engine
            .execute(
                "test_entity",
                r#"
                if entity["amount"] > 10000 {
                    log("Big deal detected: " + entity["name"]);
                }
                entity["amount"]
            "#,
                &ctx,
            )
            .unwrap();

        let amount = result
            .clone()
            .try_cast::<i64>()
            .or_else(|| result.clone().try_cast::<i32>().map(i64::from))
            .or_else(|| result.clone().try_cast::<f64>().map(|v| v as i64))
            .expect("entity amount should be numeric");
        assert_eq!(amount, 50000);
    }

    #[test]
    fn test_operation_limit() {
        let config = EngineConfig {
            max_operations: 100,
            ..Default::default()
        };
        let mut engine = ScriptEngine::new(config);
        bridge::register_utils(engine.engine_mut());

        let ctx = ExecutionContext::new(ExecutionPhase::Manual);

        let result = engine.execute(
            "test_infinite",
            r#"
                let i = 0;
                while i < 1000000 {
                    i += 1;
                }
                i
            "#,
            &ctx,
        );

        assert!(matches!(result, Err(ScriptError::OperationLimit { .. })));
    }

    #[test]
    fn test_string_resource_limit() {
        let config = EngineConfig {
            max_string_size: 8,
            ..Default::default()
        };
        let mut engine = ScriptEngine::new(config);
        bridge::register_utils(engine.engine_mut());

        let ctx = ExecutionContext::new(ExecutionPhase::Manual);
        let result = engine.execute("test_string_limit", r#""123456789""#, &ctx);

        assert!(matches!(result, Err(ScriptError::ResourceLimit { .. })));
    }

    #[test]
    fn test_engine_limits_snapshot() {
        let config = EngineConfig::strict();
        let limits = config.limits();

        assert_eq!(limits.max_operations, 10_000);
        assert_eq!(limits.timeout_ms, 50);
        assert_eq!(limits.max_call_depth, 8);
    }

    #[test]
    fn test_cache_invalidation() {
        let engine = create_default_engine();
        let ctx = ExecutionContext::new(ExecutionPhase::Manual);

        let result1 = engine.execute("cache_test", "let x = 1; x", &ctx).unwrap();
        assert_eq!(result1.as_int().unwrap(), 1);

        let result2 = engine.execute("cache_test", "let x = 2; x", &ctx).unwrap();
        assert_eq!(result2.as_int().unwrap(), 2);

        engine.invalidate("cache_test");
        let result3 = engine.execute("cache_test", "let x = 3; x", &ctx).unwrap();
        assert_eq!(result3.as_int().unwrap(), 3);
    }

    #[test]
    fn test_invalidate_all() {
        let engine = create_default_engine();
        let ctx = ExecutionContext::new(ExecutionPhase::Manual);

        let _ = engine.execute("script1", "1", &ctx).unwrap();
        let _ = engine.execute("script2", "2", &ctx).unwrap();

        engine.invalidate_all();

        let result = engine.execute("script1", "10", &ctx).unwrap();
        assert_eq!(result.as_int().unwrap(), 10);
    }

    #[test]
    fn test_create_engine_for_phase() {
        let engine = create_engine_for_phase(ExecutionPhase::Before);
        let ctx = ExecutionContext::new(ExecutionPhase::Before);

        let result = engine
            .execute(
                "validation_test",
                r#"
                    let email = "test@example.com";
                    validate_email(email)
                "#,
                &ctx,
            )
            .unwrap();

        assert!(result.as_bool().unwrap());
    }

    #[test]
    fn test_validation_helpers() {
        let engine = create_engine_for_phase(ExecutionPhase::Before);
        let ctx = ExecutionContext::new(ExecutionPhase::Before);

        let result = engine
            .execute(
                "validation_test",
                r#"
                    let valid = true;
                    valid = valid && validate_email("test@example.com");
                    valid = valid && !validate_email("invalid-email");
                    valid = valid && validate_required("hello");
                    valid = valid && !validate_required("   ");
                    valid = valid && validate_min_length("hello", 3);
                    valid = valid && validate_max_length("hi", 5);
                    valid = valid && validate_range(50, 0, 100);
                    valid
                "#,
                &ctx,
            )
            .unwrap();

        assert!(result.as_bool().unwrap());
    }

    #[test]
    fn test_entity_changes() {
        let engine = create_default_engine();

        let data: std::collections::HashMap<String, Dynamic> = std::collections::HashMap::from([
            ("amount".to_string(), Dynamic::from(1000_i64)),
            ("status".to_string(), Dynamic::from("pending")),
        ]);

        let entity = EntityProxy::new("1", "order", data);
        let ctx = ExecutionContext::new(ExecutionPhase::Before).with_entity_proxy(entity);

        let result = engine
            .execute(
                "change_test",
                r#"
                    entity["status"] = "approved";
                    entity["discount"] = 10;
                    entity["amount"]
                "#,
                &ctx,
            )
            .unwrap();

        let amount = result
            .clone()
            .try_cast::<i64>()
            .or_else(|| result.clone().try_cast::<i32>().map(i64::from))
            .or_else(|| result.clone().try_cast::<f64>().map(|v| v as i64))
            .expect("entity amount should be numeric");
        assert_eq!(amount, 1000);

        let entity = ctx.entity_proxy.as_ref().unwrap();
        assert!(entity.is_changed("status"));
        assert!(entity.is_changed("discount"));
        assert!(!entity.is_changed("amount"));
        assert!(entity.has_changes());
    }

    #[tokio::test]
    async fn test_orchestrator_integration() {
        let storage = Arc::new(InMemoryStorage::new());
        let orchestrator = create_orchestrator(storage.clone());

        let mut script = Script::new(
            "test_validation",
            r#"
                if entity["value"] < 0 {
                    abort("Value must be positive");
                }
                entity["processed"] = true;
            "#,
            ScriptTrigger::Event {
                entity_type: "test".into(),
                event: EventType::BeforeCreate,
            },
        );
        script.activate();
        storage.save(script).await.unwrap();

        let data: std::collections::HashMap<String, Dynamic> =
            std::collections::HashMap::from([("value".to_string(), Dynamic::from(100_i64))]);
        let entity = EntityProxy::new("test-1", "test", data);

        let outcome = orchestrator
            .run_before("test", EventType::BeforeCreate, entity, None)
            .await;

        match outcome {
            HookOutcome::Continue { changes } => {
                assert!(changes.contains_key("processed"));
            }
            _ => panic!("Expected Continue outcome"),
        }
    }

    #[tokio::test]
    async fn manual_execution_persists_execution_log_with_user_and_tenant() {
        let storage = Arc::new(InMemoryStorage::new());
        let execution_log = Arc::new(CapturingExecutionLog::default());
        let orchestrator = ScriptOrchestrator::with_execution_log(
            Arc::new(create_default_engine()),
            Arc::clone(&storage),
            execution_log.clone(),
        );

        let mut script = Script::new(
            "manual_audit_smoke",
            r#"params["value"] + 1"#,
            ScriptTrigger::Manual,
        );
        script.tenant_id = uuid::Uuid::new_v4();
        script.activate();
        let tenant_id = script.tenant_id;
        storage.save(script).await.unwrap();

        let result = orchestrator
            .run_manual(
                "manual_audit_smoke",
                std::collections::HashMap::from([("value".to_string(), Dynamic::from(41_i64))]),
                Some("operator-1".to_string()),
            )
            .await
            .unwrap();

        assert!(result.is_success());
        let entries = execution_log.snapshot();
        assert_eq!(entries.len(), 1);
        let (logged_result, logged_ctx) = &entries[0];
        assert_eq!(logged_result.script_id, result.script_id);
        assert_eq!(logged_result.phase, ExecutionPhase::Manual);
        assert_eq!(logged_ctx.user_id.as_deref(), Some("operator-1"));
        let tenant_id_str = tenant_id.to_string();
        assert_eq!(
            logged_ctx.tenant_id.as_deref(),
            Some(tenant_id_str.as_str())
        );
    }

    #[tokio::test]
    async fn before_hook_persists_execution_log_with_entity_changes() {
        let storage = Arc::new(InMemoryStorage::new());
        let execution_log = Arc::new(CapturingExecutionLog::default());
        let orchestrator = ScriptOrchestrator::with_execution_log(
            Arc::new(create_default_engine()),
            Arc::clone(&storage),
            execution_log.clone(),
        );

        let mut script = Script::new(
            "before_audit_smoke",
            r#"entity["status"] = "approved";"#,
            ScriptTrigger::Event {
                entity_type: "order".into(),
                event: EventType::BeforeUpdate,
            },
        );
        script.tenant_id = uuid::Uuid::new_v4();
        script.activate();
        let tenant_id = script.tenant_id;
        storage.save(script).await.unwrap();

        let entity = EntityProxy::new(
            "order-1",
            "order",
            std::collections::HashMap::from([("status".to_string(), Dynamic::from("pending"))]),
        );
        let outcome = orchestrator
            .run_before(
                "order",
                EventType::BeforeUpdate,
                entity,
                Some("operator-2".to_string()),
            )
            .await;

        match outcome {
            HookOutcome::Continue { changes } => {
                assert_eq!(
                    changes
                        .get("status")
                        .and_then(|v| v.clone().try_cast::<String>()),
                    Some("approved".to_string())
                );
            }
            other => panic!("expected hook continue, got {other:?}"),
        }

        let entries = execution_log.snapshot();
        assert_eq!(entries.len(), 1);
        let (logged_result, logged_ctx) = &entries[0];
        assert_eq!(logged_result.phase, ExecutionPhase::Before);
        assert_eq!(logged_ctx.user_id.as_deref(), Some("operator-2"));
        let tenant_id_str2 = tenant_id.to_string();
        assert_eq!(
            logged_ctx.tenant_id.as_deref(),
            Some(tenant_id_str2.as_str())
        );
        assert!(matches!(
            &logged_result.outcome,
            ExecutionOutcome::Success { entity_changes, .. } if entity_changes.contains_key("status")
        ));
    }

    #[tokio::test]
    async fn on_commit_persists_one_execution_log_per_script() {
        let storage = Arc::new(InMemoryStorage::new());
        let execution_log = Arc::new(CapturingExecutionLog::default());
        let orchestrator = ScriptOrchestrator::with_execution_log(
            Arc::new(create_default_engine()),
            Arc::clone(&storage),
            execution_log.clone(),
        );

        for script_name in ["on_commit_audit_one", "on_commit_audit_two"] {
            let mut script = Script::new(
                script_name,
                "1",
                ScriptTrigger::Event {
                    entity_type: "invoice".into(),
                    event: EventType::OnCommit,
                },
            );
            script.activate();
            storage.save(script).await.unwrap();
        }

        let results = orchestrator
            .run_on_commit(
                "invoice",
                EntityProxy::empty("invoice"),
                Some("operator-3".to_string()),
            )
            .await;

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(ExecutionResult::is_success));
        let entries = execution_log.snapshot();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|(result, ctx)| {
            result.phase == ExecutionPhase::OnCommit
                && ctx.phase == ExecutionPhase::OnCommit
                && ctx.user_id.as_deref() == Some("operator-3")
        }));
    }

    #[test]
    fn module_metadata() {
        let module = AlloyModule;
        assert_eq!(module.slug(), "alloy");
        assert_eq!(module.name(), "Alloy");
        assert_eq!(
            module.description(),
            "Alloy runtime and scripting capability"
        );
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert!(module.permissions().contains(&Permission::SCRIPTS_MANAGE));
    }
}
