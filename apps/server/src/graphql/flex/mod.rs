//! GraphQL module for Flex — custom field definitions (Phase 2).

use async_graphql::{FieldError, Result};
use rustok_core::field_schema::is_valid_field_key;

mod mutation;
mod query;
pub mod types;

pub use mutation::FlexMutation;
pub use query::FlexQuery;

pub(super) fn resolve_entity_type(entity_type: Option<String>) -> Result<String> {
    let resolved = entity_type
        .unwrap_or_else(|| "user".to_string())
        .trim()
        .to_ascii_lowercase();

    if resolved.is_empty() {
        return Err(FieldError::new("entity_type must not be empty"));
    }

    if !is_valid_field_key(&resolved) {
        return Err(FieldError::new(
            "entity_type must match ^[a-z][a-z0-9_]{0,127}$",
        ));
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::resolve_entity_type;

    #[test]
    fn resolve_entity_type_defaults_to_user() {
        assert_eq!(resolve_entity_type(None).expect("default"), "user");
    }

    #[test]
    fn resolve_entity_type_normalizes_input() {
        assert_eq!(
            resolve_entity_type(Some(" Product ".to_string())).expect("normalize"),
            "product"
        );
    }

    #[test]
    fn resolve_entity_type_rejects_empty() {
        assert!(resolve_entity_type(Some("   ".to_string())).is_err());
    }

    #[test]
    fn resolve_entity_type_rejects_invalid_format() {
        assert!(resolve_entity_type(Some("product-type".to_string())).is_err());
    }
}
