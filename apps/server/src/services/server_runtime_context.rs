use std::{
    any::{Any, TypeId},
    collections::{HashMap, hash_map::Entry},
    sync::{Arc, RwLock},
};

use axum::extract::FromRef;
use sea_orm::DatabaseConnection;

use crate::auth::AuthConfig;
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

    fn insert_if_absent<T>(&self, value: T) -> bool
    where
        T: 'static + Send + Sync,
    {
        let mut values = self
            .values
            .write()
            .expect("server shared values lock poisoned");
        match values.entry(TypeId::of::<T>()) {
            Entry::Vacant(entry) => {
                entry.insert(Arc::new(value));
                true
            }
            Entry::Occupied(_) => false,
        }
    }

    fn take<T>(&self) -> Option<T>
    where
        T: 'static + Send + Sync,
    {
        let value = self
            .values
            .write()
            .expect("server shared values lock poisoned")
            .remove(&TypeId::of::<T>())?;
        let typed = Arc::downcast::<T>(value).ok()?;
        Arc::try_unwrap(typed).ok()
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

    /// Atomically insert a typed runtime value only when no value of that type exists.
    ///
    /// Returns `true` to the caller that reserved the lifecycle and `false` to concurrent callers
    /// that observed an existing owner. This avoids check-then-insert races around spawned tasks.
    pub fn shared_insert_if_absent<T>(&self, value: T) -> bool
    where
        T: 'static + Send + Sync,
    {
        self.shared_values.insert_if_absent(value)
    }

    pub fn shared_take<T>(&self) -> Option<T>
    where
        T: 'static + Send + Sync,
    {
        self.shared_values.take::<T>()
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

    pub fn runtime_ctx(&self) -> &ServerRuntimeContext {
        &self.runtime_ctx
    }

    pub fn auth_config(&self) -> Option<&AuthConfig> {
        self.auth_config.as_ref()
    }
}

impl FromRef<ServerAuthRuntime> for ServerRuntimeContext {
    fn from_ref(input: &ServerAuthRuntime) -> Self {
        input.runtime_ctx.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::ServerSharedValues;

    #[test]
    fn shared_take_returns_and_removes_the_typed_value() {
        let values = ServerSharedValues::default();
        values.insert(String::from("tenant-listener"));

        assert_eq!(values.take::<String>().as_deref(), Some("tenant-listener"));
        assert!(!values.contains::<String>());
        assert!(values.take::<String>().is_none());
    }

    #[test]
    fn shared_take_does_not_disturb_other_types() {
        let values = ServerSharedValues::default();
        values.insert(String::from("listener"));
        values.insert(42_u64);

        assert_eq!(values.take::<String>().as_deref(), Some("listener"));
        assert_eq!(values.get::<u64>(), Some(42));
    }

    #[test]
    fn shared_insert_if_absent_has_one_owner_and_preserves_the_first_value() {
        let values = ServerSharedValues::default();

        assert!(values.insert_if_absent(String::from("first")));
        assert!(!values.insert_if_absent(String::from("second")));
        assert_eq!(values.get::<String>().as_deref(), Some("first"));
    }
}
