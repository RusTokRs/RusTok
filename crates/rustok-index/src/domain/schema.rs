use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{DomainError, FieldName, LinkName, SchemaRef};
use crate::domain::IndexValueType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocaleMode {
    None,
    Optional,
    Required,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldCardinality {
    One,
    Many,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkCardinality {
    One,
    Many,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexField {
    pub name: FieldName,
    pub value_type: IndexValueType,
    pub cardinality: FieldCardinality,
    pub nullable: bool,
    pub selectable: bool,
    pub filterable: bool,
    pub sortable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexLink {
    pub name: LinkName,
    pub source_fields: Vec<FieldName>,
    pub target_schema: SchemaRef,
    pub target_fields: Vec<FieldName>,
    pub cardinality: LinkCardinality,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexSchema {
    pub reference: SchemaRef,
    pub locale_mode: LocaleMode,
    pub fields: Vec<IndexField>,
    pub links: Vec<IndexLink>,
}

impl IndexSchema {
    pub fn validate(&self) -> Result<(), DomainError> {
        let mut field_names = BTreeSet::new();
        for field in &self.fields {
            if !field_names.insert(field.name.clone()) {
                return Err(DomainError::DuplicateField(field.name.to_string()));
            }
        }

        let mut link_names = BTreeSet::new();
        for link in &self.links {
            if !link_names.insert(link.name.clone()) {
                return Err(DomainError::DuplicateLink(link.name.to_string()));
            }

            for source_field in &link.source_fields {
                if !field_names.contains(source_field) {
                    return Err(DomainError::UnknownLinkSourceField(
                        source_field.to_string(),
                    ));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EntityName, ModuleName, SchemaVersion};

    fn schema_ref(entity: &str) -> SchemaRef {
        SchemaRef {
            module: ModuleName::new("rustok-product").unwrap(),
            entity: EntityName::new(entity).unwrap(),
            version: SchemaVersion::INITIAL,
        }
    }

    fn field(name: &str) -> IndexField {
        IndexField {
            name: FieldName::new(name).unwrap(),
            value_type: IndexValueType::Uuid,
            cardinality: FieldCardinality::One,
            nullable: false,
            selectable: true,
            filterable: true,
            sortable: false,
        }
    }

    #[test]
    fn rejects_duplicate_fields() {
        let schema = IndexSchema {
            reference: schema_ref("product"),
            locale_mode: LocaleMode::Required,
            fields: vec![field("id"), field("id")],
            links: Vec::new(),
        };

        assert_eq!(
            schema.validate(),
            Err(DomainError::DuplicateField("id".to_owned()))
        );
    }

    #[test]
    fn validates_generic_product_to_channel_link() {
        let schema = IndexSchema {
            reference: schema_ref("product"),
            locale_mode: LocaleMode::Required,
            fields: vec![field("id"), field("sales_channel_id")],
            links: vec![IndexLink {
                name: LinkName::new("sales_channel").unwrap(),
                source_fields: vec![FieldName::new("sales_channel_id").unwrap()],
                target_schema: schema_ref("sales_channel"),
                target_fields: vec![FieldName::new("id").unwrap()],
                cardinality: LinkCardinality::Many,
            }],
        };

        assert!(schema.validate().is_ok());
    }
}
