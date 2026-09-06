use rustok_ui_core::normalize_optional_ui_text;

use crate::i18n::t;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductAdminListInput {
    pub search: Option<String>,
    pub status: Option<String>,
    pub category_id: Option<String>,
    pub sort_by: Option<String>,
    pub sort_direction: Option<String>,
    pub attribute_filters: Vec<String>,
}

pub(crate) fn build_product_admin_list_input(
    search: Option<String>,
    status: Option<String>,
    category_id: Option<String>,
    sort_by: Option<String>,
    sort_direction: Option<String>,
    attribute_filters: Option<String>,
) -> ProductAdminListInput {
    ProductAdminListInput {
        search: normalize_optional_ui_text(search),
        status: normalize_optional_ui_text(status),
        category_id: normalize_optional_ui_text(category_id),
        sort_by: normalize_optional_ui_text(sort_by).or_else(|| Some("published_at".to_string())),
        sort_direction: normalize_optional_ui_text(sort_direction)
            .or_else(|| Some("desc".to_string())),
        attribute_filters: normalize_attribute_filters(attribute_filters),
    }
}

pub(crate) fn serialize_attribute_filters(filters: &[String]) -> String {
    filters.join(";")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminCatalogControlsLabels {
    pub title: String,
    pub subtitle: String,
    pub category: String,
    pub all_categories: String,
    pub attribute_filters: String,
    pub attribute_filters_placeholder: String,
    pub attribute_filters_help: String,
    pub sort_by: String,
    pub published_at: String,
    pub created_at: String,
    pub sort_direction: String,
    pub descending: String,
    pub ascending: String,
    pub apply: String,
}

pub(crate) fn build_product_admin_catalog_controls_labels(
    locale: Option<&str>,
) -> ProductAdminCatalogControlsLabels {
    ProductAdminCatalogControlsLabels {
        title: t(locale, "product.list.catalogControls", "Catalog filters"),
        subtitle: t(
            locale,
            "product.list.catalogControlsSubtitle",
            "Filter the Product-owned admin list by category, typed attributes, and deterministic date order.",
        ),
        category: t(locale, "product.list.category", "Primary category"),
        all_categories: t(locale, "product.list.allCategories", "All categories"),
        attribute_filters: t(locale, "product.list.attributeFilters", "Attribute filters"),
        attribute_filters_placeholder: t(
            locale,
            "product.list.attributeFiltersPlaceholder",
            "color=red;weight=12.5",
        ),
        attribute_filters_help: t(
            locale,
            "product.list.attributeFiltersHelp",
            "Use filterable attribute codes as code=value, separated by semicolons.",
        ),
        sort_by: t(locale, "product.list.sortBy", "Sort by"),
        published_at: t(locale, "product.list.publishedAt", "Publication date"),
        created_at: t(locale, "product.list.createdAt", "Creation date"),
        sort_direction: t(locale, "product.list.sortDirection", "Direction"),
        descending: t(locale, "product.list.descending", "Newest first"),
        ascending: t(locale, "product.list.ascending", "Oldest first"),
        apply: t(locale, "product.list.applyCatalogControls", "Apply"),
    }
}

fn normalize_attribute_filters(value: Option<String>) -> Vec<String> {
    normalize_optional_ui_text(value)
        .map(|value| {
            value
                .split(';')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_catalog_controls_normalize_and_default_sorting() {
        let controls = build_product_admin_list_input(
            Some("  camera  ".to_string()),
            Some(" ACTIVE ".to_string()),
            Some(" category ".to_string()),
            None,
            None,
            Some(" color=red ; weight = 12.5 ".to_string()),
        );
        assert_eq!(controls.search.as_deref(), Some("camera"));
        assert_eq!(controls.status.as_deref(), Some("ACTIVE"));
        assert_eq!(controls.category_id.as_deref(), Some("category"));
        assert_eq!(controls.sort_by.as_deref(), Some("published_at"));
        assert_eq!(controls.sort_direction.as_deref(), Some("desc"));
        assert_eq!(
            controls.attribute_filters,
            vec!["color=red".to_string(), "weight = 12.5".to_string()]
        );
        assert_eq!(
            serialize_attribute_filters(controls.attribute_filters.as_slice()),
            "color=red;weight = 12.5"
        );
    }
}
