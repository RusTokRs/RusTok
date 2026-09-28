//! Leptos context providers for form and field state.
//!
//! `FormContext` is provided by [`Form`](crate::Form) and consumed by structural
//! field components. `FieldContext` is provided by [`FormField`](crate::FormField)
//! and consumed by [`FormLabel`](crate::FormLabel), [`FormMessage`](crate::FormMessage),
//! and input controls.

use leptos::prelude::*;
use rustok_forms::{DirtyTracker, FormState};

/// Shared form state provided to all descendant components via Leptos context.
///
/// Created by `<Form>` and consumed by `<FormField>` and lower-level primitives.
#[derive(Clone, Copy)]
pub struct FormContext {
    /// Reactive read access to the form state.
    pub state: Signal<FormState>,
    /// Optional direct read-write access to form state for descendant mutating components.
    pub rw_state: Option<RwSignal<FormState>>,
    /// Optional dirty field tracking signal.
    pub dirty_tracker: Option<RwSignal<DirtyTracker>>,
}

impl FormContext {
    /// Create a FormContext with read-only state signal.
    pub fn new(state: impl Into<Signal<FormState>>) -> Self {
        Self {
            state: state.into(),
            rw_state: None,
            dirty_tracker: None,
        }
    }

    /// Create a FormContext with read-write access.
    pub fn from_rw(rw: RwSignal<FormState>) -> Self {
        Self {
            state: rw.into(),
            rw_state: Some(rw),
            dirty_tracker: None,
        }
    }

    /// Attach a reactive dirty tracker to this form context.
    pub fn with_dirty_tracker(mut self, tracker: RwSignal<DirtyTracker>) -> Self {
        self.dirty_tracker = Some(tracker);
        self
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

    /// Create a standalone FieldContext (with an idle fallback FormContext).
    pub fn standalone(name: impl Into<String>) -> Self {
        let n = name.into();
        Self {
            id: n.clone(),
            name: n,
            form: FormContext {
                state: Signal::derive(FormState::idle),
                rw_state: None,
                dirty_tracker: None,
            },
        }
    }

    /// Whether this field currently has a validation error.
    pub fn is_invalid(&self) -> bool {
        self.form.state.get().is_field_invalid(&self.name)
    }

    /// Whether this field is marked dirty in the form's dirty tracker.
    pub fn is_dirty(&self) -> bool {
        if let Some(ref tracker) = self.form.dirty_tracker {
            tracker.get().is_dirty(&self.name)
        } else {
            false
        }
    }

    /// Mark this field as modified in the parent form's dirty tracker.
    pub fn mark_dirty(&self) {
        if let Some(ref tracker) = self.form.dirty_tracker {
            tracker.update(|t| t.mark(&self.name));
        }
    }

    /// Mark this field as clean in the parent form's dirty tracker.
    pub fn mark_clean(&self) {
        if let Some(ref tracker) = self.form.dirty_tracker {
            tracker.update(|t| t.unmark(&self.name));
        }
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
        let form_ctx = FormContext::new(state_signal);
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
        let state_signal = Signal::derive(FormState::idle);
        let dirty = RwSignal::new(DirtyTracker::new());
        let form_ctx = FormContext::new(state_signal).with_dirty_tracker(dirty);
        let field_ctx = FieldContext::new("bio", "bio", form_ctx);

        assert!(!field_ctx.is_dirty());
        field_ctx.mark_dirty();
        assert!(field_ctx.is_dirty());
        field_ctx.mark_clean();
        assert!(!field_ctx.is_dirty());
    }

    #[test]
    fn test_standalone_field_context() {
        let field_ctx = FieldContext::standalone("title");
        assert_eq!(field_ctx.name, "title");
        assert_eq!(field_ctx.id, "title");
        assert!(!field_ctx.is_invalid());
        assert_eq!(field_ctx.error_message(), None);
        assert!(!field_ctx.is_submitting());
    }
}
