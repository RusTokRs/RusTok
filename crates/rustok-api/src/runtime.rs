use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

use sea_orm::DatabaseConnection;

/// Immutable host configuration snapshot provided to internal server-function
/// adapters. It keeps adapters independent of a framework-specific app context.
#[derive(Clone, Debug)]
pub struct HostSettingsSnapshot {
    value: serde_json::Value,
}

impl HostSettingsSnapshot {
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }

    pub fn value(&self) -> &serde_json::Value {
        &self.value
    }
}

#[derive(Clone)]
pub struct HostRuntimeContext {
    db: DatabaseConnection,
    shared_values: Arc<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl HostRuntimeContext {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            shared_values: Arc::new(HashMap::new()),
        }
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub fn db_clone(&self) -> DatabaseConnection {
        self.db.clone()
    }

    pub fn with_shared_value<T>(mut self, value: T) -> Self
    where
        T: 'static + Send + Sync,
    {
        let mut shared_values = (*self.shared_values).clone();
        shared_values.insert(TypeId::of::<T>(), Arc::new(value));
        self.shared_values = Arc::new(shared_values);
        self
    }

    /// Adds typed values published by a module runtime extension registry.
    ///
    /// This remains a neutral platform seam: hosts transfer every registered
    /// value without importing capability-specific types. Existing host values
    /// win so deployments retain ownership of their infrastructure handles.
    pub fn with_extension_values(
        mut self,
        values: impl IntoIterator<Item = (TypeId, Arc<dyn Any + Send + Sync>)>,
    ) -> Self {
        let mut shared_values = (*self.shared_values).clone();
        for (type_id, value) in values {
            shared_values.entry(type_id).or_insert(value);
        }
        self.shared_values = Arc::new(shared_values);
        self
    }

    pub fn shared_get<T>(&self) -> Option<T>
    where
        T: 'static + Send + Sync + Clone,
    {
        self.shared_values
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref::<T>())
            .cloned()
    }
}
