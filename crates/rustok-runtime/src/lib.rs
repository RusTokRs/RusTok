use sea_orm::DatabaseConnection;
use std::collections::BTreeMap;
use std::sync::Arc;
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

/// Generic runtime scheduler for tenant-scoped durable module work.
///
/// Module owners provide queue persistence through `ModuleWorkSource`; the
/// runtime never imports capability-specific work payloads or database tables.
#[derive(Clone)]
pub struct ModuleWorkScheduler {
    source: Arc<dyn ModuleWorkSource>,
    handlers: Arc<RwLock<BTreeMap<String, Arc<dyn ModuleWorkHandler>>>>,
}

impl ModuleWorkScheduler {
    pub fn new(source: Arc<dyn ModuleWorkSource>) -> Self {
        Self {
            source,
            handlers: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    pub async fn register(
        &self,
        handler: Arc<dyn ModuleWorkHandler>,
    ) -> Result<(), ModuleWorkError> {
        let slug = handler.worker_slug().to_string();
        let mut handlers = self.handlers.write().await;
        if handlers.contains_key(&slug) {
            return Err(ModuleWorkError::DuplicateHandler(slug));
        }
        handlers.insert(slug, handler);
        Ok(())
    }

    pub async fn run_once(&self) -> Result<usize, ModuleWorkError> {
        let handlers = self.handlers.read().await.clone();
        let mut executed = 0;
        for (slug, handler) in handlers {
            let Some(item) = self.source.claim(&slug).await? else {
                continue;
            };
            let outcome = match handler.execute(item.clone()).await {
                Ok(outcome) => outcome,
                Err(error) => ModuleWorkOutcome::Retryable {
                    message: error.to_string(),
                },
            };
            self.source.complete(&item, outcome).await?;
            executed += 1;
        }
        Ok(executed)
    }
}

#[cfg(test)]
mod tests {
    use super::{RuntimeComposition, RuntimeHandleError};

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
}
