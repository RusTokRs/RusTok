use sea_orm::DatabaseConnection;
use std::collections::HashSet;

use rustok_core::ModuleRegistry;
use rustok_secrets::SecretResolverRegistry;

use crate::{
    ArtifactDataExportAuthorizer, ArtifactDataPurgeAuthorizer, ArtifactDataSnapshotAuthorizer,
    ArtifactDataSnapshotCollectionAuthorizer, ArtifactDataSnapshotRetentionAuthorizer,
    ArtifactEventDeliveryConfig, ArtifactEventDeliveryError, ArtifactLifecycleExecutor,
    ArtifactScheduleDeliveryConfig, ArtifactScheduleDeliveryError, ArtifactSecretAuthorizer,
    ArtifactSecretHandleAuthorizer, ArtifactSecretUseAuthorizer, ArtifactSecretValueConsumer,
    ControlPlaneInfrastructure, ModuleDefinitionCatalog, ModuleDefinitionError,
    ModuleLifecycleDbWriter, ModuleStaticDistributionAuthorizer,
    ModuleStaticDistributionReleaseAuthorizer, ModuleStaticDistributionReleaseVerifier,
    ModuleStaticDistributionWorkerAuthorizer, ModuleStaticPromotionAuthorizer,
    SeaOrmArtifactBindingIdempotencyStore, SeaOrmArtifactDataCapabilityBrokerResolver,
    SeaOrmArtifactDataExportService, SeaOrmArtifactDataObjectCapabilityBrokerResolver,
    SeaOrmArtifactDataObjectGcService, SeaOrmArtifactDataPurgeService,
    SeaOrmArtifactDataSnapshotCollectionService, SeaOrmArtifactDataSnapshotRetentionService,
    SeaOrmArtifactDataSnapshotService, SeaOrmArtifactEventSubscriptionProjector,
    SeaOrmArtifactExecutionObserver, SeaOrmArtifactInstallationStore,
    SeaOrmArtifactSandboxPolicyResolver, SeaOrmArtifactScheduleDeliveryQueue,
    SeaOrmArtifactSecretCapabilityBrokerResolver, SeaOrmArtifactSecretService,
    SeaOrmArtifactSecretUseService, SeaOrmModuleBuildService, SeaOrmModuleCompositionService,
    SeaOrmModuleGovernanceService, SeaOrmModulePromotionService,
    SeaOrmModuleStaticDistributionReleaseService, SeaOrmModuleStaticDistributionService,
    SeaOrmModuleStaticDistributionWorkerService, StorageArtifactBlobStore,
};
use rustok_storage::StorageRuntime;

