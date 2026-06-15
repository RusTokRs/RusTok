use std::sync::Arc;

use loco_rs::app::AppContext;
use uuid::Uuid;

use crate::{
    create_default_engine, Scheduler, ScriptEngine, ScriptExecutor, ScriptOrchestrator,
    SeaOrmExecutionLog, SeaOrmStorage,
};

#[derive(Clone)]
pub struct AlloyRuntime {
    pub engine: Arc<ScriptEngine>,
    pub storage: Arc<SeaOrmStorage>,
    pub execution_log: Arc<SeaOrmExecutionLog>,
}

#[derive(Clone)]
pub struct ScopedAlloyRuntime {
    pub engine: Arc<ScriptEngine>,
    pub storage: Arc<SeaOrmStorage>,
    pub orchestrator: Arc<ScriptOrchestrator<SeaOrmStorage>>,
    pub execution_log: Arc<SeaOrmExecutionLog>,
}

#[derive(Clone)]
pub struct SharedAlloyRuntime(pub Arc<AlloyRuntime>);

impl AlloyRuntime {
    pub fn scoped(&self, tenant_id: Uuid) -> ScopedAlloyRuntime {
        let storage = Arc::new(self.storage.for_tenant(tenant_id));
        let orchestrator = Arc::new(ScriptOrchestrator::with_execution_log(
            self.engine.clone(),
            storage.clone(),
            self.execution_log.clone(),
        ));

        ScopedAlloyRuntime {
            engine: self.engine.clone(),
            storage,
            orchestrator,
            execution_log: self.execution_log.clone(),
        }
    }
}

pub fn init(ctx: &AppContext) -> Arc<AlloyRuntime> {
    if let Some(shared) = ctx.shared_store.get::<SharedAlloyRuntime>() {
        return shared.0.clone();
    }

    let engine = Arc::new(create_default_engine());
    let storage = Arc::new(SeaOrmStorage::new(ctx.db.clone()));
    let execution_log = Arc::new(SeaOrmExecutionLog::new(ctx.db.clone()));

    let executor = ScriptExecutor::new(engine.clone(), storage.clone())
        .with_execution_log(execution_log.clone());
    let scheduler = Arc::new(Scheduler::new(executor, storage.clone()));
    tokio::spawn(async move {
        if let Err(error) = scheduler.load_jobs().await {
            tracing::warn!("Failed to load Alloy scheduler jobs: {}", error);
        }
        scheduler.start().await;
    });

    let runtime = Arc::new(AlloyRuntime {
        engine,
        storage,
        execution_log,
    });

    ctx.shared_store.insert(SharedAlloyRuntime(runtime.clone()));
    runtime
}

pub fn runtime_from_ctx(ctx: &AppContext) -> Arc<AlloyRuntime> {
    ctx.shared_store
        .get::<SharedAlloyRuntime>()
        .expect("Alloy runtime not initialised")
        .0
        .clone()
}

pub fn scoped_runtime(ctx: &AppContext, tenant_id: Uuid) -> ScopedAlloyRuntime {
    runtime_from_ctx(ctx).scoped(tenant_id)
}
