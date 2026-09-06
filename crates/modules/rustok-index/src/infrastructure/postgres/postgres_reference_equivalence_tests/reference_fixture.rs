use std::{cmp::Ordering, collections::BTreeMap};

use crate::{
    CursorCodec, EntityKey, ExecutableQueryPlan, FieldPath, FilterExpr, IndexNestedRelationItem,
    IndexNestedRelationProjection, IndexProjectedValue, IndexQuery, IndexQueryItem, IndexQueryPage,
    IndexRecord, IndexRelationIdentity, IndexValue, LinkName, OrderDirection, Pagination,
    PlannedField, SchemaRegistry,
};

pub(super) struct ReferenceFixture<'a> {
    registry: &'a SchemaRegistry,
    records: BTreeMap<EntityKey, IndexRecord>,
}

impl<'a> ReferenceFixture<'a> {
    pub(super) fn new(registry: &'a SchemaRegistry, records: &[IndexRecord]) -> Self {
        Self {
            registry,
            records: records
                .iter()
                .cloned()
                .map(|record| (record.key.clone(), record))
                .collect(),
        }
    }

    pub(super) fn page(&self, query: &IndexQuery) -> IndexQueryPage {
        self.registry
            .validate_query(query)
            .expect("equivalence query should validate");
        let plan = self
            .registry
            .plan_query(query)
            .expect("equivalence query should plan");

        let mut records = self
            .records
            .values()
            .filter(|record| record.key.schema == query.schema)
            .filter(|record| record.key.tenant_id == query.scope.tenant_id)
            .filter(|record| record.key.locale == query.scope.locale)
            .filter(|record| {
                query
                    .filter
                    .as_ref()
                    .is_none_or(|filter| self.matches_filter(record, filter))
            })
            .collect::<Vec<_>>();
        records.sort_by(|left, right| self.compare_records(left, right, query));
        let exact_count = query.include_exact_count.then_some(records.len() as u64);

        if let Pagination::Cursor {
            after: Some(encoded),
            ..
        } = &query.pagination
        {
            let cursor = CursorCodec::decode_scoped_for_query(encoded, query, self.registry)
                .expect("production cursor should decode in the reference fixture");
            records.retain(|record| {
                self.compare_record_to_cursor(record, &cursor, query)
                    .is_gt()
            });
        }

        let (page_size, offset) = match &query.pagination {
            Pagination::Cursor { first, .. } => (*first as usize, 0),
            Pagination::Offset { limit, offset } => (*limit as usize, *offset as usize),
        };
        let mut window = records
            .into_iter()
            .skip(offset)
            .take(page_size + 1)
            .collect::<Vec<_>>();
        let has_more = window.len() > page_size;
        window.truncate(page_size);

        let items = window
            .iter()
            .map(|record| self.project_item(&plan, record))
            .collect::<Vec<_>>();
        let next_cursor = if has_more && matches!(&query.pagination, Pagination::Cursor { .. }) {
            window.last().map(|record| {
                CursorCodec::encode_for_query(&self.cursor_for(record, query), query, self.registry)
                    .expect("reference cursor should encode")
            })
        } else {
            None
        };

        IndexQueryPage {
            items,
            exact_count,
            has_more,
            next_cursor,
        }
    }

    fn project_item(&self, plan: &ExecutableQueryPlan, root: &IndexRecord) -> IndexQueryItem {
        let relations = plan
            .outer_joins()
            .filter(|join| {
                plan.projection.iter().any(|field| {
                    !field.traverses_many && field.path.links().starts_with(&join.path)
                })
            })
            .map(|join| IndexRelationIdentity {
                path: join.path.clone(),
                entity_id: self
                    .relation_chains(root, &join.path)
                    .first()
                    .and_then(|chain| chain.last())
                    .map(|record| record.key.entity_id),
            })
            .collect();
        let fields = plan
            .outer_projection()
            .map(|field| IndexProjectedValue {
                path: field.path.clone(),
                value: self.projected_value(root, field),
            })
            .collect();
        let nested_relations = plan
            .many_projections
            .iter()
            .map(|projection| {
                let items = self
                    .relation_chains(root, &projection.path)
                    .into_iter()
                    .map(|chain| {
                        let terminal = chain
                            .last()
                            .expect("many projection path should have a terminal record");
                        IndexNestedRelationItem {
                            relations: projection
                                .identity_paths
                                .iter()
                                .cloned()
                                .zip(chain.iter())
                                .map(|(path, record)| IndexRelationIdentity {
                                    path,
                                    entity_id: Some(record.key.entity_id),
                                })
                                .collect(),
                            fields: projection
                                .fields
                                .iter()
                                .map(|field| IndexProjectedValue {
                                    path: field.path.clone(),
                                    value: terminal
                                        .fields
                                        .get(field.path.field())
                                        .cloned()
                                        .unwrap_or(IndexValue::Null),
                                })
                                .collect(),
                        }
                    })
                    .collect();
                IndexNestedRelationProjection {
                    path: projection.path.clone(),
                    items,
                }
            })
            .collect();

        IndexQueryItem {
            entity_id: root.key.entity_id,
            relations,
            fields,
            nested_relations,
        }
    }

