use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
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

    pub fn clear_errors(&mut self) {
        self.form_error = None;
        self.field_errors.clear();
    }

    pub fn has_errors(&self) -> bool {
        self.form_error.is_some() || !self.field_errors.is_empty()
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
