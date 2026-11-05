use thiserror::Error;

use crate::domain::{
    FieldCardinality, FieldName, FieldPath, IndexQuery, IndexValueType, LinkCardinality,
    Pagination, SchemaRef,
};

use super::{QueryValidationError, SchemaRegistry, SchemaRegistryError};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AggregateOrderValidationError {
    #[error(transparent)]
    Query(#[from] QueryValidationError),
    #[error(transparent)]
    Registry(#[from] SchemaRegistryError),
    #[error("aggregate ordering requires a path that traverses a many-cardinality link: {0:?}")]
    AggregateRequiresManyLink(FieldPath),
    #[error("many-link aggregate ordering requires a supported ordered scalar field: {0:?}")]
    AggregateRequiresOrderedScalar(FieldPath),
    #[error("many-link aggregate ordering currently requires bounded offset pagination")]
    AggregateRequiresOffsetPagination,
}

impl SchemaRegistry {
    /// Validate ordinary query semantics plus explicit `min_*` / `max_*` ordering
    /// over paths that cross at least one many-cardinality link.
    ///
    /// The legacy `validate_query` contract remains unchanged and continues to
    /// reject every many-link order as ambiguous. Planning and execution use this
    /// stronger boundary so aggregate ordering cannot silently inherit first-row
    /// or storage-order semantics.
    ///
    /// This first bounded policy supports offset pages only. Aggregate cursor
    /// encoding and continuation remain a separate contract because they must bind
    /// the derived value without changing legacy cursor semantics.
    pub fn validate_query_with_aggregate_ordering(
        &self,
        query: &IndexQuery,
    ) -> Result<(), AggregateOrderValidationError> {
        query.validate_shape().map_err(QueryValidationError::from)?;

        let has_aggregate = query
            .order_by
            .iter()
            .any(|order| order.direction.aggregate().is_some());
        if has_aggregate && !matches!(&query.pagination, Pagination::Offset { .. }) {
            return Err(AggregateOrderValidationError::AggregateRequiresOffsetPagination);
        }

        let mut ordinary = query.clone();
        ordinary
            .order_by
            .retain(|order| order.direction.aggregate().is_none());
        self.validate_query(&ordinary)?;

        for order in query
            .order_by
            .iter()
            .filter(|order| order.direction.aggregate().is_some())
        {
            let resolved = resolve_order_field(self, &query.schema, &order.field)?;
            if !resolved.traverses_many {
                return Err(AggregateOrderValidationError::AggregateRequiresManyLink(
                    order.field.clone(),
                ));
            }
            if !resolved.sortable {
                return Err(QueryValidationError::FieldNotSortable {
                    schema: resolved.schema,
                    field: resolved.field,
                }
                .into());
            }
            if resolved.cardinality != FieldCardinality::One
                || !is_aggregate_ordered_type(resolved.value_type)
            {
                return Err(
                    AggregateOrderValidationError::AggregateRequiresOrderedScalar(
                        order.field.clone(),
                    ),
                );
            }
        }

        Ok(())
    }
}

struct ResolvedOrderField {
    schema: SchemaRef,
    field: FieldName,
    cardinality: FieldCardinality,
    value_type: IndexValueType,
    sortable: bool,
    traverses_many: bool,
}

fn resolve_order_field(
    registry: &SchemaRegistry,
    root: &SchemaRef,
    path: &FieldPath,
) -> Result<ResolvedOrderField, AggregateOrderValidationError> {
    let mut registered = registry
        .get(root)
        .ok_or_else(|| SchemaRegistryError::SchemaNotFound(root.clone()))?;
    let mut traverses_many = false;

    for link_name in path.links() {
        let link = registered
            .schema
            .links
            .iter()
            .find(|link| link.name == *link_name)
            .ok_or_else(|| QueryValidationError::UnknownLink {
                schema: registered.schema.reference.clone(),
                link: link_name.clone(),
            })?;
        traverses_many |= link.cardinality == LinkCardinality::Many;
        registered = registry
            .get(&link.target_schema)
            .ok_or_else(|| SchemaRegistryError::SchemaNotFound(link.target_schema.clone()))?;
    }

    let field = registered
        .schema
        .fields
        .iter()
        .find(|field| field.name == *path.field())
        .ok_or_else(|| QueryValidationError::UnknownField {
            schema: registered.schema.reference.clone(),
            field: path.field().clone(),
        })?;

    Ok(ResolvedOrderField {
        schema: registered.schema.reference.clone(),
        field: field.name.clone(),
        cardinality: field.cardinality,
        value_type: field.value_type,
        sortable: field.sortable,
        traverses_many,
    })
}

fn is_aggregate_ordered_type(value_type: IndexValueType) -> bool {
    matches!(
        value_type,
        IndexValueType::Integer
            | IndexValueType::Decimal
            | IndexValueType::String
            | IndexValueType::Timestamp
    )
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::{
        EntityName, FieldName, IndexField, IndexLink, IndexQueryScope, IndexSchema, LinkName,
        LocaleKey, LocaleMode, ModuleName, OrderDirection, OrderExpr, Pagination, SchemaVersion,
    };

    fn reference(entity: &str) -> SchemaRef {
        SchemaRef {
            module: ModuleName::new("test").unwrap(),
            entity: EntityName::new(entity).unwrap(),
            version: SchemaVersion::INITIAL,
        }
    }

    fn field(name: &str, value_type: IndexValueType, sortable: bool) -> IndexField {
        IndexField {
            name: FieldName::new(name).unwrap(),
            value_type,
            cardinality: FieldCardinality::One,
            nullable: true,
            selectable: true,
            filterable: true,
            sortable,
        }
    }

    fn registry(child_type: IndexValueType, child_sortable: bool) -> SchemaRegistry {
        let child = IndexSchema {
            reference: reference("child"),
            locale_mode: LocaleMode::Required,
            fields: vec![field("score", child_type, child_sortable)],
            links: Vec::new(),
        };
        let root = IndexSchema {
            reference: reference("root"),
            locale_mode: LocaleMode::Required,
            fields: vec![field("id", child_type, true)],
            links: vec![IndexLink {
                name: LinkName::new("children").unwrap(),
                source_fields: vec![FieldName::new("id").unwrap()],
                target_schema: child.reference.clone(),
                target_fields: vec![FieldName::new("score").unwrap()],
                cardinality: LinkCardinality::Many,
            }],
        };
        let mut registry = SchemaRegistry::new();
        registry.register_batch([root, child]).unwrap();
        registry
    }

    fn query(direction: OrderDirection, linked: bool) -> IndexQuery {
        let field = if linked {
            FieldPath::linked(
                [LinkName::new("children").unwrap()],
                FieldName::new("score").unwrap(),
            )
        } else {
            FieldPath::new(FieldName::new("id").unwrap())
        };
        IndexQuery {
            scope: IndexQueryScope {
                tenant_id: Uuid::new_v4(),
                locale: Some(LocaleKey::new("en-US").unwrap()),
            },
            schema: reference("root"),
            fields: vec![FieldPath::new(FieldName::new("id").unwrap())],
            filter: None,
            order_by: vec![OrderExpr { field, direction }],
            pagination: Pagination::Offset {
                limit: 20,
                offset: 0,
            },
            include_exact_count: false,
        }
    }

    #[test]
    fn accepts_explicit_min_and_max_over_many_link() {
        for value_type in [
            IndexValueType::Integer,
            IndexValueType::Decimal,
            IndexValueType::String,
            IndexValueType::Timestamp,
        ] {
            let registry = registry(value_type, true);
            assert!(
                registry
                    .validate_query_with_aggregate_ordering(&query(OrderDirection::MinAsc, true))
                    .is_ok()
            );
            assert!(
                registry
                    .validate_query_with_aggregate_ordering(&query(OrderDirection::MaxDesc, true))
                    .is_ok()
            );
        }
    }

    #[test]
    fn ordinary_many_link_order_remains_ambiguous() {
        let registry = registry(IndexValueType::Integer, true);
        assert!(matches!(
            registry.validate_query_with_aggregate_ordering(&query(OrderDirection::Asc, true)),
            Err(AggregateOrderValidationError::Query(
                QueryValidationError::AmbiguousManyLinkSort(_)
            ))
        ));
    }

    #[test]
    fn aggregate_mode_is_rejected_on_singular_path() {
        let registry = registry(IndexValueType::Integer, true);
        assert!(matches!(
            registry.validate_query_with_aggregate_ordering(&query(OrderDirection::MinAsc, false)),
            Err(AggregateOrderValidationError::AggregateRequiresManyLink(_))
        ));
    }

    #[test]
    fn aggregate_requires_supported_sortable_scalar() {
        let uuid = registry(IndexValueType::Uuid, true);
        assert!(matches!(
            uuid.validate_query_with_aggregate_ordering(&query(OrderDirection::MaxAsc, true)),
            Err(AggregateOrderValidationError::AggregateRequiresOrderedScalar(_))
        ));

        let unsortable = registry(IndexValueType::Integer, false);
        assert!(matches!(
            unsortable
                .validate_query_with_aggregate_ordering(&query(OrderDirection::MaxDesc, true)),
            Err(AggregateOrderValidationError::Query(
                QueryValidationError::FieldNotSortable { .. }
            ))
        ));
    }

    #[test]
    fn aggregate_cursor_pagination_remains_rejected() {
        let registry = registry(IndexValueType::Integer, true);
        let mut query = query(OrderDirection::MinAsc, true);
        query.pagination = Pagination::Cursor {
            first: 20,
            after: None,
        };
        assert_eq!(
            registry.validate_query_with_aggregate_ordering(&query),
            Err(AggregateOrderValidationError::AggregateRequiresOffsetPagination)
        );
    }
}
