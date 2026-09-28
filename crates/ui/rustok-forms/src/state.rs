//! Form submission lifecycle and state tracking.

use serde::{Deserialize, Serialize};

use crate::FieldError;

/// Submission lifecycle of a form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
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

    pub fn idle() -> Self {
        Self {
            is_submitting: false,
            form_error: None,
            field_errors: Vec::new(),
        }
    }

    pub fn submitting() -> Self {
        Self {
            is_submitting: true,
            form_error: None,
            field_errors: Vec::new(),
        }
    }

    pub fn with_form_error(message: impl Into<String>) -> Self {
        Self {
            is_submitting: false,
            form_error: Some(message.into()),
            field_errors: Vec::new(),
        }
    }

    pub fn with_field_errors(field_errors: Vec<FieldError>) -> Self {
        Self {
            is_submitting: false,
            form_error: None,
            field_errors,
        }
    }

    // ── Builder ─────────────────────────────────────────────────────

    pub fn with_field_error(mut self, field: impl Into<String>, message: impl Into<String>) -> Self {
        self.field_errors.push(FieldError {
            field: field.into(),
            message: message.into(),
        });
        self
    }

    // ── Transitions ─────────────────────────────────────────────────

    pub fn set_submitting(&mut self) {
        self.is_submitting = true;
        self.form_error = None;
    }

    pub fn set_submitted_success(&mut self) {
        self.is_submitting = false;
        self.form_error = None;
        self.field_errors.clear();
    }

    pub fn set_submitted_failure(&mut self, message: impl Into<String>) {
        self.is_submitting = false;
        self.form_error = Some(message.into());
    }

    pub fn set_field_errors(&mut self, errors: Vec<FieldError>) {
        self.is_submitting = false;
        self.field_errors = errors;
    }

    // ── Queries ─────────────────────────────────────────────────────

    pub fn submission_status(&self) -> FormSubmissionStatus {
        if self.is_submitting {
            FormSubmissionStatus::Submitting
        } else if let Some(ref err) = self.form_error {
            FormSubmissionStatus::Failure(err.clone())
        } else {
            FormSubmissionStatus::Idle
        }
    }

    pub fn is_success(&self) -> bool {
        !self.is_submitting && self.form_error.is_none() && self.field_errors.is_empty()
    }

    /// Whether the form currently has no form-level error and no field-level errors.
    pub fn is_valid(&self) -> bool {
        !self.has_errors()
    }

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

    pub fn is_field_invalid(&self, field: &str) -> bool {
        self.field_errors.iter().any(|fe| fe.field == field)
    }

    pub fn has_errors(&self) -> bool {
        self.form_error.is_some() || !self.field_errors.is_empty()
    }

    /// Clear all validation errors for a specific field name.
    pub fn clear_field_error(&mut self, field: &str) {
        self.field_errors.retain(|fe| fe.field != field);
    }

    pub fn clear_errors(&mut self) {
        self.form_error = None;
        self.field_errors.clear();
    }

    pub fn reset(&mut self) {
        self.is_submitting = false;
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

        state.set_submitting();
        assert!(state.is_submitting);
        assert_eq!(state.submission_status(), FormSubmissionStatus::Submitting);

        state.set_submitted_success();
        assert!(!state.is_submitting);
        assert!(state.is_success());
        assert_eq!(state.submission_status(), FormSubmissionStatus::Idle);
    }

    #[test]
    fn lifecycle_submitting_to_failure() {
        let mut state = FormState::submitting();
        state.set_submitted_failure("Network error");
        assert!(!state.is_submitting);
        assert!(state.has_errors());
        assert_eq!(
            state.submission_status(),
            FormSubmissionStatus::Failure("Network error".to_string())
        );
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
    }

    #[test]
    fn set_field_errors_replaces() {
        let mut state = FormState::submitting();
        state.set_field_errors(vec![FieldError {
            field: "slug".into(),
            message: "Already taken".into(),
        }]);
        assert!(!state.is_submitting);
        assert!(state.is_field_invalid("slug"));
    }

    #[test]
    fn reset_clears_everything() {
        let mut state = FormState::with_form_error("Boom")
            .with_field_error("x", "bad");
        state.reset();
        assert!(!state.is_submitting);
        assert!(!state.has_errors());
        assert!(state.is_valid());
        assert!(state.is_success());
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
}
