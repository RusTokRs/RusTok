//! Interactive form control components styled with RusToK design tokens.
//!
//! All controls automatically detect when rendered inside a [`FormField`](crate::FormField)
//! via [`FieldContext`](crate::context::FieldContext), inheriting the field name,
//! accessibility attributes (`aria-invalid`, `aria-describedby`, `aria-errormessage`),
//! error state styling (`border-destructive`), and dirty tracking integration.

use dioxus::prelude::*;
use rustok_forms::FieldOption;

use crate::context::FieldContext;

// ─── FormInput ─────────────────────────────────────────────────────────────

/// Text-like input component (text, email, password, number, search, url, date, etc.).
#[component]
pub fn FormInput(
    /// Input value.
    value: String,
    /// Callback on input.
    #[props(default)]
    on_input: Option<EventHandler<String>>,
    /// Callback on change.
    #[props(default)]
    on_change: Option<EventHandler<String>>,
    /// Callback on blur.
    #[props(default)]
    on_blur: Option<EventHandler<()>>,
    /// Callback on focus.
    #[props(default)]
    on_focus: Option<EventHandler<()>>,
    /// Input type (e.g. "text", "email", "password", "number", "search", "url"). Defaults to "text".
    #[props(default = "text")]
    input_type: &'static str,
    /// Field name. Defaults to the enclosing `FormField` name if omitted.
    #[props(default)]
    name: Option<String>,
    /// Input ID. Defaults to enclosing `FormField` ID if omitted.
    #[props(default)]
    id: Option<String>,
    /// Placeholder text.
    #[props(default)]
    placeholder: Option<String>,
    /// Disabled state. If omitted, disables automatically during form submission.
    #[props(default = false)]
    disabled: bool,
    /// Required attribute.
    #[props(default = false)]
    required: bool,
    /// Read-only state.
    #[props(default = false)]
    readonly: bool,
    /// Autofocus state.
    #[props(default = false)]
    autofocus: bool,
    /// Minimum value (for numbers, dates).
    #[props(default)]
    min: Option<String>,
    /// Maximum value (for numbers, dates).
    #[props(default)]
    max: Option<String>,
    /// Step value (for numbers).
    #[props(default)]
    step: Option<String>,
    /// Minimum text length.
    #[props(default)]
    minlength: Option<usize>,
    /// Maximum text length.
    #[props(default)]
    maxlength: Option<usize>,
    /// Regex pattern for client-side hint.
    #[props(default)]
    pattern: Option<String>,
    /// Autocomplete attribute.
    #[props(default)]
    autocomplete: Option<String>,
    /// Explicit ARIA label.
    #[props(default)]
    aria_label: Option<String>,
    /// Explicit ARIA describedby override.
    #[props(default)]
    aria_describedby: Option<String>,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
) -> Element {
    let field = try_use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.map(|f| f.name.to_string()));
    let input_id = id.or_else(|| field.map(|f| f.id.to_string()).or_else(|| name_attr.clone()));

    let is_submitting = field.map(|f| f.is_submitting()).unwrap_or(false);
    let is_disabled = disabled || is_submitting;
    let is_invalid = field.map(|f| f.is_invalid()).unwrap_or(false);

    let aria_desc = if let Some(ref explicit) = aria_describedby {
        Some(explicit.clone())
    } else {
        input_id.as_ref().map(|n| {
            if is_invalid {
                format!("{n}-message")
            } else {
                format!("{n}-description")
            }
        })
    };

    let aria_err = if is_invalid {
        input_id.as_ref().map(|n| format!("{n}-message"))
    } else {
        None
    };

    let base = "w-full rounded-xl border bg-background px-3 py-2 text-xs text-foreground placeholder:text-muted-foreground/60 outline-none transition disabled:cursor-not-allowed disabled:opacity-50";
    let state_border = if is_invalid {
        "border-destructive text-destructive focus:border-destructive focus:ring-1 focus:ring-destructive/30"
    } else {
        "border-border focus:border-primary focus:ring-1 focus:ring-primary/20"
    };
    let custom_class = class.unwrap_or_default();
    let input_class = format!("{base} {state_border} {custom_class}").trim().to_string();

    rsx! {
        input {
            id: input_id,
            r#type: input_type,
            name: name_attr,
            placeholder: placeholder,
            value: "{value}",
            disabled: is_disabled,
            required: required,
            readonly: readonly,
            autofocus: autofocus,
            min: min,
            max: max,
            step: step,
            minlength: minlength.map(|l| l.to_string()),
            maxlength: maxlength.map(|l| l.to_string()),
            pattern: pattern,
            autocomplete: autocomplete,
            aria_label: aria_label,
            "aria-invalid": if is_invalid { "true" } else { "false" },
            aria_describedby: aria_desc,
            aria_errormessage: aria_err,
            class: "{input_class}",
            oninput: move |ev| {
                if let Some(ref f) = field {
                    f.mark_dirty();
                }
                if let Some(ref cb) = on_input {
                    cb.call(ev.value());
                }
            },
            onchange: move |ev| {
                if let Some(ref cb) = on_change {
                    cb.call(ev.value());
                }
            },
            onblur: move |_| {
                if let Some(ref cb) = on_blur {
                    cb.call(());
                }
            },
            onfocus: move |_| {
                if let Some(ref cb) = on_focus {
                    cb.call(());
                }
            }
        }
    }
}

