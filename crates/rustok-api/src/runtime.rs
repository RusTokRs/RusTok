use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

use sea_orm::DatabaseConnection;
use sea_orm::{ConnectionTrait, DbErr, Statement};
use uuid::Uuid;

/// Returns whether an optional module is enabled for the tenant snapshot that
/// owns the current request.
///
/// GraphQL and native server-function transports share this query so neither
/// transport can bypass tenant module lifecycle policy.
pub async fn is_tenant_module_enabled(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    module_slug: &str,
) -> Result<bool, DbErr> {
    let backend = db.get_database_backend();
    let query = match backend {
        sea_orm::DbBackend::Sqlite => {
            "SELECT 1 FROM tenant_modules WHERE tenant_id = ?1 AND module_slug = ?2 AND enabled = 1 LIMIT 1"
        }
        _ => {
            "SELECT 1 FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 AND enabled = true LIMIT 1"
        }
    };

    db.query_one(Statement::from_sql_and_values(
        backend,
        query,
        vec![tenant_id.into(), module_slug.into()],
    ))
    .await
    .map(|row| row.is_some())
}

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

#[cfg(all(test, feature = "runtime"))]
mod tests {
    use super::*;
    use sea_orm::Database;

    #[tokio::test]
    async fn tenant_module_enablement_is_exact_and_fail_closed() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("runtime module evidence SQLite should connect");
        db.execute_unprepared(
            r#"
CREATE TABLE tenant_modules (
    tenant_id TEXT NOT NULL,
    module_slug TEXT NOT NULL,
    enabled INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, module_slug)
);
"#,
        )
        .await
        .expect("tenant_modules evidence table should create");

        let tenant_id = Uuid::new_v4();
        let foreign_tenant_id = Uuid::new_v4();
        db.execute_unprepared(&format!(
            "INSERT INTO tenant_modules (tenant_id, module_slug, enabled) VALUES \
             ('{tenant_id}', 'forum', 1), \
             ('{tenant_id}', 'pages', 0), \
             ('{foreign_tenant_id}', 'forum', 1)"
        ))
        .await
        .expect("tenant module evidence rows should insert");

        assert!(
            is_tenant_module_enabled(&db, tenant_id, "forum")
                .await
                .expect("enabled Forum lookup should succeed")
        );
        assert!(
            !is_tenant_module_enabled(&db, tenant_id, "pages")
                .await
                .expect("disabled module lookup should succeed")
        );
        assert!(
            !is_tenant_module_enabled(&db, Uuid::new_v4(), "forum")
                .await
                .expect("foreign tenant lookup should succeed")
        );
        assert!(
            !is_tenant_module_enabled(&db, tenant_id, "missing")
                .await
                .expect("missing module lookup should succeed")
        );
    }
}