/// Owner composition root for module control-plane services backed by one
/// database connection. Hosts obtain domain services through this facade rather
/// than constructing storage adapters at arbitrary call sites.
#[derive(Clone)]
pub struct ModuleControlPlane {
    db: DatabaseConnection,
    infrastructure: ControlPlaneInfrastructure,
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
        let infrastructure = ControlPlaneInfrastructure::for_database(db.clone());
        Self { db, infrastructure }
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self { db, infrastructure }
    }

    pub fn infrastructure(&self) -> ControlPlaneInfrastructure {
        self.infrastructure.clone()
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
        SeaOrmModuleBuildService::with_infrastructure(self.db.clone(), self.infrastructure.clone())
    }

    /// Release and publication operations share the one transactional
    /// governance aggregate service.
    pub fn release(&self) -> SeaOrmModuleGovernanceService {
        SeaOrmModuleGovernanceService::with_infrastructure(
            self.db.clone(),
            self.infrastructure.clone(),
        )
    }

    /// Release and publication operations share the one transactional
    /// governance aggregate service.
    pub fn publication(&self) -> SeaOrmModuleGovernanceService {
        SeaOrmModuleGovernanceService::with_infrastructure(
            self.db.clone(),
            self.infrastructure.clone(),
        )
    }

    /// Returns the platform-scoped owner service for reviewed static promotion.
    /// It records evidence for distribution tooling and cannot compile or load
    /// native code in the running server.
    pub fn promotion<A>(&self, authorizer: A) -> SeaOrmModulePromotionService<A>
    where
        A: ModuleStaticPromotionAuthorizer,
    {
        SeaOrmModulePromotionService::with_infrastructure(
            self.db.clone(),
            authorizer,
            self.infrastructure.clone(),
        )
    }

    /// Returns the platform-scoped owner for immutable reviewed native-module
    /// composition snapshots. Every accepted change queues a new distribution
    /// build intent and cannot mutate the running server composition.
    pub fn static_distribution<A>(&self, authorizer: A) -> SeaOrmModuleStaticDistributionService<A>
    where
        A: ModuleStaticDistributionAuthorizer,
    {
        SeaOrmModuleStaticDistributionService::with_infrastructure(
            self.db.clone(),
            authorizer,
            self.infrastructure.clone(),
        )
    }

    /// Returns the separately authorized worker boundary for claiming,
    /// heartbeating, and completing immutable static distribution build intents.
    pub fn static_distribution_worker<A>(
        &self,
        authorizer: A,
    ) -> SeaOrmModuleStaticDistributionWorkerService<A>
    where
        A: ModuleStaticDistributionWorkerAuthorizer,
    {
        SeaOrmModuleStaticDistributionWorkerService::with_infrastructure(
            self.db.clone(),
            authorizer,
            self.infrastructure.clone(),
        )
    }

    /// Returns the separately authorized release owner for admitting one
    /// successfully completed immutable distribution build. Activation records
    /// a verified release head and never mutates the running server composition.
    pub fn static_distribution_release<A, V>(
        &self,
        authorizer: A,
        verifier: V,
    ) -> SeaOrmModuleStaticDistributionReleaseService<A, V>
    where
        A: ModuleStaticDistributionReleaseAuthorizer,
        V: ModuleStaticDistributionReleaseVerifier,
    {
        SeaOrmModuleStaticDistributionReleaseService::with_infrastructure(
            self.db.clone(),
            authorizer,
            verifier,
            self.infrastructure.clone(),
        )
    }

    pub fn installation(&self) -> SeaOrmArtifactInstallationStore {
        SeaOrmArtifactInstallationStore::with_infrastructure(
            self.db.clone(),
            self.infrastructure.clone(),
        )
    }

    pub fn artifact_sandbox_policy(&self) -> SeaOrmArtifactSandboxPolicyResolver {
        SeaOrmArtifactSandboxPolicyResolver::new(self.db.clone())
    }

    /// Returns the platform-storage-backed admitted artifact CAS using this
    /// facade's identity source for private staging objects.
    pub fn artifact_blob_store(&self, storage: StorageRuntime) -> StorageArtifactBlobStore {
        StorageArtifactBlobStore::with_infrastructure(storage, self.infrastructure.clone())
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
        storage: StorageRuntime,
    ) -> SeaOrmArtifactDataObjectCapabilityBrokerResolver {
        SeaOrmArtifactDataObjectCapabilityBrokerResolver::with_infrastructure(
            self.db.clone(),
            storage,
            self.infrastructure.clone(),
        )
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
        SeaOrmArtifactEventSubscriptionProjector::with_infrastructure(
            self.db.clone(),
            config,
            self.infrastructure.clone(),
        )
    }

    /// Returns the owner store for HTTP/command binding idempotency leases and
    /// replay responses.
    pub fn artifact_binding_idempotency(&self) -> SeaOrmArtifactBindingIdempotencyStore {
        SeaOrmArtifactBindingIdempotencyStore::with_infrastructure(
            self.db.clone(),
            self.infrastructure.clone(),
        )
    }

    /// Returns the owner queue for immutable artifact Schedule delivery slots.
    pub fn artifact_schedule_delivery(
        &self,
        config: ArtifactScheduleDeliveryConfig,
    ) -> Result<SeaOrmArtifactScheduleDeliveryQueue, ArtifactScheduleDeliveryError> {
        SeaOrmArtifactScheduleDeliveryQueue::with_infrastructure(
            self.db.clone(),
            config,
            self.infrastructure.clone(),
        )
    }

    /// Returns the owner-only, audited structured-data export service. Its
    /// host-supplied authorizer remains responsible for operator, retention,
    /// and legal-hold policy before the owner transaction starts.
    pub fn artifact_data_export<A>(&self, authorizer: A) -> SeaOrmArtifactDataExportService<A>
    where
        A: ArtifactDataExportAuthorizer,
    {
        SeaOrmArtifactDataExportService::with_infrastructure(
            self.db.clone(),
            authorizer,
            self.infrastructure.clone(),
        )
    }

    /// Returns the retention-aware owner service for unreachable private data
    /// object bytes. The retention decision remains an explicit operation input.
    pub fn artifact_data_object_gc(
        &self,
        storage: StorageRuntime,
    ) -> SeaOrmArtifactDataObjectGcService {
        SeaOrmArtifactDataObjectGcService::new(self.db.clone(), storage)
    }

    /// Returns the owner-only irreversible namespace purge service.
    pub fn artifact_data_purge<A>(&self, authorizer: A) -> SeaOrmArtifactDataPurgeService<A>
    where
        A: ArtifactDataPurgeAuthorizer,
    {
        SeaOrmArtifactDataPurgeService::with_infrastructure(
            self.db.clone(),
            authorizer,
            self.infrastructure.clone(),
        )
    }

    /// Returns the owner-only durable namespace snapshot/restore service.
    /// Snapshot object bytes remain private platform storage and restore cannot
    /// clear a purge tombstone or replace a non-empty namespace.
    pub fn artifact_data_snapshot<A>(
        &self,
        storage: StorageRuntime,
        authorizer: A,
    ) -> SeaOrmArtifactDataSnapshotService<A>
    where
        A: ArtifactDataSnapshotAuthorizer,
    {
        SeaOrmArtifactDataSnapshotService::with_infrastructure(
            self.db.clone(),
            storage,
            authorizer,
            self.infrastructure.clone(),
        )
    }

    /// Returns the CAS-guarded owner service for extending snapshot retention
    /// or applying/releasing legal hold. Retention can never be shortened.
    pub fn artifact_data_snapshot_retention<A>(
        &self,
        authorizer: A,
    ) -> SeaOrmArtifactDataSnapshotRetentionService<A>
    where
        A: ArtifactDataSnapshotRetentionAuthorizer,
    {
        SeaOrmArtifactDataSnapshotRetentionService::with_infrastructure(
            self.db.clone(),
            authorizer,
            self.infrastructure.clone(),
        )
    }

    /// Returns the bounded snapshot collector. Each pass still requires an
    /// explicit fail-closed policy snapshot before ready data can enter the
    /// durable collecting state.
    pub fn artifact_data_snapshot_collection<A>(
        &self,
        storage: StorageRuntime,
        authorizer: A,
    ) -> SeaOrmArtifactDataSnapshotCollectionService<A>
    where
        A: ArtifactDataSnapshotCollectionAuthorizer,
    {
        SeaOrmArtifactDataSnapshotCollectionService::with_infrastructure(
            self.db.clone(),
            storage,
            authorizer,
            self.infrastructure.clone(),
        )
    }

    /// Returns the owner-only logical secret-binding service. The supplied
    /// authorizer is responsible for resolver-registry, lifecycle, and RBAC
    /// policy; the service persists only redacted resolver references.
    pub fn artifact_secret_bindings<A>(&self, authorizer: A) -> SeaOrmArtifactSecretService<A>
    where
        A: ArtifactSecretAuthorizer,
    {
        SeaOrmArtifactSecretService::with_infrastructure(
            self.db.clone(),
            authorizer,
            self.infrastructure.clone(),
        )
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

    /// Composes a host-only secret value-use boundary. The selected consumer is
    /// fixed here and receives a `SecretString` borrow; callers receive only the
    /// redacted owner receipt.
    pub fn artifact_secret_use<A, C>(
        &self,
        resolvers: SecretResolverRegistry,
        authorizer: A,
        consumer: C,
    ) -> SeaOrmArtifactSecretUseService<A, C>
    where
        A: ArtifactSecretUseAuthorizer,
        C: ArtifactSecretValueConsumer,
    {
        SeaOrmArtifactSecretUseService::new(self.db.clone(), resolvers, authorizer, consumer)
    }

    pub fn lifecycle<'a>(
        &self,
        registry: &'a ModuleRegistry,
        default_enabled_modules: Vec<String>,
    ) -> ModuleLifecycleDbWriter<'a> {
        ModuleLifecycleDbWriter::with_infrastructure(
            self.db.clone(),
            registry,
            default_enabled_modules,
            self.infrastructure.clone(),
        )
    }

    /// Returns the artifact-only lifecycle/settings owner for a resolved
    /// immutable definition catalog. Dynamic settings therefore use the same
    /// facade infrastructure as lifecycle binding dispatch.
    pub fn artifact_lifecycle<'a>(
        &self,
        catalog: ModuleDefinitionCatalog,
        artifact_executor: &'a dyn ArtifactLifecycleExecutor,
        default_enabled_modules: Vec<String>,
    ) -> ModuleLifecycleDbWriter<'a> {
        ModuleLifecycleDbWriter::artifact_only_with_infrastructure(
            self.db.clone(),
            catalog,
            artifact_executor,
            default_enabled_modules,
            self.infrastructure.clone(),
        )
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
