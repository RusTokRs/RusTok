use std::collections::{BTreeMap, BTreeSet};

use rustok_core::ModuleRuntimeExtensions;
use thiserror::Error;

use crate::{PostgresQueryEntityAdmission, PostgresQueryEntityAdmissionError, SchemaRef};

const ENTITY_ALIAS_TOKEN: &str = "{{entity}}";
const RUNTIME_PASSTHROUGH_OWNER: &str = "index";
const RUNTIME_PASSTHROUGH_RULE: &str = "{{entity}}.entity_id IS NOT NULL";
const AVAILABILITY_LINK_ALIAS: &str = "availability_link";
const AVAILABILITY_TARGET_ALIAS: &str = "availability_target";

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
    required_link_targets: BTreeMap<SchemaRef, String>,
}

impl PostgresIndexQueryAdmissionCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.required_link_targets.is_empty()
    }

    pub fn link_availability_len(&self) -> usize {
        self.required_link_targets.len()
    }

    pub fn get(&self, schema: &SchemaRef) -> Option<&PostgresIndexQueryAdmissionDescriptor> {
        self.entries.get(schema)
    }

    pub fn iter(&self) -> impl Iterator<Item = &PostgresIndexQueryAdmissionDescriptor> {
        self.entries.values()
    }

    pub fn link_availability_iter(&self) -> impl Iterator<Item = (&SchemaRef, &str)> {
        self.required_link_targets
            .iter()
            .map(|(schema, owner)| (schema, owner.as_str()))
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

    pub fn require_current_link_targets(
        &mut self,
        owner_module: impl Into<String>,
        schema: SchemaRef,
    ) -> Result<(), PostgresIndexQueryAdmissionError> {
        let owner_module = owner_module.into();
        validate_owner_module(&owner_module)?;
        if let Some(existing_owner) = self.required_link_targets.get(&schema) {
            return Err(PostgresIndexQueryAdmissionError::DuplicateLinkAvailabilitySchema {
                schema,
                existing_owner: existing_owner.clone(),
                incoming_owner: owner_module,
            });
        }
        self.required_link_targets.insert(schema, owner_module);
        self.rebuild_composite()
    }

    /// Adds a runtime-local pass-through root descriptor for an otherwise ungoverned registered
    /// schema. Pass-through descriptors are never published as owner rules; they exist only in the
    /// immutable query runtime so a query rooted at any registered schema still applies the same
    /// composite owner admission and generic link-target availability policy.
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
        let owner_rules = self
            .entries
            .values()
            .filter_map(|descriptor| {
                descriptor
                    .rule
                    .as_ref()
                    .map(|rule| (descriptor.schema.clone(), rule.template().to_owned()))
            })
            .collect::<BTreeMap<_, _>>();

        let mut governed_schemas = owner_rules.keys().cloned().collect::<BTreeSet<_>>();
        governed_schemas.extend(self.required_link_targets.keys().cloned());
        if governed_schemas.is_empty() {
            return Ok(());
        }

        let target_owner_admission = owner_dispatch_for_alias(&owner_rules, AVAILABILITY_TARGET_ALIAS);
        let link_availability = require_current_link_targets_predicate(&target_owner_admission);
        let guards = governed_schemas
            .iter()
            .map(schema_guard)
            .collect::<Vec<_>>();
        let allowed = governed_schemas
            .iter()
            .map(|schema| {
                let mut predicates = Vec::new();
                if let Some(rule) = owner_rules.get(schema) {
                    predicates.push(format!("({rule})"));
                }
                if self.required_link_targets.contains_key(schema) {
                    predicates.push(format!("({link_availability})"));
                }
                format!(
                    "({} AND {})",
                    schema_guard(schema),
                    predicates.join(" AND ")
                )
            })
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
    #[error(
        "PostgreSQL Index link-target availability for {schema} has multiple owners: existing={existing_owner}, incoming={incoming_owner}"
    )]
    DuplicateLinkAvailabilitySchema {
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

pub fn register_postgres_index_query_link_target_availability(
    extensions: &mut ModuleRuntimeExtensions,
    owner_module: impl Into<String>,
    schema: SchemaRef,
) -> Result<(), PostgresIndexQueryAdmissionError> {
    extensions
        .get_or_insert_with::<PostgresIndexQueryAdmissionCatalog, _>(
            PostgresIndexQueryAdmissionCatalog::new,
        )
        .require_current_link_targets(owner_module, schema)
}

fn require_current_link_targets_predicate(target_owner_admission: &str) -> String {
    format!(
        "NOT EXISTS (SELECT 1 FROM index_links AS {link} WHERE {link}.tenant_id = {entity}.tenant_id AND {link}.source_module = {entity}.module_name AND {link}.source_entity = {entity}.entity_name AND {link}.source_schema_version = {entity}.schema_version AND {link}.source_entity_id = {entity}.entity_id AND {link}.source_locale_key = {entity}.locale_key AND {link}.source_version = {entity}.source_version AND NOT EXISTS (SELECT 1 FROM index_entities AS {target} WHERE {target}.tenant_id = {link}.tenant_id AND {target}.module_name = {link}.target_module AND {target}.entity_name = {link}.target_entity AND {target}.schema_version = {link}.target_schema_version AND {target}.entity_id = {link}.target_entity_id AND {target}.locale_key = {link}.target_locale_key AND {target}.is_deleted = FALSE AND ({target_owner_admission})))",
        link = AVAILABILITY_LINK_ALIAS,
        entity = ENTITY_ALIAS_TOKEN,
        target = AVAILABILITY_TARGET_ALIAS,
    )
}

fn owner_dispatch_for_alias(
    owner_rules: &BTreeMap<SchemaRef, String>,
    alias: &str,
) -> String {
    if owner_rules.is_empty() {
        return "TRUE".to_owned();
    }
    let guards = owner_rules
        .keys()
        .map(|schema| schema_guard_for_alias(schema, alias))
        .collect::<Vec<_>>();
    let allowed = owner_rules
        .iter()
        .map(|(schema, rule)| {
            format!(
                "({} AND ({}))",
                schema_guard_for_alias(schema, alias),
                rule.replace(ENTITY_ALIAS_TOKEN, alias)
            )
        })
        .collect::<Vec<_>>();
    format!("(NOT ({}) OR {})", guards.join(" OR "), allowed.join(" OR "))
}

fn schema_guard(schema: &SchemaRef) -> String {
    schema_guard_for_alias(schema, ENTITY_ALIAS_TOKEN)
}

fn schema_guard_for_alias(schema: &SchemaRef, alias: &str) -> String {
    format!(
        "({alias}.module_name = '{}' AND {alias}.entity_name = '{}' AND {alias}.schema_version = {})",
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

    #[test]
    fn link_target_availability_uses_current_source_links_and_owner_admitted_targets() {
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
        catalog
            .require_current_link_targets("product", schema("product"))
            .unwrap();

        assert_eq!(catalog.link_availability_len(), 1);
        let template = catalog
            .get(&schema("product"))
            .unwrap()
            .admission()
            .template();
        for marker in [
            "NOT EXISTS (SELECT 1 FROM index_links AS availability_link",
            "availability_link.source_version = {{entity}}.source_version",
            "NOT EXISTS (SELECT 1 FROM index_entities AS availability_target",
            "availability_target.entity_id = availability_link.target_entity_id",
            "availability_target.locale_key = availability_link.target_locale_key",
            "availability_target.is_deleted = FALSE",
            "availability_target.source_version > 20",
        ] {
            assert!(template.contains(marker), "missing {marker}");
        }
    }

    #[test]
    fn duplicate_link_target_availability_owner_fails_closed() {
        let mut catalog = PostgresIndexQueryAdmissionCatalog::new();
        catalog
            .require_current_link_targets("product", schema("product"))
            .unwrap();
        assert!(matches!(
            catalog.require_current_link_targets("other", schema("product")),
            Err(PostgresIndexQueryAdmissionError::DuplicateLinkAvailabilitySchema { .. })
        ));
    }
}