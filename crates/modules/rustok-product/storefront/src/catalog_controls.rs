use rustok_ui_core::normalize_optional_ui_text;

use rustok_grid::facet_panel::{
    FacetPanelLabels, has_selection_for_key, is_selection_selected, selection_after_clear_key,
    selection_after_toggle, split_selection,
};

use crate::i18n::t;

const SORT_BY_PUBLISHED_AT: &str = "published_at";
const SORT_BY_CREATED_AT: &str = "created_at";
const SORT_DIRECTION_ASC: &str = "asc";
const SORT_DIRECTION_DESC: &str = "desc";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CatalogListInput {
    pub search: Option<String>,
    pub category_id: Option<String>,
    pub sort_by: Option<String>,
    pub sort_direction: Option<String>,
    pub attribute_filters: Vec<String>,
    /// Optional display currency of the catalog-card price snapshot.
    pub currency_code: Option<String>,
}

impl CatalogListInput {
    /// Applies the optional catalog-card display currency read from the route.
    pub fn with_currency_code(mut self, currency_code: Option<String>) -> Self {
        self.currency_code = normalize_currency_code(currency_code);
        self
    }
}

fn normalize_currency_code(currency_code: Option<String>) -> Option<String> {
    let currency_code = normalize_optional_ui_text(currency_code)?;
    let normalized = currency_code.to_ascii_uppercase();
    if normalized.len() == 3
        && normalized
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        Some(normalized)
    } else {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogSearchLabels {
    pub search_label: String,
    pub search_placeholder: String,
    pub category_label: String,
    pub all_categories: String,
    pub attribute_filters_label: String,
    pub attribute_filters_placeholder: String,
    pub attribute_filters_help: String,
    pub sort_by_label: String,
    pub sort_by_published_at: String,
    pub sort_by_created_at: String,
    pub sort_direction_label: String,
    pub sort_direction_desc: String,
    pub sort_direction_asc: String,
    pub submit: String,
}

pub fn build_catalog_list_input(
    search: Option<String>,
    category_id: Option<String>,
    sort_by: Option<String>,
    sort_direction: Option<String>,
    attribute_filters: Option<String>,
) -> CatalogListInput {
    CatalogListInput {
        search: normalize_optional_ui_text(search),
        category_id: normalize_category_id(category_id),
        sort_by: normalize_sort_by(sort_by),
        sort_direction: normalize_sort_direction(sort_direction),
        attribute_filters: normalize_attribute_filters(attribute_filters),
        currency_code: None,
    }
}

pub fn serialize_attribute_filters(filters: &[String]) -> String {
    filters.join(";")
}

/// Splits the `code=value` transport entry of one attribute filter into its parts.
///
/// The vocabulary itself belongs to the table toolkit (`rustok_grid::facet_panel`): the storefront
/// panel and the admin grid mutate one selection language, so neither owns a private copy.
pub fn parse_attribute_filter(entry: &str) -> Option<(String, String)> {
    split_selection(entry).map(|(code, value)| (code.to_string(), value.to_string()))
}

/// True when `code=value` is already part of the active attribute-filter list.
pub fn is_attribute_filter_selected(filters: &[String], code: &str, value: &str) -> bool {
    is_selection_selected(filters, code, value)
}

/// Flips `code=value` in the attribute-filter list: unknown selections are appended, an already
/// selected entry is removed, and the original order of the remaining entries is preserved.
pub fn toggle_attribute_filter(filters: &[String], code: &str, value: &str) -> Vec<String> {
    selection_after_toggle(filters, code, value)
}

/// True when `code=<any value>` is part of the active attribute-filter list.
pub fn has_attribute_filter_for_code(filters: &[String], code: &str) -> bool {
    has_selection_for_key(filters, code)
}

/// Clears every selection that belongs to `code`, keeping the other facet selections.
pub fn clear_attribute_filter_code(filters: &[String], code: &str) -> Vec<String> {
    selection_after_clear_key(filters, code)
}

/// Copy of the storefront facet filter panel; the Leptos adapter renders it verbatim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogFacetLabels {
    pub title: String,
    /// Sub-label of one unbounded facet (text, numeric, date): the free-form input stays the UI.
    pub unbounded_hint: String,
    /// Shown under a facet whose bucket list was cut by the owner value limit.
    pub truncated_hint: String,
    pub clear_label: String,
    pub empty_message: String,
    /// Bucket count template, e.g. `({count})`.
    pub count_template: String,
    pub selected_marker: String,
    pub unselected_marker: String,
}

