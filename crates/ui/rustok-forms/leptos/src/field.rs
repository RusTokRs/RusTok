//! Structural field components: `FormField`, `FormItem`, `FormLabel`,
//! `FormControl`, `FormMessage`, `FormDescription`, `FormHelperText`, `FormError`.
//!
//! These are the building blocks for consistent form layout with automatic
//! error display and accessibility attributes.

use leptos::prelude::*;
use rustok_forms::FormState;

use crate::context::{FieldContext, FormContext};

// ─── FormField ──────────────────────────────────────────────────────────────

/// Provides [`FieldContext`] for a named field.
///
/// Creates a field-scoped context that [`FormLabel`], [`FormMessage`], and
/// [`FormDescription`] read. Renders its children inside a layout wrapper
/// with consistent spacing.
///
/// # Usage
///
/// ```rust,ignore
/// <FormField name="title">
///     <FormLabel required=true>"Title"</FormLabel>
///     <FormInput value=title on_input=set_title />
///     <FormDescription>"Enter the post title"</FormDescription>
///     <FormMessage />
/// </FormField>
/// ```
#[component]
pub fn FormField(
    /// The field name. Must match the key used in `FormState::field_error`.
    #[prop(into)]
    name: String,
    /// Explicit target HTML id. If omitted, defaults to `name`.
    #[prop(optional, into)]
    id: Option<String>,
    /// Extra CSS classes on the wrapper `<div>`.
    #[prop(optional, into)]
    class: String,
    children: Children,
) -> impl IntoView {
    let form_ctx = use_context::<FormContext>().unwrap_or_else(|| {
        FormContext {
            state: Signal::derive(FormState::idle),
            rw_state: None,
            dirty_tracker: None,
        }
    });

    let field_id = id.unwrap_or_else(|| name.clone());

    provide_context(FieldContext {
        name,
        id: field_id,
        form: form_ctx,
    });

    let base = "grid gap-2";
    let merged = if class.is_empty() {
        base.to_string()
    } else {
        format!("{base} {class}")
    };

    view! {
        <div data-slot="form-item" class=merged>
            {children()}
        </div>
    }
}

// ─── FormItem ───────────────────────────────────────────────────────────────

/// Bare layout wrapper (`<div class="grid gap-2">`).
///
/// Useful when you need the spacing without a `FieldContext`. Prefer
/// `<FormField>` for fields that should show automatic error messages.
#[component]
pub fn FormItem(
    #[prop(optional, into)]
    class: String,
    children: Children,
) -> impl IntoView {
    let base = "grid gap-2";
    let merged = if class.is_empty() {
        base.to_string()
    } else {
        format!("{base} {class}")
    };
    view! { <div data-slot="form-item" class=merged>{children()}</div> }
}

// ─── FormLabel ──────────────────────────────────────────────────────────────

/// Label that reacts to field error state.
///
/// When the parent field has a validation error, applies destructive styling.
/// Renders a `<label>` with a required-indicator when `required` is set.
#[component]
pub fn FormLabel(
    /// Explicit target input ID. If omitted, defaults to the enclosing `FormField` ID.
    #[prop(optional, into)]
    html_for: Option<String>,
    /// Show a required indicator (`*`).
    #[prop(optional)]
    required: bool,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
    children: Children,
) -> impl IntoView {
    let field = use_context::<FieldContext>();
    let for_attr = html_for.or_else(|| field.as_ref().map(|f| f.id.clone()));

    let label_class = move || {
        let base = "text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70";
        let error_class = field
            .as_ref()
            .map(|f| {
                if f.is_invalid() {
                    " text-destructive"
                } else {
                    ""
                }
            })
            .unwrap_or("");
        if class.is_empty() {
            format!("{base}{error_class}")
        } else {
            format!("{base}{error_class} {class}")
        }
    };

    view! {
        <label for=for_attr data-slot="form-label" class=label_class>
            {children()}
            {required.then(|| view! {
                <span class="ml-1 text-destructive font-bold" aria-hidden="true">"*"</span>
            })}
        </label>
    }
}

// ─── FormControl ────────────────────────────────────────────────────────────

/// Wrapper that sets `aria-invalid` on its container for accessibility slot coordination.
#[component]
pub fn FormControl(
    #[prop(optional, into)]
    class: String,
    children: Children,
) -> impl IntoView {
    let field = use_context::<FieldContext>();

    let aria_invalid = move || {
        field.as_ref().map(|f| f.is_invalid()).unwrap_or(false)
    };

    view! {
        <div
            data-slot="form-control"
            class=class
            aria-invalid=move || aria_invalid().then_some("true")
        >
            {children()}
        </div>
    }
}

// ─── FormMessage ────────────────────────────────────────────────────────────

