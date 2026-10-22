//! Validation helpers for import operations.

use crate::PortabilityError;
use serde::{Deserialize, Serialize};

/// Validation result for a single field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldValidation {
    pub field_name: String,
    pub is_valid: bool,
    pub error_message: Option<String>,
}

impl FieldValidation {
    pub fn valid(field_name: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
            is_valid: true,
            error_message: None,
        }
    }

    pub fn invalid(field_name: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
            is_valid: false,
            error_message: Some(error.into()),
        }
    }
}

/// Collection of validation results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationResult {
    pub fields: Vec<FieldValidation>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, validation: FieldValidation) {
        self.fields.push(validation);
    }

    pub fn is_valid(&self) -> bool {
        self.fields.iter().all(|f| f.is_valid)
    }

    pub fn errors(&self) -> Vec<String> {
        self.fields
            .iter()
            .filter(|f| !f.is_valid)
            .filter_map(|f| {
                f.error_message
                    .as_ref()
                    .map(|msg| format!("{}: {}", f.field_name, msg))
            })
            .collect()
    }

    pub fn into_portability_error(self) -> Result<(), PortabilityError> {
        if self.is_valid() {
            Ok(())
        } else {
            Err(PortabilityError::validation(self.errors().join("; ")))
        }
    }
}

/// Trait for validating import sources.
pub trait Validatable {
    /// Validate the source and return validation results.
    fn validate(&self) -> ValidationResult;
}

/// Helper macro for building validation results.
///
/// # Example
///
/// ```rust,ignore
/// use rustok_content_portability_api::{validate_fields, ValidationResult};
///
/// let result = validate_fields! {
///     "title" => title.is_empty() => "title is required",
///     "email" => !email.contains('@') => "invalid email format",
///     "age" => age < 0 => "age cannot be negative"
/// };
/// ```
#[macro_export]
macro_rules! validate_fields {
    ($($field:expr => $condition:expr => $error:expr),* $(,)?) => {{
        let mut result = ValidationResult::new();
        $(
            if $condition {
                result.add(FieldValidation::invalid($field, $error));
            } else {
                result.add(FieldValidation::valid($field));
            }
        )*
        result
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_result_tracks_valid_and_invalid_fields() {
        let mut result = ValidationResult::new();
        result.add(FieldValidation::valid("title"));
        result.add(FieldValidation::invalid("email", "invalid format"));

        assert!(!result.is_valid());
        assert_eq!(result.errors().len(), 1);
        assert!(result.errors()[0].contains("email"));
    }

    #[test]
    fn validation_result_converts_to_error() {
        let mut result = ValidationResult::new();
        result.add(FieldValidation::invalid("title", "required"));

        let err = result.into_portability_error();
        assert!(err.is_err());
    }

    #[test]
    fn valid_result_converts_to_ok() {
        let mut result = ValidationResult::new();
        result.add(FieldValidation::valid("title"));

        let ok = result.into_portability_error();
        assert!(ok.is_ok());
    }
}
