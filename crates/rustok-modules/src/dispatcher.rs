//! Definition-aware dispatch for module runtime bindings.

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use rustok_core::{ModuleContext, ModuleRegistry};

use crate::{
    ArtifactReleaseRef, ModuleDefinitionCatalog, ModuleDefinitionSource, ModuleRuntimeBinding,
    ModuleRuntimeBindingKind,
};

/// The v1 lifecycle binding set. Other binding classes are added to the same
/// envelope rather than becoming new host-specific call paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleLifecycleHookPhase {
    PreEnable,
    PostEnable,
    PreDisable,
    PostDisable,
}

/// Resolves a definition before reaching a static implementation handle or the
/// admitted artifact sandbox adapter.
pub struct ModuleExecutionDispatcher<'a> {
    catalog: &'a ModuleDefinitionCatalog,
    static_registry: &'a ModuleRegistry,
    artifact_executor: Option<&'a dyn ArtifactLifecycleExecutor>,
}

impl<'a> ModuleExecutionDispatcher<'a> {
    pub fn new(catalog: &'a ModuleDefinitionCatalog, static_registry: &'a ModuleRegistry) -> Self {
        Self {
            catalog,
            static_registry,
            artifact_executor: None,
        }
    }

    pub fn with_artifact_executor(mut self, executor: &'a dyn ArtifactLifecycleExecutor) -> Self {
        self.artifact_executor = Some(executor);
        self
    }

    pub fn catalog(&self) -> &ModuleDefinitionCatalog {
        self.catalog
    }

    pub async fn dispatch_lifecycle(
        &self,
        db: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        module_slug: &str,
        config: &serde_json::Value,
        phase: ModuleLifecycleHookPhase,
    ) -> Result<(), ModuleDispatchError> {
        let definition = self
            .catalog
            .get(module_slug)
            .ok_or_else(|| ModuleDispatchError::UnknownDefinition(module_slug.to_string()))?;
        match &definition.source {
            ModuleDefinitionSource::Static { .. } => {
                let module = self.static_registry.get(module_slug).ok_or_else(|| {
                    ModuleDispatchError::MissingStaticImplementation(module_slug.to_string())
                })?;
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
                result.map_err(|error| ModuleDispatchError::StaticHook(error.to_string()))
            }
            ModuleDefinitionSource::Artifact { release } => {
                let kind = lifecycle_binding_kind(phase);
                let binding = definition
                    .bindings
                    .iter()
                    .find(|binding| binding.kind == kind)
                    .ok_or_else(|| {
                        ModuleDispatchError::ArtifactBindingUnavailable(module_slug.to_string())
                    })?;
                let executor = self.artifact_executor.ok_or_else(|| {
                    ModuleDispatchError::ArtifactExecutorUnavailable(module_slug.to_string())
                })?;
                executor
                    .dispatch_lifecycle(ArtifactLifecycleDispatch {
                        release,
                        binding,
                        tenant_id,
                        config,
                        phase,
                    })
                    .await
                    .map_err(ModuleDispatchError::ArtifactHook)
            }
        }
    }
}

/// Narrow adapter owned by the artifact runtime integration. It must resolve an
/// admitted installation and execute it through `SandboxRuntime`; a static
/// callback cannot implement this port.
#[async_trait]
pub trait ArtifactLifecycleExecutor: Send + Sync {
    async fn dispatch_lifecycle(
        &self,
        dispatch: ArtifactLifecycleDispatch<'_>,
    ) -> Result<(), String>;
}

pub struct ArtifactLifecycleDispatch<'a> {
    pub release: &'a ArtifactReleaseRef,
    pub binding: &'a ModuleRuntimeBinding,
    pub tenant_id: uuid::Uuid,
    pub config: &'a serde_json::Value,
    pub phase: ModuleLifecycleHookPhase,
}

fn lifecycle_binding_kind(phase: ModuleLifecycleHookPhase) -> ModuleRuntimeBindingKind {
    match phase {
        ModuleLifecycleHookPhase::PreEnable => ModuleRuntimeBindingKind::PreEnable,
        ModuleLifecycleHookPhase::PostEnable => ModuleRuntimeBindingKind::PostEnable,
        ModuleLifecycleHookPhase::PreDisable => ModuleRuntimeBindingKind::PreDisable,
        ModuleLifecycleHookPhase::PostDisable => ModuleRuntimeBindingKind::PostDisable,
    }
}

#[derive(Debug, Error)]
pub enum ModuleDispatchError {
    #[error("module definition `{0}` is not active")]
    UnknownDefinition(String),
    #[error("static module definition `{0}` has no compiled implementation")]
    MissingStaticImplementation(String),
    #[error("artifact module `{0}` has no admitted lifecycle binding")]
    ArtifactBindingUnavailable(String),
    #[error("artifact module `{0}` has no sandbox lifecycle executor")]
    ArtifactExecutorUnavailable(String),
    #[error("artifact lifecycle binding failed: {0}")]
    ArtifactHook(String),
    #[error("module lifecycle binding failed: {0}")]
    StaticHook(String),
}
