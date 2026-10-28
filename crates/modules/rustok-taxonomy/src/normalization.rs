/// Maximum persisted length for canonical and localized Taxonomy route keys.
pub const TAXONOMY_ROUTE_KEY_MAX_CHARS: usize = 120;

/// Maximum persisted length for a module scope value in Taxonomy.
pub const TAXONOMY_SCOPE_VALUE_MAX_CHARS: usize = 64;

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
    use super::{
        TAXONOMY_ROUTE_KEY_MAX_CHARS, TAXONOMY_SCOPE_VALUE_MAX_CHARS, normalize_term_locale,
        normalize_term_route_key,
    };

    #[test]
    fn route_key_normalization_matches_taxonomy_slug_contract() {
        assert_eq!(
            normalize_term_route_key("  Summer Sale  "),
            Some("summer-sale".to_owned())
        );
        assert_eq!(normalize_term_route_key("   "), None);
    }

    #[test]
    fn unicode_route_keys_can_expand_beyond_input_length_but_stay_storage_bounded() {
        let input = "北".repeat(100);
        let normalized = normalize_term_route_key(input.as_str())
            .expect("Unicode should have a routable Taxonomy representation");
        assert!(normalized.chars().count() > 120);
    }

    #[test]
    fn route_key_storage_limit_is_explicitly_120_characters() {
        assert_eq!(TAXONOMY_ROUTE_KEY_MAX_CHARS, 120);
    }

    #[test]
    fn scope_value_storage_limit_is_explicitly_64_characters() {
        assert_eq!(TAXONOMY_SCOPE_VALUE_MAX_CHARS, 64);
    }

    #[test]
    fn locale_normalization_matches_taxonomy_storage_contract() {
        assert_eq!(normalize_term_locale("EN-us"), Some("en-US".to_owned()));
        assert_eq!(normalize_term_locale("not a locale"), None);
    }
}
