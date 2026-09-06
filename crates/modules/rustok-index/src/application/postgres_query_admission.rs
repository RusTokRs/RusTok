use std::collections::BTreeSet;

use thiserror::Error;

use super::{CompiledPostgresQuery, postgres_compiler::quote_identifier};

const ENTITY_ALIAS_TOKEN: &str = "{{entity}}";
const MAX_ENTITY_ADMISSION_BYTES: usize = 32 * 1024;
const INDEX_ENTITY_ALIAS_MARKER: &str = "index_entities AS \"";

/// Trusted PostgreSQL predicate applied to every compiler-owned `index_entities` relation before
/// filtering, ordering, pagination, nested projection, and exact-count evaluation.
///
/// Module-owned admission rules are source code, not user input. They intentionally accept no bind
/// placeholders: tenant/entity/locale correlation must flow through the controlled entity alias.
/// This keeps query bind numbering, plan fingerprints, and cursor contracts independent of runtime
/// admission state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresQueryEntityAdmission {
    template: String,
}

impl PostgresQueryEntityAdmission {
    pub fn new(template: impl Into<String>) -> Result<Self, PostgresQueryEntityAdmissionError> {
        let template = template.into();
        validate_template(&template)?;
        Ok(Self { template })
    }

    pub fn template(&self) -> &str {
        &self.template
    }

    /// Applies the trusted predicate to every materialized entity relation in the page query and
    /// optional exact-count query without changing binds, selected columns, plan fingerprint, or
    /// cursor semantics.
    ///
    /// Relation aliases are discovered only from compiler-owned `index_entities AS "..."` clauses
    /// and must match one of the canonical compiler alias families. Every discovered alias must have
    /// at least one canonical `is_deleted = FALSE` anchor. Compiler drift therefore fails closed
    /// instead of silently bypassing owner admission on one query path.
    pub fn apply(
        &self,
        compiled: &mut CompiledPostgresQuery,
    ) -> Result<(), PostgresQueryEntityAdmissionApplyError> {
        apply_to_sql(&mut compiled.sql, self)?;
        if let Some(exact_count) = compiled.exact_count.as_mut() {
            apply_to_sql(&mut exact_count.sql, self)?;
        }
        Ok(())
    }

    pub fn render(&self, entity_alias: &str) -> String {
        self.template
            .replace(ENTITY_ALIAS_TOKEN, &quote_identifier(entity_alias))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum PostgresQueryEntityAdmissionError {
    #[error("PostgreSQL entity query admission predicate is empty")]
    Empty,
    #[error("PostgreSQL entity query admission predicate is too large")]
    TooLarge,
    #[error(
        "PostgreSQL entity query admission predicate must reference the controlled entity alias"
    )]
    MissingEntityAlias,
    #[error("PostgreSQL entity query admission predicate contains a forbidden SQL boundary")]
    ForbiddenSqlBoundary,
    #[error("PostgreSQL entity query admission predicate contains a bind placeholder")]
    BindPlaceholderForbidden,
    #[error("PostgreSQL entity query admission predicate contains a control character")]
    ControlCharacter,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum PostgresQueryEntityAdmissionApplyError {
    #[error("compiled PostgreSQL query contains no materialized entity relation")]
    MissingEntityRelation,
    #[error("compiled PostgreSQL query contains an invalid materialized entity alias: {0}")]
    InvalidEntityAlias(String),
    #[error("compiled PostgreSQL query has no canonical admission anchor for entity alias {0}")]
    EntityAnchorMissing(String),
}

fn validate_template(template: &str) -> Result<(), PostgresQueryEntityAdmissionError> {
    if template.trim().is_empty() {
        return Err(PostgresQueryEntityAdmissionError::Empty);
    }
    if template.len() > MAX_ENTITY_ADMISSION_BYTES {
        return Err(PostgresQueryEntityAdmissionError::TooLarge);
    }
    if !template.contains(ENTITY_ALIAS_TOKEN) {
        return Err(PostgresQueryEntityAdmissionError::MissingEntityAlias);
    }
    if template.contains(';')
        || template.contains("--")
        || template.contains("/*")
        || template.contains("*/")
    {
        return Err(PostgresQueryEntityAdmissionError::ForbiddenSqlBoundary);
    }
    if template.contains('$') {
        return Err(PostgresQueryEntityAdmissionError::BindPlaceholderForbidden);
    }
    if template
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(PostgresQueryEntityAdmissionError::ControlCharacter);
    }
    Ok(())
}

fn entity_aliases(sql: &str) -> Result<BTreeSet<String>, PostgresQueryEntityAdmissionApplyError> {
    let mut aliases = BTreeSet::new();
    let mut remainder = sql;
    while let Some(offset) = remainder.find(INDEX_ENTITY_ALIAS_MARKER) {
        let after_marker = &remainder[offset + INDEX_ENTITY_ALIAS_MARKER.len()..];
        let Some(end) = after_marker.find('"') else {
            return Err(PostgresQueryEntityAdmissionApplyError::InvalidEntityAlias(
                after_marker.to_owned(),
            ));
        };
        let alias = &after_marker[..end];
        validate_relation_alias(alias)?;
        aliases.insert(alias.to_owned());
        remainder = &after_marker[end + 1..];
    }
    if aliases.is_empty() {
        return Err(PostgresQueryEntityAdmissionApplyError::MissingEntityRelation);
    }
    Ok(aliases)
}

