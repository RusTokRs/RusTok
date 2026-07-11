use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::{Arc, RwLock},
};

use axum::extract::FromRef;
use loco_rs::app::AppContext;
use sea_orm::DatabaseConnection;

use crate::auth::{auth_config_from_host_settings, AuthConfig};
use crate::common::settings::RustokSettings;

#[derive(Clone, Default)]
struct ServerSharedValues {
    values: Arc<RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>>,
}

impl ServerSharedValues {
    fn get<T>(&self) -> Option<T>
    where
        T: 'static + Send + Sync + Clone,
    {
        self.values
            .read()
            .expect("server shared values lock poisoned")
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref::<T>())
            .cloned()
    }

    fn insert<T>(&self, value: T)
    where
        T: 'static + Send + Sync,
    {
        self.values
            .write()
            .expect("server shared values lock poisoned")
            .insert(TypeId::of::<T>(), Arc::new(value));
    }

    fn contains<T>(&self) -> bool
    where
        T: 'static + Send + Sync,
    {
        self.values
            .read()
            .expect("server shared values lock poisoned")
            .contains_key(&TypeId::of::<T>())
    }
}

#[derive(Clone)]
pub struct ServerRuntimeContext {
    db: DatabaseConnection,
    settings: Arc<RustokSettings>,
    shared_values: ServerSharedValues,
}

impl ServerRuntimeContext {
    pub fn new(db: DatabaseConnection, settings: RustokSettings) -> Self {
        Self {
            db,
            settings: Arc::new(settings),
            shared_values: ServerSharedValues::default(),
        }
    }

    pub fn from_loco_app_context(ctx: &AppContext) -> Self {
        if let Some(runtime_ctx) = ctx.shared_store.get::<ServerRuntimeContext>() {
            return runtime_ctx;
        }

        let settings = RustokSettings::from_settings(&ctx.config.settings).unwrap_or_default();
        let runtime_ctx = Self::new(ctx.db.clone(), settings);
        ctx.shared_store.insert(runtime_ctx.clone());
        runtime_ctx
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
        self.shared_values.get::<T>()
    }

    pub fn shared_insert<T>(&self, value: T)
    where
        T: 'static + Send + Sync,
    {
        self.shared_values.insert(value);
    }

    pub fn shared_contains<T>(&self) -> bool
    where
        T: 'static + Send + Sync,
    {
        self.shared_values.contains::<T>()
    }

    pub fn shared_map<T, R>(&self, map: impl FnOnce(&T) -> R) -> Option<R>
    where
        T: 'static + Send + Sync,
    {
        self.shared_values
            .values
            .read()
            .expect("server shared values lock poisoned")
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref::<T>())
            .map(map)
    }
}

impl FromRef<AppContext> for ServerRuntimeContext {
    fn from_ref(input: &AppContext) -> Self {
        Self::from_loco_app_context(input)
    }
}

pub fn auth_config_from_loco_app_context(ctx: &AppContext) -> crate::error::Result<AuthConfig> {
    let auth = ctx
        .config
        .auth
        .as_ref()
        .and_then(|auth| auth.jwt.as_ref())
        .ok_or(crate::error::Error::InternalServerError)?;
    auth_config_from_host_settings(
        auth.secret.clone(),
        auth.expiration,
        ctx.config.settings.as_ref(),
    )
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
            auth_config: auth_config_from_loco_app_context(ctx).ok(),
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
