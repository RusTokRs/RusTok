//! Dioxus context providers for form and field state.
//!
//! `FormContext` is provided by [`Form`](crate::Form) and consumed by structural
//! field components. `FieldContext` is provided by [`FormField`](crate::FormField)
//! and consumed by [`FormLabel`](crate::FormLabel), [`FormMessage`](crate::FormMessage),
//! and input controls.

use dioxus::prelude::*;
use rustok_forms::{DirtyTracker, FormState};

/// Shared form state provided to all descendant components via Dioxus context.
///
/// Created by `<Form>` and consumed by `<FormField>` and lower-level primitives.
#[derive(Clone, Copy)]
pub struct FormContext {
    /// Reactive read/write access to the form state.
    pub state: Signal<FormState>,
    /// Optional dirty field tracking signal.
    pub dirty_tracker: Option<Signal<DirtyTracker>>,
}

impl FormContext {
    /// Create a `FormContext` wrapping a reactive `Signal<FormState>`.
    pub fn new(state: Signal<FormState>) -> Self {
        Self {
            state,
            dirty_tracker: None,
        }
    }

    /// Attach a reactive dirty tracker to this form context.
    pub fn with_dirty_tracker(mut self, tracker: Signal<DirtyTracker>) -> Self {
        self.dirty_tracker = Some(tracker);
        self
    }

    /// Returns `true` if the form is currently submitting.
    pub fn is_submitting(&self) -> bool {
        self.state.read().is_submitting
    }

    /// Returns `true` if a specific field has a validation error.
    pub fn is_field_invalid(&self, field: &str) -> bool {
        self.state.read().is_field_invalid(field)
    }

    /// Returns `true` if a specific field is marked dirty in the dirty tracker.
    pub fn is_field_dirty(&self, field: &str) -> bool {
        if let Some(ref tracker) = self.dirty_tracker {
            tracker.read().is_dirty(field)
        } else {
            false
        }
    }

    /// Mark a field as modified in the dirty tracker.
    pub fn mark_dirty(&self, field: impl Into<String>) {
        if let Some(mut tracker) = self.dirty_tracker {
            tracker.write().mark(field);
        }
    }

    /// Mark a field as clean in the dirty tracker.
    pub fn mark_clean(&self, field: &str) {
        if let Some(mut tracker) = self.dirty_tracker {
            tracker.write().unmark(field);
        }
    }

    /// Get the first validation error message for a field, if any.
    pub fn field_error(&self, field: &str) -> Option<String> {
        self.state.read().field_error(field).map(String::from)
    }

    /// Get all validation error messages for a field.
    pub fn field_errors_for(&self, field: &str) -> Vec<String> {
        self.state
            .read()
            .field_errors_for(field)
            .into_iter()
            .map(String::from)
            .collect()
    }

    /// Get the form-level (non-field) error message, if any.
    pub fn form_error(&self) -> Option<String> {
        self.state.read().form_error.clone()
    }
}

/// Per-field context provided by `<FormField>` to its children.
///
/// Consumed by `<FormLabel>`, `<FormMessage>`, `<FormDescription>`, and inputs.
#[derive(Clone)]
pub struct FieldContext {
    /// The field name this context represents.
    pub name: String,
    /// The HTML id associated with this field control.
    pub id: String,
    /// Reference to the parent form context.
    pub form: FormContext,
}

impl FieldContext {
    /// Create a new `FieldContext` with explicit name, id, and parent `FormContext`.
    pub fn new(name: impl Into<String>, id: impl Into<String>, form: FormContext) -> Self {
        Self {
            name: name.into(),
            id: id.into(),
            form,
        }
    }

    /// Whether this field currently has a validation error.
    pub fn is_invalid(&self) -> bool {
        self.form.is_field_invalid(&self.name)
    }

    /// Whether this field is marked dirty in the form's dirty tracker.
    pub fn is_dirty(&self) -> bool {
        self.form.is_field_dirty(&self.name)
    }

    /// Mark this field as modified in the parent form's dirty tracker.
    pub fn mark_dirty(&self) {
        self.form.mark_dirty(&self.name);
    }

    /// Mark this field as clean in the parent form's dirty tracker.
    pub fn mark_clean(&self) {
        self.form.mark_clean(&self.name);
    }

    /// The first error message for this field, if any.
    pub fn error_message(&self) -> Option<String> {
        self.form.field_error(&self.name)
    }

    /// All error messages for this field.
    pub fn all_error_messages(&self) -> Vec<String> {
        self.form.field_errors_for(&self.name)
    }

    /// Whether the parent form is currently submitting.
    pub fn is_submitting(&self) -> bool {
        self.form.is_submitting()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_context_queries() {
        let state = Signal::new(
            FormState::idle()
                .with_field_error("username", "Already taken")
                .with_field_error("username", "Too short"),
        );
        let form_ctx = FormContext::new(state);
        let field_ctx = FieldContext::new("username", "username", form_ctx);

        assert!(field_ctx.is_invalid());
        assert_eq!(field_ctx.error_message(), Some("Already taken".to_string()));
        assert_eq!(
            field_ctx.all_error_messages(),
            vec!["Already taken".to_string(), "Too short".to_string()]
        );
        assert!(!field_ctx.is_submitting());

        let clean_field_ctx = FieldContext::new("email", "email", form_ctx);
        assert!(!clean_field_ctx.is_invalid());
        assert_eq!(clean_field_ctx.error_message(), None);
        assert!(clean_field_ctx.all_error_messages().is_empty());
    }

    #[test]
    fn test_field_context_dirty_tracking() {
        let state = Signal::new(FormState::idle());
        let dirty = Signal::new(DirtyTracker::new());
        let form_ctx = FormContext::new(state).with_dirty_tracker(dirty);
        let field_ctx = FieldContext::new("bio", "bio", form_ctx);

        assert!(!field_ctx.is_dirty());
        field_ctx.mark_dirty();
        assert!(field_ctx.is_dirty());
        field_ctx.mark_clean();
        assert!(!field_ctx.is_dirty());
    }
}
