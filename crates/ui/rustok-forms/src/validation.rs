//! Declarative, framework-agnostic form validation rules and builder.

use crate::FieldError;

/// Helper functions for common validation constraints.
pub mod rules {
    use super::FieldError;

    /// Validates that a string is not empty or whitespace-only.
    pub fn required(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        if value.trim().is_empty() {
            Err(FieldError::new(field, message))
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
            Err(FieldError::new(field, message))
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
            Err(FieldError::new(field, message))
        } else {
            Ok(())
        }
    }

    /// Validates that string length falls within inclusive range [min, max] (unicode safe).
    pub fn length_between(
        field: impl Into<String>,
        value: &str,
        min: usize,
        max: usize,
        message: impl Into<String>,
    ) -> Result<(), FieldError> {
        let count = value.chars().count();
        if count < min || count > max {
            Err(FieldError::new(field, message))
        } else {
            Ok(())
        }
    }

    /// Validates minimum byte length.
    pub fn min_length_bytes(
        field: impl Into<String>,
        value: &str,
        min: usize,
        message: impl Into<String>,
    ) -> Result<(), FieldError> {
        if value.len() < min {
            Err(FieldError::new(field, message))
        } else {
            Ok(())
        }
    }

    /// Validates maximum byte length.
    pub fn max_length_bytes(
        field: impl Into<String>,
        value: &str,
        max: usize,
        message: impl Into<String>,
    ) -> Result<(), FieldError> {
        if value.len() > max {
            Err(FieldError::new(field, message))
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
            && !user.starts_with('.')
            && !user.ends_with('.')
            && !user.contains("..")
            && !user.contains(char::is_whitespace)
            && !domain.contains(char::is_whitespace)
            && !domain.starts_with('.')
            && !domain.ends_with('.')
            && !domain.contains("..")
            && domain.contains('.')
            && domain.split('.').all(|seg| {
                !seg.is_empty()
                    && !seg.starts_with('-')
                    && !seg.ends_with('-')
                    && seg.chars().all(|c| c.is_alphanumeric() || c == '-')
            });

        if is_valid {
            Ok(())
        } else {
            Err(FieldError::new(field, message))
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
            return Err(FieldError::new(field, message));
        };

        if rest.is_empty() || rest.contains(char::is_whitespace) || rest.starts_with('/') || rest.starts_with(':') {
            return Err(FieldError::new(field, message));
        }

        let host = rest
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("");

        if host.is_empty() || host.starts_with('.') || host.ends_with('.') || host.contains("..") {
            return Err(FieldError::new(field, message));
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
            Err(FieldError::new(field, message))
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
            Err(FieldError::new(field, message))
        }
    }

    /// Validates numeric range [min, max] inclusive for any comparable type.
    pub fn range<T: PartialOrd + Copy>(
        field: impl Into<String>,
        value: T,
        min: Option<T>,
        max: Option<T>,
        message: impl Into<String>,
    ) -> Result<(), FieldError> {
        if let Some(min_val) = min {
            if value < min_val {
                return Err(FieldError::new(field, message));
            }
        }
        if let Some(max_val) = max {
            if value > max_val {
                return Err(FieldError::new(field, message));
            }
        }
        Ok(())
    }

    /// Validates that a string parses as a valid finite number.
    pub fn numeric(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(());
        }
        if let Ok(n) = v.parse::<f64>() {
            if n.is_finite() {
                return Ok(());
            }
        }
        Err(FieldError::new(field, message))
    }

