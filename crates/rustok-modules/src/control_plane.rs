use sea_orm::DatabaseConnection;
use std::collections::HashSet;

use rustok_core::ModuleRegistry;

use crate::{
    ArtifactDataExportAuthorizer, ArtifactEventDeliveryConfig, ArtifactEventDeliveryError,
    ArtifactSecretAuthorizer, ArtifactSecretHandleAuthorizer, ModuleDefinitionCatalog,
    ModuleDefinitionError, ModuleLifecycleDbWriter, SeaOrmArtifactBindingIdempotencyStore,
    SeaOrmArtifactDataCapabilityBrokerResolver, SeaOrmArtifactDataExportService,
    SeaOrmArtifactDataObjectCapabilityBrokerResolver, SeaOrmArtifactEventSubscriptionProjector,
    SeaOrmArtifactExecutionObserver, SeaOrmArtifactInstallationStore,
    SeaOrmArtifactSandboxPolicyResolver, SeaOrmArtifactSecretCapabilityBrokerResolver,
    SeaOrmArtifactSecretService, SeaOrmModuleBuildService, SeaOrmModuleCompositionService,
    SeaOrmModuleGovernanceService,
};
use rustok_storage::StorageService;

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

    /// Returns the owner-scoped structured-data capability resolver for exact
    /// admitted artifact executions.
    pub fn artifact_data_capability(&self) -> SeaOrmArtifactDataCapabilityBrokerResolver {
        SeaOrmArtifactDataCapabilityBrokerResolver::new(self.db.clone())
    }

    /// Returns the owner-scoped private-object capability resolver. Storage is
    /// deployment infrastructure; the returned resolver never exposes it to a
    /// sandbox guest.
    pub fn artifact_data_object_capability(
        &self,
        storage: StorageService,
    ) -> SeaOrmArtifactDataObjectCapabilityBrokerResolver {
        SeaOrmArtifactDataObjectCapabilityBrokerResolver::new(self.db.clone(), storage)
    }

    /// Returns the owner persistence adapter for redacted artifact execution
    /// audit evidence.
    pub fn artifact_execution_audit(&self) -> SeaOrmArtifactExecutionObserver {
        SeaOrmArtifactExecutionObserver::new(self.db.clone())
    }

    /// Returns the owner projector that turns a durable platform event into
    /// exact admitted artifact event deliveries.
    pub fn artifact_event_projector(
        &self,
        config: ArtifactEventDeliveryConfig,
    ) -> Result<SeaOrmArtifactEventSubscriptionProjector, ArtifactEventDeliveryError> {
        SeaOrmArtifactEventSubscriptionProjector::new(self.db.clone(), config)
    }

    /// Returns the owner store for HTTP/command binding idempotency leases and
    /// replay responses.
    pub fn artifact_binding_idempotency(&self) -> SeaOrmArtifactBindingIdempotencyStore {
        SeaOrmArtifactBindingIdempotencyStore::new(self.db.clone())
    }

    /// Returns the owner-only, audited structured-data export service. Its
    /// host-supplied authorizer remains responsible for operator, retention,
    /// and legal-hold policy before the owner transaction starts.
    pub fn artifact_data_export<A>(&self, authorizer: A) -> SeaOrmArtifactDataExportService<A>
    where
        A: ArtifactDataExportAuthorizer,
    {
        SeaOrmArtifactDataExportService::new(self.db.clone(), authorizer)
    }

    /// Returns the owner-only logical secret-binding service. The supplied
    /// authorizer is responsible for resolver-registry, lifecycle, and RBAC
    /// policy; the service persists only redacted resolver references.
    pub fn artifact_secret_bindings<A>(&self, authorizer: A) -> SeaOrmArtifactSecretService<A>
    where
        A: ArtifactSecretAuthorizer,
    {
        SeaOrmArtifactSecretService::new(self.db.clone(), authorizer)
    }

    /// Returns the dynamic `platform.secrets` resolver for exact admitted
    /// artifact executions. It can return only logical handles and never a
    /// resolver key or secret value to the sandbox.
    pub fn artifact_secret_capability<A>(
        &self,
        authorizer: A,
    ) -> SeaOrmArtifactSecretCapabilityBrokerResolver<A>
    where
        A: ArtifactSecretHandleAuthorizer + Clone,
    {
        SeaOrmArtifactSecretCapabilityBrokerResolver::new(self.db.clone(), authorizer)
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