fn validate_relation_alias(alias: &str) -> Result<(), PostgresQueryEntityAdmissionApplyError> {
    let plain = numeric_suffix(alias, "t");
    let many_projection = alias.strip_prefix("mp").is_some_and(|remainder| {
        let Some((projection, target)) = remainder.split_once("_t") else {
            return false;
        };
        numeric_component(projection) && numeric_component(target)
    });
    let many_filter = alias.strip_prefix("mx_t").is_some_and(numeric_component);
    let many_order = alias.strip_prefix("mo_t").is_some_and(numeric_component);
    if plain || many_projection || many_filter || many_order {
        Ok(())
    } else {
        Err(PostgresQueryEntityAdmissionApplyError::InvalidEntityAlias(
            alias.to_owned(),
        ))
    }
}

fn numeric_suffix(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(numeric_component)
}

fn numeric_component(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn apply_to_sql(
    sql: &mut String,
    admission: &PostgresQueryEntityAdmission,
) -> Result<(), PostgresQueryEntityAdmissionApplyError> {
    let aliases = entity_aliases(sql)?;
    for alias in aliases {
        let alias_q = quote_identifier(&alias);
        let anchor = format!("{alias_q}.is_deleted = FALSE");
        if !sql.contains(&anchor) {
            return Err(PostgresQueryEntityAdmissionApplyError::EntityAnchorMissing(
                alias,
            ));
        }
        let rendered = admission.render(&alias);
        let replacement = format!("{anchor} AND ({rendered})");
        *sql = sql.replace(&anchor, &replacement);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_requires_controlled_alias_without_sql_boundaries_or_binds() {
        assert!(PostgresQueryEntityAdmission::new("{{entity}}.source_version > 0").is_ok());
        for invalid in [
            "",
            "TRUE",
            "{{entity}}.source_version = $1",
            "{{entity}}.source_version > 0; SELECT 1",
            "{{entity}}.source_version > 0 -- bypass",
        ] {
            assert!(
                PostgresQueryEntityAdmission::new(invalid).is_err(),
                "{invalid}"
            );
        }
    }

    #[test]
    fn admission_renders_template_with_quoted_identifier() {
        let admission = PostgresQueryEntityAdmission::new("{{entity}}.source_version > 0").unwrap();
        assert_eq!(admission.render("t1"), "\"t1\".source_version > 0");
    }

    #[test]
    fn admission_renders_only_the_compiler_owned_entity_alias() {
        let admission = PostgresQueryEntityAdmission::new(
            "EXISTS (SELECT 1 WHERE {{entity}}.source_version > 0 AND {{entity}}.is_deleted = FALSE)",
        )
        .unwrap();
        let rendered = admission.render("mx_t1");
        assert!(rendered.contains("\"mx_t1\".source_version"));
        assert!(!rendered.contains(ENTITY_ALIAS_TOKEN));
    }

    #[test]
    fn admission_covers_root_outer_many_projection_filter_and_aggregate_aliases() {
        let mut sql = concat!(
            "SELECT 1 FROM index_entities AS \"t0\" ",
            "LEFT JOIN index_entities AS \"t1\" ON \"t1\".is_deleted = FALSE ",
            "WHERE \"t0\".is_deleted = FALSE ",
            "AND EXISTS (SELECT 1 FROM index_entities AS \"mp0_t1\" WHERE \"mp0_t1\".is_deleted = FALSE) ",
            "AND EXISTS (SELECT 1 FROM index_entities AS \"mx_t1\" WHERE \"mx_t1\".is_deleted = FALSE) ",
            "AND EXISTS (SELECT 1 FROM index_entities AS \"mo_t1\" WHERE \"mo_t1\".is_deleted = FALSE)"
        )
        .to_owned();
        let admission = PostgresQueryEntityAdmission::new("{{entity}}.source_version > 0").unwrap();
        apply_to_sql(&mut sql, &admission).unwrap();
        for alias in ["t0", "t1", "mp0_t1", "mx_t1", "mo_t1"] {
            let marker =
                format!("\"{alias}\".is_deleted = FALSE AND (\"{alias}\".source_version > 0)");
            assert!(sql.contains(&marker), "missing {marker}");
        }
    }

    #[test]
    fn compiler_alias_or_anchor_drift_fails_closed() {
        assert_eq!(
            entity_aliases("SELECT 1 FROM index_entities AS \"future_alias\""),
            Err(PostgresQueryEntityAdmissionApplyError::InvalidEntityAlias(
                "future_alias".to_owned()
            ))
        );
        let mut sql = "SELECT 1 FROM index_entities AS \"t0\"".to_owned();
        let admission = PostgresQueryEntityAdmission::new("{{entity}}.source_version > 0").unwrap();
        assert_eq!(
            apply_to_sql(&mut sql, &admission),
            Err(PostgresQueryEntityAdmissionApplyError::EntityAnchorMissing(
                "t0".to_owned()
            ))
        );
    }
}
