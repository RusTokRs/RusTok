use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{FieldCardinality, FieldPath, IndexQuery, IndexValue, LinkName, Pagination};

use super::{
    CompiledManyRelationColumn, CompiledPostgresQuery, CompiledQueryColumn, CursorCodec,
    CursorValidationError, ExecutableQueryPlan, IndexCursor, PlannedField, PlannedManyProjection,
    PostgresBindValue, PostgresQueryBuildError, QueryPlanError, QueryPlanFingerprint,
    SchemaRegistry, SchemaRegistryError,
};

const EXACT_COUNT_ALIAS: &str = "__exact_count";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum CompiledPostgresCell {
    Null,
    Uuid(Uuid),
    Json(JsonValue),
    Integer(i64),
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CompiledPostgresRow {
    values: BTreeMap<String, CompiledPostgresCell>,
}

impl CompiledPostgresRow {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_values(values: impl IntoIterator<Item = (String, CompiledPostgresCell)>) -> Self {
        Self {
            values: values.into_iter().collect(),
        }
    }

    pub fn insert(
        &mut self,
        output_alias: impl Into<String>,
        value: CompiledPostgresCell,
    ) -> Option<CompiledPostgresCell> {
        self.values.insert(output_alias.into(), value)
    }

    pub fn get(&self, output_alias: &str) -> Option<&CompiledPostgresCell> {
        self.values.get(output_alias)
    }
}

/// Opaque page-execution contract produced only by `SchemaRegistry`.
///
/// The wrapper deliberately has no serde implementation so untrusted bytes cannot
/// replace controlled SQL, bind values, or the requested page size before execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledPostgresPageQuery {
    compiled: CompiledPostgresQuery,
    requested_page_size: u32,
}

impl CompiledPostgresPageQuery {
    pub fn compiled(&self) -> &CompiledPostgresQuery {
        &self.compiled
    }

    pub fn requested_page_size(&self) -> u32 {
        self.requested_page_size
    }

