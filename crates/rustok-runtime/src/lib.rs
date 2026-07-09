use sea_orm::DatabaseConnection;
use thiserror::Error;

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
