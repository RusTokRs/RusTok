use std::{fmt, sync::Arc};

use rustok_api::{Permission, has_effective_permission};
use rustok_core::ModuleRuntimeExtensions;
use sea_orm::DatabaseConnection;
use thiserror::Error;

use super::reconciliation_operator::IndexReconciliationOperatorContext;
use crate::error::{Error as ServerError, Result};
use crate::services::rbac_request_scope::permissions_for;

type IndexDriftDiagnosisProducer = rustok_index::IndexDriftDigestProducer<
    rustok_index::PostgresIndexDriftSnapshotReader,
    rustok_index::infrastructure::postgres::PostgresIndexDriftFindingWriter,
>;

#[derive(Debug, Error)]
pub enum IndexDriftDiagnosisOperatorError {
    #[error("Index drift diagnosis key tenant does not match the authorized operator tenant")]
    TenantMismatch,
    #[error("Index drift diagnosis requires a request-bound effective permission snapshot")]
    MissingRequestAuthority,
    #[error("modules:manage is required for Index drift diagnosis")]
    Forbidden,
    #[error(transparent)]
    Diagnosis(#[from] rustok_index::IndexDriftDigestError),
}

/// Server-owned guarded exact-entity drift diagnosis boundary.
///
/// The runtime composes the production snapshot reader, database-neutral digest producer, and
/// PostgreSQL finding writer behind one request-bound `modules:manage` check. It accepts only one
/// exact `EntityKey` and exposes no scan, discovery, lifecycle, repair, connection, source registry,
/// snapshot reader, or writer handle. The missing-only method returns only a bounded candidate
/// outcome and never exposes the captured typed states.
#[derive(Clone)]
pub struct IndexDriftDiagnosisOperatorRuntime {
    inner: Arc<IndexDriftDiagnosisProducer>,
}

impl IndexDriftDiagnosisOperatorRuntime {
    fn new(inner: IndexDriftDiagnosisProducer) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    pub async fn diagnose_entity(
        &self,
        context: IndexReconciliationOperatorContext,
        key: rustok_index::EntityKey,
    ) -> std::result::Result<rustok_index::IndexDriftDigestOutcome, IndexDriftDiagnosisOperatorError>
    {
        authorize_for(&context, key.tenant_id)?;
        let request = rustok_index::IndexDriftDigestRequest::new(key)?;
        self.inner.produce(request).await.map_err(Into::into)
    }

    pub async fn diagnose_missing_entity_candidate(
        &self,
        context: IndexReconciliationOperatorContext,
        key: rustok_index::EntityKey,
    ) -> std::result::Result<
        rustok_index::IndexDriftMissingEntityCandidateOutcome,
        IndexDriftDiagnosisOperatorError,
    > {
        authorize_for(&context, key.tenant_id)?;
        let request = rustok_index::IndexDriftDigestRequest::new(key)?;
        self.inner
            .produce_missing_entity_candidate(request)
            .await
            .map_err(Into::into)
    }
}

impl fmt::Debug for IndexDriftDiagnosisOperatorRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftDiagnosisOperatorRuntime")
            .finish_non_exhaustive()
    }
}

fn authorize_for(
    context: &IndexReconciliationOperatorContext,
    requested_tenant: uuid::Uuid,
) -> std::result::Result<(), IndexDriftDiagnosisOperatorError> {
    if requested_tenant != context.tenant_id() {
        return Err(IndexDriftDiagnosisOperatorError::TenantMismatch);
    }
    let permissions = permissions_for(&context.tenant_id(), &context.actor_id())
        .ok_or(IndexDriftDiagnosisOperatorError::MissingRequestAuthority)?;
    if !has_effective_permission(&permissions, &Permission::MODULES_MANAGE) {
        return Err(IndexDriftDiagnosisOperatorError::Forbidden);
    }
    Ok(())
}

