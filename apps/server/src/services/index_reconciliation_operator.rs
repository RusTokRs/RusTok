use std::fmt;

use rustok_api::{Permission, has_effective_permission};
use rustok_core::ModuleRuntimeExtensions;
use sea_orm::DatabaseConnection;
use thiserror::Error;
use uuid::Uuid;

use crate::error::{Error as ServerError, Result};
use crate::services::rbac_request_scope::permissions_for;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexReconciliationOperatorContext {
    tenant_id: Uuid,
    actor_id: Uuid,
}

impl IndexReconciliationOperatorContext {
    pub fn new(
        tenant_id: Uuid,
        actor_id: Uuid,
    ) -> std::result::Result<Self, IndexReconciliationOperatorError> {
        if tenant_id.is_nil() || actor_id.is_nil() {
            return Err(IndexReconciliationOperatorError::InvalidContext);
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

    fn authorize_for(
        &self,
        requested_tenant: Uuid,
    ) -> std::result::Result<(), IndexReconciliationOperatorError> {
        if requested_tenant != self.tenant_id {
            return Err(IndexReconciliationOperatorError::TenantMismatch);
        }
        let permissions = permissions_for(&self.tenant_id, &self.actor_id)
            .ok_or(IndexReconciliationOperatorError::MissingRequestAuthority)?;
        if !has_effective_permission(&permissions, &Permission::MODULES_MANAGE) {
            return Err(IndexReconciliationOperatorError::Forbidden);
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum IndexReconciliationOperatorError {
    #[error("Index reconciliation operator tenant and actor must not be nil")]
    InvalidContext,
    #[error("Index reconciliation request tenant does not match the authorized operator tenant")]
    TenantMismatch,
    #[error(
        "Index reconciliation operations require a request-bound effective permission snapshot"
    )]
    MissingRequestAuthority,
    #[error("modules:manage is required for Index reconciliation operations")]
    Forbidden,
    #[error(transparent)]
    Reconciliation(#[from] rustok_index::IndexReconciliationRunError),
    #[error(transparent)]
    Inspection(
        #[from]
        rustok_index::infrastructure::postgres::IndexReconciliationDeadLetterInspectionError,
    ),
    #[error(transparent)]
    DriftInspection(
        #[from] rustok_index::infrastructure::postgres::IndexDriftFindingInspectionError,
    ),
    #[error(transparent)]
    Recovery(#[from] rustok_index::infrastructure::postgres::IndexReconciliationRecoveryError),
}

/// Server-owned guarded operator boundary over the canonical PostgreSQL Index reconciliation
/// runner, bounded dead-letter and drift-finding inspectors, and audited recovery store.
///
/// Transport adapters must provide an exact request-bound tenant/actor context. The boundary
/// accepts only `modules:manage`, rejects cross-tenant run requests before database access, derives
/// cancellation, inspection, drift-diagnosis, and recovery tenant scope from the authorized
/// context, binds recovery actor identity to that same context, and exposes no connection, source
/// registry, scheduler, task handle, or worker-spawn capability.
#[derive(Clone)]
pub struct IndexReconciliationOperatorRuntime {
    inner: rustok_index::PostgresIndexReconciliationRunner,
    dead_letters:
        rustok_index::infrastructure::postgres::PostgresIndexReconciliationDeadLetterInspector,
    drift_findings: rustok_index::infrastructure::postgres::PostgresIndexDriftFindingInspector,
    recovery: rustok_index::infrastructure::postgres::PostgresIndexReconciliationRecoveryStore,
}

impl IndexReconciliationOperatorRuntime {
    fn new(
        inner: rustok_index::PostgresIndexReconciliationRunner,
        dead_letters: rustok_index::infrastructure::postgres::PostgresIndexReconciliationDeadLetterInspector,
        drift_findings: rustok_index::infrastructure::postgres::PostgresIndexDriftFindingInspector,
        recovery: rustok_index::infrastructure::postgres::PostgresIndexReconciliationRecoveryStore,
    ) -> Self {
        Self {
            inner,
            dead_letters,
            drift_findings,
            recovery,
        }
    }

    pub async fn run(
        &self,
        context: IndexReconciliationOperatorContext,
        request: rustok_index::IndexReconciliationRunRequest,
    ) -> std::result::Result<
        rustok_index::IndexReconciliationRunOutcome,
        IndexReconciliationOperatorError,
    > {
        context.authorize_for(request.tenant_id())?;
        self.inner.run(request).await.map_err(Into::into)
    }

    pub async fn request_cancel(
        &self,
        context: IndexReconciliationOperatorContext,
        job_id: Uuid,
    ) -> std::result::Result<
        rustok_index::IndexReconciliationCancelOutcome,
        IndexReconciliationOperatorError,
    > {
        context.authorize_for(context.tenant_id())?;
        self.inner
            .request_cancel(context.tenant_id(), job_id)
            .await
            .map_err(Into::into)
    }

    pub async fn inspect_dead_letter(
        &self,
        context: IndexReconciliationOperatorContext,
        job_id: Uuid,
    ) -> std::result::Result<
        Option<rustok_index::infrastructure::postgres::IndexReconciliationDeadLetterInspection>,
        IndexReconciliationOperatorError,
    > {
        context.authorize_for(context.tenant_id())?;
        self.dead_letters
            .inspect(context.tenant_id(), job_id)
            .await
            .map_err(Into::into)
    }

    pub async fn inspect_drift_finding(
        &self,
        context: IndexReconciliationOperatorContext,
        finding_id: Uuid,
    ) -> std::result::Result<
        Option<rustok_index::infrastructure::postgres::IndexDriftFindingInspection>,
        IndexReconciliationOperatorError,
    > {
        context.authorize_for(context.tenant_id())?;
        self.drift_findings
            .inspect(context.tenant_id(), finding_id)
            .await
            .map_err(Into::into)
    }

    pub async fn requeue_dead_letter(
        &self,
        context: IndexReconciliationOperatorContext,
        job_id: Uuid,
        reason: impl Into<String>,
    ) -> std::result::Result<
        rustok_index::infrastructure::postgres::IndexReconciliationRequeueOutcome,
        IndexReconciliationOperatorError,
    > {
        context.authorize_for(context.tenant_id())?;
        let request =
            rustok_index::infrastructure::postgres::IndexReconciliationRequeueRequest::new(
                context.tenant_id(),
                job_id,
                context.actor_id(),
                reason,
            )?;
        self.recovery
            .requeue_failed(request)
            .await
            .map_err(Into::into)
    }
}

impl fmt::Debug for IndexReconciliationOperatorRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexReconciliationOperatorRuntime")
            .finish_non_exhaustive()
    }
}

/// Publishes the guarded reconciliation operator after replay composition has frozen the complete
/// immutable source and schema registries.
///
/// This function performs no database I/O and starts no task. An absent source registry remains an
/// optional capability; a source registry without its shared schema registry fails closed.
pub(super) fn materialize_index_reconciliation_operator(
    extensions: &mut ModuleRuntimeExtensions,
    db: DatabaseConnection,
) -> Result<()> {
    if extensions.contains::<IndexReconciliationOperatorRuntime>() {
        return Err(ServerError::Message(
            "guarded Index reconciliation runtime is already materialized".to_string(),
        ));
    }

    let Some(sources) = extensions
        .get::<rustok_index::SharedIndexSourceRegistry>()
        .cloned()
    else {
        return Ok(());
    };
    let schemas = extensions
        .get::<rustok_index::SharedIndexSchemaRegistry>()
        .cloned()
        .ok_or_else(|| {
            ServerError::Message(
                "Index reconciliation source registry exists without the shared schema registry"
                    .to_string(),
            )
        })?;

    let recovery =
        rustok_index::infrastructure::postgres::PostgresIndexReconciliationRecoveryStore::new(
            db.clone(),
        );
    let dead_letters =
        rustok_index::infrastructure::postgres::PostgresIndexReconciliationDeadLetterInspector::new(
            db.clone(),
        );
    let drift_findings =
        rustok_index::infrastructure::postgres::PostgresIndexDriftFindingInspector::new(db.clone());
    let runner =
        rustok_index::PostgresIndexReconciliationRunner::new(db, sources, schemas.shared());
    extensions.insert(IndexReconciliationOperatorRuntime::new(
        runner,
        dead_letters,
        drift_findings,
        recovery,
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_trait::async_trait;
    use rustok_api::Permission;
    use rustok_core::{ModuleRuntimeExtensions, UserRole};
    use rustok_index::{
        EntityName, FieldCardinality, FieldName, IndexField, IndexReconciliationRunRequest,
        IndexSchema, IndexSource, IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest,
        IndexSourcePage, IndexSourceScanRequest, IndexValueType, LocaleMode, ModuleName, SchemaRef,
        SchemaVersion, SharedIndexSchemaRegistry, SharedIndexSourceRegistry,
        materialize_index_schema_registry, materialize_index_source_registry,
        register_index_schema_source, register_index_source,
    };
    use sea_orm::Database;
    use uuid::Uuid;

    use super::{
        IndexReconciliationOperatorContext, IndexReconciliationOperatorError,
        IndexReconciliationOperatorRuntime, materialize_index_reconciliation_operator,
    };
    use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};

    struct NoopSource;

    #[async_trait]
    impl IndexSource for NoopSource {
        async fn scan(
            &self,
            request: IndexSourceScanRequest,
        ) -> std::result::Result<IndexSourcePage, IndexSourceFailure> {
            Ok(IndexSourcePage::new(&request, Vec::new(), None)
                .expect("empty final reconciliation page should be valid"))
        }

        async fn load(
            &self,
            request: IndexSourceLoadRequest,
        ) -> std::result::Result<IndexSourceLoadBatch, IndexSourceFailure> {
            Ok(IndexSourceLoadBatch::new(&request, Vec::new())
                .expect("empty targeted load should be valid"))
        }
    }

    fn schema() -> IndexSchema {
        IndexSchema {
            reference: SchemaRef {
                module: ModuleName::new("server-reconciliation").unwrap(),
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

    fn catalogs() -> ModuleRuntimeExtensions {
        let mut extensions = ModuleRuntimeExtensions::default();
        let schema = schema();
        register_index_schema_source(&mut extensions, "server_reconciliation", schema.clone())
            .expect("schema source should register");
        register_index_source(
            &mut extensions,
            "server_reconciliation",
            "server-reconciliation-primary",
            [schema.reference],
            NoopSource,
        )
        .expect("reconciliation source should register");
        extensions
    }

    fn complete_registries() -> ModuleRuntimeExtensions {
        let mut extensions = catalogs();
        let schemas = materialize_index_schema_registry(&extensions)
            .expect("schema registry materialization")
            .expect("schema registry");
        let sources = materialize_index_source_registry(&extensions)
            .expect("source registry materialization")
            .expect("source registry");
        extensions.insert(schemas);
        extensions.insert(sources);
        extensions
    }

    #[tokio::test]
    async fn missing_sources_do_not_publish_false_reconciliation_capability() {
        let mut extensions = ModuleRuntimeExtensions::default();
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");

        materialize_index_reconciliation_operator(&mut extensions, db)
            .expect("missing sources should remain optional");
        assert!(!extensions.contains::<IndexReconciliationOperatorRuntime>());
    }

    #[tokio::test]
    async fn source_registry_without_shared_schema_registry_fails_closed() {
        let mut extensions = catalogs();
        let sources = materialize_index_source_registry(&extensions)
            .expect("source registry materialization")
            .expect("source registry");
        extensions.insert(sources);
        assert!(!extensions.contains::<SharedIndexSchemaRegistry>());
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");

        let error = materialize_index_reconciliation_operator(&mut extensions, db)
            .expect_err("missing shared schema registry must fail");
        assert!(
            error
                .to_string()
                .contains("without the shared schema registry")
        );
    }

    #[tokio::test]
    async fn complete_registries_publish_guarded_runtime_to_host_context() {
        let mut extensions = complete_registries();
        assert!(extensions.contains::<SharedIndexSchemaRegistry>());
        assert!(extensions.contains::<SharedIndexSourceRegistry>());
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");

        materialize_index_reconciliation_operator(&mut extensions, db.clone())
            .expect("guarded reconciliation runtime should compose");
        assert!(extensions.contains::<IndexReconciliationOperatorRuntime>());

        let host = extensions.apply_to_host_runtime(rustok_api::HostRuntimeContext::new(db));
        assert!(
            host.shared_get::<IndexReconciliationOperatorRuntime>()
                .is_some()
        );
    }

    #[tokio::test]
    async fn duplicate_guarded_reconciliation_materialization_fails_closed() {
        let mut extensions = complete_registries();
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        materialize_index_reconciliation_operator(&mut extensions, db.clone())
            .expect("first materialization");

        let error = materialize_index_reconciliation_operator(&mut extensions, db)
            .expect_err("duplicate materialization must fail");
        assert!(error.to_string().contains("already materialized"));
    }

    #[tokio::test]
    async fn operator_authorization_requires_exact_tenant_actor_and_modules_manage() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let context = IndexReconciliationOperatorContext::new(tenant_id, actor_id).unwrap();
        assert!(matches!(
            context.authorize_for(tenant_id),
            Err(IndexReconciliationOperatorError::MissingRequestAuthority)
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
                    Err(IndexReconciliationOperatorError::Forbidden)
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
                    Err(IndexReconciliationOperatorError::TenantMismatch)
                ));
            },
        )
        .await;
    }

    #[tokio::test]
    async fn cross_tenant_run_is_denied_before_database_access() {
        let mut extensions = complete_registries();
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database without Index migrations");
        materialize_index_reconciliation_operator(&mut extensions, db)
            .expect("runtime materialization");
        let runtime = extensions
            .get::<IndexReconciliationOperatorRuntime>()
            .cloned()
            .expect("guarded runtime");
        let authorized_tenant = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let request = IndexReconciliationRunRequest::new(
            Uuid::new_v4(),
            schema().reference,
            "server-reconciliation-worker",
            1,
            1,
            1,
            1,
            Duration::from_secs(30),
        )
        .expect("valid bounded request");

        let error = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                authorized_tenant,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            runtime.run(
                IndexReconciliationOperatorContext::new(authorized_tenant, actor_id).unwrap(),
                request,
            ),
        )
        .await
        .expect_err("cross-tenant run must fail before touching the database");
        assert!(matches!(
            error,
            IndexReconciliationOperatorError::TenantMismatch
        ));
    }

    #[tokio::test]
    async fn dead_letter_inspection_authorizes_before_adapter_validation() {
        let mut extensions = complete_registries();
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database without Index migrations");
        materialize_index_reconciliation_operator(&mut extensions, db)
            .expect("runtime materialization");
        let runtime = extensions
            .get::<IndexReconciliationOperatorRuntime>()
            .cloned()
            .expect("guarded runtime");
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let context = IndexReconciliationOperatorContext::new(tenant_id, actor_id).unwrap();

        let missing = runtime
            .inspect_dead_letter(context, Uuid::nil())
            .await
            .expect_err("missing request authority must fail before adapter validation");
        assert!(matches!(
            missing,
            IndexReconciliationOperatorError::MissingRequestAuthority
        ));

        let forbidden = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_READ],
                UserRole::Admin,
            )),
            runtime.inspect_dead_letter(context, Uuid::nil()),
        )
        .await
        .expect_err("modules:read must not inspect dead letters");
        assert!(matches!(
            forbidden,
            IndexReconciliationOperatorError::Forbidden
        ));

