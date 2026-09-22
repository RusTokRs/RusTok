//! Shared form submission state and field-error mapping.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FormSubmissionStatus {
    #[default]
    Idle,
    Submitting,
    Success,
    Failure(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormState {
    pub is_submitting: bool,
    pub form_error: Option<String>,
    pub field_errors: Vec<FieldError>,
}

impl FormState {
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

    pub fn with_field_error(mut self, field: impl Into<String>, message: impl Into<String>) -> Self {
        self.field_errors.push(FieldError {
            field: field.into(),
            message: message.into(),
        });
        self
    }

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

    pub fn field_error(&self, field: &str) -> Option<&str> {
        self.field_errors
            .iter()
            .find(|fe| fe.field == field)
            .map(|fe| fe.message.as_str())
    }

    pub fn is_field_invalid(&self, field: &str) -> bool {
        self.field_errors.iter().any(|fe| fe.field == field)
    }

    pub fn clear_errors(&mut self) {
        self.form_error = None;
        self.field_errors.clear();
    }

    pub fn reset(&mut self) {
        self.is_submitting = false;
        self.clear_errors();
    }

    pub fn has_errors(&self) -> bool {
        self.form_error.is_some() || !self.field_errors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_form_state_lifecycle() {
        let mut state = FormState::idle();
        assert!(!state.has_errors());
        assert!(!state.is_submitting);

        state = FormState::submitting();
        assert!(state.is_submitting);

        state = FormState::with_form_error("Submission failed");
        assert_eq!(state.form_error.as_deref(), Some("Submission failed"));
        assert!(state.has_errors());

        state = state.with_field_error("email", "Invalid email address");
        assert!(state.is_field_invalid("email"));
        assert!(!state.is_field_invalid("password"));
        assert_eq!(state.field_error("email"), Some("Invalid email address"));

        state.reset();
        assert!(!state.has_errors());
        assert!(!state.is_submitting);
        assert_eq!(state.submission_status(), FormSubmissionStatus::Idle);

        state.set_submitting();
        assert!(state.is_submitting);
        assert_eq!(state.submission_status(), FormSubmissionStatus::Submitting);

        state.set_submitted_failure("Network error");
        assert!(!state.is_submitting);
        assert_eq!(
            state.submission_status(),
            FormSubmissionStatus::Failure("Network error".to_string())
        );

        state.set_submitted_success();
        assert!(!state.is_submitting);
        assert!(state.is_success());
        assert_eq!(state.submission_status(), FormSubmissionStatus::Idle);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub path: Vec<String>,
    pub message: String,
}

pub fn issues_to_field_errors(issues: &[ValidationIssue]) -> Vec<FieldError> {
    issues
        .iter()
        .map(|issue| FieldError {
            field: issue.path.join("."),
            message: issue.message.clone(),
        })
        .collect()
}
