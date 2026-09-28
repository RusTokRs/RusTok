//! Structural field components: `FormField`, `FormItem`, `FormLabel`,
//! `FormControl`, `FormMessage`, `FormDescription`, `FormHelperText`, `FormError`.
//!
//! These are the building blocks for consistent form layout with automatic
//! error display and accessibility attributes.

use dioxus::prelude::*;
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
/// rsx! {
///     FormField {
///         name: "title",
///         FormLabel { required: true, "Title" }
///         FormInput { value: title, on_input: move |v| title.set(v) }
///         FormDescription { "Enter the post title" }
///         FormMessage {}
///     }
/// }
/// ```
#[component]
pub fn FormField(
    /// The field name. Must match the key used in `FormState::field_error`.
    name: String,
    /// Explicit target HTML id. If omitted, defaults to `name`.
    #[props(default)]
    id: Option<String>,
    /// Extra CSS classes on the wrapper `<div>`.
    #[props(default)]
    class: Option<String>,
    /// Child form controls and labels.
    children: Element,
) -> Element {
    let form_ctx = use_context::<FormContext>();
    let field_id = id.unwrap_or_else(|| name.clone());

    provide_context(FieldContext::new(name, field_id, form_ctx));

    let custom_class = class.unwrap_or_default();
    let merged_class = format!("grid gap-2 {custom_class}").trim().to_string();

    rsx! {
        div {
            "data-slot": "form-item",
            class: "{merged_class}",
            {children}
        }
    }
}

// ─── FormItem ───────────────────────────────────────────────────────────────

/// Bare layout wrapper (`<div class="grid gap-2">`).
///
/// Useful when you need the spacing without a `FieldContext`. Prefer
/// `<FormField>` for fields that should show automatic error messages.
#[component]
pub fn FormItem(
    /// Extra CSS classes on the container `<div>`.
    #[props(default)]
    class: Option<String>,
    /// Child elements.
    children: Element,
) -> Element {
    let custom_class = class.unwrap_or_default();
    let merged_class = format!("grid gap-2 {custom_class}").trim().to_string();

    rsx! {
        div {
            "data-slot": "form-item",
            class: "{merged_class}",
            {children}
        }
    }
}

// ─── FormLabel ──────────────────────────────────────────────────────────────

/// Label that reacts to field error state.
///
/// When the parent field has a validation error, applies destructive styling.
/// Renders a `<label>` with a required-indicator when `required` is set.
#[component]
pub fn FormLabel(
    /// Explicit target input ID. If omitted, defaults to the enclosing `FormField` ID.
    #[props(default)]
    html_for: Option<String>,
    /// Show a required indicator (`*`).
    #[props(default = false)]
    required: bool,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
    /// Label content.
    children: Element,
) -> Element {
    let field = try_use_context::<FieldContext>();
    let for_attr = html_for.or_else(|| field.map(|f| f.id.to_string()));

    let is_invalid = field.map(|f| f.is_invalid()).unwrap_or(false);
    let base = "text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70";
    let error_class = if is_invalid { " text-destructive" } else { "" };
    let custom_class = class.unwrap_or_default();
    let label_class = format!("{base}{error_class} {custom_class}").trim().to_string();

    rsx! {
        label {
            r#for: for_attr,
            "data-slot": "form-label",
            class: "{label_class}",
            {children}
            if required {
                span {
                    class: "ml-1 text-destructive font-bold",
                    "aria-hidden": "true",
                    "*"
                }
            }
        }
    }
}

// ─── FormControl ────────────────────────────────────────────────────────────

