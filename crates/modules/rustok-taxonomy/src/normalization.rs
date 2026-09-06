/// Normalize a human-facing Taxonomy label or localized route value into the
/// canonical route-key representation used by Taxonomy storage and lookup.
///
/// `None` means the value has no routable representation after normalization.
pub fn normalize_term_route_key(value: &str) -> Option<String> {
    let route_key = slug::slugify(value);
    (!route_key.is_empty()).then_some(route_key)
}

/// Normalize a locale exactly as Taxonomy does for persisted translations and
/// localized route keys.
pub fn normalize_term_locale(value: &str) -> Option<String> {
    rustok_content::normalize_locale_code(value)
}

#[cfg(test)]
mod tests {
    use super::{normalize_term_locale, normalize_term_route_key};

    #[test]
    fn route_key_normalization_matches_taxonomy_slug_contract() {
        assert_eq!(
            normalize_term_route_key("  Summer Sale  "),
            Some("summer-sale".to_owned())
        );
        assert_eq!(normalize_term_route_key("   "), None);
    }

    #[test]
    fn locale_normalization_matches_taxonomy_storage_contract() {
        assert_eq!(normalize_term_locale("EN-us"), Some("en-US".to_owned()));
        assert_eq!(normalize_term_locale("not a locale"), None);
    }
}
