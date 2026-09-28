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

    /// Validates basic email syntax (non-empty local part and domain, valid domain dots, no whitespace).
    pub fn email(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(()); // Use required() to enforce presence
        }

        let mut parts = v.split('@');
        let user = parts.next().unwrap_or("");
        let domain = parts.next().unwrap_or("");

        let is_valid = parts.next().is_none()
            && !user.is_empty()
            && !domain.is_empty()
            && !user.contains(char::is_whitespace)
            && !domain.contains(char::is_whitespace)
            && !domain.starts_with('.')
            && !domain.ends_with('.')
            && domain.contains('.')
            && domain.split('.').all(|seg| !seg.is_empty());

        if is_valid {
            Ok(())
        } else {
            Err(FieldError {
                field: field.into(),
                message: message.into(),
            })
        }
    }

    /// Validates web URL syntax (starts with http:// or https://, valid host without whitespace).
    pub fn url(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(());
        }

        let rest = if let Some(r) = v.strip_prefix("https://") {
            r
        } else if let Some(r) = v.strip_prefix("http://") {
            r
        } else {
            return Err(FieldError {
                field: field.into(),
                message: message.into(),
            });
        };

        if rest.is_empty() || rest.contains(char::is_whitespace) || rest.starts_with('/') || rest.starts_with(':') {
            return Err(FieldError {
                field: field.into(),
                message: message.into(),
            });
        }

        let host = rest
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("");

        if host.is_empty() || host.starts_with('.') || host.ends_with('.') {
            return Err(FieldError {
                field: field.into(),
                message: message.into(),
            });
        }

        Ok(())
    }

    /// Validates kebab-case slug syntax (e.g. `blog-post-1`, lowercase ASCII, numbers, hyphens, no consecutive hyphens).
    pub fn slug(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(());
        }
        let is_valid = !v.starts_with('-')
            && !v.ends_with('-')
            && !v.contains("--")
            && v.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');

        if is_valid {
            Ok(())
        } else {
            Err(FieldError {
                field: field.into(),
                message: message.into(),
            })
        }
    }

    /// Validates that two field values match (e.g. password confirmation).
    pub fn matches(
        field: impl Into<String>,
        value: &str,
        expected: &str,
        message: impl Into<String>,
    ) -> Result<(), FieldError> {
        if value == expected {
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

    /// Validates kebab-case slug format.
    pub fn slug(self, field: impl Into<String>, value: &str, message: impl Into<String>) -> Self {
        self.check(rules::slug(field, value, message))
    }

    /// Validates that a value matches an expected string (e.g. password confirmation).
    pub fn matches(
        self,
        field: impl Into<String>,
        value: &str,
        expected: &str,
        message: impl Into<String>,
    ) -> Self {
        self.check(rules::matches(field, value, expected, message))
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

    /// Validates custom predicate condition.
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

    /// Validates custom closure predicate.
    pub fn validate<F>(
        mut self,
        field: impl Into<String>,
        predicate: F,
        message: impl Into<String>,
    ) -> Self
    where
        F: FnOnce() -> bool,
    {
        if !predicate() {
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
            .url("website", "https://example.com", "Invalid URL")
            .slug("slug", "Invalid Slug", "Slug invalid")
            .matches("confirm", "pwd1", "pwd2", "Passwords must match");

        let errs = validator.finish().unwrap_err();
        assert_eq!(errs.len(), 5);
        assert_eq!(errs[0].field, "title");
        assert_eq!(errs[1].field, "username");
        assert_eq!(errs[2].field, "email");
        assert_eq!(errs[3].field, "slug");
        assert_eq!(errs[4].field, "confirm");
    }

    #[test]
    fn test_validator_success() {
        let validator = FormValidator::new()
            .required("title", "Hello", "Title required")
            .min_length("username", "alex", 3, "Username >= 3")
            .email("email", "user@example.com", "Email")
            .url("website", "https://example.com/blog", "URL")
            .slug("slug", "my-first-post-2026", "Slug")
            .matches("confirm", "secret", "secret", "Passwords match")
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

        let res_max = rules::max_length("name", "Привет", 6, "Max 6 chars");
        assert!(res_max.is_ok());

        let res_max_exceeded = rules::max_length("name", "Привет!", 6, "Max 6 chars");
        assert!(res_max_exceeded.is_err());
    }

    #[test]
    fn test_email_edge_cases() {
        assert!(rules::email("email", "", "Invalid").is_ok()); // optional
        assert!(rules::email("email", "user@example.com", "Invalid").is_ok());
        assert!(rules::email("email", "first.last@domain.co.uk", "Invalid").is_ok());

        // Invalid emails
        assert!(rules::email("email", "@example.com", "Invalid").is_err());
        assert!(rules::email("email", "user@", "Invalid").is_err());
        assert!(rules::email("email", "user@.com", "Invalid").is_err());
        assert!(rules::email("email", "user@com.", "Invalid").is_err());
        assert!(rules::email("email", "user@domain..com", "Invalid").is_err());
        assert!(rules::email("email", "user name@example.com", "Invalid").is_err());
        assert!(rules::email("email", "user@exam ple.com", "Invalid").is_err());
        assert!(rules::email("email", "user@a@b.com", "Invalid").is_err());
    }

    #[test]
    fn test_url_edge_cases() {
        assert!(rules::url("url", "", "Invalid").is_ok()); // optional
        assert!(rules::url("url", "https://rustok.dev", "Invalid").is_ok());
        assert!(rules::url("url", "http://localhost:3000/api", "Invalid").is_ok());
        assert!(rules::url("url", "https://example.com/path?q=1#hash", "Invalid").is_ok());

        // Invalid URLs
        assert!(rules::url("url", "http://", "Invalid").is_err());
        assert!(rules::url("url", "http://    ", "Invalid").is_err());
        assert!(rules::url("url", "not-a-url", "Invalid").is_err());
        assert!(rules::url("url", "ftp://example.com", "Invalid").is_err());
        assert!(rules::url("url", "https://.com", "Invalid").is_err());
    }

    #[test]
    fn test_slug_edge_cases() {
        assert!(rules::slug("slug", "", "Invalid").is_ok()); // optional
        assert!(rules::slug("slug", "valid-slug-123", "Invalid").is_ok());
        assert!(rules::slug("slug", "article", "Invalid").is_ok());

        // Invalid slugs
        assert!(rules::slug("slug", "-leading-dash", "Invalid").is_err());
        assert!(rules::slug("slug", "trailing-dash-", "Invalid").is_err());
        assert!(rules::slug("slug", "double--dash", "Invalid").is_err());
        assert!(rules::slug("slug", "UPPERCASE", "Invalid").is_err());
        assert!(rules::slug("slug", "has spaces", "Invalid").is_err());
        assert!(rules::slug("slug", "special!char", "Invalid").is_err());
    }

    #[test]
    fn test_matches_rule() {
        assert!(rules::matches("pwd", "pass123", "pass123", "Mismatch").is_ok());
        assert!(rules::matches("pwd", "pass123", "pass456", "Mismatch").is_err());
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
    fn test_custom_and_validate_closure() {
        let validator = FormValidator::new()
            .custom("terms", false, "Must accept terms")
            .validate("code", || 1 + 1 == 3, "Math error");

        let errs = validator.into_errors();
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].field, "terms");
        assert_eq!(errs[1].field, "code");
    }
}
