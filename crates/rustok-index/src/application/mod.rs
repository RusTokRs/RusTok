mod aggregate_ordering;
mod cursor;
mod planner;
mod postgres_compiler;
mod postgres_query_result;
mod postgres_query_sql;
mod query_port;
mod query_runtime;
mod registry;
mod source_event_id;
mod source_registry;
mod source_replay;
mod source_schema_registry;
mod source_timeout;
mod validation;

#[cfg(test)]
mod aggregate_ordering_tests;
#[cfg(test)]
mod planner_tests;
#[cfg(test)]
mod postgres_compiler_tests;
#[cfg(test)]
mod postgres_many_projection_tests;
#[cfg(test)]
mod postgres_query_result_tests;
#[cfg(test)]
mod query_snapshot_tests;
#[cfg(test)]
mod reference;
#[cfg(test)]
mod source_replay_tests;

pub use aggregate_ordering::AggregateOrderValidationError;
pub use cursor::{CursorCodec, CursorCodecError, CursorValidationError, IndexCursor};
pub use planner::{
    ExecutableQueryPlan, PlannedField, PlannedJoin, PlannedManyProjection, PlannedOrder,
    QueryPlanError, QueryPlanFingerprint,
};
pub use postgres_compiler::{
    CompiledManyRelationColumn, CompiledPostgresCount, CompiledPostgresQuery,
    CompiledQueryColumn, PostgresBindValue, PostgresQueryBuildError,
    PostgresQueryCompileError,
};
pub use postgres_query_result::{
    CompiledPostgresCell, CompiledPostgresPageQuery, CompiledPostgresRow,
    IndexNestedRelationItem, IndexNestedRelationProjection, IndexProjectedValue, IndexQueryItem,
    IndexQueryPage, IndexRelationIdentity, PostgresQueryDecodeError,
    PostgresQueryPageBuildError,
};
pub use query_port::{
    IndexQueryExecutionError, IndexQueryPort, PersistedSchemaReadinessFailure,
};
pub use query_runtime::SharedIndexQueryRuntime;
pub use registry::{
    LinkPathStep, RegisteredSchema, RegistrationOutcome, SchemaRegistry, SchemaRegistryError,
};
pub use source_event_id::{
    IndexSourceEventIdError, derive_index_source_event_id,
};
pub use source_registry::{
    IndexSource, IndexSourceCatalog, IndexSourceCursor, IndexSourceDescriptor, IndexSourceError,
    IndexSourceFailure, IndexSourceFailureKind, IndexSourceLoadBatch, IndexSourceLoadRequest,
    IndexSourcePage, IndexSourceScanRequest, SharedIndexSourceRegistry,
    materialize_index_source_registry,
};
pub use source_replay::{
    IndexReplayCheckpoint, IndexReplayCheckpointKey, IndexReplayCheckpointStore, IndexReplayError,
    IndexReplayFailure, IndexReplayFailureKind, IndexReplayMutationOutcome,
    IndexReplayMutationSink, IndexReplayPageOutcome, IndexReplayPageRequest,
    IndexReplayPageStatus, IndexReplayWorker,
};
pub use source_schema_registry::{
    IndexSchemaSourceCatalog, IndexSchemaSourceDescriptor, IndexSchemaSourceError,
    SharedIndexSchemaRegistry, materialize_index_schema_registry, register_index_schema_source,
};
pub use source_timeout::register_index_source;
pub use validation::{QueryValidationError, RecordValidationError};