/// Wrapper that sets `aria-invalid` on its container for accessibility slot coordination.
#[component]
pub fn FormControl(
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
    /// Child input control.
    children: Element,
) -> Element {
    let field = try_use_context::<FieldContext>();
    let is_invalid = field.map(|f| f.is_invalid()).unwrap_or(false);
    let custom_class = class.unwrap_or_default();

    rsx! {
        div {
            "data-slot": "form-control",
            class: "{custom_class}",
            "aria-invalid": if is_invalid { "true" } else { "false" },
            {children}
        }
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
    #[props(default)]
    message: Option<String>,
    /// Whether to show all error messages if multiple errors exist for this field. Defaults to false.
    #[props(default = false)]
    show_all: bool,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
) -> Element {
    let field_ctx = try_use_context::<FieldContext>();
    let form_ctx = try_use_context::<FormContext>();

    let msg_id = field_ctx.map(|f| format!("{}-message", f.id));

    let errors_list: Vec<String> = if let Some(m) = message {
        vec![m]
    } else if let Some(field) = field_ctx {
        if show_all {
            field.all_error_messages()
        } else {
            field.error_message().into_iter().collect()
        }
    } else if let Some(form) = form_ctx {
        form.form_error().into_iter().collect()
    } else {
        Vec::new()
    };

    if errors_list.is_empty() {
        return rsx! {};
    }

    let custom_class = class.unwrap_or_default();
    let text_class = format!("text-destructive text-sm font-medium {custom_class}").trim().to_string();

    rsx! {
        div {
            id: msg_id,
            role: "alert",
            "data-slot": "form-message",
            class: "space-y-1",
            for msg in errors_list {
                p {
                    class: "{text_class}",
                    "{msg}"
                }
            }
        }
    }
}

// ─── FormDescription / FormHelperText ───────────────────────────────────────

/// Help text below a form field.
#[component]
pub fn FormDescription(
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
    /// Descriptive help content.
    children: Element,
) -> Element {
    let field_ctx = try_use_context::<FieldContext>();
    let desc_id = field_ctx.map(|f| format!("{}-description", f.id));

    let custom_class = class.unwrap_or_default();
    let merged_class = format!("text-muted-foreground text-sm {custom_class}").trim().to_string();

    rsx! {
        p {
            id: desc_id,
            "data-slot": "form-description",
            class: "{merged_class}",
            {children}
        }
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
    #[props(default)]
    title: Option<String>,
    /// Optional explicit error message override.
    #[props(default)]
    message: Option<String>,
    /// Optional callback fired when the dismiss button is clicked.
    #[props(default)]
    on_dismiss: Option<EventHandler<()>>,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
) -> Element {
    let form_ctx = try_use_context::<FormContext>();

    let error_text = if let Some(m) = message {
        Some(m)
    } else {
        form_ctx.and_then(|ctx| ctx.form_error())
    };

    let msg = match error_text {
        Some(m) => m,
        None => return rsx! {},
    };

    let custom_class = class.unwrap_or_default();
    let base = "rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-destructive text-sm relative flex items-start justify-between gap-3";
    let merged_class = format!("{base} {custom_class}").trim().to_string();

    let has_dismiss = on_dismiss.is_some() || form_ctx.is_some();

    rsx! {
        div {
            role: "alert",
            "data-slot": "form-error",
            class: "{merged_class}",
            div {
                class: "flex-1",
                if let Some(ref t) = title {
                    div { class: "font-semibold mb-0.5", "{t}" }
                }
                div { "{msg}" }
            }
            if has_dismiss {
                button {
                    r#type: "button",
                    "aria-label": "Dismiss error",
                    class: "text-destructive/80 hover:text-destructive p-0.5 rounded transition outline-none",
                    onclick: move |_| {
                        if let Some(cb) = on_dismiss {
                            cb.call(());
                        }
                        if let Some(mut ctx) = form_ctx {
                            ctx.state.write().clear_form_error();
                        }
                    },
                    svg {
                        class: "w-4 h-4",
                        fill: "none",
                        view_box: "0 0 24 24",
                        stroke: "currentColor",
                        stroke_width: "2",
                        path {
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            d: "M6 18L18 6M6 6l12 12"
                        }
                    }
                }
            }
        }
    }
}
