//! Leptos context providers for form and field state.
//!
//! `FormContext` is provided by [`Form`](crate::Form) and consumed by structural
//! field components. `FieldContext` is provided by [`FormField`](crate::FormField)
//! and consumed by [`FormLabel`](crate::FormLabel), [`FormMessage`](crate::FormMessage),
//! and input controls.

use leptos::prelude::*;
use rustok_forms::FormState;

/// Shared form state provided to all descendant components via Leptos context.
///
/// Created by `<Form>` and consumed by `<FormField>` and lower-level primitives.
#[derive(Clone, Copy)]
pub struct FormContext {
    /// Reactive read access to the form state.
    pub state: Signal<FormState>,
}

impl FormContext {
    pub fn new(state: impl Into<Signal<FormState>>) -> Self {
        Self {
            state: state.into(),
        }
    }
}

/// Per-field context provided by `<FormField>` to its children.
///
/// Consumed by `<FormLabel>`, `<FormMessage>`, `<FormDescription>`, and inputs.
#[derive(Clone)]
pub struct FieldContext {
    /// The field name this context represents.
    pub name: String,
    /// Reference to the parent form state.
    pub form: FormContext,
}

impl FieldContext {
    pub fn new(name: impl Into<String>, form: FormContext) -> Self {
        Self {
            name: name.into(),
            form,
        }
    }

    /// Create a standalone FieldContext (with an idle fallback FormContext).
    pub fn standalone(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            form: FormContext {
                state: Signal::derive(FormState::idle),
            },
        }
    }

    /// Whether this field currently has a validation error.
    pub fn is_invalid(&self) -> bool {
        self.form.state.get().is_field_invalid(&self.name)
    }

    /// The first error message for this field, if any.
    pub fn error_message(&self) -> Option<String> {
        self.form
            .state
            .get()
            .field_error(&self.name)
            .map(String::from)
    }

    /// All error messages for this field.
    pub fn all_error_messages(&self) -> Vec<String> {
        self.form
            .state
            .get()
            .field_errors_for(&self.name)
            .into_iter()
            .map(String::from)
            .collect()
    }

    /// Whether the parent form is currently submitting.
    pub fn is_submitting(&self) -> bool {
        self.form.state.get().is_submitting
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_context_queries() {
        let state = FormState::idle()
            .with_field_error("username", "Already taken")
            .with_field_error("username", "Too short");

        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext { state: state_signal };
        let field_ctx = FieldContext::new("username", form_ctx);

        assert!(field_ctx.is_invalid());
        assert_eq!(field_ctx.error_message(), Some("Already taken".to_string()));
        assert_eq!(
            field_ctx.all_error_messages(),
            vec!["Already taken".to_string(), "Too short".to_string()]
        );
        assert!(!field_ctx.is_submitting());

        let clean_field_ctx = FieldContext::new("email", form_ctx);
        assert!(!clean_field_ctx.is_invalid());
        assert_eq!(clean_field_ctx.error_message(), None);
        assert!(clean_field_ctx.all_error_messages().is_empty());
    }

    #[test]
    fn test_standalone_field_context() {
        let field_ctx = FieldContext::standalone("title");
        assert_eq!(field_ctx.name, "title");
        assert!(!field_ctx.is_invalid());
        assert_eq!(field_ctx.error_message(), None);
        assert!(!field_ctx.is_submitting());
    }
}