    /// Validates that a string parses as a valid 64-bit integer.
    pub fn integer(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(());
        }
        if v.parse::<i64>().is_ok() {
            Ok(())
        } else {
            Err(FieldError::new(field, message))
        }
    }

    /// Validates that a string parses as a number within the specified inclusive [min, max] range.
    pub fn range_numeric(
        field: impl Into<String>,
        value: &str,
        min: Option<f64>,
        max: Option<f64>,
        message: impl Into<String>,
    ) -> Result<(), FieldError> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(());
        }
        match v.parse::<f64>() {
            Ok(n) if n.is_finite() => range(field, n, min, max, message),
            _ => Err(FieldError::new(field, message)),
        }
    }

    /// Validates that a string contains only alphanumeric characters (unicode-safe).
    pub fn alphanumeric(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(());
        }
        if v.chars().all(char::is_alphanumeric) {
            Ok(())
        } else {
            Err(FieldError::new(field, message))
        }
    }

    /// Validates that a string contains only alphabetic characters (unicode-safe).
    pub fn alphabetic(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(());
        }
        if v.chars().all(char::is_alphabetic) {
            Ok(())
        } else {
            Err(FieldError::new(field, message))
        }
    }

    /// Validates standard phone number format (optional leading +, digits, spaces, hyphens, parentheses; at least 7 digits).
    pub fn tel(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(());
        }
        let digits_count = v.chars().filter(|c| c.is_ascii_digit()).count();
        let valid_chars = v.chars().all(|c| c.is_ascii_digit() || c == '+' || c == ' ' || c == '-' || c == '(' || c == ')');
        if valid_chars && digits_count >= 7 {
            Ok(())
        } else {
            Err(FieldError::new(field, message))
        }
    }

    /// Validates that a string is one of the allowed choices.
    pub fn one_of<'a>(
        field: impl Into<String>,
        value: &str,
        allowed: &[&'a str],
        message: impl Into<String>,
    ) -> Result<(), FieldError> {
        if allowed.contains(&value) {
            Ok(())
        } else {
            Err(FieldError::new(field, message))
        }
    }

    /// Validates that a string is NOT one of the disallowed choices.
    pub fn none_of<'a>(
        field: impl Into<String>,
        value: &str,
        disallowed: &[&'a str],
        message: impl Into<String>,
    ) -> Result<(), FieldError> {
        if disallowed.contains(&value) {
            Err(FieldError::new(field, message))
        } else {
            Ok(())
        }
    }

    /// Validates standard 8-4-4-4-12 hex UUID format.
    pub fn uuid(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(());
        }
        let parts: Vec<&str> = v.split('-').collect();
        let is_valid = parts.len() == 5
            && parts[0].len() == 8
            && parts[1].len() == 4
            && parts[2].len() == 4
            && parts[3].len() == 4
            && parts[4].len() == 12
            && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_hexdigit()));

        if is_valid {
            Ok(())
        } else {
            Err(FieldError::new(field, message))
        }
    }

    /// Validates standard YYYY-MM-DD date format with basic calendar validation.
    pub fn date_ymd(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(());
        }
        let parts: Vec<&str> = v.split('-').collect();
        if parts.len() == 3 && parts[0].len() == 4 && parts[1].len() == 2 && parts[2].len() == 2 {
            if let (Ok(year), Ok(month), Ok(day)) = (
                parts[0].parse::<u32>(),
                parts[1].parse::<u32>(),
                parts[2].parse::<u32>(),
            ) {
                if (1..=9999).contains(&year) && (1..=12).contains(&month) && (1..=31).contains(&day) {
                    return Ok(());
                }
            }
        }
        Err(FieldError::new(field, message))
    }

    /// Validates HH:MM or HH:MM:SS 24-hour time format.
    pub fn time_hhmm(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(());
        }
        let parts: Vec<&str> = v.split(':').collect();
        if parts.len() == 2 || parts.len() == 3 {
            if let (Ok(h), Ok(m)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                if h < 24 && m < 60 {
                    if parts.len() == 3 {
                        if let Ok(s) = parts[2].parse::<u32>() {
                            if s < 60 {
                                return Ok(());
                            }
                        }
                    } else {
                        return Ok(());
                    }
                }
            }
        }
        Err(FieldError::new(field, message))
    }

    /// Validates standard IPv4 address format (4 decimal octets 0-255).
    pub fn ipv4(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(());
        }
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() == 4 {
            let all_valid = parts.iter().all(|part| {
                if part.is_empty() || (part.len() > 1 && part.starts_with('0')) {
                    false
                } else if let Ok(n) = part.parse::<u16>() {
                    n <= 255
                } else {
                    false
                }
            });
            if all_valid {
                return Ok(());
            }
        }
        Err(FieldError::new(field, message))
    }

    /// Validates hex color code format (`#RGB`, `#RRGGBB`, or `#RRGGBBAA`).
    pub fn hex_color(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(());
        }
        if let Some(rest) = v.strip_prefix('#') {
            if (rest.len() == 3 || rest.len() == 6 || rest.len() == 8)
                && rest.chars().all(|c| c.is_ascii_hexdigit())
            {
                return Ok(());
            }
        }
        Err(FieldError::new(field, message))
    }

    /// Validates that a string is valid JSON syntax.
    pub fn json(field: impl Into<String>, value: &str, message: impl Into<String>) -> Result<(), FieldError> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(());
        }
        if serde_json::from_str::<serde_json::Value>(v).is_ok() {
            Ok(())
        } else {
            Err(FieldError::new(field, message))
        }
    }

    /// Validates that a string contains a specific substring.
    pub fn contains(
        field: impl Into<String>,
        value: &str,
        needle: &str,
        message: impl Into<String>,
    ) -> Result<(), FieldError> {
        if value.contains(needle) {
            Ok(())
        } else {
            Err(FieldError::new(field, message))
        }
    }

    /// Validates that a string starts with a specific prefix.
    pub fn starts_with(
        field: impl Into<String>,
        value: &str,
        prefix: &str,
        message: impl Into<String>,
    ) -> Result<(), FieldError> {
        if value.starts_with(prefix) {
            Ok(())
        } else {
            Err(FieldError::new(field, message))
        }
    }

    /// Validates that a string ends with a specific suffix.
    pub fn ends_with(
        field: impl Into<String>,
        value: &str,
        suffix: &str,
        message: impl Into<String>,
    ) -> Result<(), FieldError> {
        if value.ends_with(suffix) {
            Ok(())
        } else {
            Err(FieldError::new(field, message))
        }
    }

    /// Validates that a collection or list has at least `min` items.
    pub fn min_items(field: impl Into<String>, len: usize, min: usize, message: impl Into<String>) -> Result<(), FieldError> {
        if len < min {
            Err(FieldError::new(field, message))
        } else {
            Ok(())
        }
    }

    /// Validates that a collection or list has at most `max` items.
    pub fn max_items(field: impl Into<String>, len: usize, max: usize, message: impl Into<String>) -> Result<(), FieldError> {
        if len > max {
            Err(FieldError::new(field, message))
        } else {
            Ok(())
        }
    }
}

