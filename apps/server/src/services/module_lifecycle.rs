use sea_orm::{DatabaseConnection, DbErr, EntityTrait};
use thiserror::Error;

use rustok_core::ModuleRegistry;
use rustok_modules::{
    failed_module_operation_recovery_plans, module_operation_recovery_plan,
    normalize_module_settings, ModuleControlPlane, ModuleLifecycleDbWriterError,
    ModuleLifecycleExecutionError, ModuleOperationRecoveryError as ModulesRecoveryError,
    ModuleOperationRecoveryPlan, ModuleOperationStoreError, ModuleToggleValidationError,
};

use crate::models::_entities::module_operations::Entity as ModuleOperationsEntity;
use crate::models::_entities::tenant_modules::Entity as TenantModulesEntity;
use crate::models::_entities::{module_operations, tenant_modules};
use crate::modules::{map_module_settings_validation_error, ManifestError, ManifestManager};
use crate::services::platform_composition::PlatformCompositionService;

pub struct ModuleLifecycleService;

#[derive(Debug, Error)]
pub enum ModuleOperationRecoveryError {
    #[error("Module operation not found")]
    OperationNotFound,
    #[error("Module operation idempotency key is invalid")]
    InvalidIdempotencyKey,
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
    #[error("Module operation idempotency key was reused for a different command")]
    IdempotencyConflict,
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
        let manifest = PlatformCompositionService::active_manifest(db)
            .await
            .map_err(|error| ToggleModuleError::Policy(error.to_string()))?;
        let result = ModuleControlPlane::new(db.clone())
            .lifecycle(registry, manifest.settings.default_enabled)
            .toggle(tenant_id, module_slug, enabled, requested_by)
            .await
            .map_err(map_lifecycle_writer_error)?;
        Ok(TenantModulesEntity::find_by_id(result.state.id)
            .one(db)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("tenant_modules.toggle_state".to_string()))?)
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
        idempotency_key: uuid::Uuid,
    ) -> Result<module_operations::Model, ModuleOperationRecoveryError> {
        let manifest = PlatformCompositionService::active_manifest(db)
            .await
            .map_err(|error| ModuleOperationRecoveryError::Policy(error.to_string()))?;
        let retry_operation = ModuleControlPlane::new(db.clone())
            .lifecycle(registry, manifest.settings.default_enabled)
            .retry_post_hook(operation_id, requested_by, idempotency_key)
            .await
            .map_err(map_lifecycle_writer_recovery_error)?;
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
        idempotency_key: uuid::Uuid,
    ) -> Result<tenant_modules::Model, ModuleOperationRecoveryError> {
        let manifest = PlatformCompositionService::active_manifest(db)
            .await
            .map_err(|error| ModuleOperationRecoveryError::Policy(error.to_string()))?;
        let result = ModuleControlPlane::new(db.clone())
            .lifecycle(registry, manifest.settings.default_enabled)
            .compensate_failed_operation(operation_id, requested_by, idempotency_key)
            .await
            .map_err(map_lifecycle_writer_recovery_error)?;
        TenantModulesEntity::find_by_id(result.state.id)
            .one(db)
            .await?
            .ok_or_else(|| {
                ModuleOperationRecoveryError::Database(DbErr::RecordNotFound(
                    "tenant_modules.compensation_state".to_string(),
                ))
            })
    }

    pub async fn update_module_settings(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        module_slug: &str,
        settings: serde_json::Value,
    ) -> Result<tenant_modules::Model, UpdateModuleSettingsError> {
        if !settings.is_object() {
            return Err(UpdateModuleSettingsError::InvalidSettings);
        }

        let manifest = PlatformCompositionService::active_manifest(db)
            .await
            .map_err(|error| UpdateModuleSettingsError::Policy(error.to_string()))?;
        let writer = ModuleControlPlane::new(db.clone())
            .lifecycle(registry, manifest.settings.default_enabled);
        writer
            .require_module_definition(module_slug)
            .map_err(map_lifecycle_writer_settings_error)?;
        let settings_schema = ManifestManager::module_settings_schema(module_slug)?;
        let settings = normalize_module_settings(module_slug, &settings_schema, settings).map_err(
            |error| {
                let message = error.to_string();
                match error {
                    rustok_modules::ModuleSettingsValidationError::InvalidValue { .. } => {
                        UpdateModuleSettingsError::Validation(message)
                    }
                    error => UpdateModuleSettingsError::Manifest(
                        map_module_settings_validation_error(error),
                    ),
                }
            },
        )?;

        let state = writer
            .persist_static_normalized_settings(tenant_id, module_slug, settings)
            .await
            .map_err(map_lifecycle_writer_settings_error)?;
        TenantModulesEntity::find_by_id(state.id)
            .one(db)
            .await?
            .ok_or_else(|| {
                DbErr::RecordNotFound("tenant_modules.settings_state".to_string()).into()
            })
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
        ModuleLifecycleExecutionError::InvalidIdempotencyKey => {
            ToggleModuleError::Policy("module lifecycle idempotency key is invalid".to_string())
        }
        ModuleLifecycleExecutionError::IdempotencyConflict => ToggleModuleError::Policy(
            "module lifecycle idempotency key was reused for a different command".to_string(),
        ),
    }
}

