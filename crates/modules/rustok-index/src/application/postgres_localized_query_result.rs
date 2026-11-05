use std::collections::BTreeSet;

use thiserror::Error;

use crate::domain::{FieldCardinality, IndexValue, LocalizedEntityQuery, Pagination};

use super::{
    CompiledPostgresCell, CompiledPostgresLocalizedPageQuery, CompiledPostgresRow,
    CompiledQueryColumn, IndexProjectedValue, IndexQueryItem, IndexQueryPage, LocalizedCursorCodec,
    LocalizedCursorValidationError, LocalizedEntityQueryValidationError, LocalizedIndexCursor,
    PostgresQueryCompileError, PostgresQueryDecodeError, QueryPlanError, SchemaRegistry,
    SchemaRegistryError, postgres_localized_query::localized_plan_fingerprint,
};

const EXACT_COUNT_ALIAS: &str = "__exact_count";

#[derive(Debug, Error)]
pub enum PostgresLocalizedQueryDecodeError {
    #[error(transparent)]
    Validation(#[from] LocalizedEntityQueryValidationError),
    #[error(transparent)]
    Plan(#[from] QueryPlanError),
    #[error(transparent)]
    Cursor(#[from] LocalizedCursorValidationError),
    #[error(transparent)]
    Compile(#[from] PostgresQueryCompileError),
    #[error(transparent)]
    Row(#[from] PostgresQueryDecodeError),
    #[error("localized compiled query fingerprint does not match the requested fold contract")]
    LocalizedPlanFingerprintMismatch,
}

impl SchemaRegistry {
    /// Decode one localized identity-fold page after PostgreSQL execution.
    ///
    /// Localized projection fields intentionally accept SQL null even when the immutable physical
    /// field contract is non-null: null means neither requested nor fallback physical locale row was
    /// admitted. The owner adapter may then apply its documented placeholder behavior. Every unlisted
    /// field keeps the ordinary schema nullability/type/cardinality decoder contract.
    pub fn decode_postgres_localized_query_page(
        &self,
        query: &LocalizedEntityQuery,
        page_query: &CompiledPostgresLocalizedPageQuery,
        rows: Vec<CompiledPostgresRow>,
        exact_count_row: Option<CompiledPostgresRow>,
    ) -> Result<IndexQueryPage, PostgresLocalizedQueryDecodeError> {
        self.validate_localized_entity_query(query)?;
        let plan = self.plan_query(&query.query)?;
        let expected_ordinary = plan
            .fingerprint()
            .map_err(PostgresQueryCompileError::from)?;
        let compiled = page_query.compiled();
        if compiled.plan_fingerprint != expected_ordinary {
            return Err(PostgresQueryDecodeError::PlanFingerprintMismatch {
                expected: expected_ordinary,
                actual: compiled.plan_fingerprint,
            }
            .into());
        }
        let expected_localized = localized_plan_fingerprint(query, expected_ordinary)?;
        if page_query.localized_plan_fingerprint() != expected_localized {
            return Err(PostgresLocalizedQueryDecodeError::LocalizedPlanFingerprintMismatch);
        }

        let expected_columns = expected_columns(&plan);
        let unique_aliases = compiled
            .columns
            .iter()
            .map(column_output_alias)
            .collect::<BTreeSet<_>>();
        if unique_aliases.len() != compiled.columns.len()
            || compiled.columns != expected_columns
            || !compiled.many_relations.is_empty()
        {
            return Err(PostgresQueryDecodeError::ColumnContractMismatch.into());
        }

        let requested_page_size = page_size(&query.query.pagination);
        if page_query.requested_page_size() != requested_page_size {
            return Err(PostgresQueryDecodeError::PageSizeMismatch {
                compiled: page_query.requested_page_size(),
                query: requested_page_size,
            }
            .into());
        }
        let maximum = requested_page_size as usize + 1;
        if rows.len() > maximum {
            return Err(PostgresQueryDecodeError::TooManyRows {
                maximum,
                actual: rows.len(),
            }
            .into());
        }

        let exact_count = decode_exact_count(
            query,
            compiled.exact_count.is_some(),
            exact_count_row.as_ref(),
        )?;
        let has_more = rows.len() > requested_page_size as usize;
        let decoded = rows
            .iter()
            .take(requested_page_size as usize)
            .map(|row| decode_row(query, compiled, row))
            .collect::<Result<Vec<_>, _>>()?;

        let next_cursor =
            if has_more && matches!(&query.query.pagination, Pagination::Cursor { .. }) {
                let last = decoded
                    .last()
                    .ok_or(PostgresQueryDecodeError::MissingCursorItem)?;
                let registered = self.get(&query.query.schema).ok_or_else(|| {
                    QueryPlanError::Registry(SchemaRegistryError::SchemaNotFound(
                        query.query.schema.clone(),
                    ))
                })?;
                let cursor = LocalizedIndexCursor {
                    tenant_id: query.query.scope.tenant_id,
                    schema: query.query.schema.clone(),
                    schema_fingerprint: registered.fingerprint,
                    requested_locale: query
                        .requested_locale()
                        .expect("validated localized query carries requested locale")
                        .clone(),
                    fallback_locale: query.canonical_fallback_locale().cloned(),
                    order_values: last.order_values.clone(),
                    entity_id: last.item.entity_id,
                };
                Some(LocalizedCursorCodec::encode_for_query(
                    &cursor, query, self,
                )?)
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

fn expected_columns(plan: &super::ExecutableQueryPlan) -> Vec<CompiledQueryColumn> {
    let mut columns = vec![CompiledQueryColumn::EntityId {
        output_alias: "__t0_entity_id".to_owned(),
        relation_alias: "t0".to_owned(),
    }];
    columns.extend(plan.projection.iter().enumerate().map(|(index, field)| {
        CompiledQueryColumn::Field {
            output_alias: format!("f{index}"),
            field: field.clone(),
        }
    }));
    columns.extend(plan.order_by.iter().enumerate().map(|(index, order)| {
        CompiledQueryColumn::OrderValue {
            output_alias: format!("__order_{index}"),
            field: order.field.clone(),
        }
    }));
    columns
}

fn column_output_alias(column: &CompiledQueryColumn) -> &str {
    match column {
        CompiledQueryColumn::EntityId { output_alias, .. }
        | CompiledQueryColumn::Field { output_alias, .. }
        | CompiledQueryColumn::OrderValue { output_alias, .. } => output_alias,
    }
}

struct DecodedLocalizedRow {
    item: IndexQueryItem,
    order_values: Vec<IndexValue>,
}

fn decode_row(
    query: &LocalizedEntityQuery,
    compiled: &super::CompiledPostgresQuery,
    row: &CompiledPostgresRow,
) -> Result<DecodedLocalizedRow, PostgresLocalizedQueryDecodeError> {
    let root_entity_id = match required_cell(row, "__t0_entity_id")? {
        CompiledPostgresCell::Uuid(value) if !value.is_nil() => *value,
        CompiledPostgresCell::Uuid(_) | CompiledPostgresCell::Null => {
            return Err(
                PostgresQueryDecodeError::NullRootIdentity("__t0_entity_id".to_owned()).into(),
            );
        }
        _ => {
            return Err(PostgresQueryDecodeError::UnexpectedCellType {
                alias: "__t0_entity_id".to_owned(),
                expected: "a non-nil UUID",
            }
            .into());
        }
    };

    let mut fields = Vec::new();
    let mut order_values = Vec::new();
    for column in &compiled.columns {
        match column {
            CompiledQueryColumn::Field {
                output_alias,
                field,
            } => {
                let value = decode_value(row, output_alias)?;
                validate_value(
                    field,
                    &value,
                    query.is_localized_projection_path(&field.path),
                )?;
                fields.push(IndexProjectedValue {
                    path: field.path.clone(),
                    value,
                });
            }
            CompiledQueryColumn::OrderValue {
                output_alias,
                field,
            } => {
                let value = decode_value(row, output_alias)?;
                validate_value(field, &value, false)?;
                order_values.push(value);
            }
            CompiledQueryColumn::EntityId { .. } => {}
        }
    }

    Ok(DecodedLocalizedRow {
        item: IndexQueryItem {
            entity_id: root_entity_id,
            relations: Vec::new(),
            fields,
            nested_relations: Vec::new(),
        },
        order_values,
    })
}

fn decode_value(
    row: &CompiledPostgresRow,
    output_alias: &str,
) -> Result<IndexValue, PostgresLocalizedQueryDecodeError> {
    match required_cell(row, output_alias)? {
        CompiledPostgresCell::Null => Ok(IndexValue::Null),
        CompiledPostgresCell::Json(value) => serde_json::from_value::<IndexValue>(value.clone())
            .map_err(|source| {
                PostgresQueryDecodeError::InvalidTaggedValue {
                    alias: output_alias.to_owned(),
                    source,
                }
                .into()
            }),
        _ => Err(PostgresQueryDecodeError::UnexpectedCellType {
            alias: output_alias.to_owned(),
            expected: "tagged IndexValue JSON or SQL null",
        }
        .into()),
    }
}

fn validate_value(
    field: &super::PlannedField,
    value: &IndexValue,
    localized_absence_allowed: bool,
) -> Result<(), PostgresLocalizedQueryDecodeError> {
    if matches!(value, IndexValue::Null) {
        if field.nullable || localized_absence_allowed {
            return Ok(());
        }
        return Err(PostgresQueryDecodeError::UnexpectedFieldNull {
            path: field.path.clone(),
        }
        .into());
    }
    let valid = match value {
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
    };
    if valid {
        Ok(())
    } else {
        Err(PostgresQueryDecodeError::InvalidFieldValue {
            path: field.path.clone(),
        }
        .into())
    }
}

fn required_cell<'a>(
    row: &'a CompiledPostgresRow,
    output_alias: &str,
) -> Result<&'a CompiledPostgresCell, PostgresLocalizedQueryDecodeError> {
    row.get(output_alias)
        .ok_or_else(|| PostgresQueryDecodeError::MissingColumn(output_alias.to_owned()).into())
}

fn decode_exact_count(
    query: &LocalizedEntityQuery,
    compiled_has_count: bool,
    row: Option<&CompiledPostgresRow>,
) -> Result<Option<u64>, PostgresLocalizedQueryDecodeError> {
    match (query.query.include_exact_count, compiled_has_count, row) {
        (false, false, None) => Ok(None),
        (true, true, Some(row)) => match required_cell(row, EXACT_COUNT_ALIAS)? {
            CompiledPostgresCell::Integer(value) if *value >= 0 => Ok(Some(*value as u64)),
            CompiledPostgresCell::Integer(value) => {
                Err(PostgresQueryDecodeError::NegativeExactCount(*value).into())
            }
            _ => Err(PostgresQueryDecodeError::UnexpectedCellType {
                alias: EXACT_COUNT_ALIAS.to_owned(),
                expected: "a non-negative bigint",
            }
            .into()),
        },
        _ => Err(PostgresQueryDecodeError::ExactCountContractMismatch.into()),
    }
}

fn page_size(pagination: &Pagination) -> u32 {
    match pagination {
        Pagination::Cursor { first, .. } => *first,
        Pagination::Offset { limit, .. } => *limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        EntityName, FieldCardinality, FieldName, FieldPath, IndexField, IndexQuery,
        IndexQueryScope, IndexSchema, IndexValueType, LocaleKey, LocaleMode, ModuleName,
        OrderDirection, OrderExpr, Pagination, SchemaRef, SchemaVersion,
    };
    use serde_json::json;
    use uuid::Uuid;

    fn schema() -> IndexSchema {
        IndexSchema {
            reference: SchemaRef {
                module: ModuleName::new("rustok-product").unwrap(),
                entity: EntityName::new("product").unwrap(),
                version: SchemaVersion::new(4),
            },
            locale_mode: LocaleMode::Required,
            fields: vec![
                IndexField {
                    name: FieldName::new("id").unwrap(),
                    value_type: IndexValueType::Uuid,
                    cardinality: FieldCardinality::One,
                    nullable: false,
                    selectable: true,
                    filterable: true,
                    sortable: true,
                },
                IndexField {
                    name: FieldName::new("title").unwrap(),
                    value_type: IndexValueType::String,
                    cardinality: FieldCardinality::One,
                    nullable: false,
                    selectable: true,
                    filterable: true,
                    sortable: false,
                },
            ],
            links: Vec::new(),
        }
    }

    fn query(schema: &IndexSchema) -> LocalizedEntityQuery {
        LocalizedEntityQuery::new(
            IndexQuery {
                scope: IndexQueryScope {
                    tenant_id: Uuid::new_v4(),
                    locale: Some(LocaleKey::new("fi").unwrap()),
                },
                schema: schema.reference.clone(),
                fields: vec![
                    FieldPath::new(FieldName::new("id").unwrap()),
                    FieldPath::new(FieldName::new("title").unwrap()),
                ],
                filter: None,
                order_by: vec![OrderExpr {
                    field: FieldPath::new(FieldName::new("id").unwrap()),
                    direction: OrderDirection::Asc,
                }],
                pagination: Pagination::Cursor {
                    first: 1,
                    after: None,
                },
                include_exact_count: true,
            },
            Some(LocaleKey::new("en").unwrap()),
            None,
        )
        .with_localized_projection_fields([FieldPath::new(FieldName::new("title").unwrap())])
    }

    #[test]
    fn decoder_allows_absent_effective_localized_field_but_not_invariant_field() {
        let schema = schema();
        let mut registry = SchemaRegistry::new();
        registry.register(schema.clone()).unwrap();
        let query = query(&schema);
        let compiled = registry
            .compile_postgres_localized_page_query(&query)
            .unwrap();
        let id = Uuid::new_v4();
        let row = CompiledPostgresRow::from_values([
            ("__t0_entity_id".to_owned(), CompiledPostgresCell::Uuid(id)),
            (
                "f0".to_owned(),
                CompiledPostgresCell::Json(json!({"type":"uuid","value":id})),
            ),
            ("f1".to_owned(), CompiledPostgresCell::Null),
            (
                "__order_0".to_owned(),
                CompiledPostgresCell::Json(json!({"type":"uuid","value":id})),
            ),
        ]);
        let count = CompiledPostgresRow::from_values([(
            EXACT_COUNT_ALIAS.to_owned(),
            CompiledPostgresCell::Integer(1),
        )]);
        let page = registry
            .decode_postgres_localized_query_page(&query, &compiled, vec![row], Some(count))
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(matches!(page.items[0].fields[1].value, IndexValue::Null));
        assert_eq!(page.exact_count, Some(1));
    }

    #[test]
    fn lookahead_emits_dedicated_localized_cursor() {
        let schema = schema();
        let mut registry = SchemaRegistry::new();
        registry.register(schema.clone()).unwrap();
        let query = query(&schema);
        let compiled = registry
            .compile_postgres_localized_page_query(&query)
            .unwrap();
        let ids = [Uuid::new_v4(), Uuid::new_v4()];
        let rows = ids
            .iter()
            .map(|id| {
                CompiledPostgresRow::from_values([
                    ("__t0_entity_id".to_owned(), CompiledPostgresCell::Uuid(*id)),
                    (
                        "f0".to_owned(),
                        CompiledPostgresCell::Json(json!({"type":"uuid","value":id})),
                    ),
                    ("f1".to_owned(), CompiledPostgresCell::Null),
                    (
                        "__order_0".to_owned(),
                        CompiledPostgresCell::Json(json!({"type":"uuid","value":id})),
                    ),
                ])
            })
            .collect::<Vec<_>>();
        let count = CompiledPostgresRow::from_values([(
            EXACT_COUNT_ALIAS.to_owned(),
            CompiledPostgresCell::Integer(2),
        )]);
        let page = registry
            .decode_postgres_localized_query_page(&query, &compiled, rows, Some(count))
            .unwrap();
        assert!(page.has_more);
        let cursor = page.next_cursor.expect("localized lookahead cursor");
        assert!(LocalizedCursorCodec::decode_scoped_for_query(&cursor, &query, &registry).is_ok());
    }
}
