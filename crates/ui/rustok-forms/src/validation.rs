//! Declarative, framework-agnostic form validation rules and builder.

use crate::FieldError;

/// Helper functions for common validation constraints.
pub mod rules {
    use super::FieldError;

    /// Validates that a string is not empty or whitespace-only.
    pub fn required(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        if value.trim().is_empty() {
            Err(FieldError {
                field: field.into(),
                message: message.into(),
            })
        } else {
            Ok(())
        }
    }

    /// Validates minimum string length in characters (unicode safe).
    pub fn min_length(
        field: impl Into<String>,
        value: &str,
        min: usize,
        message: impl Into<String>,
    ) -> Result<(), FieldError> {
        if value.chars().count() < min {
            Err(FieldError {
                field: field.into(),
                message: message.into(),
            })
        } else {
            Ok(())
        }
    }

    /// Validates maximum string length in characters (unicode safe).
    pub fn max_length(
        field: impl Into<String>,
        value: &str,
        max: usize,
        message: impl Into<String>,
    ) -> Result<(), FieldError> {
        if value.chars().count() > max {
            Err(FieldError {
                field: field.into(),
                message: message.into(),
            })
        } else {
            Ok(())
        }
    }

    /// Validates basic email syntax (contains `@` and `.` after `@`, non-empty parts).
    pub fn email(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(()); // Use required() to enforce presence
        }
        let valid = v.contains('@')
            && v.split('@').count() == 2
            && v.split('@').nth(1).map_or(false, |domain| domain.contains('.') && !domain.ends_with('.'));

        if valid {
            Ok(())
        } else {
            Err(FieldError {
                field: field.into(),
                message: message.into(),
            })
        }
    }

    /// Validates basic URL syntax (starts with http:// or https:// and has host).
    pub fn url(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(());
        }
        if (v.starts_with("http://") || v.starts_with("https://")) && v.len() > 10 {
            Ok(())
        } else {
            Err(FieldError {
                field: field.into(),
                message: message.into(),
            })
        }
    }

    /// Validates numeric range [min, max] inclusive.
    pub fn range<T: PartialOrd + Copy>(
        field: impl Into<String>,
        value: T,
        min: Option<T>,
        max: Option<T>,
        message: impl Into<String>,
    ) -> Result<(), FieldError> {
        if let Some(min_val) = min {
            if value < min_val {
                return Err(FieldError {
                    field: field.into(),
                    message: message.into(),
                });
            }
        }
        if let Some(max_val) = max {
            if value > max_val {
                return Err(FieldError {
                    field: field.into(),
                    message: message.into(),
                });
            }
        }
        Ok(())
    }
}

/// Accumulator for running multiple validation rules across fields.
#[derive(Debug, Default, Clone)]
pub struct FormValidator {
    errors: Vec<FieldError>,
}

impl FormValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Adds a check that must pass, appending an error on failure.
    pub fn check(mut self, result: Result<(), FieldError>) -> Self {
        if let Err(err) = result {
            self.errors.push(err);
        }
        self
    }

    /// Validates required field.
    pub fn required(self, field: impl Into<String>, value: &str, message: impl Into<String>) -> Self {
        self.check(rules::required(field, value, message))
    }

    /// Validates minimum string length.
    pub fn min_length(
        self,
        field: impl Into<String>,
        value: &str,
        min: usize,
        message: impl Into<String>,
    ) -> Self {
        self.check(rules::min_length(field, value, min, message))
    }

    /// Validates maximum string length.
    pub fn max_length(
        self,
        field: impl Into<String>,
        value: &str,
        max: usize,
        message: impl Into<String>,
    ) -> Self {
        self.check(rules::max_length(field, value, max, message))
    }

    /// Validates email address format.
    pub fn email(self, field: impl Into<String>, value: &str, message: impl Into<String>) -> Self {
        self.check(rules::email(field, value, message))
    }

    /// Validates URL format.
    pub fn url(self, field: impl Into<String>, value: &str, message: impl Into<String>) -> Self {
        self.check(rules::url(field, value, message))
    }

    /// Validates numeric range.
    pub fn range<T: PartialOrd + Copy>(
        self,
        field: impl Into<String>,
        value: T,
        min: Option<T>,
        max: Option<T>,
        message: impl Into<String>,
    ) -> Self {
        self.check(rules::range(field, value, min, max, message))
    }

    /// Validates custom predicate.
    pub fn custom(
        mut self,
        field: impl Into<String>,
        condition: bool,
        message: impl Into<String>,
    ) -> Self {
        if !condition {
            self.errors.push(FieldError {
                field: field.into(),
                message: message.into(),
            });
        }
        self
    }

    /// Returns true if no errors were accumulated.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Finishes validation, returning `Ok(())` if valid or `Err(Vec<FieldError>)`.
    pub fn finish(self) -> Result<(), Vec<FieldError>> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors)
        }
    }

    /// Returns accumulated field errors.
    pub fn into_errors(self) -> Vec<FieldError> {
        self.errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_rules() {
        let validator = FormValidator::new()
            .required("title", "", "Title is required")
            .min_length("username", "ab", 3, "Username must be >= 3 chars")
            .email("email", "bad-email", "Invalid email")
            .url("website", "https://example.com", "Invalid URL");

        let errs = validator.finish().unwrap_err();
        assert_eq!(errs.len(), 3);
        assert_eq!(errs[0].field, "title");
        assert_eq!(errs[1].field, "username");
        assert_eq!(errs[2].field, "email");
    }

    #[test]
    fn test_validator_success() {
        let validator = FormValidator::new()
            .required("title", "Hello", "Title required")
            .min_length("username", "alex", 3, "Username >= 3")
            .email("email", "user@example.com", "Email")
            .range("age", 25, Some(18), Some(120), "Age between 18 and 120");

        assert!(validator.finish().is_ok());
    }

    #[test]
    fn test_whitespace_only_is_invalid_for_required() {
        let res = rules::required("name", "   \t\n  ", "Name required");
        assert!(res.is_err());
    }

    #[test]
    fn test_unicode_char_count_not_byte_count() {
        // "Привет" is 6 characters, 12 bytes in UTF-8
        let res = rules::min_length("name", "Привет", 6, "Min 6 chars");
        assert!(res.is_ok());

        let res_too_short = rules::min_length("name", "Привет", 7, "Min 7 chars");
        assert!(res_too_short.is_err());
    }

    #[test]
    fn test_empty_email_is_allowed_if_not_required() {
        let res = rules::email("optional_email", "", "Invalid email");
        assert!(res.is_ok());
    }

    #[test]
    fn test_range_bounds() {
        assert!(rules::range("val", 10, Some(10), Some(20), "Range").is_ok());
        assert!(rules::range("val", 20, Some(10), Some(20), "Range").is_ok());
        assert!(rules::range("val", 9, Some(10), Some(20), "Range").is_err());
        assert!(rules::range("val", 21, Some(10), Some(20), "Range").is_err());
        assert!(rules::range("val", 5, Some(0), None, "Range").is_ok());
        assert!(rules::range("val", -1, Some(0), None, "Range").is_err());
    }

    #[test]
    fn test_custom_rule() {
        let validator = FormValidator::new()
            .custom("terms", false, "Must accept terms");
        assert_eq!(validator.into_errors().len(), 1);
    }
}
