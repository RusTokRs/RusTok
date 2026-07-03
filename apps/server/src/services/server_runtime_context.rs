use std::sync::Arc;

use axum::extract::FromRef;
use loco_rs::app::{AppContext, SharedStore};
use loco_rs::mailer::EmailSender;
use sea_orm::DatabaseConnection;

use crate::auth::{auth_config_from_ctx, AuthConfig};
use crate::common::settings::RustokSettings;

#[derive(Clone)]
pub struct ServerRuntimeContext {
    db: DatabaseConnection,
    settings: Arc<RustokSettings>,
    shared_store: Arc<SharedStore>,
}

impl ServerRuntimeContext {
    pub fn new(
        db: DatabaseConnection,
        settings: RustokSettings,
        shared_store: Arc<SharedStore>,
    ) -> Self {
        Self {
            db,
            settings: Arc::new(settings),
            shared_store,
        }
    }

    pub fn from_loco_app_context(ctx: &AppContext) -> Self {
        let settings = RustokSettings::from_settings(&ctx.config.settings).unwrap_or_default();
        Self::new(ctx.db.clone(), settings, ctx.shared_store.clone())
    }

    pub fn with_empty_shared_store(db: DatabaseConnection, settings: RustokSettings) -> Self {
        Self::new(db, settings, Arc::new(SharedStore::default()))
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub fn db_clone(&self) -> DatabaseConnection {
        self.db.clone()
    }

    pub fn settings(&self) -> &RustokSettings {
        &self.settings
    }

    pub fn shared_get<T>(&self) -> Option<T>
    where
        T: 'static + Send + Sync + Clone,
    {
        self.shared_store.get::<T>()
    }

    pub fn shared_insert<T>(&self, value: T)
    where
        T: 'static + Send + Sync,
    {
        self.shared_store.insert(value);
    }

    pub fn shared_contains<T>(&self) -> bool
    where
        T: 'static + Send + Sync,
    {
        self.shared_store.contains::<T>()
    }

    pub fn shared_map<T, R>(&self, map: impl FnOnce(&T) -> R) -> Option<R>
    where
        T: 'static + Send + Sync,
    {
        self.shared_store.get_ref::<T>().map(|value| map(&value))
    }
}

impl FromRef<AppContext> for ServerRuntimeContext {
    fn from_ref(input: &AppContext) -> Self {
        Self::from_loco_app_context(input)
    }
}

#[derive(Clone)]
pub struct ServerAuthRuntime {
    runtime_ctx: ServerRuntimeContext,
    auth_config: Option<AuthConfig>,
}

impl ServerAuthRuntime {
    pub fn new(runtime_ctx: ServerRuntimeContext, auth_config: AuthConfig) -> Self {
        Self {
            runtime_ctx,
            auth_config: Some(auth_config),
        }
    }

    pub fn from_loco_app_context(ctx: &AppContext) -> Self {
        Self {
            runtime_ctx: ServerRuntimeContext::from_loco_app_context(ctx),
            auth_config: auth_config_from_ctx(ctx).ok(),
        }
    }

    pub fn runtime_ctx(&self) -> &ServerRuntimeContext {
        &self.runtime_ctx
    }

    pub fn auth_config(&self) -> Option<&AuthConfig> {
        self.auth_config.as_ref()
    }
}

impl FromRef<AppContext> for ServerAuthRuntime {
    fn from_ref(input: &AppContext) -> Self {
        Self::from_loco_app_context(input)
    }
}

#[derive(Clone)]
pub struct ServerEmailRuntime {
    mailer: Option<EmailSender>,
}

impl ServerEmailRuntime {
    pub fn mailer_initialized(&self) -> bool {
        self.mailer.is_some()
    }

    pub fn mailer_clone(&self) -> Option<EmailSender> {
        self.mailer.clone()
    }
}

impl FromRef<AppContext> for ServerEmailRuntime {
    fn from_ref(input: &AppContext) -> Self {
        Self {
            mailer: input.mailer.clone(),
        }
    }
}
