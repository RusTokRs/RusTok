use serde::{Deserialize, Serialize};

use super::{DomainError, FieldPath, FilterExpr, IndexQuery, LocaleKey, OrderDirection};

const MAX_LOCALIZED_FILTER_NODES: usize = 128;

fn default_identity_order_direction() -> OrderDirection {
    OrderDirection::Asc
}

/// Explicit localized-entity fold request layered on top of the ordinary exact-locale query shape.
///
/// `query.scope.locale` remains the requested locale. `fallback_locale` is a secondary projection
/// locale only; when it equals the requested locale it is canonically treated as absent. The ordinary
/// query filter keeps its exact planned semantics, while `any_locale_filter` is reserved for the
/// identity-level existential predicate evaluated across admitted physical locale rows by the folded
/// PostgreSQL compiler.
///
/// `localized_projection_fields` is explicit because generic Index schema fields intentionally do not
/// encode owner-specific localization semantics. A listed root field is projected only from the
/// requested row, then fallback row, then SQL null. Unlisted root fields are read from the deterministic
/// admitted identity anchor. This prevents a third-locale anchor from becoming visible localized
/// content when requested/fallback rows are absent without changing the immutable schema fingerprint.
///
/// `identity_order_direction` controls only the final root entity UUID tie-break after all explicit
/// `query.order_by` terms. Ordinary `IndexQuery` keeps its existing always-ascending identity tie-break.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalizedEntityQuery {
    pub query: IndexQuery,
    pub fallback_locale: Option<LocaleKey>,
    pub any_locale_filter: Option<FilterExpr>,
    #[serde(default)]
    pub localized_projection_fields: Vec<FieldPath>,
    #[serde(default = "default_identity_order_direction")]
    pub identity_order_direction: OrderDirection,
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
            localized_projection_fields: Vec::new(),
            identity_order_direction: OrderDirection::Asc,
        }
    }

    pub fn with_localized_projection_fields(
        mut self,
        fields: impl IntoIterator<Item = FieldPath>,
    ) -> Self {
        self.localized_projection_fields = fields.into_iter().collect();
        self
    }

    pub fn with_identity_order_direction(mut self, direction: OrderDirection) -> Self {
        self.identity_order_direction = direction;
        self
    }

    pub fn requested_locale(&self) -> Option<&LocaleKey> {
        self.query.scope.locale.as_ref()
    }

    /// Return the canonical fallback role used by planning, cursor identity and execution.
    pub fn canonical_fallback_locale(&self) -> Option<&LocaleKey> {
        self.fallback_locale
            .as_ref()
            .filter(|fallback| Some(*fallback) != self.requested_locale())
    }

    pub fn is_localized_projection_path(&self, path: &FieldPath) -> bool {
        self.localized_projection_fields
            .iter()
            .any(|localized| localized == path)
    }

    pub fn validate_shape(&self) -> Result<(), DomainError> {
        self.query.validate_shape()?;
        let ordinary_nodes = self.query.filter.as_ref().map_or(0, FilterExpr::node_count);
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

    #[test]
    fn localized_projection_roles_are_explicit_and_default_empty() {
        let title = FieldPath::new(FieldName::new("title").unwrap());
        let plain = LocalizedEntityQuery::new(base_query("en-US"), None, None);
        assert!(!plain.is_localized_projection_path(&title));
        let localized = plain.with_localized_projection_fields([title.clone()]);
        assert!(localized.is_localized_projection_path(&title));
    }

    #[test]
    fn identity_order_defaults_ascending_and_can_be_selected() {
        let query = LocalizedEntityQuery::new(base_query("en-US"), None, None);
        assert_eq!(query.identity_order_direction, OrderDirection::Asc);
        assert_eq!(
            query
                .with_identity_order_direction(OrderDirection::Desc)
                .identity_order_direction,
            OrderDirection::Desc
        );
    }
}
