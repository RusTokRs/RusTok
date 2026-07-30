mod cursor;
mod planner;
mod postgres_compiler;
mod postgres_query_result;
mod postgres_query_sql;
mod query_port;
mod registry;
mod source_schema_registry;
mod validation;

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
pub use registry::{
    LinkPathStep, RegisteredSchema, RegistrationOutcome, SchemaRegistry, SchemaRegistryError,
};
pub use source_schema_registry::{
    IndexSchemaSourceCatalog, IndexSchemaSourceDescriptor, IndexSchemaSourceError,
    SharedIndexSchemaRegistry, materialize_index_schema_registry, register_index_schema_source,
};
pub use validation::{QueryValidationError, RecordValidationError};