    fn projected_value(&self, root: &IndexRecord, field: &PlannedField) -> IndexValue {
        if field.path.links().is_empty() {
            return root
                .fields
                .get(field.path.field())
                .cloned()
                .unwrap_or(IndexValue::Null);
        }
        self.relation_chains(root, field.path.links())
            .first()
            .and_then(|chain| chain.last())
            .and_then(|record| record.fields.get(field.path.field()))
            .cloned()
            .unwrap_or(IndexValue::Null)
    }

    fn relation_chains<'b>(
        &'b self,
        root: &'b IndexRecord,
        path: &[LinkName],
    ) -> Vec<Vec<&'b IndexRecord>> {
        let mut chains = vec![(root, Vec::<&IndexRecord>::new())];
        for link_name in path {
            let mut next = Vec::new();
            for (source, chain) in chains {
                let Some(link) = source.links.iter().find(|link| link.name == *link_name) else {
                    continue;
                };
                for target in &link.targets {
                    let key = EntityKey {
                        tenant_id: source.key.tenant_id,
                        schema: target.schema.clone(),
                        entity_id: target.entity_id,
                        locale: target.locale.clone(),
                    };
                    if let Some(record) = self.records.get(&key) {
                        let mut child_chain = chain.clone();
                        child_chain.push(record);
                        next.push((record, child_chain));
                    }
                }
            }
            chains = next;
        }
        chains.into_iter().map(|(_, chain)| chain).collect()
    }

    fn values_for_path<'b>(
        &'b self,
        root: &'b IndexRecord,
        path: &FieldPath,
    ) -> Vec<&'b IndexValue> {
        if path.links().is_empty() {
            return root.fields.get(path.field()).into_iter().collect();
        }
        self.relation_chains(root, path.links())
            .into_iter()
            .filter_map(|chain| {
                chain
                    .last()
                    .and_then(|record| record.fields.get(path.field()))
            })
            .collect()
    }

    fn matches_filter(&self, record: &IndexRecord, filter: &FilterExpr) -> bool {
        match filter {
            FilterExpr::And(children) => children
                .iter()
                .all(|child| self.matches_filter(record, child)),
            FilterExpr::Or(children) => children
                .iter()
                .any(|child| self.matches_filter(record, child)),
            FilterExpr::Not(child) => !self.matches_filter(record, child),
            FilterExpr::Eq(path, expected) => self
                .values_for_path(record, path)
                .into_iter()
                .any(|value| value == expected),
            FilterExpr::Ne(path, expected) => {
                let values = self.values_for_path(record, path);
                !values.is_empty()
                    && values
                        .into_iter()
                        .all(|value| !matches!(value, IndexValue::Null) && value != expected)
            }
            FilterExpr::In(path, expected) => self
                .values_for_path(record, path)
                .into_iter()
                .any(|value| expected.contains(value)),
            FilterExpr::Gt(path, expected) => {
                self.matches_ordered(record, path, expected, Ordering::is_gt)
            }
            FilterExpr::Gte(path, expected) => {
                self.matches_ordered(record, path, expected, |ordering| {
                    ordering.is_gt() || ordering.is_eq()
                })
            }
            FilterExpr::Lt(path, expected) => {
                self.matches_ordered(record, path, expected, Ordering::is_lt)
            }
            FilterExpr::Lte(path, expected) => {
                self.matches_ordered(record, path, expected, |ordering| {
                    ordering.is_lt() || ordering.is_eq()
                })
            }
            FilterExpr::Contains(path, expected) => self
                .values_for_path(record, path)
                .into_iter()
                .any(|value| match value {
                    IndexValue::List(values) => values.contains(expected),
                    _ => false,
                }),
            FilterExpr::IsNull(path, expected_null) => {
                let values = self.values_for_path(record, path);
                let is_null = values.is_empty()
                    || values
                        .into_iter()
                        .all(|value| matches!(value, IndexValue::Null));
                is_null == *expected_null
            }
            FilterExpr::TextLike(path, pattern) => self
                .values_for_path(record, path)
                .into_iter()
                .any(|value| match value {
                    IndexValue::String(value) => text_like_matches(value, pattern),
                    _ => false,
                }),
        }
    }

    fn matches_ordered(
        &self,
        record: &IndexRecord,
        path: &FieldPath,
        expected: &IndexValue,
        predicate: impl Fn(Ordering) -> bool,
    ) -> bool {
        self.values_for_path(record, path)
            .into_iter()
            .filter_map(|value| compare_values(value, expected))
            .any(predicate)
    }

    fn compare_records(
        &self,
        left: &IndexRecord,
        right: &IndexRecord,
        query: &IndexQuery,
    ) -> Ordering {
        for order in &query.order_by {
            let comparison = compare_optional_values(
                self.values_for_path(left, &order.field).into_iter().next(),
                self.values_for_path(right, &order.field).into_iter().next(),
            );
            if comparison != Ordering::Equal {
                return apply_direction(comparison, order.direction);
            }
        }
        left.key.entity_id.cmp(&right.key.entity_id)
    }

    fn compare_record_to_cursor(
        &self,
        record: &IndexRecord,
        cursor: &crate::IndexCursor,
        query: &IndexQuery,
    ) -> Ordering {
        for (order, cursor_value) in query.order_by.iter().zip(&cursor.order_values) {
            let comparison = compare_optional_to_cursor(
                self.values_for_path(record, &order.field)
                    .into_iter()
                    .next(),
                cursor_value,
            );
            if comparison != Ordering::Equal {
                return apply_direction(comparison, order.direction);
            }
        }
        record.key.entity_id.cmp(&cursor.entity_id)
    }

    fn cursor_for(&self, record: &IndexRecord, query: &IndexQuery) -> crate::IndexCursor {
        let schema = self
            .registry
            .get(&query.schema)
            .expect("validated query schema should remain registered");
        crate::IndexCursor {
            tenant_id: query.scope.tenant_id,
            schema: query.schema.clone(),
            schema_fingerprint: schema.fingerprint,
            locale: query.scope.locale.clone(),
            order_values: query
                .order_by
                .iter()
                .map(|order| {
                    self.values_for_path(record, &order.field)
                        .into_iter()
                        .next()
                        .cloned()
                        .unwrap_or(IndexValue::Null)
                })
                .collect(),
            entity_id: record.key.entity_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextLikeToken {
    AnyMany,
    AnyOne,
    Literal(char),
}

fn text_like_matches(value: &str, pattern: &str) -> bool {
    let mut tokens = Vec::new();
    let mut characters = pattern.chars();
    while let Some(character) = characters.next() {
        match character {
            '%' => tokens.push(TextLikeToken::AnyMany),
            '_' => tokens.push(TextLikeToken::AnyOne),
            '\\' => {
                let Some(literal) = characters.next() else {
                    return false;
                };
                tokens.push(TextLikeToken::Literal(literal));
            }
            literal => tokens.push(TextLikeToken::Literal(literal)),
        }
    }

    let value = value.chars().collect::<Vec<_>>();
    let mut previous = vec![false; value.len() + 1];
    previous[0] = true;
    for token in tokens {
        let mut current = vec![false; value.len() + 1];
        match token {
            TextLikeToken::AnyMany => {
                current[0] = previous[0];
                for index in 1..=value.len() {
                    current[index] = previous[index] || current[index - 1];
                }
            }
            TextLikeToken::AnyOne => {
                current[1..=value.len()].copy_from_slice(&previous[..value.len()]);
            }
            TextLikeToken::Literal(expected) => {
                for index in 1..=value.len() {
                    current[index] = previous[index - 1] && value[index - 1] == expected;
                }
            }
        }
        previous = current;
    }
    previous[value.len()]
}

fn apply_direction(ordering: Ordering, direction: OrderDirection) -> Ordering {
    match direction.base_direction() {
        OrderDirection::Asc => ordering,
        OrderDirection::Desc => ordering.reverse(),
        _ => unreachable!("base_direction returns a physical direction"),
    }
}

fn compare_optional_values(left: Option<&IndexValue>, right: Option<&IndexValue>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => compare_values(left, right).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_optional_to_cursor(
    record_value: Option<&IndexValue>,
    cursor_value: &IndexValue,
) -> Ordering {
    match (record_value, cursor_value) {
        (None, IndexValue::Null) | (Some(IndexValue::Null), IndexValue::Null) => Ordering::Equal,
        (None, _) | (Some(IndexValue::Null), _) => Ordering::Greater,
        (Some(_), IndexValue::Null) => Ordering::Less,
        (Some(record_value), cursor_value) => {
            compare_values(record_value, cursor_value).unwrap_or(Ordering::Equal)
        }
    }
}

fn compare_values(left: &IndexValue, right: &IndexValue) -> Option<Ordering> {
    match (left, right) {
        (IndexValue::Boolean(left), IndexValue::Boolean(right)) => Some(left.cmp(right)),
        (IndexValue::Integer(left), IndexValue::Integer(right)) => Some(left.cmp(right)),
        (IndexValue::Decimal(left), IndexValue::Decimal(right)) => Some(left.cmp(right)),
        (IndexValue::String(left), IndexValue::String(right)) => Some(left.cmp(right)),
        (IndexValue::Uuid(left), IndexValue::Uuid(right)) => Some(left.cmp(right)),
        (IndexValue::Timestamp(left), IndexValue::Timestamp(right)) => Some(left.cmp(right)),
        _ => None,
    }
}
