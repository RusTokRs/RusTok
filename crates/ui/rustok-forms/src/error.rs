//! Field-level and form-level validation error contracts.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::Hash;

/// A validation error attached to a specific field.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldError {
    /// The target field name (or empty string for form-level errors).
    pub field: String,
    /// The localized or human-readable error message.
    pub message: String,
}

impl FieldError {
    /// Create a new `FieldError` for a specific field name.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rustok_forms::FieldError;
    ///
    /// let err = FieldError::new("email", "Please enter a valid email address");
    /// assert_eq!(err.field, "email");
    /// assert_eq!(err.message, "Please enter a valid email address");
    /// ```
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Create a form-level error (empty field name).
    ///
    /// # Example
    ///
    /// ```rust
    /// use rustok_forms::FieldError;
    ///
    /// let err = FieldError::form("Server unreachable, please try again later.");
    /// assert!(err.is_form_level());
    /// ```
    pub fn form(message: impl Into<String>) -> Self {
        Self {
            field: String::new(),
            message: message.into(),
        }
    }

    /// Whether this error applies to the form as a whole rather than a specific field.
    pub fn is_form_level(&self) -> bool {
        self.field.is_empty()
    }
}

impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.field.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}: {}", self.field, self.message)
        }
    }
}

impl std::error::Error for FieldError {}

impl From<(&str, &str)> for FieldError {
    fn from((field, message): (&str, &str)) -> Self {
        Self::new(field, message)
    }
}

impl From<(String, String)> for FieldError {
    fn from((field, message): (String, String)) -> Self {
        Self::new(field, message)
    }
}

impl From<(&str, String)> for FieldError {
    fn from((field, message): (&str, String)) -> Self {
        Self::new(field, message)
    }
}

impl From<(String, &str)> for FieldError {
    fn from((field, message): (String, &str)) -> Self {
        Self::new(field, message)
    }
}

/// A validation issue with a structured path (for nested/array fields).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ValidationIssue {
    /// Segments identifying the nested path of the invalid value (e.g. `["items", "0", "price"]`).
    pub path: Vec<String>,
    /// The localized or human-readable error message.
    pub message: String,
}

impl ValidationIssue {
    /// Create a new `ValidationIssue` with an explicit path vector.
    pub fn new(path: Vec<String>, message: impl Into<String>) -> Self {
        Self {
            path,
            message: message.into(),
        }
    }

    /// Convenience for a single-segment path.
    pub fn field(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: vec![field.into()],
            message: message.into(),
        }
    }

    /// Convenience for a multi-segment path given as an iterator/slice of string-like items.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rustok_forms::ValidationIssue;
    ///
    /// let issue = ValidationIssue::nested(["addresses", "0", "city"], "City is required");
    /// assert_eq!(issue.joined_path(), "addresses.0.city");
    /// ```
    pub fn nested<I, S>(segments: I, message: impl Into<String>) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            path: segments.into_iter().map(Into::into).collect(),
            message: message.into(),
        }
    }

    /// Return the joined path with '.' as separator.
    pub fn joined_path(&self) -> String {
        self.path.join(".")
    }
}

impl fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path_str = self.joined_path();
        if path_str.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}: {}", path_str, self.message)
        }
    }
}

impl std::error::Error for ValidationIssue {}

impl From<ValidationIssue> for FieldError {
    fn from(issue: ValidationIssue) -> Self {
        FieldError {
            field: issue.joined_path(),
            message: issue.message,
        }
    }
}