/// Accumulator for running multiple validation rules across fields.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FormValidator {
    errors: Vec<FieldError>,
}

impl FormValidator {
    /// Create a new empty `FormValidator`.
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Adds a check result that must pass, appending an error on failure.
    pub fn check(mut self, result: Result<(), FieldError>) -> Self {
        if let Err(err) = result {
            self.errors.push(err);
        }
        self
    }

    /// Validates required field (non-empty, non-whitespace).
    pub fn required(self, field: impl Into<String>, value: &str, message: impl Into<String>) -> Self {
        self.check(rules::required(field, value, message))
    }

    /// Validates minimum string length in unicode characters.
    pub fn min_length(
        self,
        field: impl Into<String>,
        value: &str,
        min: usize,
        message: impl Into<String>,
    ) -> Self {
        self.check(rules::min_length(field, value, min, message))
    }

    /// Validates maximum string length in unicode characters.
    pub fn max_length(
        self,
        field: impl Into<String>,
        value: &str,
        max: usize,
        message: impl Into<String>,
    ) -> Self {
        self.check(rules::max_length(field, value, max, message))
    }

    /// Validates string length is within inclusive range [min, max].
    pub fn length_between(
        self,
        field: impl Into<String>,
        value: &str,
        min: usize,
        max: usize,
        message: impl Into<String>,
    ) -> Self {
        self.check(rules::length_between(field, value, min, max, message))
    }

    /// Validates minimum byte length.
    pub fn min_length_bytes(
        self,
        field: impl Into<String>,
        value: &str,
        min: usize,
        message: impl Into<String>,
    ) -> Self {
        self.check(rules::min_length_bytes(field, value, min, message))
    }

