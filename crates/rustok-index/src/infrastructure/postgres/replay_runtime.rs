use std::{fmt, sync::Arc};

use rustok_core::ModuleRuntimeExtensions;
use sea_orm::DatabaseConnection;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    IndexReplayCancelOutcome, IndexReplayDryRunRuntimeCompositionError, IndexReplayRunError,
    IndexReplayRunOutcome, IndexReplayRunRequest, SharedIndexSchemaRegistry,
    SharedIndexSourceRegistry, materialize_index_replay_dry_run_runtime,
};

use super::{
    IndexReconciliationSchedulerCompositionError, PostgresIndexReplayRunner,
    register_postgres_index_reconciliation_work,
};

/// Cloneable operator capability for bounded Index replay execution.
///
/// Construction is Index-owned so executable hosts publish one canonical runner assembled from
/// the complete immutable schema/source registries. Consumers receive only bounded run, cooperative
/// interruptible run, and cancel operations; the database connection and registry internals remain private.
#[derive(Clone)]
pub struct SharedIndexReplayRuntime {
    runner: Arc<PostgresIndexReplayRunner>,
}

impl SharedIndexReplayRuntime {
    fn new(runner: PostgresIndexReplayRunner) -> Self {
        Self {
            runner: Arc::new(runner),
        }
    }

    pub async fn run(
        &self,
        request: IndexReplayRunRequest,
    ) -> Result<IndexReplayRunOutcome, IndexReplayRunError> {
        self.runner.run(request).await
    }

    /// Runs one bounded replay invocation with a host-owned cooperative stop probe.
    ///
    /// The probe is intentionally synchronous and carries no transport or storage failure surface.
    /// It is sampled only by the runner's existing durable safe points. Callers that do not own a
    /// host lifecycle should use [`Self::run`] instead.
    pub async fn run_interruptible<Check>(
        &self,
        request: IndexReplayRunRequest,
        should_interrupt: Check,
    ) -> Result<IndexReplayRunOutcome, IndexReplayRunError>
    where
        Check: FnMut() -> bool,
    {
        self.runner
            .run_interruptible(request, should_interrupt)
            .await
    }

    pub async fn request_cancel(
        &self,
        tenant_id: Uuid,
        job_id: Uuid,
    ) -> Result<IndexReplayCancelOutcome, IndexReplayRunError> {
        self.runner.request_cancel(tenant_id, job_id).await
    }
}

