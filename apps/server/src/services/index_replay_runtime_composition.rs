#[path = "index_reconciliation_operator.rs"]
mod reconciliation_operator;
#[path = "index_drift_diagnosis_operator.rs"]
mod drift_diagnosis_operator;
#[path = "index_source_continuation_runtime.rs"]
mod source_continuation_runtime;
#[path = "index_drift_source_page_diagnosis.rs"]
mod drift_source_page_diagnosis;
#[path = "index_replay_shadow_transport.rs"]
mod replay_shadow_transport;

pub use drift_diagnosis_operator::{
    IndexDriftDiagnosisOperatorError, IndexDriftDiagnosisOperatorRuntime,
};
pub use drift_source_page_diagnosis::{
    IndexDriftSourcePageDiagnosisError, IndexDriftSourcePageDiagnosisOutcome,
    IndexDriftSourcePageDiagnosisRuntime, IndexDriftSourcePageDiagnosisSealedOutcome,
};
pub use reconciliation_operator::{
    IndexReconciliationOperatorContext, IndexReconciliationOperatorError,
    IndexReconciliationOperatorRuntime,
};
pub use replay_shadow_transport::{
    IndexReplayShadowTransportError, IndexReplayShadowTransportOutcome,
    IndexReplayShadowTransportRuntime,
};

use std::fmt;

use rustok_api::{Permission, has_effective_permission};
use rustok_core::ModuleRuntimeExtensions;
use sea_orm::DatabaseConnection;
use thiserror::Error;
use uuid::Uuid;

use crate::error::{Error as ServerError, Result};
use crate::services::rbac_request_scope::permissions_for;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexReplayOperatorContext {
    tenant_id: Uuid,
    actor_id: Uuid,
}

