use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashSet},
};

use uuid::Uuid;

use crate::domain::{
    EntityKey, FieldPath, FilterExpr, IndexMutation, IndexQuery, IndexRecord, IndexValue,
    LocaleMode, OrderDirection, Pagination,
};

use super::{QueryValidationError, RecordValidationError, SchemaRegistry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApplyOutcome {
    Applied,
    DuplicateEvent,
    StaleIgnored,
}

struct ReferenceIndex<'a> {
    registry: &'a SchemaRegistry,
    records: BTreeMap<EntityKey, IndexRecord>,
    tombstones: BTreeMap<EntityKey, u64>,
    events: HashSet<Uuid>,
}

impl<'a> ReferenceIndex<'a> {
    fn new(registry: &'a SchemaRegistry) -> Self {
        Self {
            registry,
            records: BTreeMap::new(),
            tombstones: BTreeMap::new(),
            events: HashSet::new(),
        }
    }

    fn apply(
        &mut self,
        mutation: IndexMutation,
    ) -> Result<ApplyOutcome, RecordValidationError> {
        if self.events.contains(&mutation.event_id()) {
            return Ok(ApplyOutcome::DuplicateEvent);
        }

        match &mutation {
            IndexMutation::Upsert { record, .. } => self.registry.validate_record(record)?,
            IndexMutation::Delete {
                key,
                source_version,
                ..
            } => self.validate_delete(key, *source_version)?,
        }

        self.events.insert(mutation.event_id());
        let key = mutation.key().clone();
        let incoming_version = mutation.source_version();
        let current_version = self
            .records
            .get(&key)
            .map(|record| record.source_version)
            .into_iter()
            .chain(self.tombstones.get(&key).copied())
            .max()
            .unwrap_or(0);

        if incoming_version <= current_version {
            return Ok(ApplyOutcome::StaleIgnored);
        }

        match mutation {
            IndexMutation::Upsert { record, .. } => {
                self.tombstones.remove(&record.key);
                self.records.insert(record.key.clone(), record);
            }
            IndexMutation::Delete {
                key,
                source_version,
                ..
            } => {
                self.records.remove(&key);
                self.tombstones.insert(key, source_version);
            }
        }

        Ok(ApplyOutcome::Applied)
    }

    fn validate_delete(
        &self,
        key: &EntityKey,
        source_version: u64,
    ) -> Result<(), RecordValidationError> {
        let schema = self
            .registry
            .get(&key.schema)
            .ok_or_else(|| RecordValidationError::SchemaNotFound(key.schema.clone()))?;
        match (schema.schema.locale_mode, key.locale.is_some()) {
            (LocaleMode::Required, false) => {
                return Err(RecordValidationError::LocaleRequired(key.schema.clone()));
            }
            (LocaleMode::None, true) => {
                return Err(RecordValidationError::LocaleForbidden(key.schema.clone()));
            }
            _ => {}
        }
        if source_version == 0 {
            return Err(RecordValidationError::ZeroSourceVersion);
        }
        Ok(())
    }