/// Publishes exact-entity diagnosis after replay composition has frozen source/schema registries.
///
/// Composition performs no SQL and starts no task. An absent source registry leaves the capability
/// unpublished; a source registry without its shared schema registry fails closed. An optional
/// explicit absence registry is frozen from the same owner-bound source composition before the
/// snapshot reader is constructed.
pub(super) fn materialize_index_drift_diagnosis_operator(
    extensions: &mut ModuleRuntimeExtensions,
    db: DatabaseConnection,
) -> Result<()> {
    if extensions.contains::<IndexDriftDiagnosisOperatorRuntime>() {
        return Err(ServerError::Message(
            "guarded Index drift diagnosis runtime is already materialized".to_string(),
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
                "Index drift diagnosis source registry exists without the shared schema registry"
                    .to_string(),
            )
        })?;

    let absence =
        rustok_index::materialize_index_source_absence_registry(extensions).map_err(|error| {
            ServerError::Message(format!(
                "Index source absence registry materialization failed: {error}"
            ))
        })?;
    if let Some(absence) = absence {
        extensions.insert(absence);
    }

    let reader =
        rustok_index::PostgresIndexDriftSnapshotReader::new(db.clone(), sources, schemas.clone());
    let reader = match extensions
        .get::<rustok_index::SharedIndexSourceAbsenceRegistry>()
        .cloned()
    {
        Some(absence) => reader.with_absence_registry(absence),
        None => reader,
    };
    let writer = rustok_index::infrastructure::postgres::PostgresIndexDriftFindingWriter::new(db);
    let producer = rustok_index::IndexDriftDigestProducer::new(schemas.shared(), reader, writer);
    extensions.insert(IndexDriftDiagnosisOperatorRuntime::new(producer));
    Ok(())
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use rustok_api::Permission;
    use rustok_core::{ModuleRuntimeExtensions, UserRole};
    use rustok_index::{
        EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexSchema, IndexSource,
        IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage,
        IndexSourceScanRequest, IndexValueType, LocaleMode, ModuleName, SchemaRef, SchemaVersion,
        SharedIndexSchemaRegistry, SharedIndexSourceRegistry, materialize_index_schema_registry,
        materialize_index_source_registry, register_index_schema_source, register_index_source,
    };
    use sea_orm::Database;
    use uuid::Uuid;

    use super::{
        IndexDriftDiagnosisOperatorError, IndexDriftDiagnosisOperatorRuntime,
        materialize_index_drift_diagnosis_operator,
    };
    use crate::services::index_replay_runtime_composition::IndexReconciliationOperatorContext;
    use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};

    struct NoopSource;

    #[async_trait]
    impl IndexSource for NoopSource {
        async fn scan(
            &self,
            request: IndexSourceScanRequest,
        ) -> std::result::Result<IndexSourcePage, IndexSourceFailure> {
            Ok(IndexSourcePage::new(&request, Vec::new(), None)
                .expect("empty final page should be valid"))
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
                module: ModuleName::new("server-drift-diagnosis").unwrap(),
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
        register_index_schema_source(&mut extensions, "server_drift_diagnosis", schema.clone())
            .expect("schema source should register");
        register_index_source(
            &mut extensions,
            "server_drift_diagnosis",
            "server-drift-diagnosis-primary",
            [schema.reference],
            NoopSource,
        )
        .expect("source should register");
        extensions
    }

    fn complete_registries() -> ModuleRuntimeExtensions {
        let mut extensions = catalogs();
        let schemas = materialize_index_schema_registry(&extensions)
            .expect("schema materialization")
            .expect("schema registry");
        let sources = materialize_index_source_registry(&extensions)
            .expect("source materialization")
            .expect("source registry");
        extensions.insert(schemas);
        extensions.insert(sources);
        extensions
    }

    fn key(tenant_id: Uuid, entity_id: Uuid) -> EntityKey {
        EntityKey {
            tenant_id,
            schema: schema().reference,
            entity_id,
            locale: None,
        }
    }

    #[tokio::test]
    async fn missing_sources_do_not_publish_false_diagnosis_capability() {
        let mut extensions = ModuleRuntimeExtensions::default();
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        materialize_index_drift_diagnosis_operator(&mut extensions, db)
            .expect("missing sources should remain optional");
        assert!(!extensions.contains::<IndexDriftDiagnosisOperatorRuntime>());
    }

    #[tokio::test]
    async fn source_registry_without_schema_registry_fails_closed() {
        let mut extensions = catalogs();
        let sources = materialize_index_source_registry(&extensions)
            .expect("source materialization")
            .expect("source registry");
        extensions.insert(sources);
        assert!(!extensions.contains::<SharedIndexSchemaRegistry>());
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        let error = materialize_index_drift_diagnosis_operator(&mut extensions, db)
            .expect_err("missing schema registry must fail");
        assert!(
            error
                .to_string()
                .contains("without the shared schema registry")
        );
    }

    #[tokio::test]
    async fn complete_registries_publish_guarded_diagnosis_to_host_context() {
        let mut extensions = complete_registries();
        assert!(extensions.contains::<SharedIndexSourceRegistry>());
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        materialize_index_drift_diagnosis_operator(&mut extensions, db.clone())
            .expect("diagnosis composition");
        assert!(extensions.contains::<IndexDriftDiagnosisOperatorRuntime>());
        let host = extensions.apply_to_host_runtime(rustok_api::HostRuntimeContext::new(db));
        assert!(
            host.shared_get::<IndexDriftDiagnosisOperatorRuntime>()
                .is_some()
        );
    }

    #[tokio::test]
    async fn duplicate_diagnosis_materialization_fails_closed() {
        let mut extensions = complete_registries();
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        materialize_index_drift_diagnosis_operator(&mut extensions, db.clone())
            .expect("first diagnosis composition");
        let error = materialize_index_drift_diagnosis_operator(&mut extensions, db)
            .expect_err("duplicate diagnosis composition must fail");
        assert!(error.to_string().contains("already materialized"));
    }

    #[tokio::test]
    async fn exact_entity_diagnosis_authorizes_before_request_validation() {
        let mut extensions = complete_registries();
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database without Index migrations");
        materialize_index_drift_diagnosis_operator(&mut extensions, db)
            .expect("diagnosis composition");
        let runtime = extensions
            .get::<IndexDriftDiagnosisOperatorRuntime>()
            .cloned()
            .expect("diagnosis runtime");
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let context = IndexReconciliationOperatorContext::new(tenant_id, actor_id).unwrap();
        let invalid = key(tenant_id, Uuid::nil());

        let missing = runtime
            .diagnose_entity(context, invalid.clone())
            .await
            .expect_err("missing authority must fail before request validation");
        assert!(matches!(
            missing,
            IndexDriftDiagnosisOperatorError::MissingRequestAuthority
        ));

        let forbidden = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_READ],
                UserRole::Admin,
            )),
            runtime.diagnose_entity(context, invalid.clone()),
        )
        .await
        .expect_err("modules:read must not diagnose drift");
        assert!(matches!(
            forbidden,
            IndexDriftDiagnosisOperatorError::Forbidden
        ));

        let delegated = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            runtime.diagnose_entity(context, invalid),
        )
        .await
        .expect_err("authorized invalid key must reach bounded request validation");
        assert!(matches!(
            delegated,
            IndexDriftDiagnosisOperatorError::Diagnosis(
                rustok_index::IndexDriftDigestError::NilEntityId
            )
        ));

        let cross_tenant = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            runtime.diagnose_entity(context, key(Uuid::new_v4(), Uuid::new_v4())),
        )
        .await
        .expect_err("cross-tenant diagnosis must fail before producer delegation");
        assert!(matches!(
            cross_tenant,
            IndexDriftDiagnosisOperatorError::TenantMismatch
        ));
    }

    #[tokio::test]
    async fn missing_candidate_diagnosis_authorizes_before_request_validation() {
        let mut extensions = complete_registries();
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database without Index migrations");
        materialize_index_drift_diagnosis_operator(&mut extensions, db)
            .expect("diagnosis composition");
        let runtime = extensions
            .get::<IndexDriftDiagnosisOperatorRuntime>()
            .cloned()
            .expect("diagnosis runtime");
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let context = IndexReconciliationOperatorContext::new(tenant_id, actor_id).unwrap();
        let invalid = key(tenant_id, Uuid::nil());

        let missing = runtime
            .diagnose_missing_entity_candidate(context, invalid.clone())
            .await
            .expect_err("missing authority must fail before candidate validation");
        assert!(matches!(
            missing,
            IndexDriftDiagnosisOperatorError::MissingRequestAuthority
        ));

        let forbidden = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_READ],
                UserRole::Admin,
            )),
            runtime.diagnose_missing_entity_candidate(context, invalid.clone()),
        )
        .await
        .expect_err("modules:read must not diagnose missing candidates");
        assert!(matches!(
            forbidden,
            IndexDriftDiagnosisOperatorError::Forbidden
        ));

        let delegated = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            runtime.diagnose_missing_entity_candidate(context, invalid),
        )
        .await
        .expect_err("authorized invalid key must reach candidate request validation");
        assert!(matches!(
            delegated,
            IndexDriftDiagnosisOperatorError::Diagnosis(
                rustok_index::IndexDriftDigestError::NilEntityId
            )
        ));
    }
}