impl CatalogFacetLabels {
    /// The shared panel labels, so one panel implementation serves every surface.
    pub fn to_grid_labels(&self) -> FacetPanelLabels {
        FacetPanelLabels {
            title: self.title.clone(),
            unbounded_hint: self.unbounded_hint.clone(),
            truncated_hint: self.truncated_hint.clone(),
            clear_label: self.clear_label.clone(),
            empty_message: self.empty_message.clone(),
            count_template: self.count_template.clone(),
            selected_marker: self.selected_marker.clone(),
            unselected_marker: self.unselected_marker.clone(),
        }
    }
}

pub fn build_catalog_facet_labels(locale: Option<&str>) -> CatalogFacetLabels {
    CatalogFacetLabels {
        title: t(locale, "product.list.facetsLabel", "Filters"),
        unbounded_hint: t(
            locale,
            "product.list.facetsUnbounded",
            "Enter a value in the filter field above.",
        ),
        truncated_hint: t(
            locale,
            "product.list.facetsTruncated",
            "More values are available than shown.",
        ),
        clear_label: t(locale, "product.list.facetsClear", "Clear filters"),
        empty_message: t(
            locale,
            "product.list.facetsEmpty",
            "No filters are available for this catalog yet.",
        ),
        count_template: t(locale, "product.list.facetsCount", "({count})"),
        selected_marker: t(locale, "product.list.facetsSelected", "[x]"),
        unselected_marker: t(locale, "product.list.facetsUnselected", "[ ]"),
    }
}

pub fn build_catalog_search_labels(locale: Option<&str>) -> CatalogSearchLabels {
    CatalogSearchLabels {
        search_label: t(locale, "product.list.searchLabel", "Search catalog"),
        search_placeholder: t(
            locale,
            "product.list.searchPlaceholder",
            "Search published products",
        ),
        category_label: t(locale, "product.list.categoryLabel", "Category"),
        all_categories: t(locale, "product.list.allCategories", "All categories"),
        attribute_filters_label: t(
            locale,
            "product.list.attributeFiltersLabel",
            "Attribute filters",
        ),
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
        sort_by_label: t(locale, "product.list.sortByLabel", "Sort by"),
        sort_by_published_at: t(locale, "product.list.sortPublishedAt", "Publication date"),
        sort_by_created_at: t(locale, "product.list.sortCreatedAt", "Creation date"),
        sort_direction_label: t(locale, "product.list.sortDirectionLabel", "Direction"),
        sort_direction_desc: t(locale, "product.list.sortDescending", "Newest first"),
        sort_direction_asc: t(locale, "product.list.sortAscending", "Oldest first"),
        submit: t(locale, "product.list.searchSubmit", "Apply"),
    }
}

fn normalize_category_id(value: Option<String>) -> Option<String> {
    normalize_optional_ui_text(value).filter(|value| uuid::Uuid::parse_str(value.as_str()).is_ok())
}

fn normalize_sort_by(value: Option<String>) -> Option<String> {
    normalize_optional_ui_text(value).and_then(|value| match value.as_str() {
        SORT_BY_PUBLISHED_AT | SORT_BY_CREATED_AT => Some(value),
        _ => None,
    })
}

fn normalize_sort_direction(value: Option<String>) -> Option<String> {
    normalize_optional_ui_text(value).and_then(|value| match value.as_str() {
        SORT_DIRECTION_ASC | SORT_DIRECTION_DESC => Some(value),
        _ => None,
    })
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
    fn catalog_controls_trim_and_drop_invalid_values() {
        let category_id = uuid::Uuid::new_v4().to_string();
        let controls = build_catalog_list_input(
            Some("  camera  ".to_string()),
            Some(format!("  {category_id}  ")),
            Some("created_at".to_string()),
            Some("asc".to_string()),
            Some(" color = red ; weight=12.5 ".to_string()),
        );

        assert_eq!(controls.search, Some("camera".to_string()));
        assert_eq!(controls.category_id, Some(category_id));
        assert_eq!(controls.sort_by, Some("created_at".to_string()));
        assert_eq!(controls.sort_direction, Some("asc".to_string()));
        assert_eq!(
            controls.attribute_filters,
            vec!["color = red".to_string(), "weight=12.5".to_string()]
        );
        assert_eq!(
            serialize_attribute_filters(controls.attribute_filters.as_slice()),
            "color = red;weight=12.5"
        );

        let invalid = build_catalog_list_input(
            Some("   ".to_string()),
            Some("not-a-uuid".to_string()),
            Some("title".to_string()),
            Some("sideways".to_string()),
            None,
        );
        assert_eq!(invalid, CatalogListInput::default());
    }
}