/// Automatic field error message display.
///
/// Reads the current field's error from `FormState` via `FieldContext`.
/// Renders nothing when there is no error.
///
/// Falls back to displaying the form-level error if no `FieldContext` is
/// available.
#[component]
pub fn FormMessage(
    /// Optional explicit message override. If omitted, reads from `FieldContext` or `FormContext`.
    #[prop(optional, into)]
    message: Option<String>,
    /// Whether to show all error messages if multiple errors exist for this field. Defaults to false (shows first error).
    #[prop(default = false)]
    show_all: bool,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let field_ctx = use_context::<FieldContext>();
    let form_ctx = use_context::<FormContext>();

    let msg_id = field_ctx.as_ref().map(|f| format!("{}-message", f.id));

    let errors_list = move || -> Vec<String> {
        if let Some(ref m) = message {
            vec![m.clone()]
        } else if let Some(ref field) = field_ctx {
            if show_all {
                field.all_error_messages()
            } else {
                field.error_message().into_iter().collect()
            }
        } else if let Some(ref form) = form_ctx {
            form.state.get().form_error.into_iter().collect()
        } else {
            Vec::new()
        }
    };

    let base = "text-destructive text-sm font-medium";
    let merged = if class.is_empty() {
        base.to_string()
    } else {
        format!("{base} {class}")
    };

    view! {
        {move || {
            let errs = errors_list();
            (!errs.is_empty()).then(|| {
                view! {
                    <div id=msg_id.clone() role="alert" data-slot="form-message" class="space-y-1">
                        {errs.into_iter().map(|msg| {
                            let c = merged.clone();
                            view! {
                                <p class=c>{msg}</p>
                            }
                        }).collect_view()}
                    </div>
                }
            })
        }}
    }
}

// ─── FormDescription / FormHelperText ───────────────────────────────────────

/// Help text below a form field.
#[component]
pub fn FormDescription(
    #[prop(optional, into)]
    class: String,
    children: Children,
) -> impl IntoView {
    let field_ctx = use_context::<FieldContext>();
    let desc_id = field_ctx.as_ref().map(|f| format!("{}-description", f.id));

    let base = "text-muted-foreground text-sm";
    let merged = if class.is_empty() {
        base.to_string()
    } else {
        format!("{base} {class}")
    };
    view! {
        <p id=desc_id data-slot="form-description" class=merged>
            {children()}
        </p>
    }
}

/// Alias for [`FormDescription`].
pub type FormHelperText = FormDescription;

// ─── FormError ──────────────────────────────────────────────────────────────

/// Displays the form-level (non-field) error from `FormState`.
///
/// Renders nothing when there is no form-level error.
#[component]
pub fn FormError(
    /// Optional header title above the error message.
    #[prop(optional, into)]
    title: Option<String>,
    /// Optional explicit error message override.
    #[prop(optional, into)]
    message: Option<String>,
    /// Optional callback fired when the dismiss button is clicked.
    #[prop(optional, into)]
    on_dismiss: Option<Callback<()>>,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let form_ctx = use_context::<FormContext>();

    let error_text = move || -> Option<String> {
        if let Some(ref m) = message {
            Some(m.clone())
        } else {
            form_ctx.as_ref().and_then(|ctx| ctx.state.get().form_error.clone())
        }
    };

    let base = "rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-destructive text-sm relative flex items-start justify-between gap-3";
    let merged = if class.is_empty() {
        base.to_string()
    } else {
        format!("{base} {class}")
    };

    let on_dismiss_click = move |_| {
        if let Some(cb) = on_dismiss {
            cb.run(());
        }
        if let Some(ctx) = form_ctx {
            if let Some(rw) = ctx.rw_state {
                rw.update(|s| s.clear_form_error());
            }
        }
    };

    let has_dismiss = on_dismiss.is_some() || form_ctx.and_then(|c| c.rw_state).is_some();

    view! {
        {move || error_text().map(|msg| view! {
            <div role="alert" data-slot="form-error" class=merged.clone()>
                <div class="flex-1">
                    {title.as_ref().map(|t| view! {
                        <div class="font-semibold mb-0.5">{t.clone()}</div>
                    })}
                    <div>{msg}</div>
                </div>
                {has_dismiss.then(|| view! {
                    <button
                        type="button"
                        aria-label="Dismiss error"
                        on:click=on_dismiss_click
                        class="text-destructive/80 hover:text-destructive p-0.5 rounded transition outline-none"
                    >
                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                    </button>
                })}
            </div>
        })}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_forms::FormState;

    #[test]
    fn test_form_field_renders_label_and_for_attr() {
        let state = FormState::idle();
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext::new(state_signal);

        let html = view! {
            <div>
                {provide_context(form_ctx)}
                <FormField name="email">
                    <FormLabel required=true>"Email"</FormLabel>
                    <FormDescription>"Enter your corporate email"</FormDescription>
                    <FormMessage />
                </FormField>
            </div>
        }
        .to_html();

        assert!(html.contains("for=\"email\""));
        assert!(html.contains("id=\"email-description\""));
        assert!(html.contains("Email"));
        assert!(html.contains("*"));
        assert!(html.contains("aria-hidden=\"true\""));
        // No error message since form state has no error
        assert!(!html.contains("data-slot=\"form-message\""));
    }

    #[test]
    fn test_form_field_renders_error_message_when_invalid() {
        let state = FormState::idle().with_field_error("email", "Email is required");
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext::new(state_signal);

        let html = view! {
            <div>
                {provide_context(form_ctx)}
                <FormField name="email">
                    <FormLabel>"Email"</FormLabel>
                    <FormMessage />
                </FormField>
            </div>
        }
        .to_html();

        assert!(html.contains("text-destructive"));
        assert!(html.contains("id=\"email-message\""));
        assert!(html.contains("role=\"alert\""));
        assert!(html.contains("Email is required"));
    }

    #[test]
    fn test_form_error_renders_form_level_message_and_title() {
        let state = FormState::with_form_error("Server unavailable");
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext::new(state_signal);

        let html = view! {
            <div>
                {provide_context(form_ctx)}
                <FormError title="Submission Failed" />
            </div>
        }
        .to_html();

        assert!(html.contains("role=\"alert\""));
        assert!(html.contains("Submission Failed"));
        assert!(html.contains("Server unavailable"));
    }
}
