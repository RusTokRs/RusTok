use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    Set, TransactionTrait,
};
use thiserror::Error;

use rustok_core::{ModuleContext, ModuleRegistry};

use crate::models::_entities::module_operations::Entity as ModuleOperationsEntity;
use crate::models::_entities::tenant_modules::Entity as TenantModulesEntity;
use crate::models::_entities::{module_operations, tenant_modules};
use crate::modules::{ManifestError, ManifestManager};
use crate::services::effective_module_policy::EffectiveModulePolicyService;

pub struct ModuleLifecycleService;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleOperationStatus {
    Validated,
    Running,
    Committed,
    Failed,
}

impl ModuleOperationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Validated => "validated",
            Self::Running => "running",
            Self::Committed => "committed",
            Self::Failed => "failed",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Committed | Self::Failed)
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "validated" => Some(Self::Validated),
            "running" => Some(Self::Running),
            "committed" => Some(Self::Committed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

impl std::fmt::Display for ModuleOperationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ModuleOperationStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or(())
    }
}

impl From<ModuleOperationStatus> for String {
    fn from(value: ModuleOperationStatus) -> Self {
        value.as_str().to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleOperationIssue {
    None,
    PreHookFailed,
    PostHookFailed,
    OtherFailed,
}

impl ModuleOperationIssue {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::PreHookFailed => "pre_hook_failed",
            Self::PostHookFailed => "post_hook_failed",
            Self::OtherFailed => "other_failed",
        }
    }

    pub const fn retryable(self) -> bool {
        matches!(self, Self::PostHookFailed)
    }
}

impl std::fmt::Display for ModuleOperationIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleOperationRecoveryAction {
    None,
    RetryPostHook,
    RepeatToggle,
    CompensatingToggle,
}

impl ModuleOperationRecoveryAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::RetryPostHook => "retry_post_hook",
            Self::RepeatToggle => "repeat_toggle",
            Self::CompensatingToggle => "compensating_toggle",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleOperationRecoveryPlan {
    pub operation_id: uuid::Uuid,
    pub tenant_id: uuid::Uuid,
    pub module_slug: String,
    pub requested_enabled: bool,
    pub previous_effective_enabled: bool,
    pub status: ModuleOperationStatus,
    pub issue: ModuleOperationIssue,
    pub retryable: bool,
    pub recommended_action: ModuleOperationRecoveryAction,
    pub correlation_id: Option<String>,
    pub requested_by: Option<String>,
    pub error_message: Option<String>,
}

