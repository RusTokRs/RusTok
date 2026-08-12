use std::collections::{BTreeMap, BTreeSet};

use rustok_core::ModuleRuntimeExtensions;
use thiserror::Error;

use crate::{
    CompiledPostgresQuery, IndexQuery, PostgresQueryEntityAdmission,
    PostgresQueryEntityAdmissionError, SchemaRef,
};

const ENTITY_ALIAS_TOKEN: &str = "{{entity}}";
const RUNTIME_PASSTHROUGH_OWNER: &str = "index";
const RUNTIME_PASSTHROUGH_RULE: &str = "{{entity}}.entity_id IS NOT NULL";
const ROOT_ALIAS: &str = "\"t0\"";
const ROOT_ANCHOR: &str = "\"t0\".is_deleted = FALSE";
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
        self.rebuild_owner_composite()
    }

    pub fn require_current_link_targets(
        &mut self,
        owner_module: impl Into<String>,
        schema: SchemaRef,
    ) -> Result<(), PostgresIndexQueryAdmissionError> {
        let owner_module = owner_module.into();
        validate_owner_module(&owner_module)?;
        if let Some(existing_owner) = self.required_link_targets.get(&schema) {
            return Err(
                PostgresIndexQueryAdmissionError::DuplicateLinkAvailabilitySchema {
                    schema,
                    existing_owner: existing_owner.clone(),
                    incoming_owner: owner_module,
                },
            );
        }
        self.required_link_targets.insert(schema, owner_module);
        Ok(())
    }

    /// Applies a root availability predicate only for link names the validated query actually uses.
    /// Scalar-only queries therefore do not become dependent on unrelated linked target materialization.
    /// The policy is one-hop by construction: deeper paths are checked at their first root link here,
    /// while target owner freshness is reused inside the target lookup. Current Product graph targets
    /// are link-free, so no recursive SQL or owner-specific compiler behavior is required.
    pub(crate) fn apply_link_target_availability(
        &self,
        query: &IndexQuery,
        compiled: &mut CompiledPostgresQuery,
    ) -> Result<(), PostgresIndexQueryLinkAvailabilityApplyError> {
        if !self.required_link_targets.contains_key(&query.schema) {
            return Ok(());
        }
        let link_names = query
            .referenced_paths()
            .into_iter()
            .filter_map(|path| path.links().first().map(|link| link.as_str().to_owned()))
            .collect::<BTreeSet<_>>();
        if link_names.is_empty() {
            return Ok(());
        }

        let owner_rules = self.owner_rules();
        let target_owner_admission =
            owner_dispatch_for_alias(&owner_rules, AVAILABILITY_TARGET_ALIAS);
        let predicate =
            require_requested_link_targets_predicate(&link_names, &target_owner_admission);
        apply_root_predicate(&mut compiled.sql, &predicate)?;
        if let Some(exact_count) = compiled.exact_count.as_mut() {
            apply_root_predicate(&mut exact_count.sql, &predicate)?;
        }
        Ok(())
    }

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
        self.rebuild_owner_composite()
    }

    fn owner_rules(&self) -> BTreeMap<SchemaRef, String> {
        self.entries
            .values()
            .filter_map(|descriptor| {
                descriptor
                    .rule
                    .as_ref()
                    .map(|rule| (descriptor.schema.clone(), rule.template().to_owned()))
            })
            .collect()
    }

    fn rebuild_owner_composite(&mut self) -> Result<(), PostgresIndexQueryAdmissionError> {
        let owner_rules = self.owner_rules();
        if owner_rules.is_empty() {
            return Ok(());
        }
        let composite = PostgresQueryEntityAdmission::new(owner_dispatch_for_alias(
            &owner_rules,
            ENTITY_ALIAS_TOKEN,
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

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub(crate) enum PostgresIndexQueryLinkAvailabilityApplyError {
    #[error(
        "compiled PostgreSQL query root admission anchor count is {actual}, expected exactly one"
    )]
    RootAnchorMismatch { actual: usize },
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

fn apply_root_predicate(
    sql: &mut String,
    predicate: &str,
) -> Result<(), PostgresIndexQueryLinkAvailabilityApplyError> {
    let actual = sql.matches(ROOT_ANCHOR).count();
    if actual != 1 {
        return Err(PostgresIndexQueryLinkAvailabilityApplyError::RootAnchorMismatch { actual });
    }
    *sql = sql.replacen(ROOT_ANCHOR, &format!("{ROOT_ANCHOR} AND ({predicate})"), 1);
    Ok(())
}

fn require_requested_link_targets_predicate(
    link_names: &BTreeSet<String>,
    target_owner_admission: &str,
) -> String {
    let requested_links = link_names
        .iter()
        .map(|link| format!("'{}'", sql_literal(link)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "NOT EXISTS (SELECT 1 FROM index_links AS {link} WHERE {link}.tenant_id = {root}.tenant_id AND {link}.source_module = {root}.module_name AND {link}.source_entity = {root}.entity_name AND {link}.source_schema_version = {root}.schema_version AND {link}.source_entity_id = {root}.entity_id AND {link}.source_locale_key = {root}.locale_key AND {link}.source_version = {root}.source_version AND {link}.link_name IN ({requested_links}) AND NOT EXISTS (SELECT 1 FROM index_entities AS {target} WHERE {target}.tenant_id = {link}.tenant_id AND {target}.module_name = {link}.target_module AND {target}.entity_name = {link}.target_entity AND {target}.schema_version = {link}.target_schema_version AND {target}.entity_id = {link}.target_entity_id AND {target}.locale_key = {link}.target_locale_key AND {target}.is_deleted = FALSE AND ({target_owner_admission})))",
        link = AVAILABILITY_LINK_ALIAS,
        root = ROOT_ALIAS,
        target = AVAILABILITY_TARGET_ALIAS,
    )
}

fn owner_dispatch_for_alias(owner_rules: &BTreeMap<SchemaRef, String>, alias: &str) -> String {
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
    format!(
        "(NOT ({}) OR {})",
        guards.join(" OR "),
        allowed.join(" OR ")
    )
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
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
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
    use crate::{
        EntityName, FieldName, FieldPath, IndexQueryScope, LinkName, LocaleKey, ModuleName,
        Pagination, SchemaVersion,
    };
    use uuid::Uuid;

    fn schema(entity: &str) -> SchemaRef {
        SchemaRef {
            module: ModuleName::new("rustok-product").unwrap(),
            entity: EntityName::new(entity).unwrap(),
            version: SchemaVersion::INITIAL,
        }
    }

    fn query_with_fields(fields: Vec<FieldPath>) -> IndexQuery {
        IndexQuery {
            scope: IndexQueryScope {
                tenant_id: Uuid::new_v4(),
                locale: Some(LocaleKey::new("en").unwrap()),
            },
            schema: schema("product"),
            fields,
            filter: None,
            order_by: Vec::new(),
            pagination: Pagination::Offset {
                limit: 10,
                offset: 0,
            },
            include_exact_count: true,
        }
    }

    fn compiled() -> CompiledPostgresQuery {
        CompiledPostgresQuery {
            sql: "SELECT 1 FROM index_entities AS \"t0\" WHERE \"t0\".is_deleted = FALSE"
                .to_owned(),
            binds: Vec::new(),
            columns: Vec::new(),
            many_relations: Vec::new(),
            exact_count: Some(crate::CompiledPostgresCount {
                sql:
                    "SELECT COUNT(*) FROM index_entities AS \"t0\" WHERE \"t0\".is_deleted = FALSE"
                        .to_owned(),
                binds: Vec::new(),
            }),
            plan_fingerprint: serde_json::from_value(serde_json::to_value([0_u8; 32]).unwrap())
                .unwrap(),
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
        assert!(matches!(
            catalog.register("other", schema("product"), admission),
            Err(PostgresIndexQueryAdmissionError::DuplicateSchema { .. })
        ));
    }

    #[test]
    fn every_runtime_root_receives_the_same_owner_entity_dispatch() {
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
        assert_eq!(
            catalog.get(&schema("product")).unwrap().admission(),
            catalog.get(&unrelated).unwrap().admission()
        );
    }

    #[test]
    fn scalar_only_query_does_not_require_unreferenced_link_targets() {
        let mut catalog = PostgresIndexQueryAdmissionCatalog::new();
        catalog
            .register(
                "product",
                schema("product"),
                PostgresQueryEntityAdmission::new("{{entity}}.source_version > 10").unwrap(),
            )
            .unwrap();
        catalog
            .require_current_link_targets("product", schema("product"))
            .unwrap();
        let query = query_with_fields(vec![FieldPath::new(FieldName::new("title").unwrap())]);
        let mut compiled = compiled();
        catalog
            .apply_link_target_availability(&query, &mut compiled)
            .unwrap();
        assert!(!compiled.sql.contains("availability_link"));
        assert!(
            !compiled
                .exact_count
                .unwrap()
                .sql
                .contains("availability_link")
        );
    }

    #[test]
    fn queried_link_requires_current_owner_admitted_target_in_page_and_count() {
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
        let query = query_with_fields(vec![FieldPath::linked(
            [LinkName::new("variants").unwrap()],
            FieldName::new("sku").unwrap(),
        )]);
        let mut compiled = compiled();
        catalog
            .apply_link_target_availability(&query, &mut compiled)
            .unwrap();
        for sql in [
            compiled.sql.as_str(),
            compiled.exact_count.as_ref().unwrap().sql.as_str(),
        ] {
            for marker in [
                "FROM index_links AS availability_link",
                "availability_link.source_version = \"t0\".source_version",
                "availability_link.link_name IN ('variants')",
                "FROM index_entities AS availability_target",
                "availability_target.entity_id = availability_link.target_entity_id",
                "availability_target.source_version > 20",
            ] {
                assert!(sql.contains(marker), "missing {marker}");
            }
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
