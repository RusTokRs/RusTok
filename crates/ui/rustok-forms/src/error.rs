//! Field-level and form-level validation error contracts.

use std::fmt;
use serde::{Deserialize, Serialize};

/// A validation error attached to a specific field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

impl FieldError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub path: Vec<String>,
    pub message: String,
}

impl ValidationIssue {
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

/// Convert structured validation issues to flat field errors.
///
/// Path segments are joined with `.` to form the field name.
pub fn issues_to_field_errors(issues: &[ValidationIssue]) -> Vec<FieldError> {
    issues.iter().cloned().map(FieldError::from).collect()
}

/// Convert flat field errors to structured validation issues.
///
/// Field names containing `.` are split into path segments.
pub fn field_errors_to_issues(errors: &[FieldError]) -> Vec<ValidationIssue> {
    errors
        .iter()
        .map(|err| {
            let path = if err.field.is_empty() {
                Vec::new()
            } else {
                err.field.split('.').map(String::from).collect()
            };
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
            FieldError::new("email", "Invalid format"),
        ];
        let issues = field_errors_to_issues(&errors);
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].path, vec!["address", "city"]);
        assert_eq!(issues[0].message, "Required");
        assert_eq!(issues[1].path, vec!["email"]);
        assert_eq!(issues[1].message, "Invalid format");
    }

    #[test]
    fn display_and_error_impls() {
        let err = FieldError::new("email", "Invalid email address");
        assert_eq!(format!("{err}"), "email: Invalid email address");

        let err_no_field = FieldError::new("", "Form failed");
        assert_eq!(format!("{err_no_field}"), "Form failed");

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
