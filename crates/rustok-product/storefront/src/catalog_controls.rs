use rustok_ui_core::normalize_optional_ui_text;

use crate::i18n::t;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CatalogListInput {
    pub search: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogSearchLabels {
    pub label: String,
    pub placeholder: String,
    pub submit: String,
}

pub fn build_catalog_list_input(search: Option<String>) -> CatalogListInput {
    CatalogListInput {
        search: normalize_optional_ui_text(search),
    }
}

pub fn build_catalog_search_labels(locale: Option<&str>) -> CatalogSearchLabels {
    CatalogSearchLabels {
        label: t(locale, "product.list.searchLabel", "Search catalog"),
        placeholder: t(
            locale,
            "product.list.searchPlaceholder",
            "Search published products",
        ),
        submit: t(locale, "product.list.searchSubmit", "Search"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_search_trims_and_drops_blank_values() {
        assert_eq!(
            build_catalog_list_input(Some("  camera  ".to_string())).search,
            Some("camera".to_string())
        );
        assert_eq!(build_catalog_list_input(Some("   ".to_string())).search, None);
    }
}
