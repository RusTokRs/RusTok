use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TryGetable};
use thiserror::Error;
use uuid::Uuid;

use rustok_core::ModuleRegistry;

use crate::{
    execute_module_toggle, resolve_effective_modules, ModuleLifecycleExecutionError,
    ModuleLifecycleToggleRequest, TenantModuleOverride,
};

/// Database-backed module seed adapter shared by installer and CLI composition.
///
/// The caller supplies the selected distribution registry and its declared
/// defaults; this adapter owns the durable override read and lifecycle write.
pub struct ModuleSeedDbWriter<'a> {
    db: DatabaseConnection,
    registry: &'a ModuleRegistry,
    default_enabled_modules: Vec<String>,
}

impl<'a> ModuleSeedDbWriter<'a> {
    pub fn new(
        db: DatabaseConnection,
        registry: &'a ModuleRegistry,
        default_enabled_modules: Vec<String>,
    ) -> Self {
        Self {
            db,
            registry,
            default_enabled_modules,
        }
    }

    pub async fn set_enabled(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
        enabled: bool,
        actor: &str,
    ) -> Result<(), ModuleSeedDbWriterError> {
        let overrides = self.overrides(tenant_id).await?;
        let effective_enabled_modules = resolve_effective_modules(
            self.registry,
            self.default_enabled_modules.iter().cloned(),
            overrides,
        );
        let current_settings = self.settings(tenant_id, module_slug).await?;
        execute_module_toggle(
            &self.db,
            self.registry,
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
        .map_err(ModuleSeedDbWriterError::Lifecycle)?;
        Ok(())
    }

    async fn overrides(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<TenantModuleOverride>, ModuleSeedDbWriterError> {
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
    ) -> Result<serde_json::Value, ModuleSeedDbWriterError> {
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
pub enum ModuleSeedDbWriterError {
    #[error("module seed persistence failed: {0}")]
    Database(String),
    #[error(transparent)]
    Lifecycle(#[from] ModuleLifecycleExecutionError),
}

fn database_error(error: impl std::fmt::Display) -> ModuleSeedDbWriterError {
    ModuleSeedDbWriterError::Database(error.to_string())
}
