//! Field-level and form-level validation error contracts.

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
}

/// Convert structured validation issues to flat field errors.
///
/// Path segments are joined with `.` to form the field name.
pub fn issues_to_field_errors(issues: &[ValidationIssue]) -> Vec<FieldError> {
    issues
        .iter()
        .map(|issue| FieldError {
            field: issue.path.join("."),
            message: issue.message.clone(),
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
        ];
        let errors = issues_to_field_errors(&issues);
        assert_eq!(errors[0].field, "address.city");
        assert_eq!(errors[0].message, "Required");
        assert_eq!(errors[1].field, "email");
        assert_eq!(errors[1].message, "Invalid format");
    }
}
