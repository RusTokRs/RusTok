use std::{collections::BTreeSet, fmt};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{DomainError, FieldName, IndexValueType, LinkName, SchemaRef};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SchemaFingerprint([u8; 32]);

impl SchemaFingerprint {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Display for SchemaFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex::encode(self.0))
    }
}

impl IndexSchema {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.fields.is_empty() {
            return Err(DomainError::EmptySchema);
        }

        let mut field_names = BTreeSet::new();
        for field in &self.fields {
            if !field_names.insert(field.name.clone()) {
                return Err(DomainError::DuplicateField(field.name.to_string()));
            }
            if field.sortable && field.cardinality == FieldCardinality::Many {
                return Err(DomainError::SortableManyField(field.name.to_string()));
            }
        }

        let mut link_names = BTreeSet::new();
        for link in &self.links {
            if !link_names.insert(link.name.clone()) {
                return Err(DomainError::DuplicateLink(link.name.to_string()));
            }
            if link.source_fields.is_empty() || link.target_fields.is_empty() {
                return Err(DomainError::EmptyLinkFields {
                    link: link.name.to_string(),
                });
            }
            if link.source_fields.len() != link.target_fields.len() {
                return Err(DomainError::LinkFieldArityMismatch {
                    link: link.name.to_string(),
                    source_count: link.source_fields.len(),
                    target_count: link.target_fields.len(),
                });
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

    /// Stable digest of the schema contract.
    ///
    /// Field and link declaration order is ignored. Composite-link field order
    /// is preserved because it defines key-column correspondence.
    pub fn fingerprint(&self) -> Result<SchemaFingerprint, DomainError> {
        self.validate()?;

        let mut hasher = Sha256::new();
        write_bytes(&mut hasher, b"rustok-index-schema-v1");
        write_schema_ref(&mut hasher, &self.reference);
        hasher.update([locale_mode_tag(self.locale_mode)]);

        let mut fields = self.fields.iter().collect::<Vec<_>>();
        fields.sort_by(|left, right| left.name.cmp(&right.name));
        write_len(&mut hasher, fields.len());
        for field in fields {
            write_str(&mut hasher, field.name.as_str());
            hasher.update([value_type_tag(field.value_type)]);
            hasher.update([field_cardinality_tag(field.cardinality)]);
            hasher.update([
                u8::from(field.nullable),
                u8::from(field.selectable),
                u8::from(field.filterable),
                u8::from(field.sortable),
            ]);
        }

        let mut links = self.links.iter().collect::<Vec<_>>();
        links.sort_by(|left, right| left.name.cmp(&right.name));
        write_len(&mut hasher, links.len());
        for link in links {
            write_str(&mut hasher, link.name.as_str());
            write_schema_ref(&mut hasher, &link.target_schema);
            hasher.update([link_cardinality_tag(link.cardinality)]);
            write_field_names(&mut hasher, &link.source_fields);
            write_field_names(&mut hasher, &link.target_fields);
        }

        Ok(SchemaFingerprint(hasher.finalize().into()))
    }
}

fn write_schema_ref(hasher: &mut Sha256, reference: &SchemaRef) {
    write_str(hasher, reference.module.as_str());
    write_str(hasher, reference.entity.as_str());
    hasher.update(reference.version.get().to_be_bytes());
}

fn write_field_names(hasher: &mut Sha256, names: &[FieldName]) {
    write_len(hasher, names.len());
    for name in names {
        write_str(hasher, name.as_str());
    }
}

fn write_str(hasher: &mut Sha256, value: &str) {
    write_bytes(hasher, value.as_bytes());
}

fn write_bytes(hasher: &mut Sha256, value: &[u8]) {
    write_len(hasher, value.len());
    hasher.update(value);
}

fn write_len(hasher: &mut Sha256, value: usize) {
    hasher.update((value as u64).to_be_bytes());
}

fn locale_mode_tag(value: LocaleMode) -> u8 {
    match value {
        LocaleMode::None => 0,
        LocaleMode::Optional => 1,
        LocaleMode::Required => 2,
    }
}

fn field_cardinality_tag(value: FieldCardinality) -> u8 {
    match value {
        FieldCardinality::One => 0,
        FieldCardinality::Many => 1,
    }
}

fn link_cardinality_tag(value: LinkCardinality) -> u8 {
    match value {
        LinkCardinality::One => 0,
        LinkCardinality::Many => 1,
    }
}

fn value_type_tag(value: IndexValueType) -> u8 {
    match value {
        IndexValueType::Boolean => 0,
        IndexValueType::Integer => 1,
        IndexValueType::Decimal => 2,
        IndexValueType::String => 3,
        IndexValueType::Uuid => 4,
        IndexValueType::Timestamp => 5,
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

    fn linked_schema() -> IndexSchema {
        IndexSchema {
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
        assert!(linked_schema().validate().is_ok());
    }

    #[test]
    fn fingerprint_ignores_declaration_order() {
        let first = linked_schema();
        let mut second = first.clone();
        second.fields.reverse();

        assert_eq!(first.fingerprint().unwrap(), second.fingerprint().unwrap());
    }

    #[test]
    fn fingerprint_changes_with_contract() {
        let first = linked_schema();
        let mut second = first.clone();
        second.fields[0].filterable = false;

        assert_ne!(first.fingerprint().unwrap(), second.fingerprint().unwrap());
    }
}