impl IndexReplayOperatorContext {
    pub fn new(tenant_id: Uuid, actor_id: Uuid) -> Result<Self, IndexReplayOperatorError> {
        if tenant_id.is_nil() || actor_id.is_nil() {
            return Err(IndexReplayOperatorError::InvalidContext);
        }
        Ok(Self {
            tenant_id,
            actor_id,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn actor_id(&self) -> Uuid {
        self.actor_id
    }

    fn authorize_for(&self, requested_tenant: Uuid) -> Result<(), IndexReplayOperatorError> {
        if requested_tenant != self.tenant_id {
            return Err(IndexReplayOperatorError::TenantMismatch);
        }
        let permissions = permissions_for(&self.tenant_id, &self.actor_id)
            .ok_or(IndexReplayOperatorError::MissingRequestAuthority)?;
        if !has_effective_permission(&permissions, &Permission::MODULES_MANAGE) {
            return Err(IndexReplayOperatorError::Forbidden);
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum IndexReplayOperatorError {
    #[error("Index replay operator tenant and actor must not be nil")]
    InvalidContext,
    #[error("Index replay request tenant does not match the authorized operator tenant")]
    TenantMismatch,
    #[error("Index replay operations require a request-bound effective permission snapshot")]
    MissingRequestAuthority,
    #[error("modules:manage is required for Index replay operations")]
    Forbidden,
    #[error(transparent)]
    Replay(#[from] rustok_index::IndexReplayRunError),
}

#[derive(Debug, Error)]
pub enum IndexReplayShadowOperatorError {
    #[error(transparent)]
    Authorization(#[from] IndexReplayOperatorError),
    #[error(transparent)]
    DryRun(#[from] rustok_index::IndexReplayDryRunError),
}

/// Server-owned guarded operator boundary over the canonical Index replay runtimes.
///
/// Transport adapters must provide an exact request-bound tenant/actor context. The boundary
/// accepts only `modules:manage`, rejects cross-tenant requests before execution, and exposes no
/// connection, source registry, scheduler, or worker-spawn handle. Durable full replay and
/// side-effect-free shadow replay remain separate execution surfaces behind the same guard.
#[derive(Clone)]
pub struct IndexReplayOperatorRuntime {
    inner: rustok_index::SharedIndexReplayRuntime,
    shadow: rustok_index::SharedIndexReplayDryRunRuntime,
}

impl IndexReplayOperatorRuntime {
    fn new(
        inner: rustok_index::SharedIndexReplayRuntime,
        shadow: rustok_index::SharedIndexReplayDryRunRuntime,
    ) -> Self {
        Self { inner, shadow }
    }

    pub async fn run(
        &self,
        context: IndexReplayOperatorContext,
        request: rustok_index::IndexReplayRunRequest,
    ) -> Result<rustok_index::IndexReplayRunOutcome, IndexReplayOperatorError> {
        context.authorize_for(request.page_request().tenant_id())?;
        self.inner.run(request).await.map_err(Into::into)
    }

    /// Runs the existing side-effect-free replay dry-run capability through the same exact
    /// request-bound authorization boundary as durable full replay.
    pub async fn run_shadow(
        &self,
        context: IndexReplayOperatorContext,
        request: rustok_index::IndexReplayDryRunRequest,
    ) -> Result<rustok_index::IndexReplayDryRunOutcome, IndexReplayShadowOperatorError> {
        context.authorize_for(request.tenant_id())?;
        self.shadow.run(request).await.map_err(Into::into)
    }

    /// Runs replay through the same authorization boundary while sampling one host-owned
    /// cooperative interruption probe at the Index runner's durable safe points.
    pub async fn run_interruptible<Check>(
        &self,
        context: IndexReplayOperatorContext,
        request: rustok_index::IndexReplayRunRequest,
        should_interrupt: Check,
    ) -> Result<rustok_index::IndexReplayRunOutcome, IndexReplayOperatorError>
    where
        Check: FnMut() -> bool,
    {
        context.authorize_for(request.page_request().tenant_id())?;
        self.inner
            .run_interruptible(request, should_interrupt)
            .await
            .map_err(Into::into)
    }

    pub async fn request_cancel(
        &self,
        context: IndexReplayOperatorContext,
        job_id: Uuid,
    ) -> Result<rustok_index::IndexReplayCancelOutcome, IndexReplayOperatorError> {
        context.authorize_for(context.tenant_id())?;
        self.inner
            .request_cancel(context.tenant_id(), job_id)
            .await
            .map_err(Into::into)
    }
}

impl fmt::Debug for IndexReplayOperatorRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexReplayOperatorRuntime")
            .finish_non_exhaustive()
    }
}

/// Materializes the host-owned Index replay capability after all modules have registered sources.
///
/// This function performs no database I/O and starts no worker. It invokes selected source
/// factories only to construct adapters, freezes the complete source catalog, binds the immutable
/// schema/source registries to the host database, and publishes the guarded bounded full/shadow
/// replay, reconciliation, exact-entity drift diagnosis, one-page source-candidate diagnosis, and
/// sealed schema-wide Shadow transport capabilities through `ModuleRuntimeExtensions`.
pub(crate) fn materialize_index_replay_runtime(
    extensions: &mut ModuleRuntimeExtensions,
    db: DatabaseConnection,
) -> Result<()> {
    if extensions.contains::<rustok_index::SharedIndexSourceRegistry>()
        || extensions.contains::<IndexReplayOperatorRuntime>()
    {
        return Err(ServerError::Message(
            "shared Index replay runtime is already materialized".to_string(),
        ));
    }

    rustok_index::materialize_postgres_index_sources(extensions, db.clone()).map_err(|error| {
        ServerError::Message(format!(
            "PostgreSQL Index source factory materialization failed: {error}"
        ))
    })?;
    let sources = rustok_index::materialize_index_source_registry(extensions).map_err(|error| {
        ServerError::Message(format!(
            "Index replay source registry materialization failed: {error}"
        ))
    })?;
    if let Some(sources) = sources {
        extensions.insert(sources);
    }

    let runtime = rustok_index::materialize_postgres_index_replay_runtime(extensions, db.clone())
        .map_err(|error| {
            ServerError::Message(format!("Index replay runtime composition failed: {error}"))
        })?;
    if let Some(runtime) = runtime {
        let shadow = extensions
            .get::<rustok_index::SharedIndexReplayDryRunRuntime>()
            .cloned()
            .ok_or_else(|| {
                ServerError::Message(
                    "Index replay shadow runtime is missing after replay composition".to_string(),
                )
            })?;
        extensions.insert(IndexReplayOperatorRuntime::new(runtime, shadow));
    }
    reconciliation_operator::materialize_index_reconciliation_operator(extensions, db.clone())?;
    drift_diagnosis_operator::materialize_index_drift_diagnosis_operator(extensions, db)?;
    let continuation = if extensions.contains::<rustok_index::SharedIndexSourceRegistry>() {
        source_continuation_runtime::materialize_index_source_continuation_keyring().map_err(
            |_| {
                ServerError::Message(
                    "Index source continuation deployment keyring composition failed".to_string(),
                )
            },
        )?
    } else {
        None
    };
    replay_shadow_transport::materialize_index_replay_shadow_transport(
        extensions,
        continuation.clone(),
    )?;
    drift_source_page_diagnosis::materialize_index_drift_source_page_diagnosis(
        extensions,
        continuation,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use rustok_api::Permission;
    use rustok_core::{
        MigrationSource, ModuleRegistry, ModuleRuntimeExtensions, RusToKModule, UserRole,
    };
    use rustok_index::{
        EntityName, FieldCardinality, FieldName, IndexField, IndexModule,
        IndexReplayDryRunRequest, IndexReplayDryRunStatus, IndexSchema, IndexSource,
        IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage,
        IndexSourceScanRequest, IndexValueType, LocaleMode, ModuleName, SchemaRef, SchemaVersion,
        SharedIndexReplayDryRunRuntime, SharedIndexReplayRuntime, SharedIndexSchemaRegistry,
        SharedIndexSourceRegistry, register_index_schema_source, register_index_source,
    };
    use sea_orm::Database;
    use sea_orm_migration::MigrationTrait;
    use uuid::Uuid;

    use super::{
        IndexDriftDiagnosisOperatorRuntime, IndexDriftSourcePageDiagnosisRuntime,
        IndexReplayOperatorContext, IndexReplayOperatorError, IndexReplayOperatorRuntime,
        IndexReplayShadowOperatorError, IndexReplayShadowTransportRuntime,
        materialize_index_replay_runtime,
    };
    use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};

    struct DemoReplayModule;
    struct NoopSource;

    #[async_trait]
    impl IndexSource for NoopSource {
        async fn scan(
            &self,
            request: IndexSourceScanRequest,
        ) -> Result<IndexSourcePage, IndexSourceFailure> {
            Ok(IndexSourcePage::new(&request, Vec::new(), None)
                .expect("empty final replay page should be valid"))
        }

        async fn load(
            &self,
            request: IndexSourceLoadRequest,
        ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
            Ok(IndexSourceLoadBatch::new(&request, Vec::new())
                .expect("empty targeted load should be valid"))
        }
    }

    impl MigrationSource for DemoReplayModule {
        fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
            Vec::new()
        }
    }

    #[async_trait]
    impl RusToKModule for DemoReplayModule {
        fn slug(&self) -> &'static str {
            "demo_replay"
        }

        fn name(&self) -> &'static str {
            "Demo replay"
        }

        fn description(&self) -> &'static str {
            "Test source-owned Index replay publisher"
        }

        fn version(&self) -> &'static str {
            "0.1.0"
        }

        fn register_runtime_extensions(
            &self,
            extensions: &mut ModuleRuntimeExtensions,
        ) -> rustok_core::Result<()> {
            let schema = demo_schema();
            register_index_schema_source(extensions, self.slug(), schema.clone()).map_err(
                |error| {
                    rustok_core::Error::Validation(format!(
                        "demo replay schema registration failed: {error}"
                    ))
                },
            )?;
            register_index_source(
                extensions,
                self.slug(),
                "demo-replay-primary",
                [schema.reference],
                NoopSource,
            )
            .map_err(|error| {
                rustok_core::Error::Validation(format!(
                    "demo replay source registration failed: {error}"
                ))
            })
        }
    }

    fn demo_schema() -> IndexSchema {
        IndexSchema {
            reference: SchemaRef {
                module: ModuleName::new("demo-replay").unwrap(),
                entity: EntityName::new("item").unwrap(),
                version: SchemaVersion::INITIAL,
            },
            locale_mode: LocaleMode::None,
            fields: vec![IndexField {
                name: FieldName::new("id").unwrap(),
                value_type: IndexValueType::Uuid,
                cardinality: FieldCardinality::One,
                nullable: false,
                selectable: true,
                filterable: true,
                sortable: true,
            }],
            links: Vec::new(),
        }
    }

    #[tokio::test]
    async fn missing_replay_sources_do_not_publish_false_host_runtime() {
        let registry = ModuleRegistry::new().register(IndexModule);
        let mut extensions = rustok_distribution::build_runtime_extensions(&registry)
            .expect("empty Index composition should build");
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");

        materialize_index_replay_runtime(&mut extensions, db)
            .expect("missing sources should remain optional");
        assert!(!extensions.contains::<SharedIndexSourceRegistry>());
        assert!(!extensions.contains::<SharedIndexReplayRuntime>());
        assert!(!extensions.contains::<SharedIndexReplayDryRunRuntime>());
        assert!(!extensions.contains::<IndexReplayOperatorRuntime>());
        assert!(!extensions.contains::<IndexReplayShadowTransportRuntime>());
        assert!(!extensions.contains::<IndexDriftDiagnosisOperatorRuntime>());
        assert!(!extensions.contains::<IndexDriftSourcePageDiagnosisRuntime>());
    }

    #[tokio::test]
    async fn complete_source_catalog_publishes_guarded_runtime_to_host_context() {
        let registry = ModuleRegistry::new()
            .register(IndexModule)
            .register(DemoReplayModule);
        let mut extensions = rustok_distribution::build_runtime_extensions(&registry)
            .expect("source schema composition should build");
        assert!(extensions.contains::<SharedIndexSchemaRegistry>());
        assert!(!extensions.contains::<SharedIndexSourceRegistry>());
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");

        materialize_index_replay_runtime(&mut extensions, db.clone())
            .expect("complete replay runtime should compose");
        assert!(extensions.contains::<SharedIndexSourceRegistry>());
        assert!(extensions.contains::<SharedIndexReplayRuntime>());
        assert!(extensions.contains::<SharedIndexReplayDryRunRuntime>());
        assert!(extensions.contains::<IndexReplayOperatorRuntime>());
        assert!(extensions.contains::<IndexReplayShadowTransportRuntime>());
        assert!(extensions.contains::<IndexDriftDiagnosisOperatorRuntime>());
        assert!(extensions.contains::<IndexDriftSourcePageDiagnosisRuntime>());

        let host = extensions.apply_to_host_runtime(rustok_api::HostRuntimeContext::new(db));
        assert!(host.shared_get::<IndexReplayOperatorRuntime>().is_some());
        assert!(host.shared_get::<IndexReplayShadowTransportRuntime>().is_some());
        assert!(host
            .shared_get::<IndexDriftDiagnosisOperatorRuntime>()
            .is_some());
        assert!(host
            .shared_get::<IndexDriftSourcePageDiagnosisRuntime>()
            .is_some());
    }

    #[tokio::test]
    async fn shadow_dispatch_reuses_request_bound_modules_manage_guard() {
        let registry = ModuleRegistry::new()
            .register(IndexModule)
            .register(DemoReplayModule);
        let mut extensions = rustok_distribution::build_runtime_extensions(&registry)
            .expect("source schema composition should build");
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        materialize_index_replay_runtime(&mut extensions, db)
            .expect("complete replay runtime should compose");
        let runtime = extensions
            .get::<IndexReplayOperatorRuntime>()
            .cloned()
            .expect("guarded replay operator runtime");

        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let context = IndexReplayOperatorContext::new(tenant_id, actor_id).unwrap();
        let request = IndexReplayDryRunRequest::new(
            tenant_id,
            demo_schema().reference,
            None,
            10,
            1,
        )
        .unwrap();

        let forbidden = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_READ],
                UserRole::Admin,
            )),
            runtime.run_shadow(context, request.clone()),
        )
        .await
        .expect_err("modules:read must not invoke shadow replay");
        assert!(matches!(
            forbidden,
            IndexReplayShadowOperatorError::Authorization(IndexReplayOperatorError::Forbidden)
        ));

        let outcome = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            runtime.run_shadow(context, request),
        )
        .await
        .expect("modules:manage should invoke side-effect-free shadow replay");
        assert_eq!(outcome.status(), IndexReplayDryRunStatus::Complete);
        assert_eq!(outcome.pages_scanned(), 1);
        assert_eq!(outcome.mutation_count(), 0);
    }

    #[tokio::test]
    async fn duplicate_host_replay_materialization_fails_closed() {
        let registry = ModuleRegistry::new()
            .register(IndexModule)
            .register(DemoReplayModule);
        let mut extensions = rustok_distribution::build_runtime_extensions(&registry)
            .expect("source schema composition should build");
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        materialize_index_replay_runtime(&mut extensions, db.clone())
            .expect("first replay materialization");

        let error = materialize_index_replay_runtime(&mut extensions, db)
            .expect_err("duplicate replay materialization must fail");
        assert!(error.to_string().contains("already materialized"));
    }

    #[tokio::test]
    async fn operator_authorization_requires_exact_tenant_actor_and_modules_manage() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let context = IndexReplayOperatorContext::new(tenant_id, actor_id).unwrap();
        assert!(matches!(
            context.authorize_for(tenant_id),
            Err(IndexReplayOperatorError::MissingRequestAuthority)
        ));

        with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_READ],
                UserRole::Admin,
            )),
            async {
                assert!(matches!(
                    context.authorize_for(tenant_id),
                    Err(IndexReplayOperatorError::Forbidden)
                ));
            },
        )
        .await;

        with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            async {
                assert!(context.authorize_for(tenant_id).is_ok());
                assert!(matches!(
                    context.authorize_for(Uuid::new_v4()),
                    Err(IndexReplayOperatorError::TenantMismatch)
                ));
            },
        )
        .await;
    }

    #[test]
    fn operator_context_rejects_nil_identity() {
        assert!(matches!(
            IndexReplayOperatorContext::new(Uuid::nil(), Uuid::new_v4()),
            Err(IndexReplayOperatorError::InvalidContext)
        ));
        assert!(matches!(
            IndexReplayOperatorContext::new(Uuid::new_v4(), Uuid::nil()),
            Err(IndexReplayOperatorError::InvalidContext)
        ));
    }
}
