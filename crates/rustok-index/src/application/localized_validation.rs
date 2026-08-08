use thiserror::Error;

use crate::domain::{FieldPath, LocalizedEntityQuery, LocaleMode, SchemaRef};

use super::{QueryValidationError, SchemaRegistry, SchemaRegistryError};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LocalizedEntityQueryValidationError {
    #[error(transparent)]
    Query(#[from] QueryValidationError),
    #[error(transparent)]
    Registry(#[from] SchemaRegistryError),
    #[error("localized entity fold requires a locale-required root schema: {0}")]
    LocaleRequiredSchema(SchemaRef),
    #[error("any-locale identity predicate must reference root fields only: {0:?}")]
    AnyLocaleLinkedPath(FieldPath),
}

impl SchemaRegistry {
    /// Validate the explicit localized-entity fold request without changing ordinary `IndexQuery`
    /// validation semantics.
    pub fn validate_localized_entity_query(
        &self,
        query: &LocalizedEntityQuery,
    ) -> Result<(), LocalizedEntityQueryValidationError> {
        query
            .validate_shape()
            .map_err(QueryValidationError::from)?;

        let registered = self
            .get(&query.query.schema)
            .ok_or_else(|| SchemaRegistryError::SchemaNotFound(query.query.schema.clone()))?;
        if registered.schema.locale_mode != LocaleMode::Required {
            return Err(LocalizedEntityQueryValidationError::LocaleRequiredSchema(
                query.query.schema.clone(),
            ));
        }

        // The embedded query remains a normal, fully validated exact-locale query shape. Fold mode is
        // additive and cannot weaken selection/filter/order/pagination/schema checks.
        self.validate_query(&query.query)?;

        if let Some(path) = query
            .any_locale_referenced_paths()
            .into_iter()
            .find(|path| !path.links().is_empty())
        {
            return Err(LocalizedEntityQueryValidationError::AnyLocaleLinkedPath(
                path.clone(),
            ));
        }

        // Reuse the canonical filter validator by substituting only the existential root predicate.
        // This keeps field existence/filterability/operator/value rules exactly aligned with ordinary
        // Index queries while preserving the separate any-locale semantic role.
        if let Some(filter) = &query.any_locale_filter {
            let mut probe = query.query.clone();
            probe.filter = Some(filter.clone());
            self.validate_query(&probe)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::{
        EntityName, FieldCardinality, FieldName, FieldPath, FilterExpr, IndexField, IndexQuery,
        IndexQueryScope, IndexSchema, IndexValue, IndexValueType, LocaleKey, ModuleName, Pagination,
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
}