    /// Validates maximum byte length.
    pub fn max_length_bytes(
        self,
        field: impl Into<String>,
        value: &str,
        max: usize,
        message: impl Into<String>,
    ) -> Self {
        self.check(rules::max_length_bytes(field, value, max, message))
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

    /// Validates numeric range for any PartialOrd + Copy type.
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

    /// Validates that string parses to a finite number.
    pub fn numeric(self, field: impl Into<String>, value: &str, message: impl Into<String>) -> Self {
        self.check(rules::numeric(field, value, message))
    }

    /// Validates that string parses to an integer.
    pub fn integer(self, field: impl Into<String>, value: &str, message: impl Into<String>) -> Self {
        self.check(rules::integer(field, value, message))
    }

    /// Validates that string parses to a number within [min, max] range.
    pub fn range_numeric(
        self,
        field: impl Into<String>,
        value: &str,
        min: Option<f64>,
        max: Option<f64>,
        message: impl Into<String>,
    ) -> Self {
        self.check(rules::range_numeric(field, value, min, max, message))
    }

    /// Validates alphanumeric string.
    pub fn alphanumeric(self, field: impl Into<String>, value: &str, message: impl Into<String>) -> Self {
        self.check(rules::alphanumeric(field, value, message))
    }

    /// Validates alphabetic string.
    pub fn alphabetic(self, field: impl Into<String>, value: &str, message: impl Into<String>) -> Self {
        self.check(rules::alphabetic(field, value, message))
    }

    /// Validates phone number.
    pub fn tel(self, field: impl Into<String>, value: &str, message: impl Into<String>) -> Self {
        self.check(rules::tel(field, value, message))
    }

    /// Validates that value is in allowed list.
    pub fn one_of<'a>(
        self,
        field: impl Into<String>,
        value: &str,
        allowed: &[&'a str],
        message: impl Into<String>,
    ) -> Self {
        self.check(rules::one_of(field, value, allowed, message))
    }

    /// Validates that value is NOT in disallowed list.
    pub fn none_of<'a>(
        self,
        field: impl Into<String>,
        value: &str,
        disallowed: &[&'a str],
        message: impl Into<String>,
    ) -> Self {
        self.check(rules::none_of(field, value, disallowed, message))
    }

    /// Validates UUID format.
    pub fn uuid(self, field: impl Into<String>, value: &str, message: impl Into<String>) -> Self {
        self.check(rules::uuid(field, value, message))
    }

    /// Validates date YYYY-MM-DD format.
    pub fn date_ymd(self, field: impl Into<String>, value: &str, message: impl Into<String>) -> Self {
        self.check(rules::date_ymd(field, value, message))
    }

    /// Validates time HH:MM format.
    pub fn time_hhmm(self, field: impl Into<String>, value: &str, message: impl Into<String>) -> Self {
        self.check(rules::time_hhmm(field, value, message))
    }

    /// Validates IPv4 address.
    pub fn ipv4(self, field: impl Into<String>, value: &str, message: impl Into<String>) -> Self {
        self.check(rules::ipv4(field, value, message))
    }

    /// Validates hex color code.
    pub fn hex_color(self, field: impl Into<String>, value: &str, message: impl Into<String>) -> Self {
        self.check(rules::hex_color(field, value, message))
    }

    /// Validates JSON string.
    pub fn json(self, field: impl Into<String>, value: &str, message: impl Into<String>) -> Self {
        self.check(rules::json(field, value, message))
    }

    /// Validates string contains substring.
    pub fn contains(
        self,
        field: impl Into<String>,
        value: &str,
        needle: &str,
        message: impl Into<String>,
    ) -> Self {
        self.check(rules::contains(field, value, needle, message))
    }

    /// Validates string starts with prefix.
    pub fn starts_with(
        self,
        field: impl Into<String>,
        value: &str,
        prefix: &str,
        message: impl Into<String>,
    ) -> Self {
        self.check(rules::starts_with(field, value, prefix, message))
    }

    /// Validates string ends with suffix.
    pub fn ends_with(
        self,
        field: impl Into<String>,
        value: &str,
        suffix: &str,
        message: impl Into<String>,
    ) -> Self {
        self.check(rules::ends_with(field, value, suffix, message))
    }

    /// Validates minimum item count in collection.
    pub fn min_items(
        self,
        field: impl Into<String>,
        len: usize,
        min: usize,
        message: impl Into<String>,
    ) -> Self {
        self.check(rules::min_items(field, len, min, message))
    }

    /// Validates maximum item count in collection.
    pub fn max_items(
        self,
        field: impl Into<String>,
        len: usize,
        max: usize,
        message: impl Into<String>,
    ) -> Self {
        self.check(rules::max_items(field, len, max, message))
    }

    /// Validates custom boolean condition.
    pub fn custom(
        mut self,
        field: impl Into<String>,
        condition: bool,
        message: impl Into<String>,
    ) -> Self {
        if !condition {
            self.errors.push(FieldError::new(field, message));
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
            self.errors.push(FieldError::new(field, message));
        }
        self
    }

    /// Conditional validator: runs validation closure only when `condition` is true.
    pub fn validate_if<F>(mut self, condition: bool, f: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        if condition {
            f(self)
        } else {
            self
        }
    }

    /// Explicitly append an error for a field.
    pub fn add_error(mut self, field: impl Into<String>, message: impl Into<String>) -> Self {
        self.errors.push(FieldError::new(field, message));
        self
    }

    /// Append multiple errors.
    pub fn add_errors(mut self, errors: impl IntoIterator<Item = FieldError>) -> Self {
        self.errors.extend(errors);
        self
    }

    /// Merge another validator's errors into this one.
    pub fn merge(mut self, other: FormValidator) -> Self {
        self.errors.extend(other.errors);
        self
    }

    /// Merge a sub-form validator's errors by prepending `{prefix}.` to all nested field names.
    pub fn merge_nested(mut self, prefix: &str, other: FormValidator) -> Self {
        for err in other.errors {
            let field_path = if err.field.is_empty() {
                prefix.to_string()
            } else {
                format!("{prefix}.{}", err.field)
            };
            self.errors.push(FieldError::new(field_path, err.message));
        }
        self
    }

    /// Returns true if no errors were accumulated.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns true if any errors were accumulated.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Returns the number of accumulated errors.
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// Returns the first error message for a specific field name, if any.
    pub fn field_error(&self, field: &str) -> Option<&str> {
        self.errors
            .iter()
            .find(|fe| fe.field == field)
            .map(|fe| fe.message.as_str())
    }

    /// Returns all error messages for a specific field name.
    pub fn field_errors_for(&self, field: &str) -> Vec<&str> {
        self.errors
            .iter()
            .filter(|fe| fe.field == field)
            .map(|fe| fe.message.as_str())
            .collect()
    }

    /// Returns a slice of all accumulated errors.
    pub fn errors(&self) -> &[FieldError] {
        &self.errors
    }

    /// Finishes validation, returning `Ok(())` if valid or `Err(Vec<FieldError>)`.
    pub fn finish(self) -> Result<(), Vec<FieldError>> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors)
        }
    }

    /// Returns accumulated field errors as a vector.
    pub fn into_errors(self) -> Vec<FieldError> {
        self.errors
    }
}