fn map_lifecycle_writer_error(error: ModuleLifecycleDbWriterError) -> ToggleModuleError {
    match error {
        ModuleLifecycleDbWriterError::Lifecycle(error) => map_toggle_execution_error(error),
        ModuleLifecycleDbWriterError::Database(error) => {
            ToggleModuleError::Database(DbErr::Custom(error))
        }
        ModuleLifecycleDbWriterError::Configuration(error) => ToggleModuleError::Policy(error),
        ModuleLifecycleDbWriterError::Definition(error) => {
            ToggleModuleError::Policy(error.to_string())
        }
        ModuleLifecycleDbWriterError::Recovery(error) => {
            ToggleModuleError::Policy(error.to_string())
        }
        ModuleLifecycleDbWriterError::UnknownModule(error) => ToggleModuleError::Policy(error),
        ModuleLifecycleDbWriterError::ArtifactSettings {
            module_slug,
            reason,
        } => ToggleModuleError::Policy(format!("artifact settings for `{module_slug}`: {reason}")),
        ModuleLifecycleDbWriterError::Settings(error) => {
            ToggleModuleError::Policy(error.to_string())
        }
    }
}

fn map_lifecycle_writer_recovery_error(
    error: ModuleLifecycleDbWriterError,
) -> ModuleOperationRecoveryError {
    match error {
        ModuleLifecycleDbWriterError::Recovery(error) => map_module_recovery_error(error),
        ModuleLifecycleDbWriterError::Lifecycle(
            ModuleLifecycleExecutionError::InvalidIdempotencyKey,
        ) => ModuleOperationRecoveryError::InvalidIdempotencyKey,
        ModuleLifecycleDbWriterError::Lifecycle(
            ModuleLifecycleExecutionError::IdempotencyConflict,
        ) => ModuleOperationRecoveryError::IdempotencyConflict,
        ModuleLifecycleDbWriterError::Lifecycle(error) => {
            ModuleOperationRecoveryError::Toggle(map_toggle_execution_error(error))
        }
        ModuleLifecycleDbWriterError::Database(error) => {
            ModuleOperationRecoveryError::Database(DbErr::Custom(error))
        }
        ModuleLifecycleDbWriterError::Configuration(error) => {
            ModuleOperationRecoveryError::Policy(error)
        }
        ModuleLifecycleDbWriterError::Definition(error) => {
            ModuleOperationRecoveryError::Policy(error.to_string())
        }
        ModuleLifecycleDbWriterError::UnknownModule(error) => {
            ModuleOperationRecoveryError::Policy(error)
        }
        ModuleLifecycleDbWriterError::ArtifactSettings {
            module_slug,
            reason,
        } => ModuleOperationRecoveryError::Policy(format!(
            "artifact settings for `{module_slug}`: {reason}"
        )),
        ModuleLifecycleDbWriterError::Settings(error) => {
            ModuleOperationRecoveryError::Database(DbErr::Custom(error.to_string()))
        }
    }
}

fn map_lifecycle_writer_settings_error(
    error: ModuleLifecycleDbWriterError,
) -> UpdateModuleSettingsError {
    match error {
        ModuleLifecycleDbWriterError::UnknownModule(_) => UpdateModuleSettingsError::UnknownModule,
        ModuleLifecycleDbWriterError::ArtifactSettings {
            module_slug,
            reason,
        } => UpdateModuleSettingsError::Validation(format!(
            "artifact settings for `{module_slug}`: {reason}"
        )),
        ModuleLifecycleDbWriterError::Settings(error) => map_module_settings_store_error(error),
        ModuleLifecycleDbWriterError::Database(error) => {
            UpdateModuleSettingsError::Database(DbErr::Custom(error))
        }
        ModuleLifecycleDbWriterError::Configuration(error) => {
            UpdateModuleSettingsError::Policy(error)
        }
        ModuleLifecycleDbWriterError::Definition(error) => {
            UpdateModuleSettingsError::Policy(error.to_string())
        }
        ModuleLifecycleDbWriterError::Lifecycle(error) => {
            UpdateModuleSettingsError::Policy(error.to_string())
        }
        ModuleLifecycleDbWriterError::Recovery(error) => {
            UpdateModuleSettingsError::Policy(error.to_string())
        }
    }
}

fn map_module_recovery_error(error: ModulesRecoveryError) -> ModuleOperationRecoveryError {
    match error {
        ModulesRecoveryError::OperationNotFound => ModuleOperationRecoveryError::OperationNotFound,
        ModulesRecoveryError::InvalidIdempotencyKey => {
            ModuleOperationRecoveryError::InvalidIdempotencyKey
        }
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
        ModulesRecoveryError::IdempotencyConflict => {
            ModuleOperationRecoveryError::IdempotencyConflict
        }
        ModulesRecoveryError::Persistence(error) => {
            ModuleOperationRecoveryError::Database(DbErr::Custom(error))
        }
    }
}

fn map_module_settings_store_error(error: ModuleOperationStoreError) -> UpdateModuleSettingsError {
    match error {
        ModuleOperationStoreError::ModuleNotEnabled(module_slug) => {
            UpdateModuleSettingsError::ModuleNotEnabled(module_slug)
        }
        ModuleOperationStoreError::Database(error) => {
            UpdateModuleSettingsError::Database(DbErr::Custom(error))
        }
        ModuleOperationStoreError::IdempotencyConflict
        | ModuleOperationStoreError::MissingIdempotencyKey => UpdateModuleSettingsError::Policy(
            "unexpected lifecycle idempotency error during settings persistence".to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{ModuleLifecycleService, UpdateModuleSettingsError};
    use crate::models::_entities::tenant_modules;
    use crate::models::tenants;
    use crate::modules::{build_registry, ManifestManager, ManifestModuleSpec, ModulesManifest};
    use rustok_core::ModuleRegistry;
    use rustok_index::IndexModule;
    use rustok_migrations::Migrator;
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
