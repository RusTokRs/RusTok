use rustok_core::field_schema::is_valid_field_key;

pub const MAX_FLEX_ENTITY_TYPE_BYTES: usize = 64;
pub const TAXONOMY_CATEGORY_ENTITY_TYPE: &str = "taxonomy.category";

/// Flex donor identifiers are product-contract names, not field keys. A donor
/// may therefore be namespaced (`taxonomy.category`) while every segment keeps
/// the same conservative identifier alphabet used by existing donors.
pub fn is_valid_flex_entity_type(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_FLEX_ENTITY_TYPE_BYTES
        && value.split('.').all(is_valid_field_key)
}

pub fn normalize_flex_entity_type(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    is_valid_flex_entity_type(&normalized).then_some(normalized)
}

#[cfg(test)]
mod tests {
    use super::{
        TAXONOMY_CATEGORY_ENTITY_TYPE, is_valid_flex_entity_type, normalize_flex_entity_type,
    };

    #[test]
    fn namespaced_taxonomy_category_is_valid() {
        assert!(is_valid_flex_entity_type(TAXONOMY_CATEGORY_ENTITY_TYPE));
        assert_eq!(
            normalize_flex_entity_type(" Taxonomy.Category ").as_deref(),
            Some(TAXONOMY_CATEGORY_ENTITY_TYPE)
        );
    }

    #[test]
    fn malformed_entity_types_fail_closed() {
        for value in ["", ".category", "taxonomy.", "taxonomy..category", "taxonomy-category"] {
            assert!(!is_valid_flex_entity_type(value), "{value}");
        }
    }
}
