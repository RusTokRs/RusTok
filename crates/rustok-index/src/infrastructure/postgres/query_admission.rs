use std::collections::BTreeMap;

use rustok_core::ModuleRuntimeExtensions;
use thiserror::Error;

use crate::{PostgresQueryEntityAdmission, PostgresQueryEntityAdmissionError, SchemaRef};

const ENTITY_ALIAS_TOKEN: &str = "{{entity}}";
const RUNTIME_PASSTHROUGH_OWNER: &str = "index";
const RUNTIME_PASSTHROUGH_RULE: &str = "{{entity}}.entity_id IS NOT NULL";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresIndexQueryAdmissionDescriptor {
    owner_module: String,
    schema: SchemaRef,
    rule: Option<PostgresQueryEntityAdmission>,
    admission: PostgresQueryEntityAdmission,
}

impl PostgresIndexQueryAdmissionDescriptor {
    pub fn owner_module(&self) -> &str {
        &self.owner_module
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    pub fn admission(&self) -> &PostgresQueryEntityAdmission {
        &self.admission
    }

    pub fn is_governed(&self) -> bool {
        self.rule.is_some()
    }
}

#[derive(Clone, Debug, Default)]
pub struct PostgresIndexQueryAdmissionCatalog {
    entries: BTreeMap<SchemaRef, PostgresIndexQueryAdmissionDescriptor>,
}

impl PostgresIndexQueryAdmissionCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, schema: &SchemaRef) -> Option<&PostgresIndexQueryAdmissionDescriptor> {
        self.entries.get(schema)
    }

    pub fn iter(&self) -> impl Iterator<Item = &PostgresIndexQueryAdmissionDescriptor> {
        self.entries.values()
    }

    pub fn register(
        &mut self,
        owner_module: impl Into<String>,
        schema: SchemaRef,
        admission: PostgresQueryEntityAdmission,
    ) -> Result<(), PostgresIndexQueryAdmissionError> {
        let owner_module = owner_module.into();
        validate_owner_module(&owner_module)?;
        if let Some(existing) = self.entries.get(&schema) {
            return Err(PostgresIndexQueryAdmissionError::DuplicateSchema {
                schema,
                existing_owner: existing.owner_module.clone(),
                incoming_owner: owner_module,
            });
        }
        self.entries.insert(
            schema.clone(),
            PostgresIndexQueryAdmissionDescriptor {
                owner_module,
                schema,
                rule: Some(admission.clone()),
                admission,
            },
        );
        self.rebuild_composite()
    }

    /// Adds a runtime-local pass-through root descriptor for an otherwise ungoverned registered
    /// schema. Pass-through descriptors are never published as owner rules; they exist only in the
    /// immutable query runtime so a query rooted at any registered schema still applies the same
    /// composite owner admission to governed linked targets.
    pub(crate) fn ensure_runtime_schema(
        &mut self,
        schema: SchemaRef,
    ) -> Result<(), PostgresIndexQueryAdmissionError> {
        if self.entries.contains_key(&schema) {
            return Ok(());
        }
        let admission = PostgresQueryEntityAdmission::new(RUNTIME_PASSTHROUGH_RULE)?;
        self.entries.insert(
            schema.clone(),
            PostgresIndexQueryAdmissionDescriptor {
                owner_module: RUNTIME_PASSTHROUGH_OWNER.to_owned(),
                schema,
                rule: None,
                admission,
            },
        );
        self.rebuild_composite()
    }

    fn rebuild_composite(&mut self) -> Result<(), PostgresIndexQueryAdmissionError> {
        let governed = self
            .entries
            .values()
            .filter_map(|descriptor| {
                descriptor
                    .rule
                    .as_ref()
                    .map(|rule| (descriptor.schema.clone(), rule.template().to_owned()))
            })
            .collect::<Vec<_>>();
        if governed.is_empty() {
            return Ok(());
        }

        let guards = governed
            .iter()
            .map(|(schema, _)| schema_guard(schema))
            .collect::<Vec<_>>();
        let allowed = governed
            .iter()
            .map(|(schema, rule)| format!("({} AND ({rule}))", schema_guard(schema)))
            .collect::<Vec<_>>();
        let composite = PostgresQueryEntityAdmission::new(format!(
            "(NOT ({}) OR {})",
            guards.join(" OR "),
            allowed.join(" OR ")
        ))?;
        for descriptor in self.entries.values_mut() {
            descriptor.admission = composite.clone();
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum PostgresIndexQueryAdmissionError {
    #[error("PostgreSQL Index query admission owner module is invalid: {0}")]
    InvalidOwnerModule(String),
    #[error(
        "PostgreSQL Index query admission for {schema} has multiple owners: existing={existing_owner}, incoming={incoming_owner}"
    )]
    DuplicateSchema {
        schema: SchemaRef,
        existing_owner: String,
        incoming_owner: String,
    },
    #[error(transparent)]
    InvalidAdmission(#[from] PostgresQueryEntityAdmissionError),
}

pub fn register_postgres_index_query_admission(
    extensions: &mut ModuleRuntimeExtensions,
    owner_module: impl Into<String>,
    schema: SchemaRef,
    admission: PostgresQueryEntityAdmission,
) -> Result<(), PostgresIndexQueryAdmissionError> {
    extensions
        .get_or_insert_with::<PostgresIndexQueryAdmissionCatalog, _>(
            PostgresIndexQueryAdmissionCatalog::new,
        )
        .register(owner_module, schema, admission)
}

fn schema_guard(schema: &SchemaRef) -> String {
    format!(
        "({ENTITY_ALIAS_TOKEN}.module_name = '{}' AND {ENTITY_ALIAS_TOKEN}.entity_name = '{}' AND {ENTITY_ALIAS_TOKEN}.schema_version = {})",
        sql_literal(schema.module.as_str()),
        sql_literal(schema.entity.as_str()),
        schema.version.get(),
    )
}

fn sql_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn validate_owner_module(value: &str) -> Result<(), PostgresIndexQueryAdmissionError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_')
        });
    if valid {
        Ok(())
    } else {
        Err(PostgresIndexQueryAdmissionError::InvalidOwnerModule(
            value.to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EntityName, ModuleName, SchemaVersion};

    fn schema(entity: &str) -> SchemaRef {
        SchemaRef {
            module: ModuleName::new("rustok-product").unwrap(),
            entity: EntityName::new(entity).unwrap(),
            version: SchemaVersion::INITIAL,
        }
    }

    #[test]
    fn exact_schema_has_one_query_admission_owner() {
        let admission = PostgresQueryEntityAdmission::new("{{entity}}.source_version > 0").unwrap();
        let mut catalog = PostgresIndexQueryAdmissionCatalog::new();
        catalog
            .register("product", schema("product"), admission.clone())
            .unwrap();
        assert_eq!(catalog.len(), 1);
        assert_eq!(
            catalog.get(&schema("product")).unwrap().owner_module(),
            "product"
        );
        assert!(matches!(
            catalog.register("other", schema("product"), admission),
            Err(PostgresIndexQueryAdmissionError::DuplicateSchema { .. })
        ));
    }

    #[test]
    fn every_runtime_root_receives_the_same_governed_entity_dispatch() {
        let mut catalog = PostgresIndexQueryAdmissionCatalog::new();
        catalog
            .register(
                "product",
                schema("product"),
                PostgresQueryEntityAdmission::new("{{entity}}.source_version > 10").unwrap(),
            )
            .unwrap();
        catalog
            .register(
                "product",
                schema("product_variant"),
                PostgresQueryEntityAdmission::new("{{entity}}.source_version > 20").unwrap(),
            )
            .unwrap();
        let unrelated = SchemaRef {
            module: ModuleName::new("other").unwrap(),
            entity: EntityName::new("item").unwrap(),
            version: SchemaVersion::INITIAL,
        };
        catalog.ensure_runtime_schema(unrelated.clone()).unwrap();

        let product = catalog.get(&schema("product")).unwrap();
        let passthrough = catalog.get(&unrelated).unwrap();
        assert!(product.is_governed());
        assert!(!passthrough.is_governed());
        assert_eq!(product.admission(), passthrough.admission());
        let template = product.admission().template();
        assert!(template.contains("entity_name = 'product'"));
        assert!(template.contains("entity_name = 'product_variant'"));
        assert!(template.contains("source_version > 10"));
        assert!(template.contains("source_version > 20"));
        assert!(!template.contains("entity_name = 'item'"));
    }
}