use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{
    FieldCardinality, FieldPath, IndexQuery, IndexValue, LinkName, Pagination,
};

use super::{
    CompiledPostgresQuery, CompiledQueryColumn, CursorCodec, CursorValidationError,
    ExecutableQueryPlan, IndexCursor, PlannedField, PostgresBindValue, PostgresQueryBuildError,
    QueryPlanError, QueryPlanFingerprint, SchemaRegistry,
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

    pub fn from_values(
        values: impl IntoIterator<Item = (String, CompiledPostgresCell)>,
    ) -> Self {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexQueryItem {
    pub entity_id: Uuid,
    pub relations: Vec<IndexRelationIdentity>,
    pub fields: Vec<IndexProjectedValue>,
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
        if compiled.columns != expected_columns(&plan) {
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
        let mut decoded = rows
            .iter()
            .take(requested_page_size as usize)
            .map(|row| decode_row(&plan, compiled, row))
            .collect::<Result<Vec<_>, _>>()?;

        let next_cursor = if has_more && matches!(query.pagination, Pagination::Cursor { .. }) {
            let last = decoded
                .last()
                .expect("lookahead requires at least one retained result row");
            let registered = self
                .get(&query.schema)
                .expect("validated query schema must remain registered");
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
            items: decoded.drain(..).map(|row| row.item).collect(),
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
            if compiled.binds.get(offset_index)
                != Some(&PostgresBindValue::Integer(*offset as i64))
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
    columns.extend(plan.joins.iter().map(|join| CompiledQueryColumn::EntityId {
        output_alias: identity_alias(&join.alias),
        relation_alias: join.alias.clone(),
    }));
    columns.extend(
        plan.projection
            .iter()
            .enumerate()
            .map(|(index, field)| CompiledQueryColumn::Field {
                output_alias: format!("f{index}"),
                field: field.clone(),
            }),
    );
    columns.extend(
        plan.order_by
            .iter()
            .enumerate()
            .map(|(index, order)| CompiledQueryColumn::OrderValue {
                output_alias: format!("__order_{index}"),
                field: order.field.clone(),
            }),
    );
    columns
}

fn identity_alias(relation_alias: &str) -> String {
    format!("__{relation_alias}_entity_id")
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
                .expect("validated column contract must resolve every join alias");
            relations.push(IndexRelationIdentity {
                path: join.path.clone(),
                entity_id: identity,
            });
        }
    }

    let root_entity_id = root_entity_id.ok_or_else(|| {
        PostgresQueryDecodeError::MissingColumn(identity_alias(&plan.root_alias))
    })?;
    let mut fields = Vec::with_capacity(plan.projection.len());
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

    Ok(DecodedRow {
        item: IndexQueryItem {
            entity_id: root_entity_id,
            relations,
            fields,
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
        CompiledPostgresCell::Json(value) => serde_json::from_value(value.clone()).map_err(
            |source| PostgresQueryDecodeError::InvalidTaggedValue {
                alias: output_alias.to_owned(),
                source,
            },
        )?,
        _ => {
            return Err(PostgresQueryDecodeError::UnexpectedCellType {
                alias: output_alias.to_owned(),
                expected: "tagged IndexValue JSON or SQL null",
            });
        }
    };

    if matches!(value, IndexValue::Null) && !field.nullable && !missing_relation {
        return Err(PostgresQueryDecodeError::UnexpectedFieldNull {
            path: field.path.clone(),
        });
    }
    if !valid_field_value(field, &value, missing_relation) {
        return Err(PostgresQueryDecodeError::InvalidFieldValue {
            path: field.path.clone(),
        });
    }
    Ok(value)
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
    match (query.include_exact_count, compiled.exact_count.is_some(), row) {
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
