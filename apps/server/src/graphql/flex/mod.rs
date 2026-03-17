//! GraphQL module for Flex — custom field definitions (Phase 2).

use async_graphql::{FieldError, Result};
use rustok_core::field_schema::is_valid_field_key;

use crate::graphql::errors::GraphQLError;

mod mutation;
mod query;
pub mod types;

pub use mutation::FlexMutation;
pub use query::FlexQuery;

pub(super) fn bad_user_input(message: impl AsRef<str>) -> FieldError {
    <FieldError as GraphQLError>::bad_user_input(message.as_ref())
}

pub(super) fn map_flex_error(error: rustok_core::field_schema::FlexError) -> FieldError {
    let mapped = flex::map_flex_error(error);

    match mapped.kind {
        flex::FlexMappedErrorKind::Internal => {
            <FieldError as GraphQLError>::internal_error(&mapped.message)
        }
        flex::FlexMappedErrorKind::NotFound => {
            <FieldError as GraphQLError>::not_found(&mapped.message)
        }
        flex::FlexMappedErrorKind::BadUserInput => bad_user_input(mapped.message),
    }
}

pub(super) fn resolve_entity_type(entity_type: Option<String>) -> Result<String> {
    let resolved = entity_type
        .unwrap_or_else(|| "user".to_string())
        .trim()
        .to_ascii_lowercase();

    if resolved.is_empty() {
        return Err(bad_user_input("entity_type must not be empty"));
    }

    if !is_valid_field_key(&resolved) {
        return Err(bad_user_input(
            "entity_type must match ^[a-z][a-z0-9_]{0,127}$",
        ));
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use async_graphql::ErrorExtensions;
    use rustok_core::field_schema::FlexError;
    use uuid::Uuid;

    use super::{bad_user_input, map_flex_error, resolve_entity_type};

    #[test]
    fn bad_user_input_sets_bad_user_input_code() {
        let gql = bad_user_input("invalid").extend();
        assert_eq!(
            gql.extensions.get("code").and_then(|v| v.as_str()),
            Some("BAD_USER_INPUT")
        );
    }

    #[test]
    fn map_flex_error_database_masks_internal_message_and_sets_internal_error_code() {
        let err = map_flex_error(FlexError::Database("db down".to_string()));
        let gql = err.extend();

        assert_eq!(gql.message, "Internal server error");
        assert_eq!(
            gql.extensions.get("code").and_then(|v| v.as_str()),
            Some("INTERNAL_ERROR")
        );
    }

    #[test]
    fn map_flex_error_not_found_sets_not_found_code() {
        let id = Uuid::new_v4();
        let err = map_flex_error(FlexError::NotFound(id));
        let gql = err.extend();

        assert!(gql.message.contains("Field definition not found"));
        assert_eq!(
            gql.extensions.get("code").and_then(|v| v.as_str()),
            Some("NOT_FOUND")
        );
    }

    #[test]
    fn map_flex_error_domain_errors_set_bad_user_input() {
        let err = map_flex_error(FlexError::InvalidFieldKey("invalid-key".to_string()));
        let gql = err.extend();

        assert_eq!(
            gql.extensions.get("code").and_then(|v| v.as_str()),
            Some("BAD_USER_INPUT")
        );
    }

    #[test]
    fn map_flex_error_all_domain_variants_map_to_bad_user_input() {
        let variants = vec![
            FlexError::InvalidFieldKey("invalid".to_string()),
            FlexError::DuplicateFieldKey("dup".to_string()),
            FlexError::TooManyFields {
                entity_type: "user".to_string(),
                max: 50,
            },
            FlexError::ValidationFailed(vec![]),
            FlexError::UnknownEntityType("unknown".to_string()),
        ];

        for error in variants {
            let gql = map_flex_error(error).extend();
            assert_eq!(
                gql.extensions.get("code").and_then(|v| v.as_str()),
                Some("BAD_USER_INPUT")
            );
        }
    }

    #[test]
    fn map_flex_error_unknown_entity_type_sets_bad_user_input_code() {
        let gql = map_flex_error(FlexError::UnknownEntityType("weird".to_string())).extend();

        assert_eq!(
            gql.extensions.get("code").and_then(|v| v.as_str()),
            Some("BAD_USER_INPUT")
        );
        assert!(gql.message.contains("Unknown entity type"));
    }

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
        let gql = resolve_entity_type(Some("   ".to_string()))
            .expect_err("empty entity type should fail")
            .extend();
        assert_eq!(
            gql.extensions.get("code").and_then(|v| v.as_str()),
            Some("BAD_USER_INPUT")
        );
    }

    #[test]
    fn resolve_entity_type_rejects_invalid_format() {
        let gql = resolve_entity_type(Some("product-type".to_string()))
            .expect_err("invalid entity type should fail")
            .extend();
        assert_eq!(
            gql.extensions.get("code").and_then(|v| v.as_str()),
            Some("BAD_USER_INPUT")
        );
    }
}
