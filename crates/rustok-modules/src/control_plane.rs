use sea_orm::DatabaseConnection;
use std::collections::HashSet;

use rustok_core::ModuleRegistry;

use crate::{
    ModuleDefinitionCatalog, ModuleDefinitionError, ModuleLifecycleDbWriter,
    SeaOrmArtifactInstallationStore, SeaOrmArtifactSandboxPolicyResolver, SeaOrmModuleBuildService,
    SeaOrmModuleCompositionService, SeaOrmModuleGovernanceService,
};

/// Owner composition root for module control-plane services backed by one
/// database connection. Hosts obtain domain services through this facade rather
/// than constructing storage adapters at arbitrary call sites.
#[derive(Clone)]
pub struct ModuleControlPlane {
    db: DatabaseConnection,
}

/// Owner query service for effective module availability in one host
/// composition. It uses the same durable override source as lifecycle writes.
pub struct EffectivePolicyService<'a> {
    lifecycle: ModuleLifecycleDbWriter<'a>,
}

impl<'a> EffectivePolicyService<'a> {
    pub async fn resolve_enabled(
        &self,
        tenant_id: uuid::Uuid,
    ) -> Result<HashSet<String>, crate::ModuleLifecycleDbWriterError> {
        self.lifecycle.effective_enabled_modules(tenant_id).await
    }
}

impl ModuleControlPlane {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn catalog(
        &self,
        registry: &ModuleRegistry,
    ) -> Result<ModuleDefinitionCatalog, ModuleDefinitionError> {
        ModuleDefinitionCatalog::from_static_registry(registry)
    }

    pub fn composition(&self) -> SeaOrmModuleCompositionService {
        SeaOrmModuleCompositionService::new(self.db.clone())
    }

    pub fn build(&self) -> SeaOrmModuleBuildService {
        SeaOrmModuleBuildService::new(self.db.clone())
    }

    /// Release and publication operations share the one transactional
    /// governance aggregate service.
    pub fn release(&self) -> SeaOrmModuleGovernanceService {
        SeaOrmModuleGovernanceService::new(self.db.clone())
    }

    /// Release and publication operations share the one transactional
    /// governance aggregate service.
    pub fn publication(&self) -> SeaOrmModuleGovernanceService {
        SeaOrmModuleGovernanceService::new(self.db.clone())
    }

    pub fn installation(&self) -> SeaOrmArtifactInstallationStore {
        SeaOrmArtifactInstallationStore::new(self.db.clone())
    }

    pub fn artifact_sandbox_policy(&self) -> SeaOrmArtifactSandboxPolicyResolver {
        SeaOrmArtifactSandboxPolicyResolver::new(self.db.clone())
    }

    pub fn lifecycle<'a>(
        &self,
        registry: &'a ModuleRegistry,
        default_enabled_modules: Vec<String>,
    ) -> ModuleLifecycleDbWriter<'a> {
        ModuleLifecycleDbWriter::new(self.db.clone(), registry, default_enabled_modules)
    }

    pub fn effective_policy<'a>(
        &self,
        registry: &'a ModuleRegistry,
        default_enabled_modules: Vec<String>,
    ) -> EffectivePolicyService<'a> {
        EffectivePolicyService {
            lifecycle: self.lifecycle(registry, default_enabled_modules),
        }
    }
}
