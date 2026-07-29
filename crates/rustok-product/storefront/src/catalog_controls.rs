use rustok_ui_core::normalize_optional_ui_text;

use crate::i18n::t;

const SORT_BY_PUBLISHED_AT: &str = "published_at";
const SORT_BY_CREATED_AT: &str = "created_at";
const SORT_DIRECTION_ASC: &str = "asc";
const SORT_DIRECTION_DESC: &str = "desc";
const MAX_ATTRIBUTE_FILTERS: usize = 8;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CatalogListInput {
    pub search: Option<String>,
    pub category_id: Option<String>,
    pub sort_by: Option<String>,
    pub sort_direction: Option<String>,
    pub attribute_filters: Vec<String>,
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
    }
}

pub fn serialize_attribute_filters(filters: &[String]) -> String {
    filters.join(";")
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
        sort_by_published_at: t(
            locale,
            "product.list.sortPublishedAt",
            "Publication date",
        ),
        sort_by_created_at: t(locale, "product.list.sortCreatedAt", "Creation date"),
        sort_direction_label: t(
            locale,
            "product.list.sortDirectionLabel",
            "Direction",
        ),
        sort_direction_desc: t(locale, "product.list.sortDescending", "Newest first"),
        sort_direction_asc: t(locale, "product.list.sortAscending", "Oldest first"),
        submit: t(locale, "product.list.searchSubmit", "Apply"),
    }
}

fn normalize_category_id(value: Option<String>) -> Option<String> {
    normalize_optional_ui_text(value)
        .filter(|value| uuid::Uuid::parse_str(value.as_str()).is_ok())
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
                .take(MAX_ATTRIBUTE_FILTERS)
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
