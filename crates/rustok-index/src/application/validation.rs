use std::collections::BTreeSet;

use thiserror::Error;

use crate::domain::{
    DomainError, FieldCardinality, FieldName, FieldPath, FilterExpr, IndexField, IndexQuery,
    IndexRecord, IndexValue, IndexValueType, LinkCardinality, LinkName, LocaleMode, SchemaRef,
};

use super::{SchemaRegistry, SchemaRegistryError};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RecordValidationError {
    #[error("schema is not registered: {0}")]
    SchemaNotFound(SchemaRef),

    #[error("record tenant id must not be nil")]
    NilTenantId,

    #[error("record entity id must not be nil")]
    NilEntityId,

    #[error("schema {0} requires a locale")]
    LocaleRequired(SchemaRef),

    #[error("schema {0} does not permit a locale")]
    LocaleForbidden(SchemaRef),

    #[error("record source version must be greater than zero")]
    ZeroSourceVersion,

    #[error("record contains unknown field {field} for schema {schema}")]
    UnknownField { schema: SchemaRef, field: FieldName },

    #[error("record is missing required field {field} for schema {schema}")]
    MissingRequiredField { schema: SchemaRef, field: FieldName },

    #[error("field {field} for schema {schema} contains an invalid value shape or type")]
    InvalidFieldValue { schema: SchemaRef, field: FieldName },

    #[error("record contains duplicate link {link} for schema {schema}")]
    DuplicateLink { schema: SchemaRef, link: LinkName },

    #[error("record contains unknown link {link} for schema {schema}")]
    UnknownLink { schema: SchemaRef, link: LinkName },

    #[error("single-cardinality link {link} for schema {schema} has multiple targets")]
    LinkCardinalityExceeded { schema: SchemaRef, link: LinkName },

    #[error("link {link} for schema {schema} targets {actual} instead of {expected}")]
    LinkTargetSchemaMismatch {
        schema: SchemaRef,
        link: LinkName,
        expected: SchemaRef,
        actual: SchemaRef,
    },

    #[error("link {link} for schema {schema} contains duplicate target {target}")]
    DuplicateLinkTarget {
        schema: SchemaRef,
        link: LinkName,
        target: String,
    },

    #[error("link {link} target locale is invalid for schema {target_schema}")]
    InvalidLinkTargetLocale {
        link: LinkName,
        target_schema: SchemaRef,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum QueryValidationError {
    #[error(transparent)]
    InvalidShape(#[from] DomainError),

    #[error(transparent)]
    Registry(#[from] SchemaRegistryError),

    #[error("query tenant id must not be nil")]
    NilTenantId,

    #[error("schema {0} requires a query locale")]
    LocaleRequired(SchemaRef),

    #[error("schema {0} does not permit a query locale")]
    LocaleForbidden(SchemaRef),

    #[error("schema {schema} has no link named {link}")]
    UnknownLink { schema: SchemaRef, link: LinkName },

    #[error("schema {schema} has no field named {field}")]
    UnknownField { schema: SchemaRef, field: FieldName },

    #[error("field {field} on schema {schema} is not selectable")]
    FieldNotSelectable { schema: SchemaRef, field: FieldName },

    #[error("field {field} on schema {schema} is not filterable")]
    FieldNotFilterable { schema: SchemaRef, field: FieldName },

    #[error("field {field} on schema {schema} is not sortable")]
    FieldNotSortable { schema: SchemaRef, field: FieldName },

    #[error("operator {operator} is not valid for field {field} on schema {schema}")]
    InvalidOperator {
        schema: SchemaRef,
        field: FieldName,
        operator: &'static str,
    },

    #[error("filter value does not match field {field} on schema {schema}")]
    InvalidFilterValue { schema: SchemaRef, field: FieldName },

    #[error("logical filter {operator} must contain at least one child")]
    EmptyLogicalFilter { operator: &'static str },

    #[error("IN filter must contain at least one value")]
    EmptyInFilter,
}

impl SchemaRegistry {
    pub fn validate_record(&self, record: &IndexRecord) -> Result<(), RecordValidationError> {
        let registered = self
            .get(&record.key.schema)
            .ok_or_else(|| RecordValidationError::SchemaNotFound(record.key.schema.clone()))?;
        let schema = &registered.schema;

        if record.key.tenant_id.is_nil() {
            return Err(RecordValidationError::NilTenantId);
        }
        if record.key.entity_id.is_nil() {
            return Err(RecordValidationError::NilEntityId);
        }
        validate_record_locale(
            &schema.reference,
            schema.locale_mode,
            record.key.locale.is_some(),
        )?;
        if record.source_version == 0 {
            return Err(RecordValidationError::ZeroSourceVersion);
        }

        for (name, value) in &record.fields {
            let field = schema
                .fields
                .iter()
                .find(|field| field.name == *name)
                .ok_or_else(|| RecordValidationError::UnknownField {
                    schema: schema.reference.clone(),
                    field: name.clone(),
                })?;
            if !valid_field_value(field, value) {
                return Err(RecordValidationError::InvalidFieldValue {
                    schema: schema.reference.clone(),
                    field: name.clone(),
                });
            }
        }

        for field in &schema.fields {
            if !field.nullable && !record.fields.contains_key(&field.name) {
                return Err(RecordValidationError::MissingRequiredField {
                    schema: schema.reference.clone(),
                    field: field.name.clone(),
                });
            }
        }

        let mut link_names = BTreeSet::new();
        for link_value in &record.links {
            if !link_names.insert(link_value.name.clone()) {
                return Err(RecordValidationError::DuplicateLink {
                    schema: schema.reference.clone(),
                    link: link_value.name.clone(),
                });
            }

            let definition = schema
                .links
                .iter()
                .find(|link| link.name == link_value.name)
                .ok_or_else(|| RecordValidationError::UnknownLink {
                    schema: schema.reference.clone(),
                    link: link_value.name.clone(),
                })?;

            if definition.cardinality == LinkCardinality::One && link_value.targets.len() > 1 {
                return Err(RecordValidationError::LinkCardinalityExceeded {
                    schema: schema.reference.clone(),
                    link: link_value.name.clone(),
                });
            }

            let mut targets = BTreeSet::new();
            for target in &link_value.targets {
                if target.entity_id.is_nil() {
                    return Err(RecordValidationError::NilEntityId);
                }
                if target.schema != definition.target_schema {
                    return Err(RecordValidationError::LinkTargetSchemaMismatch {
                        schema: schema.reference.clone(),
                        link: link_value.name.clone(),
                        expected: definition.target_schema.clone(),
                        actual: target.schema.clone(),
                    });
                }
                if !targets.insert(target.clone()) {
                    return Err(RecordValidationError::DuplicateLinkTarget {
                        schema: schema.reference.clone(),
                        link: link_value.name.clone(),
                        target: format!("{}:{}", target.schema, target.entity_id),
                    });
                }

                let target_schema = self
                    .get(&target.schema)
                    .ok_or_else(|| RecordValidationError::SchemaNotFound(target.schema.clone()))?;
                if !locale_presence_is_valid(
                    target_schema.schema.locale_mode,
                    target.locale.is_some(),
                ) {
                    return Err(RecordValidationError::InvalidLinkTargetLocale {
                        link: link_value.name.clone(),
                        target_schema: target.schema.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    pub fn validate_query(&self, query: &IndexQuery) -> Result<(), QueryValidationError> {
        query.validate_shape()?;
        if query.scope.tenant_id.is_nil() {
            return Err(QueryValidationError::NilTenantId);
        }

        let root = self
            .get(&query.schema)
            .ok_or_else(|| SchemaRegistryError::SchemaNotFound(query.schema.clone()))?;
        validate_query_locale(
            &root.schema.reference,
            root.schema.locale_mode,
            query.scope.locale.is_some(),
        )?;

        for path in &query.fields {
            let (schema, field) = self.resolve_field(&query.schema, path)?;
            if !field.selectable {
                return Err(QueryValidationError::FieldNotSelectable {
                    schema: schema.clone(),
                    field: field.name.clone(),
                });
            }
        }

        if let Some(filter) = &query.filter {
            self.validate_filter(&query.schema, filter)?;
        }

        for order in &query.order_by {
            let (schema, field) = self.resolve_field(&query.schema, &order.field)?;
            if !field.sortable {
                return Err(QueryValidationError::FieldNotSortable {
                    schema: schema.clone(),
                    field: field.name.clone(),
                });
            }
        }

        Ok(())
    }

    fn validate_filter(
        &self,
        root: &SchemaRef,
        filter: &FilterExpr,
    ) -> Result<(), QueryValidationError> {
        match filter {
            FilterExpr::And(children) => {
                if children.is_empty() {
                    return Err(QueryValidationError::EmptyLogicalFilter { operator: "and" });
                }
                for child in children {
                    self.validate_filter(root, child)?;
                }
            }
            FilterExpr::Or(children) => {
                if children.is_empty() {
                    return Err(QueryValidationError::EmptyLogicalFilter { operator: "or" });
                }
                for child in children {
                    self.validate_filter(root, child)?;
                }
            }
            FilterExpr::Not(child) => self.validate_filter(root, child)?,
            FilterExpr::Eq(path, value) | FilterExpr::Ne(path, value) => {
                self.validate_scalar_filter(root, path, value, "equality", false)?;
            }
            FilterExpr::In(path, values) => {
                if values.is_empty() {
                    return Err(QueryValidationError::EmptyInFilter);
                }
                for value in values {
                    self.validate_scalar_filter(root, path, value, "in", false)?;
                }
            }
            FilterExpr::Gt(path, value)
            | FilterExpr::Gte(path, value)
            | FilterExpr::Lt(path, value)
            | FilterExpr::Lte(path, value) => {
                self.validate_scalar_filter(root, path, value, "range", true)?;
            }
            FilterExpr::Contains(path, value) => {
                let (schema, field) = self.resolve_filterable_field(root, path)?;
                if field.cardinality != FieldCardinality::Many {
                    return Err(QueryValidationError::InvalidOperator {
                        schema: schema.clone(),
                        field: field.name.clone(),
                        operator: "contains",
                    });
                }
                if !scalar_matches_type(value, field.value_type) {
                    return Err(QueryValidationError::InvalidFilterValue {
                        schema: schema.clone(),
                        field: field.name.clone(),
                    });
                }
            }
            FilterExpr::IsNull(path, _) => {
                let (schema, field) = self.resolve_filterable_field(root, path)?;
                if !field.nullable {
                    return Err(QueryValidationError::InvalidOperator {
                        schema: schema.clone(),
                        field: field.name.clone(),
                        operator: "is_null",
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_scalar_filter(
        &self,
        root: &SchemaRef,
        path: &FieldPath,
        value: &IndexValue,
        operator: &'static str,
        require_ordered: bool,
    ) -> Result<(), QueryValidationError> {
        let (schema, field) = self.resolve_filterable_field(root, path)?;
        if field.cardinality != FieldCardinality::One {
            return Err(QueryValidationError::InvalidOperator {
                schema: schema.clone(),
                field: field.name.clone(),
                operator,
            });
        }
        if require_ordered && !is_ordered_type(field.value_type) {
            return Err(QueryValidationError::InvalidOperator {
                schema: schema.clone(),
                field: field.name.clone(),
                operator,
            });
        }
        if matches!(value, IndexValue::Null) || !scalar_matches_type(value, field.value_type) {
            return Err(QueryValidationError::InvalidFilterValue {
                schema: schema.clone(),
                field: field.name.clone(),
            });
        }
        Ok(())
    }

    fn resolve_filterable_field(
        &self,
        root: &SchemaRef,
        path: &FieldPath,
    ) -> Result<(&SchemaRef, &IndexField), QueryValidationError> {
        let (schema, field) = self.resolve_field(root, path)?;
        if !field.filterable {
            return Err(QueryValidationError::FieldNotFilterable {
                schema: schema.clone(),
                field: field.name.clone(),
            });
        }
        Ok((schema, field))
    }

    fn resolve_field(
        &self,
        root: &SchemaRef,
        path: &FieldPath,
    ) -> Result<(&SchemaRef, &IndexField), QueryValidationError> {
        let mut registered = self
            .get(root)
            .ok_or_else(|| SchemaRegistryError::SchemaNotFound(root.clone()))?;

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
            registered = self
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

        Ok((&registered.schema.reference, field))
    }
}

fn validate_record_locale(
    schema: &SchemaRef,
    mode: LocaleMode,
    has_locale: bool,
) -> Result<(), RecordValidationError> {
    match (mode, has_locale) {
        (LocaleMode::Required, false) => {
            Err(RecordValidationError::LocaleRequired(schema.clone()))
        }
        (LocaleMode::None, true) => Err(RecordValidationError::LocaleForbidden(schema.clone())),
        _ => Ok(()),
    }
}

fn validate_query_locale(
    schema: &SchemaRef,
    mode: LocaleMode,
    has_locale: bool,
) -> Result<(), QueryValidationError> {
    match (mode, has_locale) {
        (LocaleMode::Required, false) => {
            Err(QueryValidationError::LocaleRequired(schema.clone()))
        }
        (LocaleMode::None, true) => Err(QueryValidationError::LocaleForbidden(schema.clone())),
        _ => Ok(()),
    }
}

fn locale_presence_is_valid(mode: LocaleMode, has_locale: bool) -> bool {
    !matches!(
        (mode, has_locale),
        (LocaleMode::Required, false) | (LocaleMode::None, true)
    )
}

fn valid_field_value(field: &IndexField, value: &IndexValue) -> bool {
    match value {
        IndexValue::Null => field.nullable,
        IndexValue::List(values) => {
            field.cardinality == FieldCardinality::Many
                && values
                    .iter()
                    .all(|value| scalar_matches_type(value, field.value_type))
        }
        value => {
            field.cardinality == FieldCardinality::One
                && scalar_matches_type(value, field.value_type)
        }
    }
}

fn scalar_matches_type(value: &IndexValue, expected: IndexValueType) -> bool {
    matches!(
        (value, expected),
        (IndexValue::Boolean(_), IndexValueType::Boolean)
            | (IndexValue::Integer(_), IndexValueType::Integer)
            | (IndexValue::Decimal(_), IndexValueType::Decimal)
            | (IndexValue::String(_), IndexValueType::String)
            | (IndexValue::Uuid(_), IndexValueType::Uuid)
            | (IndexValue::Timestamp(_), IndexValueType::Timestamp)
    )
}

fn is_ordered_type(value_type: IndexValueType) -> bool {
    matches!(
        value_type,
        IndexValueType::Integer
            | IndexValueType::Decimal
            | IndexValueType::String
            | IndexValueType::Uuid
            | IndexValueType::Timestamp
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use uuid::Uuid;

    use super::*;
    use crate::domain::{
        EntityKey, EntityName, IndexLink, IndexLinkValue, IndexQueryScope, LinkedEntityKey,
        ModuleName, OrderDirection, OrderExpr, Pagination, SchemaVersion,
    };

    fn reference(entity: &str) -> SchemaRef {
        SchemaRef {
            module: ModuleName::new("rustok-product").unwrap(),
            entity: EntityName::new(entity).unwrap(),
            version: SchemaVersion::INITIAL,
        }
    }

    fn field(name: &str, value_type: IndexValueType) -> IndexField {
        IndexField {
            name: FieldName::new(name).unwrap(),
            value_type,
            cardinality: FieldCardinality::One,
            nullable: false,
            selectable: true,
            filterable: true,
            sortable: true,
        }
    }

    fn registry() -> SchemaRegistry {
        let channel = crate::domain::IndexSchema {
            reference: reference("sales_channel"),
            locale_mode: LocaleMode::None,
            fields: vec![field("id", IndexValueType::Uuid)],
            links: Vec::new(),
        };
        let product = crate::domain::IndexSchema {
            reference: reference("product"),
            locale_mode: LocaleMode::Required,
            fields: vec![
                field("id", IndexValueType::Uuid),
                field("sales_channel_id", IndexValueType::Uuid),
            ],
            links: vec![IndexLink {
                name: LinkName::new("sales_channel").unwrap(),
                source_fields: vec![FieldName::new("sales_channel_id").unwrap()],
                target_schema: channel.reference.clone(),
                target_fields: vec![FieldName::new("id").unwrap()],
                cardinality: LinkCardinality::Many,
            }],
        };

        let mut registry = SchemaRegistry::new();
        registry.register_batch([product, channel]).unwrap();
        registry
    }

    #[test]
    fn validates_tenant_scoped_localized_record() {
        let registry = registry();
        let channel_id = Uuid::new_v4();
        let mut fields = BTreeMap::new();
        fields.insert(FieldName::new("id").unwrap(), IndexValue::Uuid(Uuid::new_v4()));
        fields.insert(
            FieldName::new("sales_channel_id").unwrap(),
            IndexValue::Uuid(channel_id),
        );
        let record = IndexRecord {
            key: EntityKey {
                tenant_id: Uuid::new_v4(),
                schema: reference("product"),
                entity_id: Uuid::new_v4(),
                locale: Some(crate::domain::LocaleKey::new("en-US").unwrap()),
            },
            source_version: 1,
            fields,
            links: vec![IndexLinkValue {
                name: LinkName::new("sales_channel").unwrap(),
                targets: vec![LinkedEntityKey {
                    schema: reference("sales_channel"),
                    entity_id: channel_id,
                    locale: None,
                }],
            }],
        };

        assert!(registry.validate_record(&record).is_ok());
    }

    #[test]
    fn rejects_missing_required_locale() {
        let registry = registry();
        let record = IndexRecord {
            key: EntityKey {
                tenant_id: Uuid::new_v4(),
                schema: reference("product"),
                entity_id: Uuid::new_v4(),
                locale: None,
            },
            source_version: 1,
            fields: BTreeMap::new(),
            links: Vec::new(),
        };

        assert!(matches!(
            registry.validate_record(&record),
            Err(RecordValidationError::LocaleRequired(_))
        ));
    }

    #[test]
    fn validates_linked_query_fields_filters_and_ordering() {
        let registry = registry();
        let query = IndexQuery {
            scope: IndexQueryScope {
                tenant_id: Uuid::new_v4(),
                locale: Some(crate::domain::LocaleKey::new("en-US").unwrap()),
            },
            schema: reference("product"),
            fields: vec![FieldPath::linked(
                [LinkName::new("sales_channel").unwrap()],
                FieldName::new("id").unwrap(),
            )],
            filter: Some(FilterExpr::Eq(
                FieldPath::new(FieldName::new("id").unwrap()),
                IndexValue::Uuid(Uuid::new_v4()),
            )),
            order_by: vec![OrderExpr {
                field: FieldPath::new(FieldName::new("id").unwrap()),
                direction: OrderDirection::Asc,
            }],
            pagination: Pagination::Cursor {
                first: 20,
                after: None,
            },
            include_exact_count: true,
        };

        assert!(registry.validate_query(&query).is_ok());
    }
}
