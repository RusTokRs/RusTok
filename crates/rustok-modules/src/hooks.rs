use sea_orm::DatabaseConnection;
use thiserror::Error;

use rustok_core::{ModuleContext, ModuleRegistry};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleLifecycleHookPhase {
    PreEnable,
    PostEnable,
    PreDisable,
    PostDisable,
}

#[derive(Debug, Error)]
pub enum ModuleLifecycleHookError {
    #[error("unknown module `{0}`")]
    UnknownModule(String),
    #[error("module lifecycle hook failed: {0}")]
    Hook(String),
}

pub async fn run_module_lifecycle_hook(
    registry: &ModuleRegistry,
    db: &DatabaseConnection,
    tenant_id: uuid::Uuid,
    module_slug: &str,
    config: &serde_json::Value,
    phase: ModuleLifecycleHookPhase,
) -> Result<(), ModuleLifecycleHookError> {
    let module = registry
        .get(module_slug)
        .ok_or_else(|| ModuleLifecycleHookError::UnknownModule(module_slug.to_string()))?;
    let context = ModuleContext {
        db,
        tenant_id,
        config,
    };
    let result = match phase {
        ModuleLifecycleHookPhase::PreEnable => module.pre_enable(context).await,
        ModuleLifecycleHookPhase::PostEnable => module.post_enable(context).await,
        ModuleLifecycleHookPhase::PreDisable => module.pre_disable(context).await,
        ModuleLifecycleHookPhase::PostDisable => module.post_disable(context).await,
    };
    result.map_err(|error| ModuleLifecycleHookError::Hook(error.to_string()))
}
