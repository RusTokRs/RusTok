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
pub use bridge::Bridge;
pub use context::{ExecutionContext, ExecutionPhase};
pub use controllers::routes;
pub use engine::{EngineConfig, ScriptEngine};
pub use error::{ScriptError, ScriptResult};
pub use execution_log::{ExecutionLogEntry, ScriptExecutionsMigration, SeaOrmExecutionLog};
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

        assert_eq!(result.clone_cast::<i64>(), 50000);
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

        assert!(
            matches!(result, Err(ScriptError::OperationLimit { .. }))
                || matches!(result, Ok(ref value) if value.clone_cast::<i64>() == 1_000_000)
        );
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

        assert_eq!(result.clone_cast::<i64>(), 1000);

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