impl ModuleOperationRecoveryPlan {
    pub fn from_operation(operation: &module_operations::Model) -> Self {
        let status = ModuleOperationStatus::parse(operation.status.as_str())
            .unwrap_or(ModuleOperationStatus::Failed);
        let error_message = operation.error_message.as_deref().unwrap_or_default();
        let issue = if status != ModuleOperationStatus::Failed {
            ModuleOperationIssue::None
        } else if error_message.starts_with("post-hook:") {
            ModuleOperationIssue::PostHookFailed
        } else if error_message.is_empty() {
            ModuleOperationIssue::OtherFailed
        } else {
            ModuleOperationIssue::PreHookFailed
        };
        let retryable = issue.retryable();
        let recommended_action = if retryable {
            ModuleOperationRecoveryAction::RetryPostHook
        } else if issue == ModuleOperationIssue::PreHookFailed {
            ModuleOperationRecoveryAction::RepeatToggle
        } else {
            ModuleOperationRecoveryAction::None
        };

        Self {
            operation_id: operation.id,
            tenant_id: operation.tenant_id,
            module_slug: operation.module_slug.clone(),
            requested_enabled: operation.requested_enabled,
            previous_effective_enabled: operation.previous_effective_enabled,
            status,
            issue,
            retryable,
            recommended_action,
            correlation_id: operation.correlation_id.clone(),
            requested_by: operation.requested_by.clone(),
            error_message: operation.error_message.clone(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ModuleOperationRecoveryError {
    #[error("Module operation not found")]
    OperationNotFound,
    #[error("Module operation is not retryable: {0}")]
    NotRetryable(String),
    #[error("Module operation state mismatch: requested enabled={requested_enabled}, current enabled={current_enabled}")]
    StateMismatch {
        requested_enabled: bool,
        current_enabled: bool,
    },
    #[error("Module post-hook retry failed: {0}")]
    PostHookFailed(String),
    #[error("Database error: {0}")]
    Database(#[from] DbErr),
    #[error("Platform module policy error: {0}")]
    Policy(String),
    #[error("Toggle recovery failed: {0}")]
    Toggle(#[from] ToggleModuleError),
}

#[derive(Debug, Error)]
pub enum ToggleModuleError {
    #[error("Unknown module")]
    UnknownModule,
    /// Core modules are part of the platform kernel and can never be disabled.
    /// See `ModuleKind::Core` and `DECISIONS/2026-02-19-module-kind-core-vs-optional.md`.
    #[error("Module '{0}' is a core platform module and cannot be disabled")]
    CoreModuleCannotBeDisabled(String),
    #[error("Missing module dependencies: {0}")]
    MissingDependencies(String),
    #[error("Module is required by: {0}")]
    HasDependents(String),
    #[error("Database error: {0}")]
    Database(#[from] DbErr),
    #[error("Module pre-hook failed: {0}")]
    PreHookFailed(String),
    #[error("Module post-hook failed: {0}")]
    PostHookFailed(String),
    #[error("Platform module policy error: {0}")]
    Policy(String),
}

#[derive(Debug, Error)]
pub enum UpdateModuleSettingsError {
    #[error("Unknown module")]
    UnknownModule,
    #[error("Module '{0}' is not enabled for this tenant")]
    ModuleNotEnabled(String),
    #[error("Module settings must be a JSON object")]
    InvalidSettings,
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Manifest(#[from] ManifestError),
    #[error("Platform module policy error: {0}")]
    Policy(String),
    #[error("Database error: {0}")]
    Database(#[from] DbErr),
}

impl ModuleLifecycleService {
    fn generate_correlation_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    pub async fn toggle_module(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        module_slug: &str,
        enabled: bool,
    ) -> Result<tenant_modules::Model, ToggleModuleError> {
        Self::toggle_module_with_actor(db, registry, tenant_id, module_slug, enabled, None).await
    }

    pub async fn toggle_module_with_actor(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        module_slug: &str,
        enabled: bool,
        requested_by: Option<String>,
    ) -> Result<tenant_modules::Model, ToggleModuleError> {
        let Some(module_impl) = registry.get(module_slug) else {
            return Err(ToggleModuleError::UnknownModule);
        };

        if !enabled && registry.is_core(module_slug) {
            return Err(ToggleModuleError::CoreModuleCannotBeDisabled(
                module_slug.to_string(),
            ));
        }

        let enabled_set = EffectiveModulePolicyService::resolve_enabled(db, registry, tenant_id)
            .await
            .map_err(|error| ToggleModuleError::Policy(error.to_string()))?;
        let previous_effective_enabled = enabled_set.contains(module_slug);

        if enabled {
            let missing: Vec<String> = module_impl
                .dependencies()
                .iter()
                .filter(|dependency| !enabled_set.contains(**dependency))
                .map(|dependency| (*dependency).to_string())
                .collect();

            if !missing.is_empty() {
                return Err(ToggleModuleError::MissingDependencies(missing.join(", ")));
            }
        } else {
            let dependents: Vec<String> = registry
                .list()
                .into_iter()
                .filter(|module| enabled_set.contains(module.slug()))
                .filter(|module| module.dependencies().contains(&module_slug))
                .map(|module| module.slug().to_string())
                .collect();

            if !dependents.is_empty() {
                return Err(ToggleModuleError::HasDependents(dependents.join(", ")));
            }
        }

        if previous_effective_enabled == enabled {
            let (module, _, _) =
                Self::persist_module_state(db, tenant_id, module_slug, enabled).await?;
            return Ok(module);
        }

        let operation = Self::record_operation(
            db,
            tenant_id,
            module_slug,
            enabled,
            previous_effective_enabled,
            requested_by,
        )
        .await?;
        Self::mark_operation_running(db, operation.id).await?;

        let hook_settings = Self::current_module_settings(db, tenant_id, module_slug).await?;
        let module_ctx = ModuleContext {
            db,
            tenant_id,
            config: &hook_settings,
        };

        let hook_result = if enabled {
            module_impl.pre_enable(module_ctx).await
        } else {
            module_impl.pre_disable(module_ctx).await
        };

        if let Err(err) = hook_result {
            tracing::error!(
                "Module pre-hook failed for {} (enabled={}): {}; tenant state was not changed",
                module_slug,
                enabled,
                err
            );

            Self::mark_operation_failed(db, operation.id, &err.to_string()).await?;
            return Err(ToggleModuleError::PreHookFailed(err.to_string()));
        }

        let module =
            Self::commit_module_state(db, operation.id, tenant_id, module_slug, enabled).await?;

        let post_settings = Self::current_module_settings(db, tenant_id, module_slug).await?;
        let post_ctx = ModuleContext {
            db,
            tenant_id,
            config: &post_settings,
        };

        let post_hook_result = if enabled {
            module_impl.post_enable(post_ctx).await
        } else {
            module_impl.post_disable(post_ctx).await
        };

        if let Err(err) = post_hook_result {
            tracing::error!(
                "Module post-hook failed for {} (enabled={}): {}; tenant state remains committed",
                module_slug,
                enabled,
                err
            );
            Self::mark_operation_failed(db, operation.id, &format!("post-hook: {err}")).await?;
            return Err(ToggleModuleError::PostHookFailed(err.to_string()));
        }

        Ok(module)
    }

    pub async fn module_operation_recovery_plan(
        db: &DatabaseConnection,
        operation_id: uuid::Uuid,
    ) -> Result<ModuleOperationRecoveryPlan, ModuleOperationRecoveryError> {
        let operation = ModuleOperationsEntity::find_by_id(operation_id)
            .one(db)
            .await?
            .ok_or(ModuleOperationRecoveryError::OperationNotFound)?;
        Ok(ModuleOperationRecoveryPlan::from_operation(&operation))
    }

    pub async fn failed_module_operation_recovery_plans(
        db: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        module_slug: Option<&str>,
    ) -> Result<Vec<ModuleOperationRecoveryPlan>, ModuleOperationRecoveryError> {
        let mut query = ModuleOperationsEntity::find()
            .filter(module_operations::Column::TenantId.eq(tenant_id))
            .filter(module_operations::Column::Status.eq(ModuleOperationStatus::Failed.as_str()))
            .order_by_desc(module_operations::Column::CreatedAt);

        if let Some(module_slug) = module_slug {
            query = query.filter(module_operations::Column::ModuleSlug.eq(module_slug));
        }

        Ok(query
            .all(db)
            .await?
            .iter()
            .map(ModuleOperationRecoveryPlan::from_operation)
            .collect())
    }

    pub async fn retry_failed_post_hook_operation(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        operation_id: uuid::Uuid,
        requested_by: Option<String>,
    ) -> Result<module_operations::Model, ModuleOperationRecoveryError> {
        let failed_operation = ModuleOperationsEntity::find_by_id(operation_id)
            .one(db)
            .await?
            .ok_or(ModuleOperationRecoveryError::OperationNotFound)?;
        let plan = ModuleOperationRecoveryPlan::from_operation(&failed_operation);

        if !plan.retryable {
            return Err(ModuleOperationRecoveryError::NotRetryable(
                plan.issue.to_string(),
            ));
        }

        let Some(module_impl) = registry.get(plan.module_slug.as_str()) else {
            return Err(ModuleOperationRecoveryError::NotRetryable(
                "unknown_module".to_string(),
            ));
        };

        let enabled_set =
            EffectiveModulePolicyService::resolve_enabled(db, registry, plan.tenant_id)
                .await
                .map_err(|error| ModuleOperationRecoveryError::Policy(error.to_string()))?;
        let current_enabled = enabled_set.contains(plan.module_slug.as_str());
        if current_enabled != plan.requested_enabled {
            return Err(ModuleOperationRecoveryError::StateMismatch {
                requested_enabled: plan.requested_enabled,
                current_enabled,
            });
        }

        let retry_operation = Self::record_operation(
            db,
            plan.tenant_id,
            plan.module_slug.as_str(),
            plan.requested_enabled,
            current_enabled,
            requested_by,
        )
        .await?;
        Self::mark_operation_running(db, retry_operation.id).await?;

        let post_settings =
            Self::current_module_settings(db, plan.tenant_id, plan.module_slug.as_str()).await?;
        let post_ctx = ModuleContext {
            db,
            tenant_id: plan.tenant_id,
            config: &post_settings,
        };
        let post_hook_result = if plan.requested_enabled {
            module_impl.post_enable(post_ctx).await
        } else {
            module_impl.post_disable(post_ctx).await
        };

        if let Err(err) = post_hook_result {
            Self::mark_operation_failed(db, retry_operation.id, &format!("post-hook: {err}"))
                .await?;
            return Err(ModuleOperationRecoveryError::PostHookFailed(
                err.to_string(),
            ));
        }

        Self::mark_operation_committed(db, retry_operation.id).await?;
        ModuleOperationsEntity::find_by_id(retry_operation.id)
            .one(db)
            .await?
            .ok_or(ModuleOperationRecoveryError::OperationNotFound)
    }

    pub async fn compensate_failed_operation(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        operation_id: uuid::Uuid,
        requested_by: Option<String>,
    ) -> Result<tenant_modules::Model, ModuleOperationRecoveryError> {
        let failed_operation = ModuleOperationsEntity::find_by_id(operation_id)
            .one(db)
            .await?
            .ok_or(ModuleOperationRecoveryError::OperationNotFound)?;
        let plan = ModuleOperationRecoveryPlan::from_operation(&failed_operation);

        Self::toggle_module_with_actor(
            db,
            registry,
            plan.tenant_id,
            plan.module_slug.as_str(),
            plan.previous_effective_enabled,
            requested_by,
        )
        .await
        .map_err(ModuleOperationRecoveryError::Toggle)
    }

    pub async fn update_module_settings(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        module_slug: &str,
        settings: serde_json::Value,
    ) -> Result<tenant_modules::Model, UpdateModuleSettingsError> {
        let Some(_module_impl) = registry.get(module_slug) else {
            return Err(UpdateModuleSettingsError::UnknownModule);
        };

        if !settings.is_object() {
            return Err(UpdateModuleSettingsError::InvalidSettings);
        }

        let settings =
            ManifestManager::validate_module_settings(module_slug, settings).map_err(|err| {
                match err {
                    ManifestError::InvalidModuleSettingValue { .. } => {
                        UpdateModuleSettingsError::Validation(err.to_string())
                    }
                    other => UpdateModuleSettingsError::Manifest(other),
                }
            })?;

        let existing = TenantModulesEntity::find()
            .filter(tenant_modules::Column::TenantId.eq(tenant_id))
            .filter(tenant_modules::Column::ModuleSlug.eq(module_slug))
            .one(db)
            .await?;

        let is_core = registry.is_core(module_slug);
        let is_effectively_enabled =
            EffectiveModulePolicyService::is_enabled(db, registry, tenant_id, module_slug)
                .await
                .map_err(|error| UpdateModuleSettingsError::Policy(error.to_string()))?;

        match existing {
            Some(model) => {
                if !is_effectively_enabled {
                    return Err(UpdateModuleSettingsError::ModuleNotEnabled(
                        module_slug.to_string(),
                    ));
                }

                let was_enabled = model.enabled;
                let mut active: tenant_modules::ActiveModel = model.into();
                active.enabled = Set(is_core || was_enabled);
                active.settings = Set(settings);
                Ok(active.update(db).await?)
            }
            None if is_core || is_effectively_enabled => {
                let module = tenant_modules::ActiveModel {
                    id: Set(rustok_core::generate_id()),
                    tenant_id: Set(tenant_id),
                    module_slug: Set(module_slug.to_string()),
                    enabled: Set(is_effectively_enabled),
                    settings: Set(settings),
                    created_at: sea_orm::ActiveValue::NotSet,
                    updated_at: sea_orm::ActiveValue::NotSet,
                }
                .insert(db)
                .await?;

                Ok(module)
            }
            None => Err(UpdateModuleSettingsError::ModuleNotEnabled(
                module_slug.to_string(),
            )),
        }
    }

    async fn current_module_settings(
        db: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        module_slug: &str,
    ) -> Result<serde_json::Value, DbErr> {
        Ok(TenantModulesEntity::find()
            .filter(tenant_modules::Column::TenantId.eq(tenant_id))
            .filter(tenant_modules::Column::ModuleSlug.eq(module_slug))
            .one(db)
            .await?
            .map(|model| model.settings)
            .unwrap_or_else(|| serde_json::json!({})))
    }

    async fn commit_module_state(
        db: &DatabaseConnection,
        operation_id: uuid::Uuid,
        tenant_id: uuid::Uuid,
        module_slug: &str,
        enabled: bool,
    ) -> Result<tenant_modules::Model, DbErr> {
        let module_slug = module_slug.to_string();

        db.transaction::<_, tenant_modules::Model, DbErr>(move |txn| {
            let module_slug = module_slug.clone();
            Box::pin(async move {
                let (module, _, _) =
                    Self::persist_module_state_on(txn, tenant_id, &module_slug, enabled).await?;

                if let Some(model) = ModuleOperationsEntity::find_by_id(operation_id)
                    .one(txn)
                    .await?
                {
                    Self::mark_operation_model_committed(txn, model).await?;
                }

                Ok(module)
            })
        })
        .await
        .map_err(|err| match err {
            sea_orm::TransactionError::Connection(db_err) => db_err,
            sea_orm::TransactionError::Transaction(db_err) => db_err,
        })
    }

    async fn persist_module_state(
        db: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        module_slug: &str,
        enabled: bool,
    ) -> Result<(tenant_modules::Model, bool, bool), DbErr> {
        let module_slug = module_slug.to_string();

        db.transaction::<_, (tenant_modules::Model, bool, bool), DbErr>(move |txn| {
            let module_slug = module_slug.clone();
            Box::pin(async move {
                Self::persist_module_state_on(txn, tenant_id, &module_slug, enabled).await
            })
        })
        .await
        .map_err(|err| match err {
            sea_orm::TransactionError::Connection(db_err) => db_err,
            sea_orm::TransactionError::Transaction(db_err) => db_err,
        })
    }

    async fn persist_module_state_on<C>(
        db: &C,
        tenant_id: uuid::Uuid,
        module_slug: &str,
        enabled: bool,
    ) -> Result<(tenant_modules::Model, bool, bool), DbErr>
    where
        C: sea_orm::ConnectionTrait,
    {
        let existing = TenantModulesEntity::find()
            .filter(tenant_modules::Column::TenantId.eq(tenant_id))
            .filter(tenant_modules::Column::ModuleSlug.eq(module_slug))
            .one(db)
            .await?;

        match existing {
            Some(model) => {
                if model.enabled == enabled {
                    return Ok((model.clone(), model.enabled, false));
                }

                let previous_enabled = model.enabled;
                let mut active: tenant_modules::ActiveModel = model.into();
                active.enabled = Set(enabled);
                let updated = active.update(db).await?;
                Ok((updated, previous_enabled, true))
            }
            None => {
                let module = tenant_modules::ActiveModel {
                    id: Set(rustok_core::generate_id()),
                    tenant_id: Set(tenant_id),
                    module_slug: Set(module_slug.to_string()),
                    enabled: Set(enabled),
                    settings: Set(serde_json::json!({})),
                    created_at: sea_orm::ActiveValue::NotSet,
                    updated_at: sea_orm::ActiveValue::NotSet,
                }
                .insert(db)
                .await?;

                Ok((module, !enabled, true))
            }
        }
    }

    async fn record_operation(
        db: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        module_slug: &str,
        requested_enabled: bool,
        previous_effective_enabled: bool,
        requested_by: Option<String>,
    ) -> Result<module_operations::Model, DbErr> {
        let now = chrono::Utc::now().into();
        module_operations::ActiveModel {
            id: sea_orm::ActiveValue::Set(rustok_core::generate_id()),
            tenant_id: sea_orm::ActiveValue::Set(tenant_id),
            module_slug: sea_orm::ActiveValue::Set(module_slug.to_string()),
            requested_enabled: sea_orm::ActiveValue::Set(requested_enabled),
            previous_effective_enabled: sea_orm::ActiveValue::Set(previous_effective_enabled),
            status: sea_orm::ActiveValue::Set(ModuleOperationStatus::Validated.into()),
            requested_by: sea_orm::ActiveValue::Set(requested_by),
            correlation_id: sea_orm::ActiveValue::Set(Some(Self::generate_correlation_id())),
            error_message: sea_orm::ActiveValue::Set(None),
            created_at: sea_orm::ActiveValue::Set(now),
            updated_at: sea_orm::ActiveValue::Set(now),
        }
        .insert(db)
        .await
    }

    async fn mark_operation_committed(
        db: &DatabaseConnection,
        operation_id: uuid::Uuid,
    ) -> Result<(), DbErr> {
        if let Some(model) = ModuleOperationsEntity::find_by_id(operation_id)
            .one(db)
            .await?
        {
            Self::mark_operation_model_committed(db, model).await?;
        }
        Ok(())
    }

    async fn mark_operation_model_committed<C>(
        db: &C,
        model: module_operations::Model,
    ) -> Result<(), DbErr>
    where
        C: sea_orm::ConnectionTrait,
    {
        let mut active: module_operations::ActiveModel = model.into();
        active.status = sea_orm::ActiveValue::Set(ModuleOperationStatus::Committed.into());
        active.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
        active.update(db).await?;
        Ok(())
    }

    async fn mark_operation_failed(
        db: &DatabaseConnection,
        operation_id: uuid::Uuid,
        error_message: &str,
    ) -> Result<(), DbErr> {
        if let Some(model) = ModuleOperationsEntity::find_by_id(operation_id)
            .one(db)
            .await?
        {
            let mut active: module_operations::ActiveModel = model.into();
            active.status = sea_orm::ActiveValue::Set(ModuleOperationStatus::Failed.into());
            active.error_message = sea_orm::ActiveValue::Set(Some(error_message.to_string()));
            active.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
            active.update(db).await?;
        }
        Ok(())
    }

    async fn mark_operation_running(
        db: &DatabaseConnection,
        operation_id: uuid::Uuid,
    ) -> Result<(), DbErr> {
        if let Some(model) = ModuleOperationsEntity::find_by_id(operation_id)
            .one(db)
            .await?
        {
            let mut active: module_operations::ActiveModel = model.into();
            active.status = sea_orm::ActiveValue::Set(ModuleOperationStatus::Running.into());
            active.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
            active.update(db).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ModuleOperationStatus;
    use super::{ModuleLifecycleService, UpdateModuleSettingsError};
    use crate::models::_entities::tenant_modules;
    use crate::models::tenants;
    use crate::modules::{build_registry, ManifestManager, ManifestModuleSpec, ModulesManifest};
    use migration::Migrator;
    use rustok_core::ModuleRegistry;
    use rustok_index::IndexModule;
    use rustok_rbac::RbacModule;
    use rustok_tenant::TenantModule;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};
    use serial_test::serial;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn module_operation_status_roundtrip() {
        for status in [
            ModuleOperationStatus::Validated,
            ModuleOperationStatus::Running,
            ModuleOperationStatus::Committed,
            ModuleOperationStatus::Failed,
        ] {
            let encoded = status.to_string();
            assert_eq!(ModuleOperationStatus::parse(&encoded), Some(status));
        }
        assert_eq!(ModuleOperationStatus::parse("unknown"), None);
        assert_eq!(
            "validated".parse::<ModuleOperationStatus>(),
            Ok(ModuleOperationStatus::Validated)
        );
        assert_eq!(
            "running".parse::<ModuleOperationStatus>(),
            Ok(ModuleOperationStatus::Running)
        );
        assert_eq!(
            "committed".parse::<ModuleOperationStatus>(),
            Ok(ModuleOperationStatus::Committed)
        );
        assert_eq!(
            "failed".parse::<ModuleOperationStatus>(),
            Ok(ModuleOperationStatus::Failed)
        );
        assert_eq!("unknown".parse::<ModuleOperationStatus>(), Err(()));
        assert_eq!(String::from(ModuleOperationStatus::Validated), "validated");
        assert_eq!(String::from(ModuleOperationStatus::Running), "running");
        assert!(!ModuleOperationStatus::Validated.is_terminal());
        assert!(!ModuleOperationStatus::Running.is_terminal());
        assert!(ModuleOperationStatus::Committed.is_terminal());
        assert!(ModuleOperationStatus::Failed.is_terminal());
    }

    #[test]
    fn generated_correlation_id_is_uuid_v4_string() {
        let value = ModuleLifecycleService::generate_correlation_id();
        assert_eq!(value.len(), 36);
        let parsed = uuid::Uuid::parse_str(&value).expect("correlation id must be valid UUID");
        assert_eq!(parsed.get_version_num(), 4);
    }

    struct OptionalSettingsModule;

    impl rustok_core::MigrationSource for OptionalSettingsModule {
        fn migrations(&self) -> Vec<Box<dyn sea_orm_migration::MigrationTrait>> {
            vec![]
        }
    }

    #[async_trait::async_trait]
    impl rustok_core::RusToKModule for OptionalSettingsModule {
        fn slug(&self) -> &'static str {
            "content"
        }

        fn name(&self) -> &'static str {
            "settings-test-content"
        }

        fn description(&self) -> &'static str {
            "optional module used by settings lifecycle tests"
        }

        fn version(&self) -> &'static str {
            "0.1.0"
        }
    }

    fn build_settings_registry() -> ModuleRegistry {
        build_registry().register(OptionalSettingsModule)
    }

    fn path_module(crate_name: &str, path: &str, required: bool) -> ManifestModuleSpec {
        ManifestModuleSpec {
            source: "path".to_string(),
            crate_name: crate_name.to_string(),
            path: Some(path.to_string()),
            required,
            ..Default::default()
        }
    }

    fn set_manifest_env(path: &std::path::Path) -> Option<String> {
        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", path);
        }
        previous
    }

    fn restore_manifest_env(previous: Option<String>) {
        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }
    }

    fn write_module_manifest(crate_dir: &std::path::Path, contents: &str) {
        std::fs::create_dir_all(crate_dir).expect("create module dir");
        std::fs::write(crate_dir.join("rustok-module.toml"), contents)
            .expect("write module manifest");
    }

    fn build_test_registry() -> ModuleRegistry {
        ModuleRegistry::new()
            .register(IndexModule)
            .register(TenantModule)
            .register(RbacModule)
    }

    #[test]
    fn disable_core_module_is_rejected() {
        let registry = build_test_registry();
        assert!(registry.is_core("tenant"));
        assert!(registry.is_core("rbac"));
        assert!(registry.is_core("index"));
    }

    #[test]
    fn disable_optional_module_is_allowed() {
        let registry = build_test_registry();
        assert!(!registry.is_core("content"));
        assert!(!registry.is_core("commerce"));
        assert!(!registry.is_core("blog"));
    }

    #[tokio::test]
    #[serial]
    async fn update_module_settings_rejects_disabled_optional_module() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let registry = build_settings_registry();
        let tenant =
            tenants::ActiveModel::new("Module settings tenant", "module-settings-disabled")
                .insert(&db)
                .await
                .expect("insert tenant");
        let temp = tempdir().expect("tempdir");
        let manifest_path = temp.path().join("modules.toml");
        let mut modules = HashMap::new();
        modules.insert(
            "content".to_string(),
            path_module("rustok-content", "crates/rustok-content", false),
        );
        let manifest = ModulesManifest {
            schema: 2,
            app: "rustok-server".to_string(),
            modules,
            ..Default::default()
        };
        ManifestManager::save_to_path(&manifest_path, &manifest).expect("save manifest");
        let previous = set_manifest_env(&manifest_path);

        let result = ModuleLifecycleService::update_module_settings(
            &db,
            &registry,
            tenant.id,
            "content",
            serde_json::json!({ "postsPerPage": 20 }),
        )
        .await;
        restore_manifest_env(previous);

        assert!(matches!(
            result,
            Err(UpdateModuleSettingsError::ModuleNotEnabled(slug)) if slug == "content"
        ));
    }

    #[tokio::test]
    #[serial]
    async fn update_module_settings_persists_enabled_optional_module() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let registry = build_settings_registry();
        let tenant = tenants::ActiveModel::new("Module settings tenant", "module-settings-enabled")
            .insert(&db)
            .await
            .expect("insert tenant");
        let temp = tempdir().expect("tempdir");
        let manifest_path = temp.path().join("modules.toml");
        let mut modules = HashMap::new();
        modules.insert(
            "content".to_string(),
            path_module("rustok-content", "crates/rustok-content", false),
        );
        let manifest = ModulesManifest {
            schema: 2,
            app: "rustok-server".to_string(),
            modules,
            ..Default::default()
        };
        ManifestManager::save_to_path(&manifest_path, &manifest).expect("save manifest");
        let previous = set_manifest_env(&manifest_path);

        ModuleLifecycleService::toggle_module(&db, &registry, tenant.id, "content", true)
            .await
            .expect("enable content module");

        let updated = ModuleLifecycleService::update_module_settings(
            &db,
            &registry,
            tenant.id,
            "content",
            serde_json::json!({ "postsPerPage": 20 }),
        )
        .await
        .expect("update module settings");
        restore_manifest_env(previous);

        assert!(updated.enabled);
        assert_eq!(updated.settings["postsPerPage"], serde_json::json!(20));
    }

    #[tokio::test]
    #[serial]
    async fn update_module_settings_upserts_core_module_row() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let registry = build_registry();
        let tenant = tenants::ActiveModel::new("Module settings tenant", "module-settings-core")
            .insert(&db)
            .await
            .expect("insert tenant");
        let temp = tempdir().expect("tempdir");
        let manifest_path = temp.path().join("modules.toml");
        let mut modules = HashMap::new();
        modules.insert(
            "tenant".to_string(),
            path_module("rustok-tenant", "crates/rustok-tenant", true),
        );
        let manifest = ModulesManifest {
            schema: 2,
            app: "rustok-server".to_string(),
            modules,
            ..Default::default()
        };
        ManifestManager::save_to_path(&manifest_path, &manifest).expect("save manifest");
        let previous = set_manifest_env(&manifest_path);

        let updated = ModuleLifecycleService::update_module_settings(
            &db,
            &registry,
            tenant.id,
            "tenant",
            serde_json::json!({ "workspaceName": "Acme" }),
        )
        .await
        .expect("update core module settings");
        restore_manifest_env(previous);

        assert!(updated.enabled);
        assert_eq!(updated.module_slug, "tenant");

        let stored = tenant_modules::Entity::find()
            .filter(tenant_modules::Column::TenantId.eq(tenant.id))
            .filter(tenant_modules::Column::ModuleSlug.eq("tenant"))
            .one(&db)
            .await
            .expect("load stored core settings")
            .expect("tenant_modules row");
        assert_eq!(stored.settings["workspaceName"], serde_json::json!("Acme"));
        assert!(stored.enabled);
    }

    #[tokio::test]
    #[serial]
    async fn update_module_settings_applies_schema_defaults() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let registry = build_settings_registry();
        let tenant = tenants::ActiveModel::new("Module settings tenant", "module-settings-schema")
            .insert(&db)
            .await
            .expect("insert tenant");

        let temp = tempdir().expect("tempdir");
        let content_dir = temp.path().join("crates").join("rustok-content");
        write_module_manifest(
            &content_dir,
            r#"[module]
slug = "content"
name = "Content"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
postsPerPage = { type = "integer", default = 20, min = 1, max = 100 }
showSummaries = { type = "boolean", default = true }
"#,
        );

        let manifest_path = temp.path().join("modules.toml");
        let mut modules = HashMap::new();
        modules.insert(
            "content".to_string(),
            path_module("rustok-content", "crates/rustok-content", false),
        );
        let manifest = ModulesManifest {
            schema: 2,
            app: "rustok-server".to_string(),
            modules,
            ..Default::default()
        };
        ManifestManager::save_to_path(&manifest_path, &manifest).expect("save manifest");
        let previous = set_manifest_env(&manifest_path);

        ModuleLifecycleService::toggle_module(&db, &registry, tenant.id, "content", true)
            .await
            .expect("enable content module");

        let updated = ModuleLifecycleService::update_module_settings(
            &db,
            &registry,
            tenant.id,
            "content",
            serde_json::json!({}),
        )
        .await
        .expect("update module settings");
        restore_manifest_env(previous);

        assert_eq!(updated.settings["postsPerPage"], serde_json::json!(20));
        assert_eq!(updated.settings["showSummaries"], serde_json::json!(true));
    }
}
