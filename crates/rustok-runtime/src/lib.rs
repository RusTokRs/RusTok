use sea_orm::DatabaseConnection;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;

use rustok_api::{ModuleWorkError, ModuleWorkHandler, ModuleWorkOutcome, ModuleWorkSource};

pub use rustok_api::HostRuntimeContext;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RuntimeHandleError {
    #[error("host runtime context is missing")]
    MissingHostContext,
    #[error("required host runtime handle is missing: {handle}")]
    MissingSharedHandle { handle: &'static str },
}

pub type RuntimeHandleResult<T> = Result<T, RuntimeHandleError>;

#[derive(Debug, Error)]
pub enum RuntimeCompositionError {
    #[error("failed to connect CLI runtime database: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("invalid RUSTOK_SETTINGS_JSON: {0}")]
    InvalidSettings(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct RuntimeComposition {
    host: Option<HostRuntimeContext>,
    settings: serde_json::Value,
}

impl RuntimeComposition {
    pub async fn from_environment() -> Result<Self, RuntimeCompositionError> {
        let settings = match std::env::var("RUSTOK_SETTINGS_JSON") {
            Ok(raw) if !raw.trim().is_empty() => serde_json::from_str(&raw)?,
            _ => serde_json::Value::Object(serde_json::Map::new()),
        };

        let database_url = std::env::var("RUSTOK_DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                std::env::var("DATABASE_URL")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            });

        match database_url {
            Some(database_url) => Ok(Self::from_database(
                sea_orm::Database::connect(database_url).await?,
                settings,
            )),
            None => Ok(Self::without_database(settings)),
        }
    }

    pub fn without_database(settings: serde_json::Value) -> Self {
        Self {
            host: None,
            settings,
        }
    }

    pub fn from_host(host: HostRuntimeContext, settings: serde_json::Value) -> Self {
        Self {
            host: Some(host),
            settings,
        }
    }

    pub fn from_database(db: DatabaseConnection, settings: serde_json::Value) -> Self {
        Self::from_host(HostRuntimeContext::new(db), settings)
    }

    pub fn host(&self) -> Option<&HostRuntimeContext> {
        self.host.as_ref()
    }

    pub fn require_host(&self) -> RuntimeHandleResult<&HostRuntimeContext> {
        self.host
            .as_ref()
            .ok_or(RuntimeHandleError::MissingHostContext)
    }

    pub fn settings(&self) -> &serde_json::Value {
        &self.settings
    }
}

pub fn db_clone(runtime: &HostRuntimeContext) -> DatabaseConnection {
    runtime.db_clone()
}

pub fn require_shared<T>(
    runtime: &HostRuntimeContext,
    handle: &'static str,
) -> RuntimeHandleResult<T>
where
    T: 'static + Send + Sync + Clone,
{
    runtime
        .shared_get::<T>()
        .ok_or(RuntimeHandleError::MissingSharedHandle { handle })
}

#[derive(Clone)]
struct RegisteredModuleWork {
    source: Arc<dyn ModuleWorkSource>,
    handler: Arc<dyn ModuleWorkHandler>,
}

/// Generic runtime scheduler for tenant-scoped durable module work.
///
/// Every worker slug owns its source/handler pair. This permits independent
/// modules to publish separate durable queues without a host-side switch or a
/// shared source that understands another module's tables.
#[derive(Clone)]
pub struct ModuleWorkScheduler {
    workers: Arc<RwLock<BTreeMap<String, RegisteredModuleWork>>>,
}

#[async_trait::async_trait]
pub trait ModuleWorkRegistration: Send + Sync {
    async fn register(
        &self,
        host: &HostRuntimeContext,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError>;
}

#[derive(Clone, Default)]
pub struct ModuleWorkRegistrations {
    entries: Arc<Vec<Arc<dyn ModuleWorkRegistration>>>,
}

impl ModuleWorkRegistrations {
    pub fn register(&mut self, entry: Arc<dyn ModuleWorkRegistration>) {
        Arc::make_mut(&mut self.entries).push(entry);
    }

    pub async fn register_all(
        &self,
        host: &HostRuntimeContext,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        for entry in self.entries.iter() {
            entry.register(host, scheduler).await?;
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl ModuleWorkScheduler {
    pub fn new() -> Self {
        Self {
            workers: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// Registers one module-owned durable source with its matching handler.
    /// A duplicate worker slug is rejected before either side becomes visible.
    pub async fn register(
        &self,
        source: Arc<dyn ModuleWorkSource>,
        handler: Arc<dyn ModuleWorkHandler>,
    ) -> Result<(), ModuleWorkError> {
        let slug = handler.worker_slug().to_string();
        let mut workers = self.workers.write().await;
        if workers.contains_key(&slug) {
            return Err(ModuleWorkError::DuplicateHandler(slug));
        }
        workers.insert(slug, RegisteredModuleWork { source, handler });
        Ok(())
    }

    pub async fn run_once(&self) -> Result<usize, ModuleWorkError> {
        let workers = self.workers.read().await.clone();
        let mut executed = 0;
        for (slug, worker) in workers {
            let Some(item) = worker.source.claim(&slug).await? else {
                continue;
            };
            let outcome = match worker.handler.execute(item.clone()).await {
                Ok(outcome) => outcome,
                Err(error) => ModuleWorkOutcome::Retryable {
                    message: error.to_string(),
                },
            };
            worker.source.complete(&item, outcome).await?;
            executed += 1;
        }
        Ok(executed)
    }

    /// Runs registered module work until the deployment-owned stop signal is
    /// raised. A stop prevents future claims; work already claimed by an
    /// adapter is allowed to finish its canonical completion path.
    pub async fn run_until_stopped(
        &self,
        mut stop: tokio::sync::watch::Receiver<bool>,
        poll_interval: Duration,
    ) {
        let mut interval = tokio::time::interval(poll_interval);
        loop {
            tokio::select! {
                changed = stop.changed() => {
                    if changed.is_err() || *stop.borrow() {
                        break;
                    }
                }
                _ = interval.tick() => {
                    if *stop.borrow() {
                        break;
                    }
                    if let Err(error) = self.run_once().await {
                        tracing::error!(%error, "module work scheduler iteration failed");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use rustok_api::{ModuleWorkHandler, ModuleWorkItem, ModuleWorkOutcome, ModuleWorkSource};
    use uuid::Uuid;

    use super::{ModuleWorkScheduler, RuntimeComposition, RuntimeHandleError};

    #[derive(Default)]
    struct TestSource {
        item: Mutex<Option<ModuleWorkItem>>,
        completed: Mutex<Vec<ModuleWorkOutcome>>,
    }

    #[async_trait]
    impl ModuleWorkSource for TestSource {
        async fn claim(
            &self,
            _worker_slug: &str,
        ) -> Result<Option<ModuleWorkItem>, rustok_api::ModuleWorkError> {
            Ok(self.item.lock().expect("test item mutex").take())
        }

        async fn complete(
            &self,
            _item: &ModuleWorkItem,
            outcome: ModuleWorkOutcome,
        ) -> Result<(), rustok_api::ModuleWorkError> {
            self.completed
                .lock()
                .expect("test outcome mutex")
                .push(outcome);
            Ok(())
        }
    }

    struct TestHandler(&'static str);

    #[async_trait]
    impl ModuleWorkHandler for TestHandler {
        fn worker_slug(&self) -> &'static str {
            self.0
        }

        async fn execute(
            &self,
            _item: ModuleWorkItem,
        ) -> Result<ModuleWorkOutcome, rustok_api::ModuleWorkError> {
            Ok(ModuleWorkOutcome::Completed)
        }
    }

    #[test]
    fn composition_keeps_host_neutral_settings_without_database() {
        let composition = RuntimeComposition::without_database(serde_json::json!({
            "environment": "test"
        }));

        assert_eq!(composition.settings()["environment"], "test");
        assert!(composition.host().is_none());
        assert!(matches!(
            composition.require_host(),
            Err(RuntimeHandleError::MissingHostContext)
        ));
    }

    #[tokio::test]
    async fn scheduler_keeps_sources_isolated_by_worker_slug() {
        let first = Arc::new(TestSource {
            item: Mutex::new(Some(ModuleWorkItem {
                id: Uuid::new_v4(),
                tenant_id: Uuid::new_v4(),
                worker_slug: "first".to_string(),
                lease_token: "first-lease".to_string(),
                payload: serde_json::json!({}),
            })),
            ..Default::default()
        });
        let second = Arc::new(TestSource {
            item: Mutex::new(Some(ModuleWorkItem {
                id: Uuid::new_v4(),
                tenant_id: Uuid::new_v4(),
                worker_slug: "second".to_string(),
                lease_token: "second-lease".to_string(),
                payload: serde_json::json!({}),
            })),
            ..Default::default()
        });
        let scheduler = ModuleWorkScheduler::new();
        scheduler
            .register(first.clone(), Arc::new(TestHandler("first")))
            .await
            .expect("first worker registers");
        scheduler
            .register(second.clone(), Arc::new(TestHandler("second")))
            .await
            .expect("second worker registers");

        assert_eq!(scheduler.run_once().await.expect("scheduler runs"), 2);
        assert_eq!(first.completed.lock().expect("first outcomes").len(), 1);
        assert_eq!(second.completed.lock().expect("second outcomes").len(), 1);
    }

    #[tokio::test]
    async fn scheduler_stops_before_claiming_new_work() {
        let scheduler = ModuleWorkScheduler::new();
        let source = Arc::new(TestSource {
            item: Mutex::new(Some(ModuleWorkItem {
                id: Uuid::new_v4(),
                tenant_id: Uuid::new_v4(),
                worker_slug: "stopped".to_string(),
                lease_token: "stopped-lease".to_string(),
                payload: serde_json::json!({}),
            })),
            ..Default::default()
        });
        scheduler
            .register(source.clone(), Arc::new(TestHandler("stopped")))
            .await
            .expect("worker registers");
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        stop_tx.send(true).expect("scheduler receiver must exist");
        scheduler
            .run_until_stopped(stop_rx, std::time::Duration::from_millis(1))
            .await;
        assert!(source.item.lock().expect("queued item mutex").is_some());
    }
}