/// Parse a field path string supporting both dot notation (`user.address.city`)
/// and bracket notation (`items[0].name` or `users[1][street]`).
///
/// # Example
///
/// ```rust
/// use rustok_forms::parse_field_path;
///
/// assert_eq!(parse_field_path("users[0].address.city"), vec!["users", "0", "address", "city"]);
/// ```
pub fn parse_field_path(path: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_bracket = false;

    for ch in path.chars() {
        match ch {
            '.' if !in_bracket => {
                if !current.is_empty() {
                    segments.push(std::mem::take(&mut current));
                }
            }
            '[' => {
                if !current.is_empty() {
                    segments.push(std::mem::take(&mut current));
                }
                in_bracket = true;
            }
            ']' => {
                if !current.is_empty() {
                    segments.push(std::mem::take(&mut current));
                }
                in_bracket = false;
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        segments.push(current);
    }

    segments
}

/// Format path segments into a unified dot-separated string.
///
/// # Example
///
/// ```rust
/// use rustok_forms::format_field_path;
///
/// let formatted = format_field_path(&["users", "0", "name"]);
/// assert_eq!(formatted, "users.0.name");
/// ```
pub fn format_field_path<S: AsRef<str>>(segments: &[S]) -> String {
    segments
        .iter()
        .map(|s| s.as_ref())
        .collect::<Vec<&str>>()
        .join(".")
}

/// Convert structured validation issues to flat field errors.
///
/// Path segments are joined with `.` to form the field name.
pub fn issues_to_field_errors(issues: &[ValidationIssue]) -> Vec<FieldError> {
    issues.iter().cloned().map(FieldError::from).collect()
}

/// Convert flat field errors to structured validation issues.
///
/// Parses field names using dot and bracket notation into path segments.
pub fn field_errors_to_issues(errors: &[FieldError]) -> Vec<ValidationIssue> {
    errors
        .iter()
        .map(|err| {
            let path = parse_field_path(&err.field);
            ValidationIssue::new(path, &err.message)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issues_to_field_errors_joins_path() {
        let issues = vec![
            ValidationIssue::new(vec!["address".into(), "city".into()], "Required"),
            ValidationIssue::field("email", "Invalid format"),
            ValidationIssue::nested(["items", "0", "name"], "Name required"),
        ];
        let errors = issues_to_field_errors(&issues);
        assert_eq!(errors.len(), 3);
        assert_eq!(errors[0].field, "address.city");
        assert_eq!(errors[0].message, "Required");
        assert_eq!(errors[1].field, "email");
        assert_eq!(errors[1].message, "Invalid format");
        assert_eq!(errors[2].field, "items.0.name");
        assert_eq!(errors[2].message, "Name required");
    }

    #[test]
    fn field_errors_to_issues_splits_path() {
        let errors = vec![
            FieldError::new("address.city", "Required"),
            FieldError::new("items[0].name", "Required"),
            FieldError::new("users[1][street]", "Too short"),
        ];
        let issues = field_errors_to_issues(&errors);
        assert_eq!(issues.len(), 3);
        assert_eq!(issues[0].path, vec!["address", "city"]);
        assert_eq!(issues[0].message, "Required");
        assert_eq!(issues[1].path, vec!["items", "0", "name"]);
        assert_eq!(issues[1].message, "Required");
        assert_eq!(issues[2].path, vec!["users", "1", "street"]);
        assert_eq!(issues[2].message, "Too short");
    }

    #[test]
    fn parse_field_path_variants() {
        assert_eq!(parse_field_path("a.b.c"), vec!["a", "b", "c"]);
        assert_eq!(
            parse_field_path("items[0].title"),
            vec!["items", "0", "title"]
        );
        assert_eq!(parse_field_path("matrix[0][1]"), vec!["matrix", "0", "1"]);
        assert_eq!(parse_field_path("name"), vec!["name"]);
        assert_eq!(parse_field_path(""), Vec::<String>::new());
    }

    #[test]
    fn display_and_error_impls() {
        let err = FieldError::new("email", "Invalid email address");
        assert_eq!(format!("{err}"), "email: Invalid email address");

        let err_form = FieldError::form("Form failed");
        assert!(err_form.is_form_level());
        assert_eq!(format!("{err_form}"), "Form failed");

        let issue = ValidationIssue::nested(["user", "profile", "age"], "Must be positive");
        assert_eq!(format!("{issue}"), "user.profile.age: Must be positive");
    }

    #[test]
    fn from_tuples() {
        let err1: FieldError = ("username", "Too short").into();
        assert_eq!(err1.field, "username");
        assert_eq!(err1.message, "Too short");

        let err2: FieldError = (String::from("age"), String::from("Invalid")).into();
        assert_eq!(err2.field, "age");
        assert_eq!(err2.message, "Invalid");
    }
}
