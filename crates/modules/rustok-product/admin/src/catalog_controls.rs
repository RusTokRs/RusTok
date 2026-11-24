use rustok_grid::{
    DEFAULT_PAGE_SIZE,
    facet::{parse_selection, serialize_selection},
};
use rustok_ui_core::normalize_optional_ui_text;

use crate::i18n::t;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProductAdminListInput {
    pub search: Option<String>,
    pub status: Option<String>,
    pub vendor: Option<String>,
    pub category_id: Option<String>,
    pub sort_by: Option<String>,
    pub sort_direction: Option<String>,
    pub attribute_filters: Vec<String>,
    pub page: u64,
    pub per_page: u64,
}

impl Default for ProductAdminListInput {
    fn default() -> Self {
        Self {
            search: None,
            status: None,
            vendor: None,
            category_id: None,
            sort_by: None,
            sort_direction: None,
            attribute_filters: Vec::new(),
            page: 1,
            per_page: DEFAULT_PAGE_SIZE as u64,
        }
    }
}

impl ProductAdminListInput {
    pub(crate) fn with_vendor(mut self, vendor: Option<String>) -> Self {
        self.vendor = normalize_optional_ui_text(vendor);
        self
    }
}

fn normalize_product_admin_status(value: Option<String>) -> Option<String> {
    normalize_optional_ui_text(value).and_then(|status| {
        match status.to_ascii_lowercase().as_str() {
            "active" | "published" => Some("active".to_string()),
            "draft" => Some("draft".to_string()),
            "archived" => Some("archived".to_string()),
            _ => None,
        }
    })
}

pub(crate) fn build_product_admin_list_input(
    search: Option<String>,
    status: Option<String>,
    category_id: Option<String>,
    sort_by: Option<String>,
    sort_direction: Option<String>,
    attribute_filters: Option<String>,
    page: Option<String>,
    per_page: Option<String>,
) -> ProductAdminListInput {
    ProductAdminListInput {
        search: normalize_optional_ui_text(search),
        status: normalize_product_admin_status(status),
        vendor: None,
        category_id: normalize_optional_ui_text(category_id),
        sort_by: normalize_optional_ui_text(sort_by).or_else(|| Some("published_at".to_string())),
        sort_direction: normalize_optional_ui_text(sort_direction)
            .or_else(|| Some("desc".to_string())),
        attribute_filters: normalize_attribute_filters(attribute_filters),
        page: normalize_page(page),
        per_page: normalize_page_size(per_page),
    }
}

pub(crate) fn serialize_attribute_filters(filters: &[String]) -> String {
    serialize_selection(filters)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminCatalogControlsLabels {
    pub title: String,
    pub subtitle: String,
    pub category: String,
    pub all_categories: String,
    pub vendor: String,
    pub vendor_placeholder: String,
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
            "Filter the Product-owned admin list by category, vendor, typed attributes, and deterministic date order.",
        ),
        category: t(locale, "product.list.category", "Primary category"),
        all_categories: t(locale, "product.list.allCategories", "All categories"),
        vendor: t(locale, "product.list.vendor", "Vendor"),
        vendor_placeholder: t(locale, "product.list.vendorPlaceholder", "Exact vendor"),
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

const MAX_ADMIN_PRODUCT_PAGE_SIZE: u64 = 100;

fn normalize_page(value: Option<String>) -> u64 {
    value
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|page| *page > 0)
        .map(|page| page.min(i32::MAX as u64))
        .unwrap_or(1)
}

fn normalize_page_size(value: Option<String>) -> u64 {
    value
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|page_size| *page_size > 0)
        .map(|page_size| page_size.min(MAX_ADMIN_PRODUCT_PAGE_SIZE))
        .unwrap_or(DEFAULT_PAGE_SIZE as u64)
}

fn normalize_attribute_filters(value: Option<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    if let Some(value) = normalize_optional_ui_text(value) {
        for entry in parse_selection(value.as_str()) {
            let selected = match entry.split_once('=') {
                Some((code, raw_value)) => {
                    format!("{}={}", code.trim().to_ascii_lowercase(), raw_value.trim())
                }
                None => entry,
            };
            // Product's owner groups attribute codes without case and deduplicates values.
            // Mirror that identity before the shared UI facet toggle compares entries.
            if !normalized.contains(&selected) {
                normalized.push(selected);
            }
        }
    }
    normalized
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
            Some("3".to_string()),
            Some("50".to_string()),
        );
        assert_eq!(controls.search.as_deref(), Some("camera"));
        assert_eq!(controls.status.as_deref(), Some("active"));
        assert_eq!(controls.category_id.as_deref(), Some("category"));
        assert_eq!(controls.sort_by.as_deref(), Some("published_at"));
        assert_eq!(controls.sort_direction.as_deref(), Some("desc"));
        assert_eq!(controls.page, 3);
        assert_eq!(controls.per_page, 50);
        assert_eq!(
            controls.attribute_filters,
            vec!["color=red".to_string(), "weight=12.5".to_string()]
        );
        assert_eq!(
            serialize_attribute_filters(controls.attribute_filters.as_slice()),
            "color=red;weight=12.5"
        );
    }

    #[test]
    fn attribute_filters_use_owner_code_identity_and_trim_values() {
        let filters = normalize_attribute_filters(Some(
            " COLOR = red ;color=red; SIZE = M ;size=M ".to_string(),
        ));
        assert_eq!(filters, vec!["color=red".to_string(), "size=M".to_string()]);
    }

    #[test]
    fn admin_status_matches_next_alias_and_invalid_value_normalization() {
        let published = build_product_admin_list_input(
            None,
            Some(" PUBLISHED ".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(published.status.as_deref(), Some("active"));

        let invalid = build_product_admin_list_input(
            None,
            Some(" deleted ".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(invalid.status, None);
    }

    #[test]
    fn vendor_filter_is_trimmed_and_blank_values_are_cleared() {
        let controls = ProductAdminListInput::default()
            .with_vendor(Some("  Acme  ".to_string()));
        assert_eq!(controls.vendor.as_deref(), Some("Acme"));

        let blank = ProductAdminListInput::default().with_vendor(Some("   ".to_string()));
        assert_eq!(blank.vendor, None);
    }

    #[test]
    fn admin_catalog_pagination_rejects_zero_and_bounds_page_size() {
        let controls = build_product_admin_list_input(
            None,
            None,
            None,
            None,
            None,
            None,
            Some("0".to_string()),
            Some("1000".to_string()),
        );

        assert_eq!(controls.page, 1);
        assert_eq!(controls.per_page, MAX_ADMIN_PRODUCT_PAGE_SIZE);
    }

    #[test]
    fn admin_catalog_pagination_defaults_invalid_values() {
        let controls = build_product_admin_list_input(
            None,
            None,
            None,
            None,
            None,
            None,
            Some("not-a-page".to_string()),
            Some("0".to_string()),
        );

        assert_eq!(controls.page, 1);
        assert_eq!(controls.per_page, DEFAULT_PAGE_SIZE as u64);
    }

    #[test]
    fn admin_catalog_page_number_fits_graphql_int() {
        let controls = build_product_admin_list_input(
            None,
            None,
            None,
            None,
            None,
            None,
            Some(u64::MAX.to_string()),
            Some("20".to_string()),
        );

        assert_eq!(controls.page, i32::MAX as u64);
    }
}