impl Extend<FieldError> for FormValidator {
    fn extend<T: IntoIterator<Item = FieldError>>(&mut self, iter: T) {
        self.errors.extend(iter);
    }
}

impl IntoIterator for FormValidator {
    type Item = FieldError;
    type IntoIter = std::vec::IntoIter<FieldError>;

    fn into_iter(self) -> Self::IntoIter {
        self.errors.into_iter()
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
    fn test_date_time_and_ipv4_rules() {
        assert!(rules::date_ymd("date", "2026-09-28", "Invalid date").is_ok());
        assert!(rules::date_ymd("date", "2026-13-01", "Invalid date").is_err());

        assert!(rules::time_hhmm("time", "14:30", "Invalid time").is_ok());
        assert!(rules::time_hhmm("time", "14:30:45", "Invalid time").is_ok());
        assert!(rules::time_hhmm("time", "25:00", "Invalid time").is_err());

        assert!(rules::ipv4("ip", "192.168.1.1", "Invalid IP").is_ok());
        assert!(rules::ipv4("ip", "256.0.0.1", "Invalid IP").is_err());

        assert!(rules::hex_color("color", "#ff00aa", "Invalid hex").is_ok());
        assert!(rules::hex_color("color", "red", "Invalid hex").is_err());

        assert!(rules::json("meta", r#"{"key": "val"}"#, "Invalid JSON").is_ok());
        assert!(rules::json("meta", "{bad json}", "Invalid JSON").is_err());
    }

    #[test]
    fn test_string_helpers() {
        assert!(rules::contains("text", "hello world", "world", "Must contain world").is_ok());
        assert!(rules::starts_with("text", "admin-panel", "admin", "Must start with admin").is_ok());
        assert!(rules::ends_with("file", "image.png", ".png", "Must end with .png").is_ok());
    }

    #[test]
    fn test_merge_nested() {
        let address_val = FormValidator::new()
            .required("city", "", "City required")
            .required("zip", "12345", "ZIP required");

        let main_val = FormValidator::new()
            .required("name", "John", "Name required")
            .merge_nested("address", address_val);

        assert_eq!(main_val.error_count(), 1);
        assert_eq!(main_val.field_error("address.city"), Some("City required"));
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

        let res_range = rules::length_between("name", "Привет", 3, 6, "Length between 3 and 6");
        assert!(res_range.is_ok());
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
        assert!(rules::email("email", ".user@example.com", "Invalid").is_err());
        assert!(rules::email("email", "user.@example.com", "Invalid").is_err());
        assert!(rules::email("email", "user..name@example.com", "Invalid").is_err());
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
    fn test_numeric_and_integer_rules() {
        assert!(rules::numeric("num", "42.5", "Not numeric").is_ok());
        assert!(rules::numeric("num", "-100", "Not numeric").is_ok());
        assert!(rules::numeric("num", "abc", "Not numeric").is_err());

        assert!(rules::integer("int", "42", "Not integer").is_ok());
        assert!(rules::integer("int", "-100", "Not integer").is_ok());
        assert!(rules::integer("int", "42.5", "Not integer").is_err());

        assert!(rules::range_numeric("val", "50", Some(10.0), Some(100.0), "Out of range").is_ok());
        assert!(rules::range_numeric("val", "5", Some(10.0), Some(100.0), "Out of range").is_err());
    }

    #[test]
    fn test_tel_rule() {
        assert!(rules::tel("phone", "+1 (555) 000-1234", "Invalid phone").is_ok());
        assert!(rules::tel("phone", "88005553535", "Invalid phone").is_ok());
        assert!(rules::tel("phone", "123", "Too short").is_err());
        assert!(rules::tel("phone", "phone#number", "Invalid chars").is_err());
    }

    #[test]
    fn test_one_of_and_none_of() {
        let roles = ["admin", "editor", "viewer"];
        assert!(rules::one_of("role", "admin", &roles, "Invalid role").is_ok());
        assert!(rules::one_of("role", "superadmin", &roles, "Invalid role").is_err());

        let reserved = ["admin", "root", "api"];
        assert!(rules::none_of("slug", "my-blog", &reserved, "Reserved").is_ok());
        assert!(rules::none_of("slug", "admin", &reserved, "Reserved").is_err());
    }

    #[test]
    fn test_uuid_rule() {
        assert!(rules::uuid("id", "550e8400-e29b-41d4-a716-446655440000", "Invalid UUID").is_ok());
        assert!(rules::uuid("id", "not-a-uuid", "Invalid UUID").is_err());
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

    #[test]
    fn test_validate_if_and_merge() {
        let is_company = true;
        let validator = FormValidator::new()
            .required("name", "John", "Name required")
            .validate_if(is_company, |v| {
                v.required("tax_id", "", "Tax ID required for companies")
            });

        assert_eq!(validator.error_count(), 1);
        assert_eq!(validator.field_error("tax_id"), Some("Tax ID required for companies"));

        let validator2 = FormValidator::new().add_error("general", "System maintenance");
        let merged = validator.merge(validator2);
        assert_eq!(merged.error_count(), 2);
    }
}
