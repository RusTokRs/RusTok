//! Form submission lifecycle and state tracking.

use std::hash::Hash;
use serde::{Deserialize, Serialize};

use crate::FieldError;

/// Submission lifecycle of a form.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum FormSubmissionStatus {
    #[default]
    Idle,
    Submitting,
    Success,
    Failure(String),
}

/// Complete form state: submission lifecycle, form-level error, and per-field errors.
///
/// Modules own field values as framework signals. `FormState` owns only the
/// submission lifecycle and validation-result evidence that structural form
/// components need to render labels, error messages, and disabled controls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormState {
    pub is_submitting: bool,
    #[serde(default)]
    pub is_success: bool,
    pub form_error: Option<String>,
    pub field_errors: Vec<FieldError>,
}

impl Default for FormState {
    fn default() -> Self {
        Self::idle()
    }
}

impl FormState {
    // ── Constructors ────────────────────────────────────────────────

    /// Create an initial idle form state.
    pub fn idle() -> Self {
        Self {
            is_submitting: false,
            is_success: false,
            form_error: None,
            field_errors: Vec::new(),
        }
    }

    /// Create a form state in the submitting lifecycle phase.
    pub fn submitting() -> Self {
        Self {
            is_submitting: true,
            is_success: false,
            form_error: None,
            field_errors: Vec::new(),
        }
    }

    /// Create a form state in the successful submission phase.
    pub fn success() -> Self {
        Self {
            is_submitting: false,
            is_success: true,
            form_error: None,
            field_errors: Vec::new(),
        }
    }

    /// Create a form state pre-populated with a form-level error message.
    pub fn with_form_error(message: impl Into<String>) -> Self {
        Self {
            is_submitting: false,
            is_success: false,
            form_error: Some(message.into()),
            field_errors: Vec::new(),
        }
    }

    /// Create a form state pre-populated with a list of field validation errors.
    pub fn with_field_errors(field_errors: Vec<FieldError>) -> Self {
        Self {
            is_submitting: false,
            is_success: false,
            form_error: None,
            field_errors,
        }
    }

    // ── Builder ─────────────────────────────────────────────────────

    /// Builder method to append a field error.
    pub fn with_field_error(mut self, field: impl Into<String>, message: impl Into<String>) -> Self {
        self.is_success = false;
        self.field_errors.push(FieldError {
            field: field.into(),
            message: message.into(),
        });
        self
    }

    // ── Transitions ─────────────────────────────────────────────────

    /// Mark the form as currently submitting.
    pub fn set_submitting(&mut self) {
        self.is_submitting = true;
        self.is_success = false;
        self.form_error = None;
    }

    /// Mark the form submission as successful and clear all errors.
    pub fn set_submitted_success(&mut self) {
        self.is_submitting = false;
        self.is_success = true;
        self.form_error = None;
        self.field_errors.clear();
    }

    /// Mark the form submission as failed with a top-level error message.
    pub fn set_submitted_failure(&mut self, message: impl Into<String>) {
        self.is_submitting = false;
        self.is_success = false;
        self.form_error = Some(message.into());
    }

    /// Replace all field errors and reset submitting state.
    pub fn set_field_errors(&mut self, errors: Vec<FieldError>) {
        self.is_submitting = false;
        self.is_success = false;
        self.field_errors = errors;
    }

    /// Add a single field error.
    pub fn add_field_error(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.is_success = false;
        self.field_errors.push(FieldError::new(field, message));
    }

    /// Add multiple field errors from an iterator.
    pub fn add_field_errors(&mut self, errors: impl IntoIterator<Item = FieldError>) {
        self.is_success = false;
        self.field_errors.extend(errors);
    }

    /// Replace or set an error for a specific field name.
    pub fn set_field_error(&mut self, field: &str, message: impl Into<String>) {
        self.is_success = false;
        self.clear_field_error(field);
        self.field_errors.push(FieldError::new(field, message));
    }

