//! Event validation framework
//!
//! This module provides validation for domain events before they are published
//! to ensure data integrity and prevent invalid events from entering the system.

use thiserror::Error;

/// Errors that can occur during event validation
#[derive(Debug, Error, Clone, PartialEq)]
pub enum EventValidationError {
    /// A required field is empty
    #[error("Field '{0}' cannot be empty")]
    EmptyField(&'static str),

    /// A field exceeds the maximum allowed length
    #[error("Field '{0}' exceeds maximum length of {1} characters")]
    FieldTooLong(&'static str, usize),

    /// A UUID field contains a nil value
    #[error("UUID field '{0}' cannot be nil")]
    NilUuid(&'static str),

    /// A field contains an invalid value
    #[error("Invalid value for field '{0}': {1}")]
    InvalidValue(&'static str, String),

    /// A required field is missing
    #[error("Missing required field '{0}'")]
    MissingField(&'static str),

    /// A numeric field is out of valid range
    #[error("Field '{0}' value {1} is out of valid range ({2}..{3})")]
    OutOfRange(&'static str, i64, i64, i64),

    /// A string field contains invalid characters
    #[error("Field '{0}' contains invalid characters")]
    InvalidCharacters(&'static str),
}

/// Trait for validating domain events
pub trait ValidateEvent {
    /// Validate the event and return an error if validation fails
    fn validate(&self) -> Result<(), EventValidationError>;
}

/// Helper functions for common validation patterns
pub mod validators {
    use super::EventValidationError;
    use uuid::Uuid;

    /// Validates that a string is not empty
    pub fn validate_not_empty(
        field_name: &'static str,
        value: &str,
    ) -> Result<(), EventValidationError> {
        if value.trim().is_empty() {
            return Err(EventValidationError::EmptyField(field_name));
        }
        Ok(())
    }

    /// Validates that a string does not exceed maximum length
    pub fn validate_max_length(
        field_name: &'static str,
        value: &str,
        max_length: usize,
    ) -> Result<(), EventValidationError> {
        if value.len() > max_length {
            return Err(EventValidationError::FieldTooLong(field_name, max_length));
        }
        Ok(())
    }

    /// Validates that a UUID is not nil
    pub fn validate_not_nil_uuid(
        field_name: &'static str,
        uuid: &Uuid,
    ) -> Result<(), EventValidationError> {
        if uuid.is_nil() {
            return Err(EventValidationError::NilUuid(field_name));
        }
        Ok(())
    }

    /// Validates that an optional UUID is not nil if present
    pub fn validate_optional_uuid(
        field_name: &'static str,
        uuid: &Option<Uuid>,
    ) -> Result<(), EventValidationError> {
        if let Some(id) = uuid {
            validate_not_nil_uuid(field_name, id)?;
        }
        Ok(())
    }

    /// Validates that a numeric value is within a range
    pub fn validate_range(
        field_name: &'static str,
        value: i64,
        min: i64,
        max: i64,
    ) -> Result<(), EventValidationError> {
        if value < min || value > max {
            return Err(EventValidationError::OutOfRange(
                field_name, value, min, max,
            ));
        }
        Ok(())
    }

    /// Validates that a string contains only allowed characters
    pub fn validate_alphanumeric_with_dash(
        field_name: &'static str,
        value: &str,
    ) -> Result<(), EventValidationError> {
        if !value
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(EventValidationError::InvalidCharacters(field_name));
        }
        Ok(())
    }

    /// Validates currency code (3 uppercase letters)
    pub fn validate_currency_code(
        field_name: &'static str,
        code: &str,
    ) -> Result<(), EventValidationError> {
        if code.len() != 3 {
            return Err(EventValidationError::InvalidValue(
                field_name,
                "currency code must be exactly 3 characters".to_string(),
            ));
        }
        if !code.chars().all(|c| c.is_ascii_uppercase()) {
            return Err(EventValidationError::InvalidValue(
                field_name,
                "currency code must be uppercase letters".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_validate_not_empty() {
        assert!(validators::validate_not_empty("test", "value").is_ok());
        assert!(validators::validate_not_empty("test", "").is_err());
        assert!(validators::validate_not_empty("test", "   ").is_err());
    }

    #[test]
    fn test_validate_max_length() {
        assert!(validators::validate_max_length("test", "short", 10).is_ok());
        assert!(validators::validate_max_length("test", "exactly10!", 10).is_ok());
        assert!(validators::validate_max_length("test", "this is too long", 10).is_err());
    }

    #[test]
    fn test_validate_not_nil_uuid() {
        let valid_uuid = Uuid::new_v4();
        let nil_uuid = Uuid::nil();

        assert!(validators::validate_not_nil_uuid("test", &valid_uuid).is_ok());
        assert!(validators::validate_not_nil_uuid("test", &nil_uuid).is_err());
    }

    #[test]
    fn test_validate_optional_uuid() {
        let valid_uuid = Some(Uuid::new_v4());
        let nil_uuid = Some(Uuid::nil());
        let none_uuid: Option<Uuid> = None;

        assert!(validators::validate_optional_uuid("test", &valid_uuid).is_ok());
        assert!(validators::validate_optional_uuid("test", &nil_uuid).is_err());
        assert!(validators::validate_optional_uuid("test", &none_uuid).is_ok());
    }

    #[test]
    fn test_validate_range() {
        assert!(validators::validate_range("test", 50, 0, 100).is_ok());
        assert!(validators::validate_range("test", 0, 0, 100).is_ok());
        assert!(validators::validate_range("test", 100, 0, 100).is_ok());
        assert!(validators::validate_range("test", -1, 0, 100).is_err());
        assert!(validators::validate_range("test", 101, 0, 100).is_err());
    }

    #[test]
    fn test_validate_currency_code() {
        assert!(validators::validate_currency_code("currency", "USD").is_ok());
        assert!(validators::validate_currency_code("currency", "EUR").is_ok());
        assert!(validators::validate_currency_code("currency", "US").is_err()); // too short
        assert!(validators::validate_currency_code("currency", "USDD").is_err()); // too long
        assert!(validators::validate_currency_code("currency", "usd").is_err()); // lowercase
        assert!(validators::validate_currency_code("currency", "US1").is_err());
        // not letters
    }

    #[test]
    fn test_validate_alphanumeric_with_dash() {
        assert!(validators::validate_alphanumeric_with_dash("test", "valid-slug").is_ok());
        assert!(validators::validate_alphanumeric_with_dash("test", "valid_slug").is_ok());
        assert!(validators::validate_alphanumeric_with_dash("test", "valid123").is_ok());
        assert!(validators::validate_alphanumeric_with_dash("test", "invalid@slug").is_err());
        assert!(validators::validate_alphanumeric_with_dash("test", "invalid slug").is_err());
    }

    #[test]
    fn test_error_display() {
        let err = EventValidationError::EmptyField("test_field");
        assert_eq!(err.to_string(), "Field 'test_field' cannot be empty");

        let err = EventValidationError::FieldTooLong("test_field", 100);
        assert_eq!(
            err.to_string(),
            "Field 'test_field' exceeds maximum length of 100 characters"
        );

        let err = EventValidationError::NilUuid("test_id");
        assert_eq!(err.to_string(), "UUID field 'test_id' cannot be nil");
    }
}
