use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{FieldPath, IndexQuery, IndexValueType, LinkCardinality, LinkName, Pagination};

use super::{
    CursorCodec, CursorValidationError, ExecutableQueryPlan, IndexCursor, PlannedField,
    PlannedManyProjection, QueryPlanError, QueryPlanFingerprint, SchemaRegistry,
    planner::derive_many_projections,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PostgresBindValue {
    Boolean(bool),
    Integer(i64),
    Decimal(Decimal),
    Text(String),
    Uuid(Uuid),
    Timestamp(DateTime<Utc>),
    Json(JsonValue),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CompiledQueryColumn {
    EntityId {
        output_alias: String,
        relation_alias: String,
    },
    Field {
        output_alias: String,
        field: PlannedField,
    },
    OrderValue {
        output_alias: String,
        field: PlannedField,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledManyRelationColumn {
    pub output_alias: String,
    pub projection: PlannedManyProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledPostgresCount {
    pub sql: String,
    pub binds: Vec<PostgresBindValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledPostgresQuery {
    pub sql: String,
    pub binds: Vec<PostgresBindValue>,
    pub columns: Vec<CompiledQueryColumn>,
    pub many_relations: Vec<CompiledManyRelationColumn>,
    pub exact_count: Option<CompiledPostgresCount>,
    pub plan_fingerprint: QueryPlanFingerprint,
}

#[derive(Debug, Error)]
pub enum PostgresQueryBuildError {
    #[error(transparent)]
    Plan(#[from] QueryPlanError),
    #[error(transparent)]
    Cursor(#[from] CursorValidationError),
    #[error(transparent)]
    Compile(#[from] PostgresQueryCompileError),
}

#[derive(Debug, Error)]
pub enum PostgresQueryCompileError {
    #[error("cursor continuation must be decoded and validated through SchemaRegistry")]
    CursorContextRequired,
    #[error("decoded cursor context does not match query pagination")]
    CursorContextMismatch,
    #[error("many-cardinality ordering requires an explicit aggregate policy: {0:?}")]
    ManyLinkOrderingPending(FieldPath),
    #[error("aggregate ordering requires a many-cardinality path: {0:?}")]
    AggregateOrderingWithoutManyLink(FieldPath),
    #[error("aggregate ordering uses an unsupported PostgreSQL MIN/MAX type: {0:?}")]
    AggregateOrderingUnsupportedType(FieldPath),
    #[error("aggregate ordering currently requires bounded offset pagination")]
    AggregateOrderingRequiresOffsetPagination,
    #[error("query plan has no join contract for path {0:?}")]
    MissingJoinPlan(Vec<LinkName>),
    #[error("query plan many-link traversal metadata is inconsistent for path {0:?}")]
    ManyTraversalMismatch(Vec<LinkName>),
    #[error("query plan nested many-projection metadata is inconsistent")]
    ManyProjectionPlanMismatch,
    #[error("query plan relation aliases do not match the path-alias map")]
    AliasMappingMismatch,
    #[error("query plan has no typed field contract for {0:?}")]
    MissingFieldPlan(FieldPath),
    #[error("query plan contains an invalid relation alias: {0}")]
    InvalidRelationAlias(String),
    #[error("query contains an invalid scalar value for {0:?}")]
    InvalidScalarValue(FieldPath),
    #[error("offset value is outside the PostgreSQL bigint range: {0}")]
    OffsetOutOfRange(u64),
    #[error("query plan fingerprint serialization failed: {0}")]
    Fingerprint(#[from] postcard::Error),
    #[error("query JSON bind serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

impl SchemaRegistry {
    /// Validate, plan, decode any continuation cursor, and compile a PostgreSQL query.
    pub fn compile_postgres_query(
        &self,
        query: &IndexQuery,
    ) -> Result<CompiledPostgresQuery, PostgresQueryBuildError> {
        let plan = self.plan_query(query)?;
        let cursor = match &query.pagination {
            Pagination::Cursor {
                after: Some(encoded),
                ..
            } => Some(CursorCodec::decode_scoped_for_query(encoded, query, self)?),
            _ => None,
        };
        Ok(plan.compile_postgres_with_cursor(cursor.as_ref())?)
    }
}

impl ExecutableQueryPlan {
    /// Compile a plan that does not carry an opaque continuation cursor.
    ///
    /// Plans with `after` must use `SchemaRegistry::compile_postgres_query` so
    /// cursor checksum, query fingerprint, scope, schema fingerprint, arity,
    /// and value types are validated before keyset SQL is emitted.
    pub fn compile_postgres(&self) -> Result<CompiledPostgresQuery, PostgresQueryCompileError> {
        if matches!(&self.pagination, Pagination::Cursor { after: Some(_), .. }) {
            return Err(PostgresQueryCompileError::CursorContextRequired);
        }
        self.compile_postgres_with_cursor(None)
    }

    fn compile_postgres_with_cursor(
        &self,
        cursor: Option<&IndexCursor>,
    ) -> Result<CompiledPostgresQuery, PostgresQueryCompileError> {
        self.validate_compiler_contract(cursor)?;
        super::postgres_query_sql::compile_postgres_plan(self, cursor)
    }

    fn validate_compiler_contract(
        &self,
        cursor: Option<&IndexCursor>,
    ) -> Result<(), PostgresQueryCompileError> {
        validate_alias(&self.root_alias)?;
        if self.path_aliases.get(&Vec::new()).map(String::as_str) != Some(self.root_alias.as_str())
        {
            return Err(PostgresQueryCompileError::AliasMappingMismatch);
        }

        for join in &self.joins {
            validate_alias(&join.source_alias)?;
            validate_alias(&join.alias)?;
            if join.path.is_empty() {
                return Err(PostgresQueryCompileError::AliasMappingMismatch);
            }
            let parent_path = join.path[..join.path.len() - 1].to_vec();
            if self.path_aliases.get(&parent_path).map(String::as_str)
                != Some(join.source_alias.as_str())
                || self.path_aliases.get(&join.path).map(String::as_str)
                    != Some(join.alias.as_str())
            {
                return Err(PostgresQueryCompileError::AliasMappingMismatch);
            }
            let parent_traverses_many = if parent_path.is_empty() {
                false
            } else {
                self.join_for_path(&parent_path)
                    .ok_or_else(|| PostgresQueryCompileError::MissingJoinPlan(parent_path.clone()))?
                    .traverses_many
            };
            let expected_traverses_many =
                parent_traverses_many || join.cardinality == LinkCardinality::Many;
            if join.traverses_many != expected_traverses_many {
                return Err(PostgresQueryCompileError::ManyTraversalMismatch(
                    join.path.clone(),
                ));
            }
        }

        for (path, field) in &self.referenced_fields {
            validate_alias(&field.relation_alias)?;
            if path != &field.path
                || self.path_aliases.get(path.links()).map(String::as_str)
                    != Some(field.relation_alias.as_str())
            {
                return Err(PostgresQueryCompileError::AliasMappingMismatch);
            }
            let expected_traverses_many = if path.links().is_empty() {
                false
            } else {
                self.join_for_path(path.links())
                    .ok_or_else(|| {
                        PostgresQueryCompileError::MissingJoinPlan(path.links().to_vec())
                    })?
                    .traverses_many
            };
            if field.traverses_many != expected_traverses_many {
                return Err(PostgresQueryCompileError::ManyTraversalMismatch(
                    path.links().to_vec(),
                ));
            }
        }
        for field in &self.projection {
            if self.referenced_fields.get(&field.path) != Some(field) {
                return Err(PostgresQueryCompileError::MissingFieldPlan(
                    field.path.clone(),
                ));
            }
        }
        validate_many_projection_contract(self)?;
        let mut has_aggregate_order = false;
        for order in &self.order_by {
            let Some(referenced) = self.referenced_fields.get(&order.field.path) else {
                return Err(PostgresQueryCompileError::MissingFieldPlan(
                    order.field.path.clone(),
                ));
            };
            let aggregate = order.direction.aggregate();
            has_aggregate_order |= aggregate.is_some();
            let mut expected = referenced.clone();
            if aggregate.is_some() {
                expected.nullable = true;
            }
            if order.field != expected {
                return Err(PostgresQueryCompileError::MissingFieldPlan(
                    order.field.path.clone(),
                ));
            }
            match (order.field.traverses_many, aggregate) {
                (true, None) => {
                    return Err(PostgresQueryCompileError::ManyLinkOrderingPending(
                        order.field.path.clone(),
                    ));
                }
                (false, Some(_)) => {
                    return Err(PostgresQueryCompileError::AggregateOrderingWithoutManyLink(
                        order.field.path.clone(),
                    ));
                }
                (true, Some(_)) if !aggregate_type_supported(order.field.value_type) => {
                    return Err(PostgresQueryCompileError::AggregateOrderingUnsupportedType(
                        order.field.path.clone(),
                    ));
                }
                _ => {}
            }
        }
        if has_aggregate_order && !matches!(&self.pagination, Pagination::Offset { .. }) {
            return Err(PostgresQueryCompileError::AggregateOrderingRequiresOffsetPagination);
        }
        if let Some(filter) = &self.filter {
            let mut paths = Vec::new();
            filter.field_paths(&mut paths);
            if let Some(path) = paths
                .into_iter()
                .find(|path| !self.referenced_fields.contains_key(*path))
            {
                return Err(PostgresQueryCompileError::MissingFieldPlan((*path).clone()));
            }
        }

        match (&self.pagination, cursor) {
            (Pagination::Cursor { after: Some(_), .. }, None) => {
                Err(PostgresQueryCompileError::CursorContextRequired)
            }
            (Pagination::Cursor { after: Some(_), .. }, Some(_))
            | (Pagination::Cursor { after: None, .. }, None)
            | (Pagination::Offset { .. }, None) => Ok(()),
            _ => Err(PostgresQueryCompileError::CursorContextMismatch),
        }
    }
}

fn validate_many_projection_contract(
    plan: &ExecutableQueryPlan,
) -> Result<(), PostgresQueryCompileError> {
    if plan.many_projections != derive_many_projections(&plan.projection) {
        return Err(PostgresQueryCompileError::ManyProjectionPlanMismatch);
    }

    for projection in &plan.many_projections {
        if projection.path.is_empty() || projection.fields.is_empty() {
            return Err(PostgresQueryCompileError::ManyProjectionPlanMismatch);
        }
        for path in &projection.identity_paths {
            if plan.join_for_path(path).is_none() {
                return Err(PostgresQueryCompileError::MissingJoinPlan(path.clone()));
            }
        }
    }
    Ok(())
}

fn aggregate_type_supported(value_type: IndexValueType) -> bool {
    matches!(
        value_type,
        IndexValueType::Integer
            | IndexValueType::Decimal
            | IndexValueType::String
            | IndexValueType::Timestamp
    )
}

fn validate_alias(alias: &str) -> Result<(), PostgresQueryCompileError> {
    let valid = alias.strip_prefix('t').is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    });
    if valid {
        Ok(())
    } else {
        Err(PostgresQueryCompileError::InvalidRelationAlias(
            alias.to_owned(),
        ))
    }
}

pub(super) fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