    // ── Queries ─────────────────────────────────────────────────────

    /// Calculate the current high-level submission status.
    pub fn submission_status(&self) -> FormSubmissionStatus {
        if self.is_submitting {
            FormSubmissionStatus::Submitting
        } else if let Some(ref err) = self.form_error {
            FormSubmissionStatus::Failure(err.clone())
        } else if let Some(first_field_err) = self.field_errors.first() {
            FormSubmissionStatus::Failure(first_field_err.message.clone())
        } else if self.is_success {
            FormSubmissionStatus::Success
        } else {
            FormSubmissionStatus::Idle
        }
    }

    /// Whether the form submission has completed successfully.
    pub fn is_success(&self) -> bool {
        self.is_success && !self.is_submitting && !self.has_errors()
    }

    /// Whether the form currently has no form-level error and no field-level errors.
    pub fn is_valid(&self) -> bool {
        !self.has_errors()
    }

    /// First error message for a specific field name, if any.
    pub fn field_error(&self, field: &str) -> Option<&str> {
        self.field_errors
            .iter()
            .find(|fe| fe.field == field)
            .map(|fe| fe.message.as_str())
    }

    /// All error messages associated with the given field name.
    pub fn field_errors_for(&self, field: &str) -> Vec<&str> {
        self.field_errors
            .iter()
            .filter(|fe| fe.field == field)
            .map(|fe| fe.message.as_str())
            .collect()
    }

    /// Check whether a specific field has validation errors.
    pub fn is_field_invalid(&self, field: &str) -> bool {
        self.field_errors.iter().any(|fe| fe.field == field)
    }

    /// Whether the form has any form-level or field-level errors.
    pub fn has_errors(&self) -> bool {
        self.form_error.is_some() || !self.field_errors.is_empty()
    }

    /// Whether the form has a form-level error.
    pub fn has_form_error(&self) -> bool {
        self.form_error.is_some()
    }

    /// Whether the form has any field-level errors.
    pub fn has_field_errors(&self) -> bool {
        !self.field_errors.is_empty()
    }

    /// Total count of errors (form-level + field-level).
    pub fn error_count(&self) -> usize {
        let form_err_count = if self.form_error.is_some() { 1 } else { 0 };
        form_err_count + self.field_errors.len()
    }

    /// Count of errors specifically for a given field name.
    pub fn field_error_count(&self, field: &str) -> usize {
        self.field_errors.iter().filter(|fe| fe.field == field).count()
    }

    /// Return the first error message available (form-level error, or first field-level error).
    pub fn first_error(&self) -> Option<&str> {
        if let Some(ref err) = self.form_error {
            Some(err.as_str())
        } else {
            self.field_errors.first().map(|fe| fe.message.as_str())
        }
    }

    /// Collect all error messages into a vector of strings.
    pub fn all_errors(&self) -> Vec<String> {
        let mut list = Vec::new();
        if let Some(ref err) = self.form_error {
            list.push(err.clone());
        }
        for fe in &self.field_errors {
            list.push(fe.message.clone());
        }
        list
    }

    /// Clear all validation errors for a specific field name.
    pub fn clear_field_error(&mut self, field: &str) {
        self.field_errors.retain(|fe| fe.field != field);
    }

    /// Clear the form-level error.
    pub fn clear_form_error(&mut self) {
        self.form_error = None;
    }

    /// Retain only field errors that satisfy the given predicate.
    pub fn retain_field_errors<F>(&mut self, f: F)
    where
        F: FnMut(&FieldError) -> bool,
    {
        self.field_errors.retain(f);
    }

    /// Clear all form-level and field-level errors.
    pub fn clear_errors(&mut self) {
        self.form_error = None;
        self.field_errors.clear();
    }

