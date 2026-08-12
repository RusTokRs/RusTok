use std::collections::BTreeSet;

use thiserror::Error;

use crate::domain::{
    FieldCardinality, FieldPath, LocaleMode, LocalizedEntityQuery, OrderDirection, SchemaRef,
};

use super::{QueryValidationError, SchemaRegistry, SchemaRegistryError};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LocalizedEntityQueryValidationError {
    #[error(transparent)]
    Query(#[from] QueryValidationError),
    #[error(transparent)]
    Registry(#[from] SchemaRegistryError),
    #[error("localized entity fold requires a locale-required root schema: {0}")]
    LocaleRequiredSchema(SchemaRef),
    #[error("localized entity identity tie-break supports only asc or desc")]
    InvalidIdentityOrderDirection,
    #[error("localized entity fold compiler currently accepts root-only query paths: {0:?}")]
    LinkedPathPending(FieldPath),
    #[error("any-locale identity predicate must reference root fields only: {0:?}")]
    AnyLocaleLinkedPath(FieldPath),
    #[error("localized projection field is duplicated: {0:?}")]
    DuplicateLocalizedProjection(FieldPath),
    #[error("localized projection field must be selected by the embedded query: {0:?}")]
    LocalizedProjectionNotSelected(FieldPath),
    #[error("localized projection field must reference the root entity: {0:?}")]
    LocalizedProjectionLinkedPath(FieldPath),
    #[error("localized projection field must be scalar in the initial fold compiler: {0:?}")]
    LocalizedProjectionMany(FieldPath),
    #[error(
        "localized projection field must not be evaluated by the ordinary identity filter: {0:?}"
    )]
    LocalizedProjectionInOrdinaryFilter(FieldPath),
    #[error("localized projection field must not drive identity ordering: {0:?}")]
    LocalizedProjectionInOrder(FieldPath),
}

