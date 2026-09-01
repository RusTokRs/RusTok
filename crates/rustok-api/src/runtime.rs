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

    db.query_one_raw(Statement::from_sql_and_values(
        backend,
        query,
        vec![tenant_id.into(), module_slug.into()],
    ))
    .await
    .map(|row| row.is_some())
}

/// Returns the settings snapshot for one exact enabled tenant module.
///
/// This is the read-only counterpart to [`is_tenant_module_enabled`]. Internal
/// GraphQL and native server-function adapters can consume the same persisted
/// tenant-module control-plane snapshot without importing tenant persistence
/// entities or inventing a parallel settings store. The caller must supply the
/// tenant id from a trusted request context; this helper only applies the exact
/// tenant/module/enabled-row lookup. Disabled or missing rows return `None`.
pub async fn tenant_module_settings(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    module_slug: &str,
) -> Result<Option<serde_json::Value>, DbErr> {
    let backend = db.get_database_backend();
    let query = match backend {
        sea_orm::DbBackend::Sqlite => {
            "SELECT CAST(settings AS TEXT) AS settings_json FROM tenant_modules WHERE tenant_id = ?1 AND module_slug = ?2 AND enabled = 1 LIMIT 1"
        }
        sea_orm::DbBackend::Postgres => {
            "SELECT settings::text AS settings_json FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 AND enabled = true LIMIT 1"
        }
        sea_orm::DbBackend::MySql => {
            "SELECT CAST(settings AS CHAR) AS settings_json FROM tenant_modules WHERE tenant_id = ? AND module_slug = ? AND enabled = true LIMIT 1"
        }
    };

    let Some(row) = db
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            query,
            vec![tenant_id.into(), module_slug.into()],
        ))
        .await?
    else {
        return Ok(None);
    };
    let encoded: String = row.try_get("", "settings_json")?;
    serde_json::from_str(&encoded).map(Some).map_err(|error| {
        DbErr::Custom(format!(
            "tenant module `{module_slug}` settings are not valid JSON: {error}"
        ))
    })
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

    async fn runtime_module_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("runtime module evidence SQLite should connect");
        db.execute_unprepared(
            r#"
CREATE TABLE tenant_modules (
    tenant_id TEXT NOT NULL,
    module_slug TEXT NOT NULL,
    enabled INTEGER NOT NULL,
    settings TEXT NOT NULL DEFAULT '{}',
    PRIMARY KEY (tenant_id, module_slug)
);
"#,
        )
        .await
        .expect("tenant_modules evidence table should create");
        db
    }

    #[tokio::test]
    async fn tenant_module_enablement_is_exact_and_fail_closed() {
        let db = runtime_module_db().await;
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

    #[tokio::test]
    async fn tenant_module_settings_returns_only_the_exact_enabled_row() {
        let db = runtime_module_db().await;
        let tenant_id = Uuid::new_v4();
        let foreign_tenant_id = Uuid::new_v4();
        db.execute_unprepared(&format!(
            r#"INSERT INTO tenant_modules (tenant_id, module_slug, enabled, settings) VALUES
             ('{tenant_id}', 'pages', 1, '{{"builder":{{"enabled":false,"preview":{{"enabled":false}},"properties":{{"enabled":true}},"publish":{{"enabled":false}}}}}}'),
             ('{tenant_id}', 'forum', 0, '{{"builder":{{"enabled":true}}}}'),
             ('{foreign_tenant_id}', 'pages', 1, '{{"builder":{{"enabled":true}}}}')"#
        ))
        .await
        .expect("tenant module settings evidence rows should insert");

        let settings = tenant_module_settings(&db, tenant_id, "pages")
            .await
            .expect("enabled Pages settings lookup should succeed")
            .expect("enabled Pages row should expose settings");
        assert_eq!(settings["builder"]["enabled"], false);
        assert_eq!(settings["builder"]["preview"]["enabled"], false);
        assert_eq!(settings["builder"]["properties"]["enabled"], true);
        assert_eq!(settings["builder"]["publish"]["enabled"], false);

        let other_tenant_settings = tenant_module_settings(&db, foreign_tenant_id, "pages")
            .await
            .expect("other exact tenant Pages lookup should succeed")
            .expect("other exact enabled Pages row should expose its own settings");
        assert_eq!(other_tenant_settings["builder"]["enabled"], true);

        assert_eq!(
            tenant_module_settings(&db, tenant_id, "forum")
                .await
                .expect("disabled Forum settings lookup should succeed"),
            None
        );
        assert_eq!(
            tenant_module_settings(&db, tenant_id, "missing")
                .await
                .expect("missing module settings lookup should succeed"),
            None
        );
    }
}