    pub fn into_compiled(self) -> CompiledPostgresQuery {
        self.compiled
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexProjectedValue {
    pub path: FieldPath,
    pub value: IndexValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexRelationIdentity {
    pub path: Vec<LinkName>,
    pub entity_id: Option<Uuid>,
}

/// One row reached through a projection path that crosses a many-cardinality link.
/// Field values remain aligned with the complete relation identity chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexNestedRelationItem {
    pub relations: Vec<IndexRelationIdentity>,
    pub fields: Vec<IndexProjectedValue>,
}

/// Deterministic nested projection for all requested fields sharing one terminal
/// many-traversing relation path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexNestedRelationProjection {
    pub path: Vec<LinkName>,
    pub items: Vec<IndexNestedRelationItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexQueryItem {
    pub entity_id: Uuid,
    pub relations: Vec<IndexRelationIdentity>,
    pub fields: Vec<IndexProjectedValue>,
    pub nested_relations: Vec<IndexNestedRelationProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexQueryPage {
    pub items: Vec<IndexQueryItem>,
    pub exact_count: Option<u64>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Error)]
pub enum PostgresQueryPageBuildError {
    #[error(transparent)]
    Query(#[from] PostgresQueryBuildError),
    #[error("compiled query pagination bind is missing")]
    PaginationBindMissing,
    #[error("compiled query pagination binds do not match the validated query")]
    PaginationBindMismatch,
}

#[derive(Debug, Error)]
pub enum PostgresQueryDecodeError {
    #[error(transparent)]
    Plan(#[from] QueryPlanError),
    #[error(transparent)]
    Cursor(#[from] CursorValidationError),
    #[error("query plan fingerprint serialization failed: {0}")]
    Fingerprint(#[from] postcard::Error),
    #[error("compiled query plan {actual} does not match expected plan {expected}")]
    PlanFingerprintMismatch {
        expected: QueryPlanFingerprint,
        actual: QueryPlanFingerprint,
    },
    #[error("compiled query column contract does not match the executable plan")]
    ColumnContractMismatch,
    #[error("compiled query exact-count contract does not match the query")]
    ExactCountContractMismatch,
    #[error("compiled page requested {compiled} rows but query requests {query}")]
    PageSizeMismatch { compiled: u32, query: u32 },
    #[error("compiled page returned {actual} rows; maximum lookahead size is {maximum}")]
    TooManyRows { maximum: usize, actual: usize },
    #[error("compiled row is missing output column {0}")]
    MissingColumn(String),
    #[error("compiled row column {alias} must contain {expected}")]
    UnexpectedCellType {
        alias: String,
        expected: &'static str,
    },
    #[error("root entity identity column {0} is null")]
    NullRootIdentity(String),
    #[error("compiled column references unknown relation alias {0}")]
    UnknownRelationAlias(String),
    #[error("lookahead cursor construction has no retained item")]
    MissingCursorItem,
    #[error("projected field {path:?} unexpectedly decoded as null")]
    UnexpectedFieldNull { path: FieldPath },
    #[error("projected field {path:?} contains an invalid value contract")]
    InvalidFieldValue { path: FieldPath },
    #[error("compiled row column {alias} contains invalid tagged IndexValue JSON")]
    InvalidTaggedValue {
        alias: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("compiled row column {alias} contains invalid nested relation JSON")]
    InvalidNestedRelation {
        alias: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("nested relation column {alias} has identity arity {actual}; expected {expected}")]
    NestedIdentityArity {
        alias: String,
        expected: usize,
        actual: usize,
    },
    #[error("nested relation column {alias} has field arity {actual}; expected {expected}")]
    NestedFieldArity {
        alias: String,
        expected: usize,
        actual: usize,
    },
    #[error("nested relation column {alias} contains a nil entity identity")]
    NilNestedIdentity { alias: String },
    #[error("nested relation column {alias} contains a duplicate identity chain")]
    DuplicateNestedIdentity { alias: String },
    #[error("exact-count value is negative: {0}")]
    NegativeExactCount(i64),
}

impl SchemaRegistry {
    /// Compile an execution-page query with one extra lookahead row.
    ///
    /// The underlying controlled SQL remains unchanged; only the validated page-limit
    /// bind is increased by one. Result decoding then truncates to the requested size
    /// and emits a continuation cursor only when the extra row exists.
    pub fn compile_postgres_page_query(
        &self,
        query: &IndexQuery,
    ) -> Result<CompiledPostgresPageQuery, PostgresQueryPageBuildError> {
        let mut compiled = self.compile_postgres_query(query)?;
        let requested_page_size = page_size(&query.pagination);
        apply_lookahead_bind(&mut compiled, &query.pagination)?;
        Ok(CompiledPostgresPageQuery {
            compiled,
            requested_page_size,
        })
    }

    pub fn decode_postgres_query_page(
        &self,
        query: &IndexQuery,
        page_query: &CompiledPostgresPageQuery,
        rows: Vec<CompiledPostgresRow>,
        exact_count_row: Option<CompiledPostgresRow>,
    ) -> Result<IndexQueryPage, PostgresQueryDecodeError> {
        let plan = self.plan_query(query)?;
        let expected_fingerprint = plan.fingerprint()?;
        let compiled = page_query.compiled();
        if compiled.plan_fingerprint != expected_fingerprint {
            return Err(PostgresQueryDecodeError::PlanFingerprintMismatch {
                expected: expected_fingerprint,
                actual: compiled.plan_fingerprint,
            });
        }
        let unique_aliases = compiled
            .columns
            .iter()
            .map(column_output_alias)
            .chain(
                compiled
                    .many_relations
                    .iter()
                    .map(|column| column.output_alias.as_str()),
            )
            .collect::<BTreeSet<_>>();
        if unique_aliases.len() != compiled.columns.len() + compiled.many_relations.len()
            || compiled.columns != expected_columns(&plan)
            || compiled.many_relations != expected_many_relations(&plan)
        {
            return Err(PostgresQueryDecodeError::ColumnContractMismatch);
        }

        let requested_page_size = page_size(&query.pagination);
        if page_query.requested_page_size() != requested_page_size {
            return Err(PostgresQueryDecodeError::PageSizeMismatch {
                compiled: page_query.requested_page_size(),
                query: requested_page_size,
            });
        }
        let maximum = requested_page_size as usize + 1;
        if rows.len() > maximum {
            return Err(PostgresQueryDecodeError::TooManyRows {
                maximum,
                actual: rows.len(),
            });
        }

        let exact_count = decode_exact_count(query, compiled, exact_count_row.as_ref())?;
        let has_more = rows.len() > requested_page_size as usize;
        let decoded = rows
            .iter()
            .take(requested_page_size as usize)
            .map(|row| decode_row(&plan, compiled, row))
            .collect::<Result<Vec<_>, _>>()?;

        let next_cursor = if has_more && matches!(&query.pagination, Pagination::Cursor { .. }) {
            let last = decoded
                .last()
                .ok_or(PostgresQueryDecodeError::MissingCursorItem)?;
            let registered = self.get(&query.schema).ok_or_else(|| {
                QueryPlanError::Registry(SchemaRegistryError::SchemaNotFound(query.schema.clone()))
            })?;
            let cursor = IndexCursor {
                tenant_id: query.scope.tenant_id,
                schema: query.schema.clone(),
                schema_fingerprint: registered.fingerprint,
                locale: query.scope.locale.clone(),
                order_values: last.order_values.clone(),
                entity_id: last.item.entity_id,
            };
            Some(CursorCodec::encode_for_query(&cursor, query, self)?)
        } else {
            None
        };

        Ok(IndexQueryPage {
            items: decoded.into_iter().map(|row| row.item).collect(),
            exact_count,
            has_more,
            next_cursor,
        })
    }
}

fn apply_lookahead_bind(
    compiled: &mut CompiledPostgresQuery,
    pagination: &Pagination,
) -> Result<(), PostgresQueryPageBuildError> {
    let length = compiled.binds.len();
    let (limit_index, expected_limit) = match pagination {
        Pagination::Cursor { first, .. } => (
            length
                .checked_sub(1)
                .ok_or(PostgresQueryPageBuildError::PaginationBindMissing)?,
            i64::from(*first),
        ),
        Pagination::Offset { limit, offset } => {
            let limit_index = length
                .checked_sub(2)
                .ok_or(PostgresQueryPageBuildError::PaginationBindMissing)?;
            let offset_index = length
                .checked_sub(1)
                .ok_or(PostgresQueryPageBuildError::PaginationBindMissing)?;
            let expected_offset = i64::try_from(*offset)
                .map_err(|_| PostgresQueryPageBuildError::PaginationBindMismatch)?;
            if compiled.binds.get(offset_index)
                != Some(&PostgresBindValue::Integer(expected_offset))
            {
                return Err(PostgresQueryPageBuildError::PaginationBindMismatch);
            }
            (limit_index, i64::from(*limit))
        }
    };

    match compiled.binds.get_mut(limit_index) {
        Some(PostgresBindValue::Integer(value)) if *value == expected_limit => {
            *value = expected_limit + 1;
            Ok(())
        }
        Some(_) => Err(PostgresQueryPageBuildError::PaginationBindMismatch),
        None => Err(PostgresQueryPageBuildError::PaginationBindMissing),
    }
}

fn page_size(pagination: &Pagination) -> u32 {
    match pagination {
        Pagination::Cursor { first, .. } => *first,
        Pagination::Offset { limit, .. } => *limit,
    }
}

fn expected_columns(plan: &ExecutableQueryPlan) -> Vec<CompiledQueryColumn> {
    let mut columns = Vec::new();
    columns.push(CompiledQueryColumn::EntityId {
        output_alias: identity_alias(&plan.root_alias),
        relation_alias: plan.root_alias.clone(),
    });
    columns.extend(
        plan.outer_joins()
            .map(|join| CompiledQueryColumn::EntityId {
                output_alias: identity_alias(&join.alias),
                relation_alias: join.alias.clone(),
            }),
    );
    columns.extend(
        plan.projection
            .iter()
            .enumerate()
            .filter(|(_, field)| !field.traverses_many)
            .map(|(index, field)| CompiledQueryColumn::Field {
                output_alias: format!("f{index}"),
                field: field.clone(),
            }),
    );
    columns.extend(plan.order_by.iter().enumerate().map(|(index, order)| {
        CompiledQueryColumn::OrderValue {
            output_alias: format!("__order_{index}"),
            field: order.field.clone(),
        }
    }));
    columns
}

fn expected_many_relations(plan: &ExecutableQueryPlan) -> Vec<CompiledManyRelationColumn> {
    plan.many_projections
        .iter()
        .enumerate()
        .map(|(index, projection)| CompiledManyRelationColumn {
            output_alias: format!("__many_{index}"),
            projection: projection.clone(),
        })
        .collect()
}

fn column_output_alias(column: &CompiledQueryColumn) -> &str {
    match column {
        CompiledQueryColumn::EntityId { output_alias, .. }
        | CompiledQueryColumn::Field { output_alias, .. }
        | CompiledQueryColumn::OrderValue { output_alias, .. } => output_alias,
    }
}

fn identity_alias(relation_alias: &str) -> String {
    format!("__{relation_alias}_entity_id")
}

fn join_is_projected(plan: &ExecutableQueryPlan, join_path: &[LinkName]) -> bool {
    plan.projection
        .iter()
        .any(|field| !field.traverses_many && field.path.links().starts_with(join_path))
}

struct DecodedRow {
    item: IndexQueryItem,
    order_values: Vec<IndexValue>,
}

fn decode_row(
    plan: &ExecutableQueryPlan,
    compiled: &CompiledPostgresQuery,
    row: &CompiledPostgresRow,
) -> Result<DecodedRow, PostgresQueryDecodeError> {
    let mut root_entity_id = None;
    let mut identities = BTreeMap::new();
    let mut relations = Vec::new();

    for column in &compiled.columns {
        let CompiledQueryColumn::EntityId {
            output_alias,
            relation_alias,
        } = column
        else {
            continue;
        };
        let identity = decode_identity(row, output_alias, relation_alias == &plan.root_alias)?;
        identities.insert(relation_alias.clone(), identity);
        if relation_alias == &plan.root_alias {
            root_entity_id = identity;
        } else {
            let join = plan
                .joins
                .iter()
                .find(|join| join.alias == *relation_alias)
                .ok_or_else(|| {
                    PostgresQueryDecodeError::UnknownRelationAlias(relation_alias.clone())
                })?;
            if join_is_projected(plan, &join.path) {
                relations.push(IndexRelationIdentity {
                    path: join.path.clone(),
                    entity_id: identity,
                });
            }
        }
    }

    let root_entity_id = root_entity_id
        .ok_or_else(|| PostgresQueryDecodeError::MissingColumn(identity_alias(&plan.root_alias)))?;
    let mut fields = Vec::with_capacity(plan.outer_projection().count());
    let mut order_values = Vec::with_capacity(plan.order_by.len());

    for column in &compiled.columns {
        match column {
            CompiledQueryColumn::Field {
                output_alias,
                field,
            } => fields.push(IndexProjectedValue {
                path: field.path.clone(),
                value: decode_field(row, output_alias, field, &identities)?,
            }),
            CompiledQueryColumn::OrderValue {
                output_alias,
                field,
            } => order_values.push(decode_field(row, output_alias, field, &identities)?),
            CompiledQueryColumn::EntityId { .. } => {}
        }
    }
    let nested_relations = compiled
        .many_relations
        .iter()
        .map(|column| decode_nested_relation(row, &column.output_alias, &column.projection))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DecodedRow {
        item: IndexQueryItem {
            entity_id: root_entity_id,
            relations,
            fields,
            nested_relations,
        },
        order_values,
    })
}

fn decode_identity(
    row: &CompiledPostgresRow,
    output_alias: &str,
    is_root: bool,
) -> Result<Option<Uuid>, PostgresQueryDecodeError> {
    match required_cell(row, output_alias)? {
        CompiledPostgresCell::Uuid(value) => Ok(Some(*value)),
        CompiledPostgresCell::Null if is_root => Err(PostgresQueryDecodeError::NullRootIdentity(
            output_alias.to_owned(),
        )),
        CompiledPostgresCell::Null => Ok(None),
        _ => Err(PostgresQueryDecodeError::UnexpectedCellType {
            alias: output_alias.to_owned(),
            expected: "a UUID or SQL null",
        }),
    }
}

fn decode_field(
    row: &CompiledPostgresRow,
    output_alias: &str,
    field: &PlannedField,
    identities: &BTreeMap<String, Option<Uuid>>,
) -> Result<IndexValue, PostgresQueryDecodeError> {
    let missing_relation = identities
        .get(&field.relation_alias)
        .is_some_and(Option::is_none);
    let value = match required_cell(row, output_alias)? {
        CompiledPostgresCell::Null => IndexValue::Null,
        CompiledPostgresCell::Json(value) => decode_tagged_value(value, output_alias)?,
        _ => {
            return Err(PostgresQueryDecodeError::UnexpectedCellType {
                alias: output_alias.to_owned(),
                expected: "tagged IndexValue JSON or SQL null",
            });
        }
    };

    validate_projected_value(field, &value, missing_relation)?;
    Ok(value)
}

#[derive(Debug, Deserialize)]
struct CompiledNestedRelationItem {
    entity_ids: Vec<Uuid>,
    values: Vec<JsonValue>,
}

fn decode_nested_relation(
    row: &CompiledPostgresRow,
    output_alias: &str,
    projection: &PlannedManyProjection,
) -> Result<IndexNestedRelationProjection, PostgresQueryDecodeError> {
    let payload = match required_cell(row, output_alias)? {
        CompiledPostgresCell::Json(value) => value.clone(),
        _ => {
            return Err(PostgresQueryDecodeError::UnexpectedCellType {
                alias: output_alias.to_owned(),
                expected: "a nested relation JSON array",
            });
        }
    };
    let wire_items =
        serde_json::from_value::<Vec<CompiledNestedRelationItem>>(payload).map_err(|source| {
            PostgresQueryDecodeError::InvalidNestedRelation {
                alias: output_alias.to_owned(),
                source,
            }
        })?;
    let mut identity_chains = BTreeSet::new();
    let mut items = Vec::with_capacity(wire_items.len());

    for wire in wire_items {
        if wire.entity_ids.len() != projection.identity_paths.len() {
            return Err(PostgresQueryDecodeError::NestedIdentityArity {
                alias: output_alias.to_owned(),
                expected: projection.identity_paths.len(),
                actual: wire.entity_ids.len(),
            });
        }
        if wire.values.len() != projection.fields.len() {
            return Err(PostgresQueryDecodeError::NestedFieldArity {
                alias: output_alias.to_owned(),
                expected: projection.fields.len(),
                actual: wire.values.len(),
            });
        }
        if wire.entity_ids.iter().any(Uuid::is_nil) {
            return Err(PostgresQueryDecodeError::NilNestedIdentity {
                alias: output_alias.to_owned(),
            });
        }
        if !identity_chains.insert(wire.entity_ids.clone()) {
            return Err(PostgresQueryDecodeError::DuplicateNestedIdentity {
                alias: output_alias.to_owned(),
            });
        }

        let relations = projection
            .identity_paths
            .iter()
            .cloned()
            .zip(wire.entity_ids)
            .map(|(path, entity_id)| IndexRelationIdentity {
                path,
                entity_id: Some(entity_id),
            })
            .collect();
        let fields = projection
            .fields
            .iter()
            .zip(wire.values.iter())
            .map(|(field, value)| {
                let decoded = decode_tagged_value(value, output_alias)?;
                validate_projected_value(field, &decoded, false)?;
                Ok(IndexProjectedValue {
                    path: field.path.clone(),
                    value: decoded,
                })
            })
            .collect::<Result<Vec<_>, PostgresQueryDecodeError>>()?;
        items.push(IndexNestedRelationItem { relations, fields });
    }

    Ok(IndexNestedRelationProjection {
        path: projection.path.clone(),
        items,
    })
}

fn decode_tagged_value(
    value: &JsonValue,
    output_alias: &str,
) -> Result<IndexValue, PostgresQueryDecodeError> {
    serde_json::from_value(value.clone()).map_err(|source| {
        PostgresQueryDecodeError::InvalidTaggedValue {
            alias: output_alias.to_owned(),
            source,
        }
    })
}

fn validate_projected_value(
    field: &PlannedField,
    value: &IndexValue,
    missing_relation: bool,
) -> Result<(), PostgresQueryDecodeError> {
    if missing_relation && !matches!(value, IndexValue::Null) {
        return Err(PostgresQueryDecodeError::InvalidFieldValue {
            path: field.path.clone(),
        });
    }
    if matches!(value, IndexValue::Null) && !field.nullable && !missing_relation {
        return Err(PostgresQueryDecodeError::UnexpectedFieldNull {
            path: field.path.clone(),
        });
    }
    if !valid_field_value(field, value, missing_relation) {
        return Err(PostgresQueryDecodeError::InvalidFieldValue {
            path: field.path.clone(),
        });
    }
    Ok(())
}

fn valid_field_value(field: &PlannedField, value: &IndexValue, missing_relation: bool) -> bool {
    match value {
        IndexValue::Null => field.nullable || missing_relation,
        IndexValue::List(values) => {
            field.cardinality == FieldCardinality::Many
                && values
                    .iter()
                    .all(|value| value.value_type() == Some(field.value_type))
        }
        value => {
            field.cardinality == FieldCardinality::One
                && value.value_type() == Some(field.value_type)
        }
    }
}

fn required_cell<'a>(
    row: &'a CompiledPostgresRow,
    output_alias: &str,
) -> Result<&'a CompiledPostgresCell, PostgresQueryDecodeError> {
    row.get(output_alias)
        .ok_or_else(|| PostgresQueryDecodeError::MissingColumn(output_alias.to_owned()))
}

fn decode_exact_count(
    query: &IndexQuery,
    compiled: &CompiledPostgresQuery,
    row: Option<&CompiledPostgresRow>,
) -> Result<Option<u64>, PostgresQueryDecodeError> {
    match (
        query.include_exact_count,
        compiled.exact_count.is_some(),
        row,
    ) {
        (false, false, None) => Ok(None),
        (true, true, Some(row)) => match required_cell(row, EXACT_COUNT_ALIAS)? {
            CompiledPostgresCell::Integer(value) if *value >= 0 => Ok(Some(*value as u64)),
            CompiledPostgresCell::Integer(value) => {
                Err(PostgresQueryDecodeError::NegativeExactCount(*value))
            }
            _ => Err(PostgresQueryDecodeError::UnexpectedCellType {
                alias: EXACT_COUNT_ALIAS.to_owned(),
                expected: "a non-negative bigint",
            }),
        },
        _ => Err(PostgresQueryDecodeError::ExactCountContractMismatch),
    }
}