        let delegated = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            runtime.inspect_dead_letter(context, Uuid::nil()),
        )
        .await
        .expect_err("authorized nil job must reach bounded adapter validation");
        assert!(matches!(
            delegated,
            IndexReconciliationOperatorError::Inspection(
                rustok_index::infrastructure::postgres::IndexReconciliationDeadLetterInspectionError::NilJobId
            )
        ));
    }

    #[tokio::test]
    async fn drift_finding_inspection_authorizes_before_adapter_validation() {
        let mut extensions = complete_registries();
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database without Index migrations");
        materialize_index_reconciliation_operator(&mut extensions, db)
            .expect("runtime materialization");
        let runtime = extensions
            .get::<IndexReconciliationOperatorRuntime>()
            .cloned()
            .expect("guarded runtime");
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let context = IndexReconciliationOperatorContext::new(tenant_id, actor_id).unwrap();

        let missing = runtime
            .inspect_drift_finding(context, Uuid::nil())
            .await
            .expect_err("missing request authority must fail before adapter validation");
        assert!(matches!(
            missing,
            IndexReconciliationOperatorError::MissingRequestAuthority
        ));

        let forbidden = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_READ],
                UserRole::Admin,
            )),
            runtime.inspect_drift_finding(context, Uuid::nil()),
        )
        .await
        .expect_err("modules:read must not inspect drift findings");
        assert!(matches!(
            forbidden,
            IndexReconciliationOperatorError::Forbidden
        ));

        let delegated = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            runtime.inspect_drift_finding(context, Uuid::nil()),
        )
        .await
        .expect_err("authorized nil finding must reach bounded adapter validation");
        assert!(matches!(
            delegated,
            IndexReconciliationOperatorError::DriftInspection(
                rustok_index::infrastructure::postgres::IndexDriftFindingInspectionError::NilFindingId
            )
        ));
    }

    #[tokio::test]
    async fn dead_letter_requeue_authorizes_before_request_validation() {
        let mut extensions = complete_registries();
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database without Index migrations");
        materialize_index_reconciliation_operator(&mut extensions, db)
            .expect("runtime materialization");
        let runtime = extensions
            .get::<IndexReconciliationOperatorRuntime>()
            .cloned()
            .expect("guarded runtime");
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let context = IndexReconciliationOperatorContext::new(tenant_id, actor_id).unwrap();

        let missing = runtime
            .requeue_dead_letter(context, Uuid::nil(), "")
            .await
            .expect_err("missing authority must fail before recovery request validation");
        assert!(matches!(
            missing,
            IndexReconciliationOperatorError::MissingRequestAuthority
        ));

        let forbidden = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_READ],
                UserRole::Admin,
            )),
            runtime.requeue_dead_letter(context, Uuid::nil(), ""),
        )
        .await
        .expect_err("modules:read must not requeue dead letters");
        assert!(matches!(
            forbidden,
            IndexReconciliationOperatorError::Forbidden
        ));

        let delegated = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            runtime.requeue_dead_letter(context, Uuid::nil(), ""),
        )
        .await
        .expect_err("authorized request must reach bounded recovery DTO validation");
        assert!(matches!(
            delegated,
            IndexReconciliationOperatorError::Recovery(
                rustok_index::infrastructure::postgres::IndexReconciliationRecoveryError::NilJobId
            )
        ));

        let reason = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            runtime.requeue_dead_letter(context, Uuid::new_v4(), ""),
        )
        .await
        .expect_err("authorized request must bind context identity before reason validation");
        assert!(matches!(
            reason,
            IndexReconciliationOperatorError::Recovery(
                rustok_index::infrastructure::postgres::IndexReconciliationRecoveryError::InvalidReason(_)
            )
        ));
    }

    #[test]
    fn operator_context_rejects_nil_identity() {
        assert!(matches!(
            IndexReconciliationOperatorContext::new(Uuid::nil(), Uuid::new_v4()),
            Err(IndexReconciliationOperatorError::InvalidContext)
        ));
        assert!(matches!(
            IndexReconciliationOperatorContext::new(Uuid::new_v4(), Uuid::nil()),
            Err(IndexReconciliationOperatorError::InvalidContext)
        ));
    }
}
