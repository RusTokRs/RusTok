//! Structural field components: `FormField`, `FormItem`, `FormLabel`,
//! `FormControl`, `FormMessage`, `FormDescription`.
//!
//! These are the building blocks for consistent form layout with automatic
//! error display and accessibility attributes.

use leptos::prelude::*;

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
///     <Input value=title set_value=set_title />
///     <FormDescription>"Enter the post title"</FormDescription>
///     <FormMessage />
/// </FormField>
/// ```
#[component]
pub fn FormField(
    /// The field name. Must match the key used in `FormState::field_error`.
    #[prop(into)]
    name: String,
    /// Extra CSS classes on the wrapper `<div>`.
    #[prop(optional, into)]
    class: String,
    children: Children,
) -> impl IntoView {
    let form_ctx = use_context::<FormContext>()
        .expect("FormField must be used inside <Form>");

    provide_context(FieldContext {
        name,
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
    /// Show a required indicator (`*`).
    #[prop(optional)]
    required: bool,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
    children: Children,
) -> impl IntoView {
    let field = use_context::<FieldContext>();
    let for_attr = field.as_ref().map(|f| f.name.clone());

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
                <span class="ml-1 text-destructive">"*"</span>
            })}
        </label>
    }
}

// ─── FormControl ────────────────────────────────────────────────────────────

/// Wrapper that sets `aria-invalid` and `aria-describedby` on its children.
///
/// In Leptos, this renders a `<div>` wrapper (unlike React's Slot). For most
/// cases, you can skip this and use `<FormField>` directly — the aria
/// attributes are primarily useful for screen-reader accessibility.
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
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let field_ctx = use_context::<FieldContext>();
    let form_ctx = use_context::<FormContext>();

    let error_text = move || -> Option<String> {
        if let Some(ref field) = field_ctx {
            field.error_message()
        } else if let Some(ref form) = form_ctx {
            form.state.get().form_error.clone()
        } else {
            None
        }
    };

    let base = "text-destructive text-sm";
    let merged = if class.is_empty() {
        base.to_string()
    } else {
        format!("{base} {class}")
    };

    view! {
        {move || error_text().map(|msg| view! {
            <p data-slot="form-message" class=merged.clone()>
                {msg}
            </p>
        })}
    }
}

// ─── FormDescription ────────────────────────────────────────────────────────

/// Help text below a form field.
#[component]
pub fn FormDescription(
    #[prop(optional, into)]
    class: String,
    children: Children,
) -> impl IntoView {
    let base = "text-muted-foreground text-sm";
    let merged = if class.is_empty() {
        base.to_string()
    } else {
        format!("{base} {class}")
    };
    view! {
        <p data-slot="form-description" class=merged>
            {children()}
        </p>
    }
}

// ─── FormError ──────────────────────────────────────────────────────────────

/// Displays the form-level (non-field) error from `FormState`.
///
/// Renders nothing when there is no form-level error.
#[component]
pub fn FormError(
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let form_ctx = use_context::<FormContext>();

    let error_text = move || -> Option<String> {
        form_ctx.as_ref().and_then(|ctx| ctx.state.get().form_error.clone())
    };

    let base = "rounded-md border border-destructive/30 bg-destructive/10 px-4 py-3 text-destructive text-sm";
    let merged = if class.is_empty() {
        base.to_string()
    } else {
        format!("{base} {class}")
    };

    view! {
        {move || error_text().map(|msg| view! {
            <div data-slot="form-error" class=merged.clone()>
                {msg}
            </div>
        })}
    }
}
