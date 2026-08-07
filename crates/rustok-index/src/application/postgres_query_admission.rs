use thiserror::Error;

use super::{CompiledPostgresQuery, CompiledQueryColumn, postgres_compiler::quote_identifier};

const ROOT_ALIAS_TOKEN: &str = "{{root}}";
const MAX_ROOT_ADMISSION_BYTES: usize = 32 * 1024;

/// Trusted PostgreSQL predicate applied to the root `index_entities` row before filtering,
/// ordering, pagination, and exact-count evaluation.
///
/// Module-owned admission rules are source code, not user input. They intentionally accept no bind
/// placeholders: all tenant/entity/locale correlation must flow through the controlled root alias.
/// This keeps query bind numbering and cursor fingerprints independent of runtime admission state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresQueryRootAdmission {
    template: String,
}

impl PostgresQueryRootAdmission {
    pub fn new(template: impl Into<String>) -> Result<Self, PostgresQueryRootAdmissionError> {
        let template = template.into();
        validate_template(&template)?;
        Ok(Self { template })
    }

    pub fn template(&self) -> &str {
        &self.template
    }

    /// Applies the trusted predicate to a validated compiler product without changing bind values,
    /// selected columns, query-plan fingerprint, or cursor semantics.
    ///
    /// The compiler-owned root baseline is intentionally treated as a fail-closed anchor. If page or
    /// exact-count SQL no longer contains exactly one canonical anchor, the query is rejected rather
    /// than silently running without admission.
    pub fn apply(
        &self,
        compiled: &mut CompiledPostgresQuery,
    ) -> Result<(), PostgresQueryRootAdmissionApplyError> {
        let root_alias = compiled
            .columns
            .iter()
            .find_map(|column| match column {
                CompiledQueryColumn::EntityId { relation_alias, .. } => Some(relation_alias.as_str()),
                _ => None,
            })
            .ok_or(PostgresQueryRootAdmissionApplyError::MissingRootIdentity)?;
        validate_relation_alias(root_alias)?;
        let rendered = self.render(root_alias);
        apply_to_sql(&mut compiled.sql, root_alias, &rendered)?;
        if let Some(exact_count) = compiled.exact_count.as_mut() {
            apply_to_sql(&mut exact_count.sql, root_alias, &rendered)?;
        }
        Ok(())
    }

    fn render(&self, root_alias: &str) -> String {
        self.template
            .replace(ROOT_ALIAS_TOKEN, &quote_identifier(root_alias))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum PostgresQueryRootAdmissionError {
    #[error("PostgreSQL root query admission predicate is empty")]
    Empty,
    #[error("PostgreSQL root query admission predicate is too large")]
    TooLarge,
    #[error("PostgreSQL root query admission predicate must reference the controlled root alias")]
    MissingRootAlias,
    #[error("PostgreSQL root query admission predicate contains a forbidden SQL boundary")]
    ForbiddenSqlBoundary,
    #[error("PostgreSQL root query admission predicate contains a bind placeholder")]
    BindPlaceholderForbidden,
    #[error("PostgreSQL root query admission predicate contains a control character")]
    ControlCharacter,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum PostgresQueryRootAdmissionApplyError {
    #[error("compiled PostgreSQL query has no root entity identity column")]
    MissingRootIdentity,
    #[error("compiled PostgreSQL query root alias is invalid")]
    InvalidRootAlias,
    #[error("compiled PostgreSQL query does not contain exactly one canonical root admission anchor")]
    RootAnchorMismatch,
}

fn validate_template(template: &str) -> Result<(), PostgresQueryRootAdmissionError> {
    if template.trim().is_empty() {
        return Err(PostgresQueryRootAdmissionError::Empty);
    }
    if template.len() > MAX_ROOT_ADMISSION_BYTES {
        return Err(PostgresQueryRootAdmissionError::TooLarge);
    }
    if !template.contains(ROOT_ALIAS_TOKEN) {
        return Err(PostgresQueryRootAdmissionError::MissingRootAlias);
    }
    if template.contains(';')
        || template.contains("--")
        || template.contains("/*")
        || template.contains("*/")
    {
        return Err(PostgresQueryRootAdmissionError::ForbiddenSqlBoundary);
    }
    if template.contains('$') {
        return Err(PostgresQueryRootAdmissionError::BindPlaceholderForbidden);
    }
    if template
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(PostgresQueryRootAdmissionError::ControlCharacter);
    }
    Ok(())
}

fn validate_relation_alias(alias: &str) -> Result<(), PostgresQueryRootAdmissionApplyError> {
    let valid = alias.strip_prefix('t').is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    });
    if valid {
        Ok(())
    } else {
        Err(PostgresQueryRootAdmissionApplyError::InvalidRootAlias)
    }
}

fn root_baseline(root_alias: &str) -> String {
    let root = quote_identifier(root_alias);
    format!(
        "{root}.tenant_id = $1 AND {root}.module_name = $2 AND {root}.entity_name = $3 AND {root}.schema_version = $4 AND {root}.is_deleted = FALSE"
    )
}

fn apply_to_sql(
    sql: &mut String,
    root_alias: &str,
    rendered: &str,
) -> Result<(), PostgresQueryRootAdmissionApplyError> {
    let anchor = root_baseline(root_alias);
    if sql.match_indices(&anchor).count() != 1 {
        return Err(PostgresQueryRootAdmissionApplyError::RootAnchorMismatch);
    }
    let replacement = format!("{anchor} AND ({rendered})");
    *sql = sql.replacen(&anchor, &replacement, 1);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_requires_controlled_alias_without_sql_boundaries_or_binds() {
        assert!(PostgresQueryRootAdmission::new("{{root}}.source_version > 0").is_ok());
        for invalid in [
            "",
            "TRUE",
            "{{root}}.source_version = $1",
            "{{root}}.source_version > 0; SELECT 1",
            "{{root}}.source_version > 0 -- bypass",
        ] {
            assert!(PostgresQueryRootAdmission::new(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn admission_renders_only_the_compiler_owned_root_alias() {
        let admission = PostgresQueryRootAdmission::new(
            "EXISTS (SELECT 1 WHERE {{root}}.source_version > 0 AND {{root}}.is_deleted = FALSE)",
        )
        .unwrap();
        let rendered = admission.render("t0");
        assert!(rendered.contains("\"t0\".source_version"));
        assert!(!rendered.contains(ROOT_ALIAS_TOKEN));
    }

    #[test]
    fn admission_anchor_is_exact_and_fail_closed() {
        let mut sql = format!(
            "SELECT 1 FROM index_entities AS \"t0\" WHERE {}",
            root_baseline("t0")
        );
        apply_to_sql(&mut sql, "t0", "\"t0\".source_version > 0").unwrap();
        assert!(sql.contains("AND (\"t0\".source_version > 0)"));
        assert_eq!(
            apply_to_sql(&mut "SELECT 1".to_owned(), "t0", "TRUE"),
            Err(PostgresQueryRootAdmissionApplyError::RootAnchorMismatch)
        );
    }
}