impl SchemaRegistry {
    /// Validate the explicit localized-entity fold request without changing ordinary `IndexQuery`
    /// validation semantics.
    pub fn validate_localized_entity_query(
        &self,
        query: &LocalizedEntityQuery,
    ) -> Result<(), LocalizedEntityQueryValidationError> {
        query.validate_shape().map_err(QueryValidationError::from)?;

        if !matches!(
            query.identity_order_direction,
            OrderDirection::Asc | OrderDirection::Desc
        ) {
            return Err(LocalizedEntityQueryValidationError::InvalidIdentityOrderDirection);
        }

        let registered = self
            .get(&query.query.schema)
            .ok_or_else(|| SchemaRegistryError::SchemaNotFound(query.query.schema.clone()))?;
        if registered.schema.locale_mode != LocaleMode::Required {
            return Err(LocalizedEntityQueryValidationError::LocaleRequiredSchema(
                query.query.schema.clone(),
            ));
        }

        self.validate_query(&query.query)?;

        if let Some(path) = query
            .query
            .referenced_paths()
            .into_iter()
            .find(|path| !path.links().is_empty())
        {
            return Err(LocalizedEntityQueryValidationError::LinkedPathPending(
                path.clone(),
            ));
        }

        if let Some(path) = query
            .any_locale_referenced_paths()
            .into_iter()
            .find(|path| !path.links().is_empty())
        {
            return Err(LocalizedEntityQueryValidationError::AnyLocaleLinkedPath(
                path.clone(),
            ));
        }

        if let Some(filter) = &query.any_locale_filter {
            let mut probe = query.query.clone();
            probe.filter = Some(filter.clone());
            self.validate_query(&probe)?;
        }

        let mut ordinary_filter_paths = Vec::new();
        if let Some(filter) = &query.query.filter {
            filter.field_paths(&mut ordinary_filter_paths);
        }
        let ordinary_filter_paths = ordinary_filter_paths
            .into_iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let order_paths = query
            .query
            .order_by
            .iter()
            .map(|order| order.field.clone())
            .collect::<BTreeSet<_>>();

        let mut localized = BTreeSet::new();
        for path in &query.localized_projection_fields {
            if !localized.insert(path.clone()) {
                return Err(
                    LocalizedEntityQueryValidationError::DuplicateLocalizedProjection(path.clone()),
                );
            }
            if !path.links().is_empty() {
                return Err(
                    LocalizedEntityQueryValidationError::LocalizedProjectionLinkedPath(
                        path.clone(),
                    ),
                );
            }
            if !query.query.fields.iter().any(|selected| selected == path) {
                return Err(
                    LocalizedEntityQueryValidationError::LocalizedProjectionNotSelected(
                        path.clone(),
                    ),
                );
            }
            let field = registered
                .schema
                .fields
                .iter()
                .find(|field| field.name == *path.field())
                .ok_or_else(|| QueryValidationError::UnknownField {
                    schema: query.query.schema.clone(),
                    field: path.field().clone(),
                })?;
            if field.cardinality != FieldCardinality::One {
                return Err(
                    LocalizedEntityQueryValidationError::LocalizedProjectionMany(path.clone()),
                );
            }
            if ordinary_filter_paths.contains(path) {
                return Err(
                    LocalizedEntityQueryValidationError::LocalizedProjectionInOrdinaryFilter(
                        path.clone(),
                    ),
                );
            }
            if order_paths.contains(path) {
                return Err(
                    LocalizedEntityQueryValidationError::LocalizedProjectionInOrder(path.clone()),
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::{
        EntityName, FieldName, FieldPath, FilterExpr, IndexField, IndexQuery, IndexQueryScope,
        IndexSchema, IndexValue, IndexValueType, LocaleKey, ModuleName, OrderExpr, Pagination,
        SchemaRef, SchemaVersion,
    };

    fn schema(locale_mode: LocaleMode) -> IndexSchema {
        IndexSchema {
            reference: SchemaRef {
                module: ModuleName::new("rustok-product").unwrap(),
                entity: EntityName::new("product").unwrap(),
                version: SchemaVersion::INITIAL,
            },
            locale_mode,
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
                    sortable: true,
                },
            ],
            links: Vec::new(),
        }
    }

    fn localized_query(schema: &IndexSchema) -> LocalizedEntityQuery {
        LocalizedEntityQuery::new(
            IndexQuery {
                scope: IndexQueryScope {
                    tenant_id: Uuid::new_v4(),
                    locale: Some(LocaleKey::new("en-US").unwrap()),
                },
                schema: schema.reference.clone(),
                fields: vec![FieldPath::new(FieldName::new("title").unwrap())],
                filter: None,
                order_by: Vec::new(),
                pagination: Pagination::Cursor {
                    first: 20,
                    after: None,
                },
                include_exact_count: true,
            },
            Some(LocaleKey::new("en").unwrap()),
            Some(FilterExpr::Eq(
                FieldPath::new(FieldName::new("title").unwrap()),
                IndexValue::String("needle".to_owned()),
            )),
        )
        .with_localized_projection_fields([FieldPath::new(FieldName::new("title").unwrap())])
    }

    #[test]
    fn validates_explicit_fold_only_for_locale_required_schema() {
        let required = schema(LocaleMode::Required);
        let mut registry = SchemaRegistry::new();
        registry.register(required.clone()).unwrap();
        assert!(
            registry
                .validate_localized_entity_query(&localized_query(&required))
                .is_ok()
        );

        let plain = schema(LocaleMode::None);
        let mut registry = SchemaRegistry::new();
        registry.register(plain.clone()).unwrap();
        let mut query = localized_query(&plain);
        query.query.scope.locale = None;
        assert!(matches!(
            registry.validate_localized_entity_query(&query),
            Err(LocalizedEntityQueryValidationError::LocaleRequiredSchema(_))
        ));
    }

    #[test]
    fn rejects_aggregate_identity_tie_break_direction() {
        let schema = schema(LocaleMode::Required);
        let mut registry = SchemaRegistry::new();
        registry.register(schema.clone()).unwrap();
        let query = localized_query(&schema).with_identity_order_direction(OrderDirection::MaxDesc);
        assert_eq!(
            registry.validate_localized_entity_query(&query),
            Err(LocalizedEntityQueryValidationError::InvalidIdentityOrderDirection)
        );
    }

    #[test]
    fn any_locale_predicate_reuses_filterability_and_type_validation() {
        let schema = schema(LocaleMode::Required);
        let mut registry = SchemaRegistry::new();
        registry.register(schema.clone()).unwrap();
        let mut query = localized_query(&schema);
        query.any_locale_filter = Some(FilterExpr::Eq(
            FieldPath::new(FieldName::new("title").unwrap()),
            IndexValue::Uuid(Uuid::new_v4()),
        ));
        assert!(matches!(
            registry.validate_localized_entity_query(&query),
            Err(LocalizedEntityQueryValidationError::Query(
                QueryValidationError::InvalidFilterValue { .. }
            ))
        ));
    }

    #[test]
    fn localized_projection_cannot_drive_anchor_filter_or_order() {
        let schema = schema(LocaleMode::Required);
        let mut registry = SchemaRegistry::new();
        registry.register(schema.clone()).unwrap();

        let mut filtered = localized_query(&schema);
        filtered.query.filter = Some(FilterExpr::Eq(
            FieldPath::new(FieldName::new("title").unwrap()),
            IndexValue::String("requested-only".to_owned()),
        ));
        assert!(matches!(
            registry.validate_localized_entity_query(&filtered),
            Err(LocalizedEntityQueryValidationError::LocalizedProjectionInOrdinaryFilter(_))
        ));

        let mut ordered = localized_query(&schema);
        ordered.query.order_by = vec![OrderExpr {
            field: FieldPath::new(FieldName::new("title").unwrap()),
            direction: OrderDirection::Asc,
        }];
        assert!(matches!(
            registry.validate_localized_entity_query(&ordered),
            Err(LocalizedEntityQueryValidationError::LocalizedProjectionInOrder(_))
        ));
    }
}