    /// Reset form state back to clean idle.
    pub fn reset(&mut self) {
        self.is_submitting = false;
        self.is_success = false;
        self.clear_errors();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_idle_to_submitting_to_success() {
        let mut state = FormState::idle();
        assert!(!state.is_submitting);
        assert!(!state.has_errors());
        assert!(!state.is_success());
        assert_eq!(state.error_count(), 0);
        assert_eq!(state.submission_status(), FormSubmissionStatus::Idle);

        state.set_submitting();
        assert!(state.is_submitting);
        assert!(!state.is_success());
        assert_eq!(state.submission_status(), FormSubmissionStatus::Submitting);

        state.set_submitted_success();
        assert!(!state.is_submitting);
        assert!(state.is_success());
        assert_eq!(state.submission_status(), FormSubmissionStatus::Success);
    }

    #[test]
    fn lifecycle_submitting_to_failure() {
        let mut state = FormState::submitting();
        state.set_submitted_failure("Network error");
        assert!(!state.is_submitting);
        assert!(!state.is_success());
        assert!(state.has_errors());
        assert_eq!(state.error_count(), 1);
        assert_eq!(
            state.submission_status(),
            FormSubmissionStatus::Failure("Network error".to_string())
        );
        assert_eq!(state.first_error(), Some("Network error"));
    }

    #[test]
    fn field_errors_query() {
        let state = FormState::idle()
            .with_field_error("email", "Invalid email")
            .with_field_error("name", "Required");
        assert!(state.is_field_invalid("email"));
        assert_eq!(state.field_error("email"), Some("Invalid email"));
        assert!(!state.is_field_invalid("password"));
        assert_eq!(state.field_error("password"), None);
        assert_eq!(state.first_error(), Some("Invalid email"));
        assert_eq!(state.all_errors(), vec!["Invalid email", "Required"]);
        assert_eq!(state.error_count(), 2);
    }

    #[test]
    fn set_field_errors_replaces() {
        let mut state = FormState::submitting();
        state.set_field_errors(vec![FieldError {
            field: "slug".into(),
            message: "Already taken".into(),
        }]);
        assert!(!state.is_submitting);
        assert!(!state.is_success());
        assert!(state.is_field_invalid("slug"));
        assert_eq!(
            state.submission_status(),
            FormSubmissionStatus::Failure("Already taken".to_string())
        );
    }

    #[test]
    fn reset_clears_everything() {
        let mut state = FormState::with_form_error("Boom")
            .with_field_error("x", "bad");
        state.reset();
        assert!(!state.is_submitting);
        assert!(!state.has_errors());
        assert!(state.is_valid());
        assert!(!state.is_success());
        assert_eq!(state.submission_status(), FormSubmissionStatus::Idle);
    }

    #[test]
    fn clear_field_error_retains_others() {
        let mut state = FormState::idle()
            .with_field_error("email", "Bad email")
            .with_field_error("username", "Too short");

        assert!(!state.is_valid());
        assert_eq!(state.field_errors_for("email"), vec!["Bad email"]);

        state.clear_field_error("email");
        assert!(!state.is_field_invalid("email"));
        assert!(state.is_field_invalid("username"));
        assert!(!state.is_valid());

        state.clear_field_error("username");
        assert!(state.is_valid());
    }

    #[test]
    fn add_and_set_field_error() {
        let mut state = FormState::idle();
        state.add_field_error("title", "Too short");
        state.add_field_error("title", "Must not contain swear words");
        assert_eq!(state.field_errors_for("title").len(), 2);
        assert_eq!(state.field_error_count("title"), 2);

        state.set_field_error("title", "New error");
        assert_eq!(state.field_errors_for("title"), vec!["New error"]);
        assert_eq!(state.field_error_count("title"), 1);
    }

    #[test]
    fn serde_backward_compatibility() {
        let json = r#"{"is_submitting":false,"form_error":null,"field_errors":[]}"#;
        let deserialized: FormState = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, FormState::idle());
        assert!(!deserialized.is_success());
    }
}
