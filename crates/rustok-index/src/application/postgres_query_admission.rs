use thiserror::Error;

use super::postgres_compiler::quote_identifier;

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

    pub(crate) fn render(&self, root_alias: &str) -> String {
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
}
