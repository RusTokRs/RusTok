use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use std::collections::HashSet;
use thiserror::Error;
use uuid::Uuid;

use rustok_core::ModuleRegistry;

use crate::{
    execute_module_toggle, module_operation_recovery_plan, retry_failed_post_hook_operation,
    ArtifactLifecycleExecutor, ModuleDefinitionCatalog, ModuleDefinitionError,
    ModuleDefinitionKind, ModuleEffectivePolicyQuery, ModuleExecutionDispatcher,
    ModuleLifecycleExecutionError, ModuleLifecycleToggleRequest, ModuleOperationIssue,
    ModuleOperationJournal, ModuleOperationRecord, ModuleOperationRecoveryError,
    ModuleOperationRequest, ModuleOperationStoreError, ModulePostHookRetryRequest,
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
        requested_by: Option<String>,
    ) -> Result<crate::ModuleLifecycleToggleResult, ModuleLifecycleDbWriterError> {
        self.toggle_with_operation_context(
            tenant_id,
            module_slug,
            enabled,
            requested_by,
            None,
            None,
        )
        .await
    }

    async fn toggle_with_operation_context(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
        enabled: bool,
        requested_by: Option<String>,
        correlation_id: Option<String>,
        idempotency_key: Option<Uuid>,
    ) -> Result<crate::ModuleLifecycleToggleResult, ModuleLifecycleDbWriterError> {
        let (catalog, effective_enabled_modules, current_settings) =
            self.execution_context(tenant_id, module_slug).await?;
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
                requested_by,
                correlation_id,
                idempotency_key,
                effective_enabled_modules,
                current_settings,
            },
        )
        .await
        .map_err(ModuleLifecycleDbWriterError::Lifecycle)
    }

    /// Retries only a post-hook failure using the same owner-owned effective
    /// policy, catalog, and dispatcher assembly as a normal lifecycle toggle.
    pub async fn retry_post_hook(
        &self,
        operation_id: Uuid,
        requested_by: Option<String>,
        idempotency_key: Uuid,
    ) -> Result<ModuleOperationRecord, ModuleLifecycleDbWriterError> {
        let plan = module_operation_recovery_plan(&self.db, operation_id)
            .await
            .map_err(ModuleLifecycleDbWriterError::Recovery)?;
        let (catalog, effective_enabled_modules, current_settings) = self
            .execution_context(plan.tenant_id, &plan.module_slug)
            .await?;
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
        retry_failed_post_hook_operation(
            &self.db,
            &dispatcher,
            ModulePostHookRetryRequest {
                operation_id,
                requested_by,
                idempotency_key,
                effective_enabled_modules,
                current_settings,
            },
        )
        .await
        .map_err(ModuleLifecycleDbWriterError::Recovery)
    }

    /// Compensates a committed operation only after the recovery contract
    /// confirms that it failed in its post-hook and remains at the requested
    /// effective state. The resulting reverse transition is a normal owner
    /// lifecycle operation with its own journal record.
    pub async fn compensate_failed_operation(
        &self,
        operation_id: Uuid,
        requested_by: Option<String>,
        idempotency_key: Uuid,
    ) -> Result<crate::ModuleLifecycleToggleResult, ModuleLifecycleDbWriterError> {
        if idempotency_key.is_nil() {
            return Err(ModuleLifecycleDbWriterError::Lifecycle(
                ModuleLifecycleExecutionError::InvalidIdempotencyKey,
            ));
        }
        let plan = module_operation_recovery_plan(&self.db, operation_id)
            .await
            .map_err(ModuleLifecycleDbWriterError::Recovery)?;
        if plan.issue != ModuleOperationIssue::PostHookFailed {
            return Err(ModuleLifecycleDbWriterError::Recovery(
                ModuleOperationRecoveryError::NotRetryable(plan.issue.as_str().to_string()),
            ));
        }
        let (_, effective_enabled_modules, _) = self
            .execution_context(plan.tenant_id, &plan.module_slug)
            .await?;
        let current_enabled = effective_enabled_modules.contains(&plan.module_slug);
        let replay_request = ModuleOperationRequest {
            tenant_id: plan.tenant_id,
            module_slug: plan.module_slug.clone(),
            requested_enabled: plan.previous_effective_enabled,
            previous_effective_enabled: current_enabled,
            requested_by: requested_by.clone(),
            correlation_id: plan.operation_id.to_string(),
            idempotency_key: Some(idempotency_key),
        };
        match ModuleOperationJournal::replay_idempotent_command(&self.db, &replay_request)
            .await
            .map_err(map_idempotency_command_error)?
        {
            Some(_) => {
                return self
                    .toggle_with_operation_context(
                        plan.tenant_id,
                        &plan.module_slug,
                        plan.previous_effective_enabled,
                        requested_by,
                        Some(plan.operation_id.to_string()),
                        Some(idempotency_key),
                    )
                    .await;
            }
            None => {}
        }
        if current_enabled != plan.requested_enabled {
            return Err(ModuleLifecycleDbWriterError::Recovery(
                ModuleOperationRecoveryError::StateMismatch {
                    requested_enabled: plan.requested_enabled,
                    current_enabled,
                },
            ));
        }
        self.toggle_with_operation_context(
            plan.tenant_id,
            &plan.module_slug,
            plan.previous_effective_enabled,
            requested_by,
            Some(plan.operation_id.to_string()),
            Some(idempotency_key),
        )
        .await
    }

    /// Persists a host-schema-normalized settings value while deriving module
    /// identity, Core status, and effective enablement from owner state.
    pub async fn persist_normalized_settings(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
        settings: serde_json::Value,
    ) -> Result<TenantModuleSettingsRecord, ModuleLifecycleDbWriterError> {
        let catalog = self.definition_catalog()?;
        let definition = catalog
            .get(module_slug)
            .ok_or_else(|| ModuleLifecycleDbWriterError::UnknownModule(module_slug.to_string()))?;
        let effective_enabled_modules = self.effective_enabled_modules(tenant_id).await?;
        TenantModuleStateStore::persist_settings(
            &self.db,
            TenantModuleSettingsRequest {
                tenant_id,
                module_slug: module_slug.to_string(),
                settings,
                is_core: definition.kind == ModuleDefinitionKind::Core,
                is_effectively_enabled: effective_enabled_modules.contains(module_slug),
            },
        )
        .await
        .map_err(ModuleLifecycleDbWriterError::Settings)
    }

    /// Confirms that the active owner catalog contains a module before a host
    /// adapter resolves its static-only settings schema.
    pub fn require_module_definition(
        &self,
        module_slug: &str,
    ) -> Result<(), ModuleLifecycleDbWriterError> {
        if self.definition_catalog()?.get(module_slug).is_none() {
            return Err(ModuleLifecycleDbWriterError::UnknownModule(
                module_slug.to_string(),
            ));
        }
        Ok(())
    }

    /// Resolves Core/default/tenant-override availability from the same owner
    /// catalog and tenant-state source used by lifecycle commands.
    pub async fn effective_enabled_modules(
        &self,
        tenant_id: Uuid,
    ) -> Result<HashSet<String>, ModuleLifecycleDbWriterError> {
        let catalog = self.definition_catalog()?;
        Ok(ModuleEffectivePolicyQuery::new(
            &catalog,
            self.default_enabled_modules.iter().cloned(),
            self.overrides(tenant_id).await?,
        )
        .execute()
        .into_enabled_modules())
    }

    async fn execution_context(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<
        (ModuleDefinitionCatalog, HashSet<String>, serde_json::Value),
        ModuleLifecycleDbWriterError,
    > {
        let catalog = self.definition_catalog()?;
        let effective_enabled_modules = self.effective_enabled_modules(tenant_id).await?;
        let current_settings = self.settings(tenant_id, module_slug).await?;
        Ok((catalog, effective_enabled_modules, current_settings))
    }

    fn definition_catalog(&self) -> Result<ModuleDefinitionCatalog, ModuleLifecycleDbWriterError> {
        match &self.catalog {
            Some(catalog) => Ok(catalog.clone()),
            None => Ok(ModuleDefinitionCatalog::from_static_registry(
                self.static_registry.ok_or_else(|| {
                    ModuleLifecycleDbWriterError::Configuration(
                        "static lifecycle writer has no module registry".into(),
                    )
                })?,
            )
            .map_err(ModuleLifecycleDbWriterError::Definition)?),
        }
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
    #[error(transparent)]
    Recovery(#[from] ModuleOperationRecoveryError),
    #[error("module `{0}` is not part of the active definition catalog")]
    UnknownModule(String),
    #[error(transparent)]
    Settings(#[from] ModuleOperationStoreError),
}

fn map_idempotency_command_error(error: ModuleOperationStoreError) -> ModuleLifecycleDbWriterError {
    match error {
        ModuleOperationStoreError::IdempotencyConflict => ModuleLifecycleDbWriterError::Lifecycle(
            ModuleLifecycleExecutionError::IdempotencyConflict,
        ),
        error => ModuleLifecycleDbWriterError::Lifecycle(
            ModuleLifecycleExecutionError::Persistence(error.to_string()),
        ),
    }
}

fn database_error(error: impl std::fmt::Display) -> ModuleLifecycleDbWriterError {
    ModuleLifecycleDbWriterError::Database(error.to_string())
}
