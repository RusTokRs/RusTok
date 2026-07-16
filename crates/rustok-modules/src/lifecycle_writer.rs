use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use thiserror::Error;
use uuid::Uuid;

use rustok_core::ModuleRegistry;

use crate::{
    execute_module_toggle, ArtifactLifecycleExecutor, ModuleDefinitionCatalog,
    ModuleDefinitionError, ModuleEffectivePolicyQuery, ModuleExecutionDispatcher,
    ModuleLifecycleExecutionError, ModuleLifecycleToggleRequest, ModuleOperationStoreError,
    TenantModuleOverride, TenantModuleSettingsRecord, TenantModuleSettingsRequest,
    TenantModuleStateStore,
};

/// Persists validated module settings through the module-owned lifecycle state store.
pub async fn persist_module_settings(
    db: &DatabaseConnection,
    request: TenantModuleSettingsRequest,
) -> Result<TenantModuleSettingsRecord, ModuleOperationStoreError> {
    TenantModuleStateStore::persist_settings(db, request).await
}

/// Database-backed adapter for module lifecycle execution in a host composition.
///
/// The caller supplies the selected distribution registry and its declared
/// defaults; this adapter owns the durable override read and lifecycle write.
pub struct ModuleLifecycleDbWriter<'a> {
    db: DatabaseConnection,
    catalog: Option<ModuleDefinitionCatalog>,
    static_registry: Option<&'a ModuleRegistry>,
    artifact_executor: Option<&'a dyn ArtifactLifecycleExecutor>,
    default_enabled_modules: Vec<String>,
}

impl<'a> ModuleLifecycleDbWriter<'a> {
    pub fn new(
        db: DatabaseConnection,
        registry: &'a ModuleRegistry,
        default_enabled_modules: Vec<String>,
    ) -> Self {
        Self {
            db,
            catalog: None,
            static_registry: Some(registry),
            artifact_executor: None,
            default_enabled_modules,
        }
    }

    /// Creates a lifecycle writer for an artifact-only composition. It has no
    /// compiled registry fallback: hooks dispatch through the admitted runtime
    /// executor supplied by the host composition.
    pub fn artifact_only(
        db: DatabaseConnection,
        catalog: ModuleDefinitionCatalog,
        artifact_executor: &'a dyn ArtifactLifecycleExecutor,
        default_enabled_modules: Vec<String>,
    ) -> Self {
        Self {
            db,
            catalog: Some(catalog),
            static_registry: None,
            artifact_executor: Some(artifact_executor),
            default_enabled_modules,
        }
    }

    pub async fn toggle(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
        enabled: bool,
        actor: &str,
    ) -> Result<(), ModuleLifecycleDbWriterError> {
        let overrides = self.overrides(tenant_id).await?;
        let catalog = match &self.catalog {
            Some(catalog) => catalog.clone(),
            None => ModuleDefinitionCatalog::from_static_registry(
                self.static_registry.ok_or_else(|| {
                    ModuleLifecycleDbWriterError::Configuration(
                        "static lifecycle writer has no module registry".into(),
                    )
                })?,
            )
            .map_err(ModuleLifecycleDbWriterError::Definition)?,
        };
        let effective_enabled_modules = ModuleEffectivePolicyQuery::new(
            &catalog,
            self.default_enabled_modules.iter().cloned(),
            overrides,
        )
        .execute()
        .into_enabled_modules();
        let current_settings = self.settings(tenant_id, module_slug).await?;
        let dispatcher = match (self.static_registry, self.artifact_executor) {
            (Some(registry), Some(executor)) => {
                ModuleExecutionDispatcher::new(&catalog, registry).with_artifact_executor(executor)
            }
            (Some(registry), None) => ModuleExecutionDispatcher::new(&catalog, registry),
            (None, Some(executor)) => ModuleExecutionDispatcher::artifact_only(&catalog, executor),
            (None, None) => {
                return Err(ModuleLifecycleDbWriterError::Configuration(
                    "artifact lifecycle writer has no runtime executor".into(),
                ));
            }
        };
        execute_module_toggle(
            &self.db,
            &dispatcher,
            ModuleLifecycleToggleRequest {
                tenant_id,
                module_slug: module_slug.to_string(),
                enabled,
                requested_by: Some(actor.to_string()),
                effective_enabled_modules,
                current_settings,
            },
        )
        .await
        .map_err(ModuleLifecycleDbWriterError::Lifecycle)?;
        Ok(())
    }

    async fn overrides(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<TenantModuleOverride>, ModuleLifecycleDbWriterError> {
        let backend = self.db.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => {
                "SELECT module_slug, enabled FROM tenant_modules WHERE tenant_id = $1"
            }
            _ => "SELECT module_slug, enabled FROM tenant_modules WHERE tenant_id = ?1",
        };
        self.db
            .query_all(Statement::from_sql_and_values(
                backend,
                sql,
                vec![tenant_id.into()],
            ))
            .await
            .map_err(database_error)?
            .into_iter()
            .map(|row| {
                Ok(TenantModuleOverride {
                    module_slug: row.try_get("", "module_slug").map_err(database_error)?,
                    enabled: row.try_get("", "enabled").map_err(database_error)?,
                })
            })
            .collect()
    }

    async fn settings(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<serde_json::Value, ModuleLifecycleDbWriterError> {
        let backend = self.db.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => {
                "SELECT settings FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 LIMIT 1"
            }
            _ => {
                "SELECT settings FROM tenant_modules WHERE tenant_id = ?1 AND module_slug = ?2 LIMIT 1"
            }
        };
        self.db
            .query_one(Statement::from_sql_and_values(
                backend,
                sql,
                vec![tenant_id.into(), module_slug.into()],
            ))
            .await
            .map_err(database_error)?
            .map(|row| row.try_get("", "settings").map_err(database_error))
            .transpose()
            .map(|settings| settings.unwrap_or_else(|| serde_json::json!({})))
    }
}

#[derive(Debug, Error)]
pub enum ModuleLifecycleDbWriterError {
    #[error("module lifecycle persistence failed: {0}")]
    Database(String),
    #[error("module lifecycle writer configuration is invalid: {0}")]
    Configuration(String),
    #[error(transparent)]
    Lifecycle(#[from] ModuleLifecycleExecutionError),
    #[error(transparent)]
    Definition(#[from] ModuleDefinitionError),
}

fn database_error(error: impl std::fmt::Display) -> ModuleLifecycleDbWriterError {
    ModuleLifecycleDbWriterError::Database(error.to_string())
}
