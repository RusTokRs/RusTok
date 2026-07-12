use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    Set,
};
use thiserror::Error;

use rustok_core::ModuleRegistry;
use rustok_modules::{
    execute_module_toggle, failed_module_operation_recovery_plans, module_operation_recovery_plan,
    retry_failed_post_hook_operation, ModuleLifecycleExecutionError, ModuleLifecycleToggleRequest,
    ModuleOperationRecoveryError as ModulesRecoveryError, ModuleOperationRecoveryPlan,
    ModuleOperationStatus, ModulePostHookRetryRequest, ModuleToggleValidationError,
};

use crate::models::_entities::module_operations::Entity as ModuleOperationsEntity;
use crate::models::_entities::tenant_modules::Entity as TenantModulesEntity;
use crate::models::_entities::{module_operations, tenant_modules};
use crate::modules::{ManifestError, ManifestManager};
use crate::services::effective_module_policy::EffectiveModulePolicyService;

pub struct ModuleLifecycleService;

#[derive(Debug, Error)]
pub enum ModuleOperationRecoveryError {
    #[error("Module operation not found")]
    OperationNotFound,
    #[error("Module operation is not retryable: {0}")]
    NotRetryable(String),
    #[error(
        "Module operation state mismatch: requested enabled={requested_enabled}, current enabled={current_enabled}"
    )]
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
        let enabled_set = EffectiveModulePolicyService::resolve_enabled(db, registry, tenant_id)
            .await
            .map_err(|error| ToggleModuleError::Policy(error.to_string()))?;
        let current_settings = Self::current_module_settings(db, tenant_id, module_slug).await?;
        let result = execute_module_toggle(
            db,
            registry,
            ModuleLifecycleToggleRequest {
                tenant_id,
                module_slug: module_slug.to_string(),
                enabled,
                requested_by,
                effective_enabled_modules: enabled_set,
                current_settings,
            },
        )
        .await
        .map_err(map_toggle_execution_error)?;
        TenantModulesEntity::find_by_id(result.state.id)
            .one(db)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("tenant_modules.toggle_state".to_string()))
    }

    pub async fn module_operation_recovery_plan(
        db: &DatabaseConnection,
        operation_id: uuid::Uuid,
    ) -> Result<ModuleOperationRecoveryPlan, ModuleOperationRecoveryError> {
        module_operation_recovery_plan(db, operation_id)
            .await
            .map_err(map_module_recovery_error)
    }

    pub async fn failed_module_operation_recovery_plans(
        db: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        module_slug: Option<&str>,
    ) -> Result<Vec<ModuleOperationRecoveryPlan>, ModuleOperationRecoveryError> {
        let plans = failed_module_operation_recovery_plans(db, tenant_id, module_slug)
            .await
            .map_err(map_module_recovery_error)?;
        Ok(plans)
    }

    pub async fn retry_failed_post_hook_operation(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        operation_id: uuid::Uuid,
        requested_by: Option<String>,
    ) -> Result<module_operations::Model, ModuleOperationRecoveryError> {
        let plan = Self::module_operation_recovery_plan(db, operation_id).await?;

        let enabled_set =
            EffectiveModulePolicyService::resolve_enabled(db, registry, plan.tenant_id)
                .await
                .map_err(|error| ModuleOperationRecoveryError::Policy(error.to_string()))?;
        let post_settings =
            Self::current_module_settings(db, plan.tenant_id, plan.module_slug.as_str()).await?;
        let retry_operation = retry_failed_post_hook_operation(
            db,
            registry,
            ModulePostHookRetryRequest {
                operation_id,
                requested_by,
                effective_enabled_modules: enabled_set,
                current_settings: post_settings,
            },
        )
        .await
        .map_err(map_module_recovery_error)?;
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
        let plan = Self::module_operation_recovery_plan(db, operation_id).await?;

        if plan.issue != ModuleOperationIssue::PostHookFailed {
            return Err(ModuleOperationRecoveryError::NotRetryable(
                plan.issue.as_str().to_string(),
            ));
        }

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
}

fn map_toggle_validation_error(error: ModuleToggleValidationError) -> ToggleModuleError {
    match error {
        ModuleToggleValidationError::UnknownModule => ToggleModuleError::UnknownModule,
        ModuleToggleValidationError::CoreModuleCannotBeDisabled(module_slug) => {
            ToggleModuleError::CoreModuleCannotBeDisabled(module_slug)
        }
        ModuleToggleValidationError::MissingDependencies(dependencies) => {
            ToggleModuleError::MissingDependencies(dependencies.join(", "))
        }
        ModuleToggleValidationError::HasDependents(dependents) => {
            ToggleModuleError::HasDependents(dependents.join(", "))
        }
    }
}

fn map_toggle_execution_error(error: ModuleLifecycleExecutionError) -> ToggleModuleError {
    match error {
        ModuleLifecycleExecutionError::Validation(error) => map_toggle_validation_error(error),
        ModuleLifecycleExecutionError::Persistence(error) => {
            ToggleModuleError::Database(DbErr::Custom(error))
        }
        ModuleLifecycleExecutionError::PreHook(error) => ToggleModuleError::PreHookFailed(error),
        ModuleLifecycleExecutionError::PostHook(error) => ToggleModuleError::PostHookFailed(error),
    }
}

fn map_module_recovery_error(error: ModulesRecoveryError) -> ModuleOperationRecoveryError {
    match error {
        ModulesRecoveryError::OperationNotFound => ModuleOperationRecoveryError::OperationNotFound,
        ModulesRecoveryError::NotRetryable(reason) => {
            ModuleOperationRecoveryError::NotRetryable(reason)
        }
        ModulesRecoveryError::StateMismatch {
            requested_enabled,
            current_enabled,
        } => ModuleOperationRecoveryError::StateMismatch {
            requested_enabled,
            current_enabled,
        },
        ModulesRecoveryError::PostHookFailed(error) => {
            ModuleOperationRecoveryError::PostHookFailed(error)
        }
        ModulesRecoveryError::Persistence(error) => {
            ModuleOperationRecoveryError::Database(DbErr::Custom(error))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ModuleLifecycleService, UpdateModuleSettingsError};
    use crate::models::_entities::tenant_modules;
    use crate::models::tenants;
    use crate::modules::{build_registry, ManifestManager, ManifestModuleSpec, ModulesManifest};
    use migration::Migrator;
    use rustok_core::ModuleRegistry;
    use rustok_index::IndexModule;
    use rustok_modules::ModuleOperationStatus;
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