impl fmt::Debug for SharedIndexReplayRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedIndexReplayRuntime")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexReplayRuntimeCompositionError {
    #[error("shared Index replay runtime is already materialized")]
    AlreadyMaterialized,
    #[error("shared Index source registry exists without the shared schema registry")]
    MissingSchemaRegistry,
    #[error(transparent)]
    DryRun(#[from] IndexReplayDryRunRuntimeCompositionError),
    #[error(transparent)]
    ReconciliationScheduler(#[from] IndexReconciliationSchedulerCompositionError),
}

/// Materializes canonical PostgreSQL-backed replay and due-reconciliation host capabilities.
///
/// An absent source registry returns `Ok(None)` and publishes neither a false replay capability
/// nor an empty Index module-work registration. Complete immutable registries publish the dry-run
/// capability, one module-owned reconciliation work registration, and the replay runtime. This
/// function performs no database I/O and starts no task; the generic host module-work lifecycle
/// owns polling and graceful shutdown after all registrations are collected.
pub fn materialize_postgres_index_replay_runtime(
    extensions: &mut ModuleRuntimeExtensions,
    db: DatabaseConnection,
) -> Result<Option<SharedIndexReplayRuntime>, IndexReplayRuntimeCompositionError> {
    if extensions.contains::<SharedIndexReplayRuntime>() {
        return Err(IndexReplayRuntimeCompositionError::AlreadyMaterialized);
    }

    let Some(sources) = extensions.get::<SharedIndexSourceRegistry>().cloned() else {
        return Ok(None);
    };
    let schemas = extensions
        .get::<SharedIndexSchemaRegistry>()
        .cloned()
        .ok_or(IndexReplayRuntimeCompositionError::MissingSchemaRegistry)?;

    materialize_index_replay_dry_run_runtime(extensions)?;
    register_postgres_index_reconciliation_work(extensions)?;
    let runtime = SharedIndexReplayRuntime::new(PostgresIndexReplayRunner::new(
        db,
        sources,
        schemas.shared(),
    ));
    extensions.insert(runtime.clone());
    Ok(Some(runtime))
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use rustok_core::ModuleRuntimeExtensions;
    use rustok_runtime::ModuleWorkRegistrations;
    use sea_orm::Database;

    use crate::{
        EntityName, FieldCardinality, FieldName, IndexField, IndexSchema, IndexSource,
        IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage,
        IndexSourceScanRequest, IndexValueType, LocaleMode, ModuleName, SchemaRef, SchemaVersion,
        SharedIndexReplayDryRunRuntime, SharedIndexReplayRuntime,
        materialize_index_schema_registry, materialize_index_source_registry,
        register_index_schema_source, register_index_source,
    };

    use super::{
        IndexReplayRuntimeCompositionError, materialize_postgres_index_replay_runtime,
    };

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

    fn schema() -> IndexSchema {
        IndexSchema {
            reference: SchemaRef {
                module: ModuleName::new("runtime-owner").unwrap(),
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

    fn source_extensions() -> ModuleRuntimeExtensions {
        let mut extensions = ModuleRuntimeExtensions::default();
        let schema = schema();
        register_index_schema_source(&mut extensions, "runtime_owner", schema.clone()).unwrap();
        register_index_source(
            &mut extensions,
            "runtime_owner",
            "runtime-owner-primary",
            [schema.reference],
            NoopSource,
        )
        .unwrap();
        extensions
    }

    async fn connection() -> sea_orm::DatabaseConnection {
        Database::connect("sqlite::memory:")
            .await
            .expect("test connection should initialize")
    }

    #[tokio::test]
    async fn missing_source_registry_does_not_publish_false_replay_or_work_runtime() {
        let mut extensions = ModuleRuntimeExtensions::default();
        register_index_schema_source(&mut extensions, "runtime_owner", schema()).unwrap();
        let schemas = materialize_index_schema_registry(&extensions)
            .unwrap()
            .expect("schema registry");
        extensions.insert(schemas);

        let runtime =
            materialize_postgres_index_replay_runtime(&mut extensions, connection().await)
                .expect("missing source registry should be accepted");
        assert!(runtime.is_none());
        assert!(!extensions.contains::<SharedIndexReplayRuntime>());
        assert!(!extensions.contains::<SharedIndexReplayDryRunRuntime>());
        assert!(!extensions.contains::<ModuleWorkRegistrations>());
    }

    #[tokio::test]
    async fn source_registry_without_shared_schema_registry_fails_closed() {
        let mut extensions = source_extensions();
        let sources = materialize_index_source_registry(&extensions)
            .unwrap()
            .expect("source registry");
        extensions.insert(sources);

        let error = materialize_postgres_index_replay_runtime(&mut extensions, connection().await)
            .expect_err("partial replay composition must fail");
        assert_eq!(
            error,
            IndexReplayRuntimeCompositionError::MissingSchemaRegistry
        );
        assert!(!extensions.contains::<ModuleWorkRegistrations>());
    }

    #[tokio::test]
    async fn complete_registries_materialize_replay_and_module_work_registration() {
        let mut extensions = source_extensions();
        let schemas = materialize_index_schema_registry(&extensions)
            .unwrap()
            .expect("schema registry");
        let sources = materialize_index_source_registry(&extensions)
            .unwrap()
            .expect("source registry");
        extensions.insert(schemas);
        extensions.insert(sources);

        let runtime =
            materialize_postgres_index_replay_runtime(&mut extensions, connection().await)
                .expect("replay runtime should materialize")
                .expect("complete registries should publish a runtime");
        assert!(extensions.contains::<SharedIndexReplayRuntime>());
        assert!(extensions.contains::<SharedIndexReplayDryRunRuntime>());
        assert!(extensions.contains::<ModuleWorkRegistrations>());
        assert!(format!("{runtime:?}").contains("SharedIndexReplayRuntime"));
    }

    #[tokio::test]
    async fn duplicate_replay_runtime_materialization_fails_closed() {
        let mut extensions = source_extensions();
        let schemas = materialize_index_schema_registry(&extensions)
            .unwrap()
            .expect("schema registry");
        let sources = materialize_index_source_registry(&extensions)
            .unwrap()
            .expect("source registry");
        extensions.insert(schemas);
        extensions.insert(sources);
        materialize_postgres_index_replay_runtime(&mut extensions, connection().await)
            .expect("first materialization")
            .expect("runtime");

        let error = materialize_postgres_index_replay_runtime(&mut extensions, connection().await)
            .expect_err("duplicate materialization must fail");
        assert_eq!(error, IndexReplayRuntimeCompositionError::AlreadyMaterialized);
    }
}