    fn query<'b>(
        &'b self,
        query: &IndexQuery,
    ) -> Result<Vec<&'b IndexRecord>, QueryValidationError> {
        self.registry.validate_query(query)?;

        let mut records = self
            .records
            .values()
            .filter(|record| record.key.schema == query.schema)
            .filter(|record| {
                query
                    .filter
                    .as_ref()
                    .is_none_or(|filter| self.matches_filter(record, filter))
            })
            .collect::<Vec<_>>();

        records.sort_by(|left, right| {
            for order in &query.order_by {
                let ordering = compare_optional_values(
                    self.values_for_path(left, &order.field).into_iter().next(),
                    self.values_for_path(right, &order.field).into_iter().next(),
                );
                if ordering != Ordering::Equal {
                    return match order.direction {
                        OrderDirection::Asc => ordering,
                        OrderDirection::Desc => ordering.reverse(),
                    };
                }
            }
            left.key.cmp(&right.key)
        });

        match &query.pagination {
            Pagination::Cursor { first, after } => {
                debug_assert!(after.is_none(), "reference evaluator does not decode cursors");
                records.truncate(*first as usize);
            }
            Pagination::Offset { limit, offset } => {
                records = records
                    .into_iter()
                    .skip(*offset as usize)
                    .take(*limit as usize)
                    .collect();
            }
        }

        Ok(records)
    }

    fn matches_filter(&self, record: &IndexRecord, filter: &FilterExpr) -> bool {
        match filter {
            FilterExpr::And(filters) => filters
                .iter()
                .all(|filter| self.matches_filter(record, filter)),
            FilterExpr::Or(filters) => filters
                .iter()
                .any(|filter| self.matches_filter(record, filter)),
            FilterExpr::Not(filter) => !self.matches_filter(record, filter),
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
                self.matches_ordered(record, path, expected, |ordering| ordering.is_gt())
            }
            FilterExpr::Gte(path, expected) => self.matches_ordered(
                record,
                path,
                expected,
                |ordering| ordering.is_gt() || ordering.is_eq(),
            ),
            FilterExpr::Lt(path, expected) => {
                self.matches_ordered(record, path, expected, |ordering| ordering.is_lt())
            }
            FilterExpr::Lte(path, expected) => self.matches_ordered(
                record,
                path,
                expected,
                |ordering| ordering.is_lt() || ordering.is_eq(),
            ),
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

    fn values_for_path<'b>(
        &'b self,
        record: &'b IndexRecord,
        path: &FieldPath,
    ) -> Vec<&'b IndexValue> {
        let mut current = vec![record];

        for link_name in path.links() {
            let mut next = Vec::new();
            for current_record in current {
                let Some(link) = current_record
                    .links
                    .iter()
                    .find(|link| link.name == *link_name)
                else {
                    continue;
                };
                for target in &link.targets {
                    let key = EntityKey {
                        tenant_id: current_record.key.tenant_id,
                        schema: target.schema.clone(),
                        entity_id: target.entity_id,
                        locale: target.locale.clone(),
                    };
                    if let Some(target_record) = self.records.get(&key) {
                        next.push(target_record);
                    }
                }
            }
            current = next;
        }

        current
            .into_iter()
            .filter_map(|record| record.fields.get(path.field()))
            .collect()
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use proptest::prelude::*;

    use super::*;
    use crate::domain::{
        EntityName, FieldCardinality, FieldName, IndexField, IndexSchema, IndexValueType,
        LocaleKey, ModuleName, SchemaRef, SchemaVersion,
    };

    fn schema_ref() -> SchemaRef {
        SchemaRef {
            module: ModuleName::new("test").unwrap(),
            entity: EntityName::new("item").unwrap(),
            version: SchemaVersion::INITIAL,
        }
    }

    fn registry() -> SchemaRegistry {
        let schema = IndexSchema {
            reference: schema_ref(),
            locale_mode: LocaleMode::Required,
            fields: vec![IndexField {
                name: FieldName::new("value").unwrap(),
                value_type: IndexValueType::Integer,
                cardinality: FieldCardinality::One,
                nullable: false,
                selectable: true,
                filterable: true,
                sortable: true,
            }],
            links: Vec::new(),
        };
        let mut registry = SchemaRegistry::new();
        registry.register(schema).unwrap();
        registry
    }

    fn record(tenant: Uuid, entity: Uuid, version: u64, value: i64) -> IndexRecord {
        IndexRecord {
            key: EntityKey {
                tenant_id: tenant,
                schema: schema_ref(),
                entity_id: entity,
                locale: Some(LocaleKey::new("en-US").unwrap()),
            },
            source_version: version,
            fields: BTreeMap::from([(
                FieldName::new("value").unwrap(),
                IndexValue::Integer(value),
            )]),
            links: Vec::new(),
        }
    }

    proptest! {
        #[test]
        fn repeated_event_delivery_is_idempotent(
            tenant in any::<u128>(),
            entity in any::<u128>(),
            version in 1u64..u64::MAX,
            value in any::<i64>(),
            event in any::<u128>(),
        ) {
            let registry = registry();
            let mut index = ReferenceIndex::new(&registry);
            let mutation = IndexMutation::Upsert {
                event_id: Uuid::from_u128(event),
                record: record(
                    Uuid::from_u128(tenant),
                    Uuid::from_u128(entity),
                    version,
                    value,
                ),
            };

            prop_assert_eq!(index.apply(mutation.clone()).unwrap(), ApplyOutcome::Applied);
            let snapshot = index.records.clone();
            prop_assert_eq!(index.apply(mutation).unwrap(), ApplyOutcome::DuplicateEvent);
            prop_assert_eq!(&index.records, &snapshot);
        }

        #[test]
        fn tenant_keys_never_collide(
            tenant_a in any::<u128>(),
            tenant_b in any::<u128>(),
            entity in any::<u128>(),
        ) {
            prop_assume!(tenant_a != tenant_b);
            let registry = registry();
            let mut index = ReferenceIndex::new(&registry);
            let entity = Uuid::from_u128(entity);

            index.apply(IndexMutation::Upsert {
                event_id: Uuid::new_v4(),
                record: record(Uuid::from_u128(tenant_a), entity, 1, 10),
            }).unwrap();
            index.apply(IndexMutation::Upsert {
                event_id: Uuid::new_v4(),
                record: record(Uuid::from_u128(tenant_b), entity, 1, 20),
            }).unwrap();

            prop_assert_eq!(index.records.len(), 2);
        }

        #[test]
        fn tombstone_prevents_stale_resurrection(
            tenant in any::<u128>(),
            entity in any::<u128>(),
            delete_version in 2u64..u64::MAX,
        ) {
            let registry = registry();
            let mut index = ReferenceIndex::new(&registry);
            let tenant = Uuid::from_u128(tenant);
            let entity = Uuid::from_u128(entity);
            let initial = record(tenant, entity, 1, 10);
            let key = initial.key.clone();

            index.apply(IndexMutation::Upsert {
                event_id: Uuid::new_v4(),
                record: initial,
            }).unwrap();
            index.apply(IndexMutation::Delete {
                event_id: Uuid::new_v4(),
                key: key.clone(),
                source_version: delete_version,
            }).unwrap();
            let outcome = index.apply(IndexMutation::Upsert {
                event_id: Uuid::new_v4(),
                record: record(tenant, entity, delete_version - 1, 99),
            }).unwrap();

            prop_assert_eq!(outcome, ApplyOutcome::StaleIgnored);
            prop_assert!(!index.records.contains_key(&key));
        }
    }

    #[test]
    fn reference_query_filters_and_sorts_deterministically() {
        use crate::domain::{IndexQuery, OrderExpr};

        let registry = registry();
        let mut index = ReferenceIndex::new(&registry);
        let tenant = Uuid::new_v4();
        for value in [3, 1, 2] {
            index
                .apply(IndexMutation::Upsert {
                    event_id: Uuid::new_v4(),
                    record: record(tenant, Uuid::new_v4(), value as u64, value),
                })
                .unwrap();
        }

        let query = IndexQuery {
            schema: schema_ref(),
            fields: vec![FieldPath::new(FieldName::new("value").unwrap())],
            filter: Some(FilterExpr::Gte(
                FieldPath::new(FieldName::new("value").unwrap()),
                IndexValue::Integer(2),
            )),
            order_by: vec![OrderExpr {
                field: FieldPath::new(FieldName::new("value").unwrap()),
                direction: OrderDirection::Asc,
            }],
            pagination: Pagination::Cursor {
                first: 10,
                after: None,
            },
            include_exact_count: true,
        };

        let values = index
            .query(&query)
            .unwrap()
            .into_iter()
            .map(|record| record.fields[&FieldName::new("value").unwrap()].clone())
            .collect::<Vec<_>>();
        assert_eq!(values, vec![IndexValue::Integer(2), IndexValue::Integer(3)]);
    }
}
