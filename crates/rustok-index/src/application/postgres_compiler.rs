use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{LinkCardinality, Pagination};

use super::{ExecutableQueryPlan, PlannedField, QueryPlanFingerprint};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PostgresBindValue {
    Uuid(Uuid),
    Text(String),
    Integer(i64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CompiledQueryColumn {
    EntityId {
        output_alias: String,
        relation_alias: String,
    },
    Field {
        output_alias: String,
        field: PlannedField,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledPostgresQuery {
    pub sql: String,
    pub binds: Vec<PostgresBindValue>,
    pub columns: Vec<CompiledQueryColumn>,
    pub plan_fingerprint: QueryPlanFingerprint,
}

#[derive(Debug, Error)]
pub enum PostgresQueryCompileError {
    #[error("query filters require the next M4 typed predicate compiler slice")]
    FilterPending,
    #[error("query ordering requires the next M4 typed ordering compiler slice")]
    OrderingPending,
    #[error("exact count requires the next M4 count compiler slice")]
    ExactCountPending,
    #[error("cursor continuation requires the next M4 keyset predicate compiler slice")]
    CursorContinuationPending,
    #[error("offset pagination requires the bounded compatibility compiler slice")]
    OffsetPaginationPending,
    #[error("many-cardinality link projection requires nested result aggregation")]
    ManyLinkProjectionPending,
    #[error("query plan relation aliases do not match the path-alias map")]
    AliasMappingMismatch,
    #[error("query plan contains an invalid relation alias: {0}")]
    InvalidRelationAlias(String),
    #[error("query plan fingerprint serialization failed: {0}")]
    Fingerprint(#[from] postcard::Error),
}

impl ExecutableQueryPlan {
    /// Compile the currently executable M4 subset into controlled PostgreSQL SQL.
    ///
    /// This slice intentionally accepts projection-only plans with root or
    /// one-cardinality link fields and a fresh cursor page. Typed predicates,
    /// explicit ordering, exact count, cursor continuation, many-link aggregation,
    /// and offset compatibility remain fail-closed follow-up work.
    pub fn compile_postgres(&self) -> Result<CompiledPostgresQuery, PostgresQueryCompileError> {
        self.validate_compiler_subset()?;

        let mut bindings = Bindings::default();
        let root_alias = quote_identifier(&self.root_alias);
        let tenant = bindings.push(PostgresBindValue::Uuid(self.scope.tenant_id));
        let module = bindings.push(PostgresBindValue::Text(
            self.root_schema.module.as_str().to_owned(),
        ));
        let entity = bindings.push(PostgresBindValue::Text(
            self.root_schema.entity.as_str().to_owned(),
        ));
        let version = bindings.push(PostgresBindValue::Integer(i64::from(
            self.root_schema.version.get(),
        )));
        let locale = bindings.push(PostgresBindValue::Text(
            self.scope
                .locale
                .as_ref()
                .map_or_else(String::new, |locale| locale.as_str().to_owned()),
        ));

        let mut select = Vec::new();
        let mut columns = Vec::new();
        push_identity_column(
            &mut select,
            &mut columns,
            &self.root_alias,
            &root_alias,
        );

        let mut joins = Vec::new();
        for (index, join) in self.joins.iter().enumerate() {
            let source_alias = quote_identifier(&join.source_alias);
            let target_alias = quote_identifier(&join.alias);
            let link_alias = format!("l{}", index + 1);
            let link_alias_q = quote_identifier(&link_alias);
            let link_name = bindings.push(PostgresBindValue::Text(join.link.as_str().to_owned()));
            let target_module = bindings.push(PostgresBindValue::Text(
                join.target_schema.module.as_str().to_owned(),
            ));
            let target_entity = bindings.push(PostgresBindValue::Text(
                join.target_schema.entity.as_str().to_owned(),
            ));
            let target_version = bindings.push(PostgresBindValue::Integer(i64::from(
                join.target_schema.version.get(),
            )));

            joins.push(format!(
                "LEFT JOIN index_links AS {link_alias_q} ON {link_alias_q}.tenant_id = {source_alias}.tenant_id AND {link_alias_q}.source_module = {source_alias}.module_name AND {link_alias_q}.source_entity = {source_alias}.entity_name AND {link_alias_q}.source_schema_version = {source_alias}.schema_version AND {link_alias_q}.source_entity_id = {source_alias}.entity_id AND {link_alias_q}.source_locale_key = {source_alias}.locale_key AND {link_alias_q}.source_version = {source_alias}.source_version AND {link_alias_q}.link_name = {link_name} AND {link_alias_q}.target_module = {target_module} AND {link_alias_q}.target_entity = {target_entity} AND {link_alias_q}.target_schema_version = {target_version} LEFT JOIN index_entities AS {target_alias} ON {target_alias}.tenant_id = {link_alias_q}.tenant_id AND {target_alias}.module_name = {link_alias_q}.target_module AND {target_alias}.entity_name = {link_alias_q}.target_entity AND {target_alias}.schema_version = {link_alias_q}.target_schema_version AND {target_alias}.entity_id = {link_alias_q}.target_entity_id AND {target_alias}.locale_key = {link_alias_q}.target_locale_key AND {target_alias}.is_deleted = FALSE",
            ));
            push_identity_column(
                &mut select,
                &mut columns,
                &join.alias,
                &target_alias,
            );
        }

        for (index, field) in self.projection.iter().enumerate() {
            let relation_alias = quote_identifier(&field.relation_alias);
            let field_name = bindings.push(PostgresBindValue::Text(
                field.path.field().as_str().to_owned(),
            ));
            let output_alias = format!("f{index}");
            select.push(format!(
                "jsonb_extract_path({relation_alias}.payload, {field_name}::text) AS {}",
                quote_identifier(&output_alias),
            ));
            columns.push(CompiledQueryColumn::Field {
                output_alias,
                field: field.clone(),
            });
        }

        let limit = match &self.pagination {
            Pagination::Cursor { first, after: None } => {
                bindings.push(PostgresBindValue::Integer(i64::from(*first)))
            }
            Pagination::Cursor { after: Some(_), .. } => {
                return Err(PostgresQueryCompileError::CursorContinuationPending);
            }
            Pagination::Offset { .. } => {
                return Err(PostgresQueryCompileError::OffsetPaginationPending);
            }
        };

        let mut sql = format!(
            "SELECT {} FROM index_entities AS {root_alias}",
            select.join(", ")
        );
        if !joins.is_empty() {
            sql.push(' ');
            sql.push_str(&joins.join(" "));
        }
        sql.push_str(&format!(
            " WHERE {root_alias}.tenant_id = {tenant} AND {root_alias}.module_name = {module} AND {root_alias}.entity_name = {entity} AND {root_alias}.schema_version = {version} AND {root_alias}.locale_key = {locale} AND {root_alias}.is_deleted = FALSE ORDER BY {root_alias}.entity_id ASC LIMIT {limit}"
        ));

        Ok(CompiledPostgresQuery {
            sql,
            binds: bindings.values,
            columns,
            plan_fingerprint: self.fingerprint()?,
        })
    }

    fn validate_compiler_subset(&self) -> Result<(), PostgresQueryCompileError> {
        validate_alias(&self.root_alias)?;
        if self.path_aliases.get(&Vec::new()).map(String::as_str)
            != Some(self.root_alias.as_str())
        {
            return Err(PostgresQueryCompileError::AliasMappingMismatch);
        }
        if self.filter.is_some() {
            return Err(PostgresQueryCompileError::FilterPending);
        }
        if !self.order_by.is_empty() {
            return Err(PostgresQueryCompileError::OrderingPending);
        }
        if self.include_exact_count {
            return Err(PostgresQueryCompileError::ExactCountPending);
        }
        for join in &self.joins {
            validate_alias(&join.source_alias)?;
            validate_alias(&join.alias)?;
            if join.path.is_empty() {
                return Err(PostgresQueryCompileError::AliasMappingMismatch);
            }
            let parent_path = join.path[..join.path.len() - 1].to_vec();
            if self.path_aliases.get(&parent_path).map(String::as_str)
                != Some(join.source_alias.as_str())
                || self.path_aliases.get(&join.path).map(String::as_str)
                    != Some(join.alias.as_str())
            {
                return Err(PostgresQueryCompileError::AliasMappingMismatch);
            }
            if join.cardinality == LinkCardinality::Many {
                return Err(PostgresQueryCompileError::ManyLinkProjectionPending);
            }
        }
        for field in &self.projection {
            validate_alias(&field.relation_alias)?;
            if self.path_aliases.get(field.path.links()).map(String::as_str)
                != Some(field.relation_alias.as_str())
            {
                return Err(PostgresQueryCompileError::AliasMappingMismatch);
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct Bindings {
    values: Vec<PostgresBindValue>,
}

impl Bindings {
    fn push(&mut self, value: PostgresBindValue) -> String {
        self.values.push(value);
        format!("${}", self.values.len())
    }
}

fn push_identity_column(
    select: &mut Vec<String>,
    columns: &mut Vec<CompiledQueryColumn>,
    relation_alias: &str,
    relation_alias_q: &str,
) {
    let output_alias = format!("__{relation_alias}_entity_id");
    select.push(format!(
        "{relation_alias_q}.entity_id AS {}",
        quote_identifier(&output_alias)
    ));
    columns.push(CompiledQueryColumn::EntityId {
        output_alias,
        relation_alias: relation_alias.to_owned(),
    });
}

fn validate_alias(alias: &str) -> Result<(), PostgresQueryCompileError> {
    let valid = alias.strip_prefix('t').is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    });
    if valid {
        Ok(())
    } else {
        Err(PostgresQueryCompileError::InvalidRelationAlias(
            alias.to_owned(),
        ))
    }
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
