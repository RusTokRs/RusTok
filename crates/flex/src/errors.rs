//! Transport-agnostic error mapping helpers for Flex attached mode.

use rustok_core::field_schema::FlexError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexMappedErrorKind {
    Internal,
    NotFound,
    BadUserInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlexMappedError {
    pub kind: FlexMappedErrorKind,
    pub message: String,
}

pub fn map_flex_error(error: FlexError) -> FlexMappedError {
    match error {
        FlexError::Database(_) => FlexMappedError {
            kind: FlexMappedErrorKind::Internal,
            message: "Internal server error".to_string(),
        },
        FlexError::NotFound(id) => FlexMappedError {
            kind: FlexMappedErrorKind::NotFound,
            message: format!("Field definition not found: {id}"),
        },
        FlexError::UnknownEntityType(message)
        | FlexError::InvalidFieldKey(message)
        | FlexError::DuplicateFieldKey(message) => FlexMappedError {
            kind: FlexMappedErrorKind::BadUserInput,
            message,
        },
        FlexError::TooManyFields { entity_type, max } => FlexMappedError {
            kind: FlexMappedErrorKind::BadUserInput,
            message: format!("Too many field definitions for {entity_type} (max {max})"),
        },
        FlexError::ValidationFailed(_) => FlexMappedError {
            kind: FlexMappedErrorKind::BadUserInput,
            message: "Custom field validation failed".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use rustok_core::field_schema::FlexError;
    use uuid::Uuid;

    use super::{map_flex_error, FlexMappedErrorKind};

    #[test]
    fn map_database_error_to_internal() {
        let mapped = map_flex_error(FlexError::Database("db down".to_string()));
        assert_eq!(mapped.kind, FlexMappedErrorKind::Internal);
        assert_eq!(mapped.message, "Internal server error");
    }

    #[test]
    fn map_not_found_error() {
        let id = Uuid::new_v4();
        let mapped = map_flex_error(FlexError::NotFound(id));
        assert_eq!(mapped.kind, FlexMappedErrorKind::NotFound);
        assert_eq!(mapped.message, format!("Field definition not found: {id}"));
    }

    #[test]
    fn map_domain_errors_to_bad_user_input() {
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
            let mapped = map_flex_error(error);
            assert_eq!(mapped.kind, FlexMappedErrorKind::BadUserInput);
        }
    }
}
