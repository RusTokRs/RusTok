//! Leptos context providers for form and field state.
//!
//! `FormContext` is provided by [`Form`](crate::Form) and consumed by structural
//! field components. `FieldContext` is provided by [`FormField`](crate::FormField)
//! and consumed by [`FormLabel`](crate::FormLabel), [`FormMessage`](crate::FormMessage),
//! and [`FormDescription`](crate::FormDescription).

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

/// Per-field context provided by `<FormField>` to its children.
///
/// Consumed by `<FormLabel>`, `<FormMessage>`, `<FormDescription>`.
#[derive(Clone)]
pub struct FieldContext {
    /// The field name this context represents.
    pub name: String,
    /// Reference to the parent form state.
    pub form: FormContext,
}

impl FieldContext {
    /// Whether this field currently has a validation error.
    pub fn is_invalid(&self) -> bool {
        self.form.state.get().is_field_invalid(&self.name)
    }

    /// The current error message for this field, if any.
    pub fn error_message(&self) -> Option<String> {
        self.form
            .state
            .get()
            .field_error(&self.name)
            .map(String::from)
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
            .with_field_error("username", "Already taken");

        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext { state: state_signal };
        let field_ctx = FieldContext {
            name: "username".to_string(),
            form: form_ctx,
        };

        assert!(field_ctx.is_invalid());
        assert_eq!(field_ctx.error_message(), Some("Already taken".to_string()));
        assert!(!field_ctx.is_submitting());

        let clean_field_ctx = FieldContext {
            name: "email".to_string(),
            form: form_ctx,
        };
        assert!(!clean_field_ctx.is_invalid());
        assert_eq!(clean_field_ctx.error_message(), None);
    }
}
