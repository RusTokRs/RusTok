use sea_orm::DatabaseConnection;

use rustok_core::ModuleRegistry;
use rustok_modules::{
    ModuleControlPlane, ModuleLifecycleDbWriterError, ModuleLifecycleSettingsCommand,
    normalize_module_settings,
};

use crate::modules::{ManifestManager, map_module_settings_validation_error};
use crate::services::module_lifecycle::{
    ModuleLifecycleStateSnapshot, UpdateModuleSettingsError, map_lifecycle_writer_settings_error,
};
use crate::services::platform_composition::PlatformCompositionService;

/// Server-owned boundary for applying one previously reviewed module-rollout
/// settings transition without overwriting a concurrent tenant settings or
/// enablement change. Authorization and promotion-review evidence stay with the
/// caller; this service only normalizes the reviewed snapshots and performs the
/// owner CAS.
pub struct ModuleRolloutPromotionSettingsService;

#[derive(Clone, Debug)]
pub enum ModuleRolloutPromotionSettingsOutcome {
    Updated(ModuleLifecycleStateSnapshot),
    Conflict,
}

impl ModuleRolloutPromotionSettingsService {
    pub async fn update_if_current(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        actor_id: uuid::Uuid,
        idempotency_key: uuid::Uuid,
        expected_revision: u64,
        module_slug: &str,
        expected_enabled: bool,
        expected_settings: serde_json::Value,
        settings: serde_json::Value,
    ) -> Result<ModuleRolloutPromotionSettingsOutcome, UpdateModuleSettingsError> {
        if !expected_settings.is_object() || !settings.is_object() {
            return Err(UpdateModuleSettingsError::InvalidSettings);
        }

        let manifest = PlatformCompositionService::active_manifest(db)
            .await
            .map_err(|error| UpdateModuleSettingsError::Policy(error.to_string()))?;
        let co_requisites = ManifestManager::module_policy_corequisites(&manifest)?;
        let writer = ModuleControlPlane::new(db.clone())
            .lifecycle(registry, manifest.settings.default_enabled)
            .with_corequisites(co_requisites);
        writer
            .require_module_definition(module_slug)
            .map_err(map_lifecycle_writer_settings_error)?;

        let settings_schema = ManifestManager::module_settings_schema(module_slug)?;
        let normalize = |value: serde_json::Value| {
            normalize_module_settings(module_slug, &settings_schema, value).map_err(|error| {
                let message = error.to_string();
                match error {
                    rustok_modules::ModuleSettingsValidationError::InvalidValue { .. } => {
                        UpdateModuleSettingsError::Validation(message)
                    }
                    error => UpdateModuleSettingsError::Manifest(
                        map_module_settings_validation_error(error),
                    ),
                }
            })
        };
        let expected_settings = normalize(expected_settings)?;
        let settings = normalize(settings)?;

        let state = writer
            .update_static_normalized_settings(ModuleLifecycleSettingsCommand {
                tenant_id,
                module_slug: module_slug.to_string(),
                settings,
                actor_id,
                idempotency_key,
                expected_revision,
                expected_enabled: Some(expected_enabled),
                expected_settings: Some(expected_settings),
            })
            .await;

        match state {
            Ok(state) => Ok(ModuleRolloutPromotionSettingsOutcome::Updated(
                ModuleLifecycleStateSnapshot {
                    module_slug: state.module_slug,
                    enabled: state.enabled,
                    settings: state.settings,
                    operation_id: None,
                    revision: state.revision,
                },
            )),
            Err(ModuleLifecycleDbWriterError::SettingsSnapshotConflict) => {
                Ok(ModuleRolloutPromotionSettingsOutcome::Conflict)
            }
            Err(error) => match map_lifecycle_writer_settings_error(error) {
                UpdateModuleSettingsError::SettingsSnapshotConflict => {
                    Ok(ModuleRolloutPromotionSettingsOutcome::Conflict)
                }
                error => Err(error),
            },
        }
    }
}
