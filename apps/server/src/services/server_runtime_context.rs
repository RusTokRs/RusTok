use std::{
    any::{Any, TypeId},
    collections::{hash_map::Entry, HashMap},
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
    fn read_lock(
        &self,
    ) -> std::sync::RwLockReadGuard<'_, HashMap<TypeId, Arc<dyn Any + Send + Sync>>> {
        self.values
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write_lock(
        &self,
    ) -> std::sync::RwLockWriteGuard<'_, HashMap<TypeId, Arc<dyn Any + Send + Sync>>> {
        self.values
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn get<T>(&self) -> Option<T>
    where
        T: 'static + Send + Sync + Clone,
    {
        self.read_lock()
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref::<T>())
            .cloned()
    }

    fn insert<T>(&self, value: T)
    where
        T: 'static + Send + Sync,
    {
        self.write_lock().insert(TypeId::of::<T>(), Arc::new(value));
    }

    fn insert_if_absent<T>(&self, value: T) -> bool
    where
        T: 'static + Send + Sync,
    {
        let mut values = self.write_lock();
        match values.entry(TypeId::of::<T>()) {
            Entry::Vacant(entry) => {
                entry.insert(Arc::new(value));
                true
            }
            Entry::Occupied(_) => false,
        }
    }

    fn get_or_insert_with<T>(&self, make: impl FnOnce() -> T) -> T
    where
        T: 'static + Send + Sync + Clone,
    {
        let mut values = self.write_lock();
        match values.entry(TypeId::of::<T>()) {
            Entry::Occupied(entry) => entry
                .get()
                .downcast_ref::<T>()
                .expect("typed shared runtime value must match its TypeId key")
                .clone(),
            Entry::Vacant(entry) => {
                let value = make();
                entry.insert(Arc::new(value.clone()));
                value
            }
        }
    }

    fn take<T>(&self) -> Option<T>
    where
        T: 'static + Send + Sync,
    {
        let mut lock = self.write_lock();
        let value = lock.remove(&TypeId::of::<T>())?;
        let Ok(typed) = Arc::downcast::<T>(value) else {
            return None;
        };
        match Arc::try_unwrap(typed) {
            Ok(inner) => Some(inner),
            Err(arc) => {
                // If another thread holds a strong reference, restore to map to prevent silent loss
                lock.insert(TypeId::of::<T>(), arc);
                None
            }
        }
    }

    fn contains<T>(&self) -> bool
    where
        T: 'static + Send + Sync,
    {
        self.read_lock().contains_key(&TypeId::of::<T>())
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
            .read_lock()
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref::<T>())
            .map(map)
    }

    pub fn effective_policy_cache(&self) -> rustok_modules::ModuleEffectivePolicyCache {
        self.shared_values
            .get_or_insert_with(rustok_modules::ModuleEffectivePolicyCache::new)
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

    pub fn without_auth(runtime_ctx: ServerRuntimeContext) -> Self {
        Self {
            runtime_ctx,
            auth_config: None,
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

impl FromRef<ServerAuthRuntime> for DatabaseConnection {
    fn from_ref(input: &ServerAuthRuntime) -> Self {
        input.runtime_ctx.db_clone()
    }
}

impl FromRef<ServerRuntimeContext> for DatabaseConnection {
    fn from_ref(input: &ServerRuntimeContext) -> Self {
        input.db_clone()
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

    #[test]
    fn shared_take_restores_value_if_strong_reference_is_held() {
        use std::any::TypeId;

        let values = ServerSharedValues::default();
        values.insert(vec![1, 2, 3]);

        // Retain an extra reference to the internal Arc
        let outer_clone = values
            .read_lock()
            .get(&TypeId::of::<Vec<i32>>())
            .unwrap()
            .clone();

        // Strong count is 2 (values map + outer_clone), unwrapping fails and restores to map
        assert!(values.take::<Vec<i32>>().is_none());
        assert!(values.contains::<Vec<i32>>());

        drop(outer_clone);
        // Strong count is now 1, unwrapping succeeds and removes from map
        let unwrapped = values.take::<Vec<i32>>();
        assert_eq!(unwrapped.as_deref(), Some(&[1, 2, 3][..]));
        assert!(!values.contains::<Vec<i32>>());
    }

    #[test]
    fn server_auth_runtime_without_auth() {
        use super::{ServerAuthRuntime, ServerRuntimeContext};
        use crate::common::settings::RustokSettings;
        use axum::extract::FromRef;
        use sea_orm::DatabaseConnection;

        let ctx =
            ServerRuntimeContext::new(DatabaseConnection::default(), RustokSettings::default());
        let auth_rt = ServerAuthRuntime::without_auth(ctx.clone());
        assert!(auth_rt.auth_config().is_none());

        let extracted_ctx: ServerRuntimeContext = FromRef::from_ref(&auth_rt);
        assert_eq!(
            extracted_ctx.settings().runtime.host_mode,
            ctx.settings().runtime.host_mode
        );

        let _: DatabaseConnection = FromRef::from_ref(&auth_rt);
        let _: DatabaseConnection = FromRef::from_ref(&ctx);
    }
}
