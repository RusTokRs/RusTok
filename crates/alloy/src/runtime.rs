use std::sync::Arc;

use sea_orm::DatabaseConnection;
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
    pub tenant_id: Uuid,
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
            tenant_id,
        }
    }
}

pub fn build_alloy_runtime(db: DatabaseConnection) -> Arc<AlloyRuntime> {
    let engine = Arc::new(create_default_engine());
    let storage = Arc::new(SeaOrmStorage::new(db.clone()));
    let execution_log = Arc::new(SeaOrmExecutionLog::new(db));

    let executor = ScriptExecutor::new(engine.clone(), storage.clone())
        .with_execution_log(execution_log.clone());
    let scheduler = Arc::new(Scheduler::new(executor, storage.clone()));
    tokio::spawn(async move {
        if let Err(error) = scheduler.load_jobs().await {
            tracing::warn!("Failed to load Alloy scheduler jobs: {}", error);
        }
        scheduler.start().await;
    });

    Arc::new(AlloyRuntime {
        engine,
        storage,
        execution_log,
    })
}
