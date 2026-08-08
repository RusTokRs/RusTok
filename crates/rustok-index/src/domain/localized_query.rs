use serde::{Deserialize, Serialize};

use super::{DomainError, FieldPath, FilterExpr, IndexQuery, LocaleKey};

const MAX_LOCALIZED_FILTER_NODES: usize = 128;

/// Explicit localized-entity fold request layered on top of the ordinary exact-locale query shape.
///
/// `query.scope.locale` remains the requested locale. `fallback_locale` is a secondary projection
/// locale only; when it equals the requested locale it is canonically treated as absent. The ordinary
/// query filter keeps its exact planned semantics, while `any_locale_filter` is reserved for the
/// identity-level existential predicate evaluated across admitted physical locale rows by the folded
/// PostgreSQL compiler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalizedEntityQuery {
    pub query: IndexQuery,
    pub fallback_locale: Option<LocaleKey>,
    pub any_locale_filter: Option<FilterExpr>,
}

impl LocalizedEntityQuery {
    pub fn new(
        query: IndexQuery,
        fallback_locale: Option<LocaleKey>,
        any_locale_filter: Option<FilterExpr>,
    ) -> Self {
        Self {
            query,
            fallback_locale,
            any_locale_filter,
        }
    }

    pub fn requested_locale(&self) -> Option<&LocaleKey> {
        self.query.scope.locale.as_ref()
    }

    /// Return the canonical fallback role used by planning, cursor identity and execution.
    ///
    /// Equal requested/fallback locales collapse to one role instead of creating duplicate physical
    /// locale work or a second cursor identity for equivalent semantics.
    pub fn canonical_fallback_locale(&self) -> Option<&LocaleKey> {
        self.fallback_locale
            .as_ref()
            .filter(|fallback| Some(*fallback) != self.requested_locale())
    }

    pub fn validate_shape(&self) -> Result<(), DomainError> {
        self.query.validate_shape()?;
        let ordinary_nodes = self
            .query
            .filter
            .as_ref()
            .map_or(0, FilterExpr::node_count);
        let any_locale_nodes = self
            .any_locale_filter
            .as_ref()
            .map_or(0, FilterExpr::node_count);
        if ordinary_nodes + any_locale_nodes > MAX_LOCALIZED_FILTER_NODES {
            return Err(DomainError::FilterTooComplex);
        }
        Ok(())
    }

    pub fn any_locale_referenced_paths(&self) -> Vec<&FieldPath> {
        let mut paths = Vec::new();
        if let Some(filter) = &self.any_locale_filter {
            filter.field_paths(&mut paths);
        }
        paths
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::{
        EntityName, FieldName, FieldPath, IndexQueryScope, IndexValue, ModuleName, Pagination,
        SchemaRef, SchemaVersion,
    };

    fn base_query(requested: &str) -> IndexQuery {
        IndexQuery {
            scope: IndexQueryScope {
                tenant_id: Uuid::new_v4(),
                locale: Some(LocaleKey::new(requested).unwrap()),
            },
            schema: SchemaRef {
                module: ModuleName::new("rustok-product").unwrap(),
                entity: EntityName::new("product").unwrap(),
                version: SchemaVersion::INITIAL,
            },
            fields: vec![FieldPath::new(FieldName::new("id").unwrap())],
            filter: None,
            order_by: Vec::new(),
            pagination: Pagination::Cursor {
                first: 20,
                after: None,
            },
            include_exact_count: true,
        }
    }

    #[test]
    fn equal_fallback_locale_is_canonically_collapsed() {
        let query = LocalizedEntityQuery::new(
            base_query("en-US"),
            Some(LocaleKey::new("en-US").unwrap()),
            None,
        );
        assert!(query.canonical_fallback_locale().is_none());
    }

    #[test]
    fn any_locale_paths_are_reported_separately_from_exact_query_paths() {
        let title = FieldPath::new(FieldName::new("title").unwrap());
        let query = LocalizedEntityQuery::new(
            base_query("en-US"),
            Some(LocaleKey::new("en").unwrap()),
            Some(FilterExpr::Eq(
                title.clone(),
                IndexValue::String("needle".to_owned()),
            )),
        );
        assert_eq!(query.any_locale_referenced_paths(), vec![&title]);
    }
}
