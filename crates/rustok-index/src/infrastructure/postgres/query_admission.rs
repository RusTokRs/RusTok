use std::collections::BTreeMap;

use rustok_core::ModuleRuntimeExtensions;
use thiserror::Error;

use crate::{PostgresQueryRootAdmission, PostgresQueryRootAdmissionError, SchemaRef};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresIndexQueryAdmissionDescriptor {
    owner_module: String,
    schema: SchemaRef,
    admission: PostgresQueryRootAdmission,
}

impl PostgresIndexQueryAdmissionDescriptor {
    pub fn owner_module(&self) -> &str {
        &self.owner_module
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    pub fn admission(&self) -> &PostgresQueryRootAdmission {
        &self.admission
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
        admission: PostgresQueryRootAdmission,
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
                admission,
            },
        );
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
    InvalidAdmission(#[from] PostgresQueryRootAdmissionError),
}

pub fn register_postgres_index_query_admission(
    extensions: &mut ModuleRuntimeExtensions,
    owner_module: impl Into<String>,
    schema: SchemaRef,
    admission: PostgresQueryRootAdmission,
) -> Result<(), PostgresIndexQueryAdmissionError> {
    extensions
        .get_or_insert_with::<PostgresIndexQueryAdmissionCatalog, _>(
            PostgresIndexQueryAdmissionCatalog::new,
        )
        .register(owner_module, schema, admission)
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

    fn schema() -> SchemaRef {
        SchemaRef {
            module: ModuleName::new("rustok-product").unwrap(),
            entity: EntityName::new("product").unwrap(),
            version: SchemaVersion::new(3),
        }
    }

    #[test]
    fn exact_schema_has_one_query_admission_owner() {
        let admission = PostgresQueryRootAdmission::new("{{root}}.source_version > 0").unwrap();
        let mut catalog = PostgresIndexQueryAdmissionCatalog::new();
        catalog
            .register("product", schema(), admission.clone())
            .unwrap();
        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog.get(&schema()).unwrap().owner_module(), "product");
        assert!(matches!(
            catalog.register("other", schema(), admission),
            Err(PostgresIndexQueryAdmissionError::DuplicateSchema { .. })
        ));
    }
}