// ─── FormPasswordInput ─────────────────────────────────────────────────────

/// Specialized password input with show/hide password toggle.
#[component]
pub fn FormPasswordInput(
    /// Input value.
    value: String,
    /// Callback on input.
    #[props(default)]
    on_input: Option<EventHandler<String>>,
    /// Callback on change.
    #[props(default)]
    on_change: Option<EventHandler<String>>,
    /// Callback on blur.
    #[props(default)]
    on_blur: Option<EventHandler<()>>,
    /// Field name. Defaults to enclosing `FormField` name if omitted.
    #[props(default)]
    name: Option<String>,
    /// Input ID. Defaults to enclosing `FormField` ID if omitted.
    #[props(default)]
    id: Option<String>,
    /// Placeholder text.
    #[props(default)]
    placeholder: Option<String>,
    /// Disabled state.
    #[props(default = false)]
    disabled: bool,
    /// Required attribute.
    #[props(default = false)]
    required: bool,
    /// Read-only state.
    #[props(default = false)]
    readonly: bool,
    /// Minimum text length.
    #[props(default)]
    minlength: Option<usize>,
    /// Maximum text length.
    #[props(default)]
    maxlength: Option<usize>,
    /// Autocomplete attribute (e.g. "current-password", "new-password").
    #[props(default)]
    autocomplete: Option<String>,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
) -> Element {
    let mut show_password = use_signal(|| false);

    let input_type = if *show_password.read() { "text" } else { "password" };
    let custom_class = class.unwrap_or_default();
    let input_class = format!("pr-9 {custom_class}").trim().to_string();

    rsx! {
        div {
            class: "relative flex items-center",
            FormInput {
                value: value,
                on_input: on_input,
                on_change: on_change,
                on_blur: on_blur,
                input_type: input_type,
                name: name,
                id: id,
                placeholder: placeholder,
                disabled: disabled,
                required: required,
                readonly: readonly,
                minlength: minlength,
                maxlength: maxlength,
                autocomplete: autocomplete,
                class: Some(input_class),
            }
            button {
                r#type: "button",
                tabindex: "-1",
                aria_label: if *show_password.read() { "Hide password" } else { "Show password" },
                class: "absolute right-2.5 p-1 text-muted-foreground hover:text-foreground transition rounded",
                onclick: move |_| {
                    let cur = *show_password.read();
                    show_password.set(!cur);
                },
                if *show_password.read() {
                    svg {
                        class: "w-3.5 h-3.5",
                        fill: "none",
                        view_box: "0 0 24 24",
                        stroke: "currentColor",
                        stroke_width: "2",
                        path {
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            d: "M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l18 18"
                        }
                    }
                } else {
                    svg {
                        class: "w-3.5 h-3.5",
                        fill: "none",
                        view_box: "0 0 24 24",
                        stroke: "currentColor",
                        stroke_width: "2",
                        path {
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            d: "M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                        }
                        path {
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            d: "M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"
                        }
                    }
                }
            }
        }
    }
}

// ─── FormNumberInput ───────────────────────────────────────────────────────

/// Numeric input component with optional min, max, and step constraints.
#[component]
pub fn FormNumberInput(
    /// Input value.
    value: String,
    /// Callback on input.
    #[props(default)]
    on_input: Option<EventHandler<String>>,
    /// Callback on change.
    #[props(default)]
    on_change: Option<EventHandler<String>>,
    /// Field name. Defaults to enclosing `FormField` name if omitted.
    #[props(default)]
    name: Option<String>,
    /// Input ID. Defaults to enclosing `FormField` ID if omitted.
    #[props(default)]
    id: Option<String>,
    /// Minimum allowed value.
    #[props(default)]
    min: Option<f64>,
    /// Maximum allowed value.
    #[props(default)]
    max: Option<f64>,
    /// Step interval.
    #[props(default)]
    step: Option<f64>,
    /// Placeholder text.
    #[props(default)]
    placeholder: Option<String>,
    /// Disabled state.
    #[props(default = false)]
    disabled: bool,
    /// Required attribute.
    #[props(default = false)]
    required: bool,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
) -> Element {
    rsx! {
        FormInput {
            value: value,
            on_input: on_input,
            on_change: on_change,
            input_type: "number",
            name: name,
            id: id,
            min: min.map(|v| v.to_string()),
            max: max.map(|v| v.to_string()),
            step: step.map(|v| v.to_string()),
            placeholder: placeholder,
            disabled: disabled,
            required: required,
            class: class,
        }
    }
}

// ─── FormSearchInput ───────────────────────────────────────────────────────

/// Search input component with leading search icon and optional clear button.
#[component]
pub fn FormSearchInput(
    /// Input value.
    value: String,
    /// Callback on input.
    #[props(default)]
    on_input: Option<EventHandler<String>>,
    /// Callback on clear button click.
    #[props(default)]
    on_clear: Option<EventHandler<()>>,
    /// Field name.
    #[props(default)]
    name: Option<String>,
    /// Input ID.
    #[props(default)]
    id: Option<String>,
    /// Placeholder text.
    #[props(default = "Search...")]
    placeholder: &'static str,
    /// Disabled state.
    #[props(default = false)]
    disabled: bool,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
) -> Element {
    let has_value = !value.is_empty();
    let custom_class = class.unwrap_or_default();
    let input_class = format!("pl-8 pr-8 {custom_class}").trim().to_string();

    rsx! {
        div {
            class: "relative flex items-center",
            svg {
                class: "absolute left-2.5 w-3.5 h-3.5 text-muted-foreground pointer-events-none",
                fill: "none",
                view_box: "0 0 24 24",
                stroke: "currentColor",
                stroke_width: "2",
                path {
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    d: "M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                }
            }
            FormInput {
                value: value,
                on_input: on_input,
                input_type: "search",
                name: name,
                id: id,
                placeholder: Some(placeholder.to_string()),
                disabled: disabled,
                class: Some(input_class),
            }
            if has_value && on_clear.is_some() {
                button {
                    r#type: "button",
                    tabindex: "-1",
                    aria_label: "Clear search",
                    class: "absolute right-2.5 p-1 text-muted-foreground hover:text-foreground transition rounded",
                    onclick: move |_| {
                        if let Some(ref cb) = on_clear {
                            cb.call(());
                        }
                    },
                    svg {
                        class: "w-3.5 h-3.5",
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

// ─── FormHiddenInput ───────────────────────────────────────────────────────

/// Hidden form input for IDs, tokens, or fixed metadata.
#[component]
pub fn FormHiddenInput(
    /// Value.
    value: String,
    /// Field name.
    name: String,
    /// Input ID.
    #[props(default)]
    id: Option<String>,
) -> Element {
    let input_id = id.unwrap_or_else(|| name.clone());
    rsx! {
        input {
            r#type: "hidden",
            id: "{input_id}",
            name: "{name}",
            value: "{value}",
        }
    }
}

// ─── FormTextarea ──────────────────────────────────────────────────────────

/// Multi-line textarea component.
#[component]
pub fn FormTextarea(
    /// Textarea value.
    value: String,
    /// Callback on input.
    #[props(default)]
    on_input: Option<EventHandler<String>>,
    /// Callback on change.
    #[props(default)]
    on_change: Option<EventHandler<String>>,
    /// Callback on blur.
    #[props(default)]
    on_blur: Option<EventHandler<()>>,
    /// Field name. Defaults to the enclosing `FormField` name if omitted.
    #[props(default)]
    name: Option<String>,
    /// Textarea ID. Defaults to enclosing `FormField` ID if omitted.
    #[props(default)]
    id: Option<String>,
    /// Placeholder text.
    #[props(default)]
    placeholder: Option<String>,
    /// Number of rows. Defaults to 3.
    #[props(default = 3)]
    rows: u32,
    /// Number of columns.
    #[props(default)]
    cols: Option<u32>,
    /// Disabled state.
    #[props(default = false)]
    disabled: bool,
    /// Required attribute.
    #[props(default = false)]
    required: bool,
    /// Read-only attribute.
    #[props(default = false)]
    readonly: bool,
    /// Minimum text length.
    #[props(default)]
    minlength: Option<usize>,
    /// Maximum text length.
    #[props(default)]
    maxlength: Option<usize>,
    /// Explicit ARIA describedby override.
    #[props(default)]
    aria_describedby: Option<String>,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
) -> Element {
    let field = try_use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.map(|f| f.name.to_string()));
    let textarea_id = id.or_else(|| field.map(|f| f.id.to_string()).or_else(|| name_attr.clone()));

    let is_submitting = field.map(|f| f.is_submitting()).unwrap_or(false);
    let is_disabled = disabled || is_submitting;
    let is_invalid = field.map(|f| f.is_invalid()).unwrap_or(false);

    let aria_desc = if let Some(ref explicit) = aria_describedby {
        Some(explicit.clone())
    } else {
        textarea_id.as_ref().map(|n| {
            if is_invalid {
                format!("{n}-message")
            } else {
                format!("{n}-description")
            }
        })
    };

    let base = "w-full rounded-xl border bg-background px-3 py-2 text-xs text-foreground placeholder:text-muted-foreground/60 outline-none transition disabled:cursor-not-allowed disabled:opacity-50 resize-y";
    let state_border = if is_invalid {
        "border-destructive text-destructive focus:border-destructive focus:ring-1 focus:ring-destructive/30"
    } else {
        "border-border focus:border-primary focus:ring-1 focus:ring-primary/20"
    };
    let custom_class = class.unwrap_or_default();
    let textarea_class = format!("{base} {state_border} {custom_class}").trim().to_string();

    rsx! {
        textarea {
            id: textarea_id,
            name: name_attr,
            rows: "{rows}",
            cols: cols.map(|c| c.to_string()),
            placeholder: placeholder,
            value: "{value}",
            disabled: is_disabled,
            required: required,
            readonly: readonly,
            minlength: minlength.map(|l| l.to_string()),
            maxlength: maxlength.map(|l| l.to_string()),
            "aria-invalid": if is_invalid { "true" } else { "false" },
            aria_describedby: aria_desc,
            class: "{textarea_class}",
            oninput: move |ev| {
                if let Some(ref f) = field {
                    f.mark_dirty();
                }
                if let Some(ref cb) = on_input {
                    cb.call(ev.value());
                }
            },
            onchange: move |ev| {
                if let Some(ref cb) = on_change {
                    cb.call(ev.value());
                }
            },
            onblur: move |_| {
                if let Some(ref cb) = on_blur {
                    cb.call(());
                }
            }
        }
    }
}

// ─── FormSelect ────────────────────────────────────────────────────────────

/// Dropdown select component supporting standard and typed [`FieldOption`] options.
#[component]
pub fn FormSelect(
    /// Selected value.
    value: String,
    /// Options list.
    options: Vec<FieldOption>,
    /// Callback on selection change.
    #[props(default)]
    on_change: Option<EventHandler<String>>,
    /// Optional placeholder (empty selection item).
    #[props(default)]
    placeholder: Option<String>,
    /// Field name. Defaults to enclosing `FormField` name if omitted.
    #[props(default)]
    name: Option<String>,
    /// Select ID. Defaults to enclosing `FormField` ID if omitted.
    #[props(default)]
    id: Option<String>,
    /// Disabled state.
    #[props(default = false)]
    disabled: bool,
    /// Required attribute.
    #[props(default = false)]
    required: bool,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
) -> Element {
    let field = try_use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.map(|f| f.name.to_string()));
    let select_id = id.or_else(|| field.map(|f| f.id.to_string()).or_else(|| name_attr.clone()));

    let is_submitting = field.map(|f| f.is_submitting()).unwrap_or(false);
    let is_disabled = disabled || is_submitting;
    let is_invalid = field.map(|f| f.is_invalid()).unwrap_or(false);

    let aria_desc = select_id.as_ref().map(|n| {
        if is_invalid {
            format!("{n}-message")
        } else {
            format!("{n}-description")
        }
    });

    let base = "w-full rounded-xl border bg-background px-3 py-2 text-xs text-foreground outline-none transition disabled:cursor-not-allowed disabled:opacity-50 appearance-none";
    let state_border = if is_invalid {
        "border-destructive text-destructive focus:border-destructive focus:ring-1 focus:ring-destructive/30"
    } else {
        "border-border focus:border-primary focus:ring-1 focus:ring-primary/20"
    };
    let custom_class = class.unwrap_or_default();
    let select_class = format!("{base} {state_border} {custom_class}").trim().to_string();

    rsx! {
        div {
            class: "relative flex items-center",
            select {
                id: select_id,
                name: name_attr,
                value: "{value}",
                disabled: is_disabled,
                required: required,
                "aria-invalid": if is_invalid { "true" } else { "false" },
                aria_describedby: aria_desc,
                class: "{select_class}",
                onchange: move |ev| {
                    if let Some(ref f) = field {
                        f.mark_dirty();
                    }
                    if let Some(ref cb) = on_change {
                        cb.call(ev.value());
                    }
                },
                if let Some(ref ph) = placeholder {
                    option {
                        value: "",
                        disabled: true,
                        selected: value.is_empty(),
                        "{ph}"
                    }
                }
                for opt in options {
                    option {
                        value: "{opt.value}",
                        disabled: opt.disabled,
                        selected: opt.value == value,
                        "{opt.label}"
                    }
                }
            }
            svg {
                class: "absolute right-2.5 w-3.5 h-3.5 text-muted-foreground pointer-events-none",
                fill: "none",
                view_box: "0 0 24 24",
                stroke: "currentColor",
                stroke_width: "2",
                path {
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    d: "M19 9l-7 7-7-7"
                }
            }
        }
    }
}

// ─── FormCheckbox ──────────────────────────────────────────────────────────

/// Accessible boolean checkbox control.
#[component]
pub fn FormCheckbox(
    /// Checked state.
    checked: bool,
    /// Callback on change.
    #[props(default)]
    on_change: Option<EventHandler<bool>>,
    /// Optional label displayed beside the checkbox.
    #[props(default)]
    label: Option<String>,
    /// Optional descriptive text below the label.
    #[props(default)]
    description: Option<String>,
    /// Field name.
    #[props(default)]
    name: Option<String>,
    /// Input ID.
    #[props(default)]
    id: Option<String>,
    /// Disabled state.
    #[props(default = false)]
    disabled: bool,
    /// Required attribute.
    #[props(default = false)]
    required: bool,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
) -> Element {
    let field = try_use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.map(|f| f.name.to_string()));
    let checkbox_id = id.or_else(|| field.map(|f| f.id.to_string()).or_else(|| name_attr.clone()));

    let is_submitting = field.map(|f| f.is_submitting()).unwrap_or(false);
    let is_disabled = disabled || is_submitting;
    let is_invalid = field.map(|f| f.is_invalid()).unwrap_or(false);

    let border_color = if is_invalid {
        "border-destructive"
    } else {
        "border-border focus:ring-primary/20"
    };

    let custom_class = class.unwrap_or_default();

    rsx! {
        div {
            class: "flex items-start gap-2.5 {custom_class}",
            input {
                id: checkbox_id.clone(),
                r#type: "checkbox",
                name: name_attr,
                checked: checked,
                disabled: is_disabled,
                required: required,
                "aria-invalid": if is_invalid { "true" } else { "false" },
                class: "mt-0.5 h-4 w-4 rounded border {border_color} text-primary focus:ring-1 focus:ring-offset-0 disabled:opacity-50 transition cursor-pointer",
                onchange: move |_| {
                    if let Some(ref f) = field {
                        f.mark_dirty();
                    }
                    if let Some(ref cb) = on_change {
                        cb.call(!checked);
                    }
                }
            }
            if label.is_some() || description.is_some() {
                label {
                    r#for: checkbox_id,
                    class: "text-xs font-medium text-foreground cursor-pointer select-none leading-relaxed",
                    if let Some(ref l) = label {
                        span { "{l}" }
                    }
                    if let Some(ref d) = description {
                        p { class: "text-muted-foreground text-xs font-normal mt-0.5", "{d}" }
                    }
                }
            }
        }
    }
}

// ─── FormSwitch ────────────────────────────────────────────────────────────

/// iOS-style animated toggle switch with ARIA switch semantics.
#[component]
pub fn FormSwitch(
    /// Checked (active) state.
    checked: bool,
    /// Callback on toggle.
    #[props(default)]
    on_change: Option<EventHandler<bool>>,
    /// Optional label displayed beside switch.
    #[props(default)]
    label: Option<String>,
    /// Optional descriptive text below label.
    #[props(default)]
    description: Option<String>,
    /// Field name.
    #[props(default)]
    name: Option<String>,
    /// Switch ID.
    #[props(default)]
    id: Option<String>,
    /// Disabled state.
    #[props(default = false)]
    disabled: bool,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
) -> Element {
    let field = try_use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.map(|f| f.name.to_string()));
    let switch_id = id.or_else(|| field.map(|f| f.id.to_string()).or_else(|| name_attr.clone()));

    let is_submitting = field.map(|f| f.is_submitting()).unwrap_or(false);
    let is_disabled = disabled || is_submitting;

    let track_bg = if checked { "bg-primary" } else { "bg-muted" };
    let thumb_translate = if checked { "translate-x-4" } else { "translate-x-0" };
    let custom_class = class.unwrap_or_default();

    rsx! {
        div {
            class: "flex items-start gap-3 {custom_class}",
            button {
                id: switch_id.clone(),
                r#type: "button",
                role: "switch",
                "aria-checked": if checked { "true" } else { "false" },
                disabled: is_disabled,
                class: "relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent {track_bg} transition-colors duration-200 ease-in-out focus:outline-none focus:ring-1 focus:ring-primary/30 disabled:cursor-not-allowed disabled:opacity-50",
                onclick: move |_| {
                    if let Some(ref f) = field {
                        f.mark_dirty();
                    }
                    if let Some(ref cb) = on_change {
                        cb.call(!checked);
                    }
                },
                span {
                    class: "pointer-events-none inline-block h-4 w-4 transform rounded-full bg-background shadow-lg ring-0 transition duration-200 ease-in-out {thumb_translate}"
                }
            }
            if label.is_some() || description.is_some() {
                label {
                    r#for: switch_id,
                    class: "text-xs font-medium text-foreground cursor-pointer select-none leading-relaxed",
                    if let Some(ref l) = label {
                        span { "{l}" }
                    }
                    if let Some(ref d) = description {
                        p { class: "text-muted-foreground text-xs font-normal mt-0.5", "{d}" }
                    }
                }
            }
        }
    }
}

// ─── FormRadioGroup ────────────────────────────────────────────────────────

/// Radio button group with accessibility and keyboard navigation support.
#[component]
pub fn FormRadioGroup(
    /// Currently selected value.
    value: String,
    /// Options list.
    options: Vec<FieldOption>,
    /// Callback on selection change.
    #[props(default)]
    on_change: Option<EventHandler<String>>,
    /// Field name. Defaults to enclosing `FormField` name if omitted.
    #[props(default)]
    name: Option<String>,
    /// Layout orientation (horizontal if true, vertical if false). Defaults to false.
    #[props(default = false)]
    horizontal: bool,
    /// Disabled state.
    #[props(default = false)]
    disabled: bool,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
) -> Element {
    let field = try_use_context::<FieldContext>();

    let name_attr = name.unwrap_or_else(|| field.map(|f| f.name.to_string()).unwrap_or_else(|| "radio_group".to_string()));
    let is_submitting = field.map(|f| f.is_submitting()).unwrap_or(false);
    let is_disabled = disabled || is_submitting;

    let layout_class = if horizontal { "flex flex-wrap gap-4" } else { "space-y-2" };
    let custom_class = class.unwrap_or_default();

    rsx! {
        div {
            role: "radiogroup",
            class: "{layout_class} {custom_class}",
            for opt in options {
                let opt_val = opt.value.clone();
                let is_checked = opt_val == value;
                let opt_disabled = is_disabled || opt.disabled;
                let item_id = format!("{name_attr}-{opt_val}");

                div {
                    class: "flex items-center gap-2",
                    input {
                        id: "{item_id}",
                        r#type: "radio",
                        name: "{name_attr}",
                        value: "{opt_val}",
                        checked: is_checked,
                        disabled: opt_disabled,
                        class: "h-3.5 w-3.5 border-border text-primary focus:ring-primary/20 disabled:opacity-50 cursor-pointer",
                        onchange: move |_| {
                            if let Some(ref f) = field {
                                f.mark_dirty();
                            }
                            if let Some(ref cb) = on_change {
                                cb.call(opt_val.clone());
                            }
                        }
                    }
                    label {
                        r#for: "{item_id}",
                        class: "text-xs font-medium text-foreground cursor-pointer select-none",
                        "{opt.label}"
                    }
                }
            }
        }
    }
}

// ─── FormFileInput ─────────────────────────────────────────────────────────

/// Browser-safe file upload input.
#[component]
pub fn FormFileInput(
    /// Field name. Defaults to enclosing `FormField` name if omitted.
    #[props(default)]
    name: Option<String>,
    /// Input ID. Defaults to enclosing `FormField` ID if omitted.
    #[props(default)]
    id: Option<String>,
    /// File type filter (e.g. "image/*", ".pdf").
    #[props(default)]
    accept: Option<String>,
    /// Whether multiple files can be selected.
    #[props(default = false)]
    multiple: bool,
    /// Disabled state.
    #[props(default = false)]
    disabled: bool,
    /// Required attribute.
    #[props(default = false)]
    required: bool,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
) -> Element {
    let field = try_use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.map(|f| f.name.to_string()));
    let input_id = id.or_else(|| field.map(|f| f.id.to_string()).or_else(|| name_attr.clone()));

    let is_submitting = field.map(|f| f.is_submitting()).unwrap_or(false);
    let is_disabled = disabled || is_submitting;
    let is_invalid = field.map(|f| f.is_invalid()).unwrap_or(false);

    let aria_desc = input_id.as_ref().map(|n| {
        if is_invalid {
            format!("{n}-message")
        } else {
            format!("{n}-description")
        }
    });

    let base = "w-full rounded-xl border bg-background px-3 py-1.5 text-xs text-foreground file:mr-3 file:py-1 file:px-2.5 file:rounded-lg file:border-0 file:text-xs file:font-medium file:bg-primary/10 file:text-primary hover:file:bg-primary/20 outline-none transition disabled:cursor-not-allowed disabled:opacity-50";
    let state_border = if is_invalid {
        "border-destructive text-destructive"
    } else {
        "border-border focus:border-primary"
    };
    let custom_class = class.unwrap_or_default();
    let input_class = format!("{base} {state_border} {custom_class}").trim().to_string();

    rsx! {
        input {
            id: input_id,
            r#type: "file",
            name: name_attr,
            accept: accept,
            multiple: multiple,
            disabled: is_disabled,
            required: required,
            "aria-invalid": if is_invalid { "true" } else { "false" },
            aria_describedby: aria_desc,
            class: "{input_class}",
            onchange: move |_| {
                if let Some(ref f) = field {
                    f.mark_dirty();
                }
            }
        }
    }
}

// ─── FormColorInput ────────────────────────────────────────────────────────

/// Color picker input component with live hex preview.
#[component]
pub fn FormColorInput(
    /// Hex color value (e.g. "#3b82f6").
    value: String,
    /// Callback on color change.
    #[props(default)]
    on_change: Option<EventHandler<String>>,
    /// Field name. Defaults to enclosing `FormField` name if omitted.
    #[props(default)]
    name: Option<String>,
    /// Input ID. Defaults to enclosing `FormField` ID if omitted.
    #[props(default)]
    id: Option<String>,
    /// Label beside the swatch.
    #[props(default)]
    label: Option<String>,
    /// Disabled state.
    #[props(default = false)]
    disabled: bool,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
) -> Element {
    let field = try_use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.map(|f| f.name.to_string()));
    let input_id = id.or_else(|| field.map(|f| f.id.to_string()).or_else(|| name_attr.clone()));

    let is_submitting = field.map(|f| f.is_submitting()).unwrap_or(false);
    let is_disabled = disabled || is_submitting;
    let custom_class = class.unwrap_or_default();

    rsx! {
        div {
            class: "flex items-center gap-2.5 {custom_class}",
            input {
                id: input_id.clone(),
                r#type: "color",
                name: name_attr,
                value: "{value}",
                disabled: is_disabled,
                class: "h-8 w-12 rounded-lg border border-border bg-background p-0.5 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed",
                oninput: move |ev| {
                    if let Some(ref f) = field {
                        f.mark_dirty();
                    }
                    if let Some(ref cb) = on_change {
                        cb.call(ev.value());
                    }
                }
            }
            if let Some(ref l) = label {
                span { class: "text-xs font-medium text-foreground", "{l}" }
            }
            span {
                class: "font-mono text-xs text-muted-foreground",
                "{value}"
            }
        }
    }
}

// ─── FormRangeInput ────────────────────────────────────────────────────────

/// Range slider component with live numeric display badge.
#[component]
pub fn FormRangeInput(
    /// Current numeric value string.
    value: String,
    /// Callback on value change.
    #[props(default)]
    on_change: Option<EventHandler<String>>,
    /// Field name. Defaults to enclosing `FormField` name if omitted.
    #[props(default)]
    name: Option<String>,
    /// Slider ID. Defaults to enclosing `FormField` ID if omitted.
    #[props(default)]
    id: Option<String>,
    /// Minimum value. Defaults to 0.0.
    #[props(default = 0.0)]
    min: f64,
    /// Maximum value. Defaults to 100.0.
    #[props(default = 100.0)]
    max: f64,
    /// Step interval. Defaults to 1.0.
    #[props(default = 1.0)]
    step: f64,
    /// Show current value badge beside slider. Defaults to true.
    #[props(default = true)]
    show_value: bool,
    /// Disabled state.
    #[props(default = false)]
    disabled: bool,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
) -> Element {
    let field = try_use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.map(|f| f.name.to_string()));
    let input_id = id.or_else(|| field.map(|f| f.id.to_string()).or_else(|| name_attr.clone()));

    let is_submitting = field.map(|f| f.is_submitting()).unwrap_or(false);
    let is_disabled = disabled || is_submitting;
    let custom_class = class.unwrap_or_default();

    rsx! {
        div {
            class: "flex items-center gap-3 {custom_class}",
            input {
                id: input_id,
                r#type: "range",
                name: name_attr,
                min: "{min}",
                max: "{max}",
                step: "{step}",
                value: "{value}",
                disabled: is_disabled,
                class: "w-full h-1.5 bg-muted rounded-lg appearance-none cursor-pointer accent-primary disabled:opacity-50 disabled:cursor-not-allowed",
                oninput: move |ev| {
                    if let Some(ref f) = field {
                        f.mark_dirty();
                    }
                    if let Some(ref cb) = on_change {
                        cb.call(ev.value());
                    }
                }
            }
            if show_value {
                span {
                    class: "text-xs font-mono font-medium text-foreground min-w-[2.5rem] text-right px-1.5 py-0.5 rounded bg-muted/60",
                    "{value}"
                }
            }
        }
    }
}

// ─── FormOtpInput ──────────────────────────────────────────────────────────

/// Multi-character OTP / 2FA verification code input.
#[component]
pub fn FormOtpInput(
    /// Current code string.
    value: String,
    /// Expected code length (e.g. 4 or 6). Defaults to 6.
    #[props(default = 6)]
    length: usize,
    /// Callback on change.
    #[props(default)]
    on_change: Option<EventHandler<String>>,
    /// Callback fired when all digits have been entered.
    #[props(default)]
    on_complete: Option<EventHandler<String>>,
    /// Field name. Defaults to enclosing `FormField` name if omitted.
    #[props(default)]
    name: Option<String>,
    /// Input ID. Defaults to enclosing `FormField` ID if omitted.
    #[props(default)]
    id: Option<String>,
    /// Disabled state.
    #[props(default = false)]
    disabled: bool,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
) -> Element {
    let field = try_use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.map(|f| f.name.to_string()));
    let input_id = id.or_else(|| field.map(|f| f.id.to_string()).or_else(|| name_attr.clone()));

    let is_submitting = field.map(|f| f.is_submitting()).unwrap_or(false);
    let is_disabled = disabled || is_submitting;
    let is_invalid = field.map(|f| f.is_invalid()).unwrap_or(false);

    let state_border = if is_invalid {
        "border-destructive text-destructive"
    } else {
        "border-border focus-within:border-primary"
    };

    let chars: Vec<char> = value.chars().collect();
    let custom_class = class.unwrap_or_default();

    rsx! {
        div {
            class: "flex items-center gap-2 {custom_class}",
            for i in 0..length {
                let char_val = chars.get(i).copied().unwrap_or(' ');
                let display_str = if char_val != ' ' { char_val.to_string() } else { String::new() };
                let cell_id = input_id.as_ref().map(|n| format!("{n}-digit-{i}"));

                input {
                    id: cell_id,
                    r#type: "text",
                    maxlength: "1",
                    inputmode: "numeric",
                    pattern: "[0-9]*",
                    disabled: is_disabled,
                    value: "{display_str}",
                    class: "w-10 h-11 text-center font-mono text-base font-semibold rounded-xl border bg-background {state_border} outline-none transition disabled:opacity-50",
                    oninput: move |ev| {
                        let entered = ev.value();
                        let mut new_code = value.clone();
                        if let Some(ch) = entered.chars().last() {
                            if i < new_code.len() {
                                new_code.replace_range(i..=i, &ch.to_string());
                            } else {
                                new_code.push(ch);
                            }
                        }
                        if let Some(ref f) = field {
                            f.mark_dirty();
                        }
                        if let Some(ref cb) = on_change {
                            cb.call(new_code.clone());
                        }
                        if new_code.len() == length {
                            if let Some(ref cb_comp) = on_complete {
                                cb_comp.call(new_code);
                            }
                        }
                    }
                }
            }
        }
    }
}
