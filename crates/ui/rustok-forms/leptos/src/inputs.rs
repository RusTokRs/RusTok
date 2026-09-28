//! Interactive form control components styled with RusToK design tokens.
//!
//! All controls automatically detect when rendered inside a [`FormField`](crate::FormField)
//! via [`FieldContext`](crate::context::FieldContext), inheriting the field name,
//! accessibility attributes (`aria-invalid`, `aria-describedby`, `aria-errormessage`),
//! error state styling (`border-destructive`), and dirty tracking integration.

use leptos::ev::Event;
use leptos::prelude::*;
use rustok_forms::FieldOption;

use crate::context::FieldContext;

// ─── FormInput ─────────────────────────────────────────────────────────────

/// Text-like input component (text, email, password, number, search, url, date, etc.).
#[component]
pub fn FormInput(
    /// Value signal.
    #[prop(into)]
    value: Signal<String>,
    /// Callback on input.
    #[prop(optional, into)]
    on_input: Option<Callback<String>>,
    /// Callback on change.
    #[prop(optional, into)]
    on_change: Option<Callback<String>>,
    /// Callback on blur.
    #[prop(optional, into)]
    on_blur: Option<Callback<()>>,
    /// Callback on focus.
    #[prop(optional, into)]
    on_focus: Option<Callback<()>>,
    /// Input type (e.g. "text", "email", "password", "number", "search", "url"). Defaults to "text".
    #[prop(optional)]
    input_type: Option<&'static str>,
    /// Field name. Defaults to the enclosing `FormField` name if omitted.
    #[prop(optional, into)]
    name: Option<String>,
    /// Input ID. Defaults to enclosing `FormField` ID if omitted.
    #[prop(optional, into)]
    id: Option<String>,
    /// Placeholder text.
    #[prop(optional, into)]
    placeholder: Option<String>,
    /// Disabled state. If omitted, disables automatically during form submission.
    #[prop(optional, into)]
    disabled: Option<Signal<bool>>,
    /// Required attribute.
    #[prop(optional)]
    required: bool,
    /// Read-only state.
    #[prop(optional)]
    readonly: bool,
    /// Autofocus state.
    #[prop(optional)]
    autofocus: bool,
    /// Minimum value (for numbers, dates).
    #[prop(optional, into)]
    min: Option<String>,
    /// Maximum value (for numbers, dates).
    #[prop(optional, into)]
    max: Option<String>,
    /// Step value (for numbers).
    #[prop(optional, into)]
    step: Option<String>,
    /// Minimum text length.
    #[prop(optional)]
    minlength: Option<usize>,
    /// Maximum text length.
    #[prop(optional)]
    maxlength: Option<usize>,
    /// Regex pattern for client-side hint.
    #[prop(optional, into)]
    pattern: Option<String>,
    /// Autocomplete attribute.
    #[prop(optional, into)]
    autocomplete: Option<String>,
    /// Explicit ARIA label.
    #[prop(optional, into)]
    aria_label: Option<String>,
    /// Explicit ARIA describedby override.
    #[prop(optional, into)]
    aria_describedby: Option<String>,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let field = use_context::<FieldContext>();

    let t = input_type.unwrap_or("text");
    let name_attr = name.or_else(|| field.as_ref().map(|f| f.name.clone()));
    let input_id = id.or_else(|| field.as_ref().map(|f| f.id.clone()).or_else(|| name_attr.clone()));

    let field_for_disabled = field.clone();
    let is_disabled = Signal::derive(move || {
        if let Some(sig) = disabled {
            sig.get()
        } else if let Some(ref f) = field_for_disabled {
            f.is_submitting()
        } else {
            false
        }
    });

    let field_for_invalid = field.clone();
    let is_invalid = Signal::derive(move || {
        field_for_invalid.as_ref().map(|f| f.is_invalid()).unwrap_or(false)
    });

    let id_for_desc = input_id.clone();
    let aria_describedby_signal = move || {
        if let Some(ref explicit) = aria_describedby {
            return Some(explicit.clone());
        }
        id_for_desc.as_ref().map(|n| {
            if is_invalid.get() {
                format!("{n}-message")
            } else {
                format!("{n}-description")
            }
        })
    };

    let id_for_err = input_id.clone();
    let aria_errormessage_signal = move || {
        if is_invalid.get() {
            id_for_err.as_ref().map(|n| format!("{n}-message"))
        } else {
            None
        }
    };

    let input_class = move || {
        let base = "w-full rounded-xl border bg-background px-3 py-2 text-xs text-foreground placeholder:text-muted-foreground/60 outline-none transition disabled:cursor-not-allowed disabled:opacity-50";
        let state_border = if is_invalid.get() {
            "border-destructive text-destructive focus:border-destructive focus:ring-1 focus:ring-destructive/30"
        } else {
            "border-border focus:border-primary focus:ring-1 focus:ring-primary/20"
        };
        if class.is_empty() {
            format!("{base} {state_border}")
        } else {
            format!("{base} {state_border} {class}")
        }
    };

    let field_for_dirty = field;
    let on_input_handler = move |ev| {
        let val = event_target_value(&ev);
        if let Some(ref f) = field_for_dirty {
            f.mark_dirty();
        }
        if let Some(cb) = on_input {
            cb.run(val);
        }
    };

    let on_change_handler = move |ev: Event| {
        if let Some(cb) = on_change {
            cb.run(event_target_value(&ev));
        }
    };

    let on_blur_handler = move |_| {
        if let Some(cb) = on_blur {
            cb.run(());
        }
    };

    let on_focus_handler = move |_| {
        if let Some(cb) = on_focus {
            cb.run(());
        }
    };

    view! {
        <input
            id=input_id
            type=t
            name=name_attr
            placeholder=placeholder
            prop:value=move || value.get()
            on:input=on_input_handler
            on:change=on_change_handler
            on:blur=on_blur_handler
            on:focus=on_focus_handler
            disabled=move || is_disabled.get()
            required=required
            readonly=readonly
            autofocus=autofocus
            min=min
            max=max
            step=step
            minlength=minlength
            maxlength=maxlength
            pattern=pattern
            autocomplete=autocomplete
            aria-label=aria_label
            aria-invalid=move || if is_invalid.get() { "true" } else { "false" }
            aria-describedby=aria_describedby_signal
            aria-errormessage=aria_errormessage_signal
            class=input_class
        />
    }
}

// ─── FormPasswordInput ─────────────────────────────────────────────────────

/// Specialized password input with show/hide password toggle.
#[component]
pub fn FormPasswordInput(
    /// Value signal.
    #[prop(into)]
    value: Signal<String>,
    /// Callback on input.
    #[prop(optional, into)]
    on_input: Option<Callback<String>>,
    /// Callback on change.
    #[prop(optional, into)]
    on_change: Option<Callback<String>>,
    /// Callback on blur.
    #[prop(optional, into)]
    on_blur: Option<Callback<()>>,
    /// Field name. Defaults to the enclosing `FormField` name if omitted.
    #[prop(optional, into)]
    name: Option<String>,
    /// Input ID. Defaults to enclosing `FormField` ID if omitted.
    #[prop(optional, into)]
    id: Option<String>,
    /// Placeholder text.
    #[prop(optional, into)]
    placeholder: Option<String>,
    /// Disabled state.
    #[prop(optional, into)]
    disabled: Option<Signal<bool>>,
    /// Required attribute.
    #[prop(optional)]
    required: bool,
    /// Read-only state.
    #[prop(optional)]
    readonly: bool,
    /// Autocomplete attribute (e.g. "current-password", "new-password").
    #[prop(optional, into)]
    autocomplete: Option<String>,
    /// Extra CSS classes on the input container.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let field = use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.as_ref().map(|f| f.name.clone()));
    let input_id = id.or_else(|| field.as_ref().map(|f| f.id.clone()).or_else(|| name_attr.clone()));

    let show_password = RwSignal::new(false);

    let field_for_disabled = field.clone();
    let is_disabled = Signal::derive(move || {
        if let Some(sig) = disabled {
            sig.get()
        } else if let Some(ref f) = field_for_disabled {
            f.is_submitting()
        } else {
            false
        }
    });

    let field_for_invalid = field.clone();
    let is_invalid = Signal::derive(move || {
        field_for_invalid.as_ref().map(|f| f.is_invalid()).unwrap_or(false)
    });

    let id_for_desc = input_id.clone();
    let aria_describedby_signal = move || {
        id_for_desc.as_ref().map(|n| {
            if is_invalid.get() {
                format!("{n}-message")
            } else {
                format!("{n}-description")
            }
        })
    };

    let field_for_dirty = field;
    let on_input_handler = move |ev| {
        let val = event_target_value(&ev);
        if let Some(ref f) = field_for_dirty {
            f.mark_dirty();
        }
        if let Some(cb) = on_input {
            cb.run(val);
        }
    };

    let on_change_handler = move |ev: Event| {
        if let Some(cb) = on_change {
            cb.run(event_target_value(&ev));
        }
    };

    let on_blur_handler = move |_| {
        if let Some(cb) = on_blur {
            cb.run(());
        }
    };

    let toggle_visibility = move |_| {
        show_password.update(|v| *v = !*v);
    };

    let input_class = move || {
        let base = "w-full rounded-xl border bg-background pl-3 pr-10 py-2 text-xs text-foreground placeholder:text-muted-foreground/60 outline-none transition disabled:cursor-not-allowed disabled:opacity-50";
        let state_border = if is_invalid.get() {
            "border-destructive text-destructive focus:border-destructive focus:ring-1 focus:ring-destructive/30"
        } else {
            "border-border focus:border-primary focus:ring-1 focus:ring-primary/20"
        };
        if class.is_empty() {
            format!("{base} {state_border}")
        } else {
            format!("{base} {state_border} {class}")
        }
    };

    view! {
        <div class="relative flex items-center">
            <input
                id=input_id
                type=move || if show_password.get() { "text" } else { "password" }
                name=name_attr
                placeholder=placeholder
                prop:value=move || value.get()
                on:input=on_input_handler
                on:change=on_change_handler
                on:blur=on_blur_handler
                disabled=move || is_disabled.get()
                required=required
                readonly=readonly
                autocomplete=autocomplete
                aria-invalid=move || if is_invalid.get() { "true" } else { "false" }
                aria-describedby=aria_describedby_signal
                class=input_class
            />
            <button
                type="button"
                tabindex="-1"
                on:click=toggle_visibility
                aria-label=move || if show_password.get() { "Hide password" } else { "Show password" }
                class="absolute right-2.5 p-1 text-muted-foreground hover:text-foreground transition rounded outline-none"
            >
                {move || if show_password.get() {
                    view! {
                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l18 18" />
                        </svg>
                    }.into_any()
                } else {
                    view! {
                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                            <path stroke-linecap="round" stroke-linejoin="round" d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
                        </svg>
                    }.into_any()
                }}
            </button>
        </div>
    }
}

// ─── FormNumberInput ───────────────────────────────────────────────────────

/// Dedicated number input component with numeric constraints.
#[component]
pub fn FormNumberInput(
    /// Numeric string value signal.
    #[prop(into)]
    value: Signal<String>,
    /// Callback on input.
    #[prop(optional, into)]
    on_input: Option<Callback<String>>,
    /// Callback on change.
    #[prop(optional, into)]
    on_change: Option<Callback<String>>,
    /// Callback on blur.
    #[prop(optional, into)]
    on_blur: Option<Callback<()>>,
    /// Field name.
    #[prop(optional, into)]
    name: Option<String>,
    /// Input ID.
    #[prop(optional, into)]
    id: Option<String>,
    /// Placeholder.
    #[prop(optional, into)]
    placeholder: Option<String>,
    /// Minimum value constraint.
    #[prop(optional)]
    min: Option<f64>,
    /// Maximum value constraint.
    #[prop(optional)]
    max: Option<f64>,
    /// Step interval.
    #[prop(optional)]
    step: Option<f64>,
    /// Disabled state.
    #[prop(optional, into)]
    disabled: Option<Signal<bool>>,
    /// Required attribute.
    #[prop(optional)]
    required: bool,
    /// Read-only state.
    #[prop(optional)]
    readonly: bool,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let min_str = min.map(|v| v.to_string());
    let max_str = max.map(|v| v.to_string());
    let step_str = step.map(|v| v.to_string());

    view! {
        <FormInput
            value=value
            on_input=on_input
            on_change=on_change
            on_blur=on_blur
            input_type="number"
            name=name
            id=id
            placeholder=placeholder
            min=min_str
            max=max_str
            step=step_str
            disabled=disabled
            required=required
            readonly=readonly
            class=class
        />
    }
}

// ─── FormSearchInput ───────────────────────────────────────────────────────

/// Specialized search input with search icon and optional clear button.
#[component]
pub fn FormSearchInput(
    /// Value signal.
    #[prop(into)]
    value: Signal<String>,
    /// Callback on input.
    #[prop(optional, into)]
    on_input: Option<Callback<String>>,
    /// Callback on clear button click.
    #[prop(optional, into)]
    on_clear: Option<Callback<()>>,
    /// Field name.
    #[prop(optional, into)]
    name: Option<String>,
    /// Input ID.
    #[prop(optional, into)]
    id: Option<String>,
    /// Placeholder text. Defaults to "Search...".
    #[prop(optional, into)]
    placeholder: Option<String>,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let ph = placeholder.unwrap_or_else(|| "Search...".to_string());

    let val = value.clone();
    let has_value = Signal::derive(move || !val.get().is_empty());

    let on_clear_click = move |_| {
        if let Some(cb) = on_clear {
            cb.run(());
        }
    };

    view! {
        <div class="relative flex items-center">
            <span class="absolute left-3 text-muted-foreground pointer-events-none">
                <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                </svg>
            </span>
            <FormInput
                value=value
                on_input=on_input
                input_type="search"
                name=name
                id=id
                placeholder=ph
                class=format!("pl-8 pr-8 {class}")
            />
            {move || has_value.get().then(|| view! {
                <button
                    type="button"
                    on:click=on_clear_click
                    aria-label="Clear search"
                    class="absolute right-2.5 p-1 text-muted-foreground hover:text-foreground transition rounded"
                >
                    <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                </button>
            })}
        </div>
    }
}

// ─── FormHiddenInput ───────────────────────────────────────────────────────

/// Hidden form input for IDs, tokens, or fixed metadata.
#[component]
pub fn FormHiddenInput(
    /// Value signal.
    #[prop(into)]
    value: Signal<String>,
    /// Field name.
    #[prop(into)]
    name: String,
    /// Input ID.
    #[prop(optional, into)]
    id: Option<String>,
) -> impl IntoView {
    let input_id = id.unwrap_or_else(|| name.clone());
    view! {
        <input
            type="hidden"
            id=input_id
            name=name
            prop:value=move || value.get()
        />
    }
}

// ─── FormTextarea ──────────────────────────────────────────────────────────

/// Multi-line textarea component.
#[component]
pub fn FormTextarea(
    /// Value signal.
    #[prop(into)]
    value: Signal<String>,
    /// Callback on input.
    #[prop(optional, into)]
    on_input: Option<Callback<String>>,
    /// Callback on change.
    #[prop(optional, into)]
    on_change: Option<Callback<String>>,
    /// Callback on blur.
    #[prop(optional, into)]
    on_blur: Option<Callback<()>>,
    /// Field name. Defaults to the enclosing `FormField` name if omitted.
    #[prop(optional, into)]
    name: Option<String>,
    /// Textarea ID. Defaults to enclosing `FormField` ID if omitted.
    #[prop(optional, into)]
    id: Option<String>,
    /// Placeholder text.
    #[prop(optional, into)]
    placeholder: Option<String>,
    /// Number of rows. Defaults to 3.
    #[prop(optional)]
    rows: Option<u32>,
    /// Number of columns.
    #[prop(optional)]
    cols: Option<u32>,
    /// Disabled state.
    #[prop(optional, into)]
    disabled: Option<Signal<bool>>,
    /// Required attribute.
    #[prop(optional)]
    required: bool,
    /// Read-only attribute.
    #[prop(optional)]
    readonly: bool,
    /// Autofocus attribute.
    #[prop(optional)]
    autofocus: bool,
    /// Minimum text length.
    #[prop(optional)]
    minlength: Option<usize>,
    /// Maximum text length.
    #[prop(optional)]
    maxlength: Option<usize>,
    /// Explicit ARIA describedby override.
    #[prop(optional, into)]
    aria_describedby: Option<String>,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let field = use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.as_ref().map(|f| f.name.clone()));
    let textarea_id = id.or_else(|| field.as_ref().map(|f| f.id.clone()).or_else(|| name_attr.clone()));
    let r = rows.unwrap_or(3);

    let field_for_disabled = field.clone();
    let is_disabled = Signal::derive(move || {
        if let Some(sig) = disabled {
            sig.get()
        } else if let Some(ref f) = field_for_disabled {
            f.is_submitting()
        } else {
            false
        }
    });

    let field_for_invalid = field.clone();
    let is_invalid = Signal::derive(move || {
        field_for_invalid.as_ref().map(|f| f.is_invalid()).unwrap_or(false)
    });

    let id_for_desc = textarea_id.clone();
    let aria_describedby_signal = move || {
        if let Some(ref explicit) = aria_describedby {
            return Some(explicit.clone());
        }
        id_for_desc.as_ref().map(|n| {
            if is_invalid.get() {
                format!("{n}-message")
            } else {
                format!("{n}-description")
            }
        })
    };

    let id_for_err = textarea_id.clone();
    let aria_errormessage_signal = move || {
        if is_invalid.get() {
            id_for_err.as_ref().map(|n| format!("{n}-message"))
        } else {
            None
        }
    };

    let textarea_class = move || {
        let base = "w-full rounded-xl border bg-background px-3 py-2 text-xs text-foreground placeholder:text-muted-foreground/60 outline-none transition resize-y disabled:cursor-not-allowed disabled:opacity-50";
        let state_border = if is_invalid.get() {
            "border-destructive text-destructive focus:border-destructive focus:ring-1 focus:ring-destructive/30"
        } else {
            "border-border focus:border-primary focus:ring-1 focus:ring-primary/20"
        };
        if class.is_empty() {
            format!("{base} {state_border}")
        } else {
            format!("{base} {state_border} {class}")
        }
    };

    let field_for_dirty = field;
    let on_input_handler = move |ev| {
        let val = event_target_value(&ev);
        if let Some(ref f) = field_for_dirty {
            f.mark_dirty();
        }
        if let Some(cb) = on_input {
            cb.run(val);
        }
    };

    let on_change_handler = move |ev: Event| {
        if let Some(cb) = on_change {
            cb.run(event_target_value(&ev));
        }
    };

    let on_blur_handler = move |_| {
        if let Some(cb) = on_blur {
            cb.run(());
        }
    };

    view! {
        <textarea
            id=textarea_id
            name=name_attr
            rows=r
            cols=cols
            placeholder=placeholder
            prop:value=move || value.get()
            on:input=on_input_handler
            on:change=on_change_handler
            on:blur=on_blur_handler
            disabled=move || is_disabled.get()
            required=required
            readonly=readonly
            autofocus=autofocus
            minlength=minlength
            maxlength=maxlength
            aria-invalid=move || if is_invalid.get() { "true" } else { "false" }
            aria-describedby=aria_describedby_signal
            aria-errormessage=aria_errormessage_signal
            class=textarea_class
        />
    }
}

// ─── FormSelect ────────────────────────────────────────────────────────────

/// Dropdown select component.
#[component]
pub fn FormSelect(
    /// Current selected value signal.
    #[prop(into)]
    value: Signal<String>,
    /// Options list.
    options: Vec<FieldOption>,
    /// Callback on value change.
    #[prop(optional, into)]
    on_change: Option<Callback<String>>,
    /// Callback on blur.
    #[prop(optional, into)]
    on_blur: Option<Callback<()>>,
    /// Field name. Defaults to the enclosing `FormField` name if omitted.
    #[prop(optional, into)]
    name: Option<String>,
    /// Select ID. Defaults to enclosing `FormField` ID if omitted.
    #[prop(optional, into)]
    id: Option<String>,
    /// Optional placeholder / empty label (e.g. "Select an option...").
    #[prop(optional, into)]
    placeholder: Option<String>,
    /// Disabled state.
    #[prop(optional, into)]
    disabled: Option<Signal<bool>>,
    /// Required attribute.
    #[prop(optional)]
    required: bool,
    /// Multiple selection support.
    #[prop(optional)]
    multiple: bool,
    /// Explicit ARIA describedby override.
    #[prop(optional, into)]
    aria_describedby: Option<String>,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let field = use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.as_ref().map(|f| f.name.clone()));
    let select_id = id.or_else(|| field.as_ref().map(|f| f.id.clone()).or_else(|| name_attr.clone()));

    let field_for_disabled = field.clone();
    let is_disabled = Signal::derive(move || {
        if let Some(sig) = disabled {
            sig.get()
        } else if let Some(ref f) = field_for_disabled {
            f.is_submitting()
        } else {
            false
        }
    });

    let field_for_invalid = field.clone();
    let is_invalid = Signal::derive(move || {
        field_for_invalid.as_ref().map(|f| f.is_invalid()).unwrap_or(false)
    });

    let id_for_desc = select_id.clone();
    let aria_describedby_signal = move || {
        if let Some(ref explicit) = aria_describedby {
            return Some(explicit.clone());
        }
        id_for_desc.as_ref().map(|n| {
            if is_invalid.get() {
                format!("{n}-message")
            } else {
                format!("{n}-description")
            }
        })
    };

    let select_class = move || {
        let base = "w-full rounded-xl border bg-background px-3 py-2 text-xs text-foreground outline-none transition disabled:cursor-not-allowed disabled:opacity-50";
        let state_border = if is_invalid.get() {
            "border-destructive text-destructive focus:border-destructive"
        } else {
            "border-border focus:border-primary focus:ring-1 focus:ring-primary/20"
        };
        if class.is_empty() {
            format!("{base} {state_border}")
        } else {
            format!("{base} {state_border} {class}")
        }
    };

    let field_for_dirty = field;
    let on_change_handler = move |ev: Event| {
        let val = event_target_value(&ev);
        if let Some(ref f) = field_for_dirty {
            f.mark_dirty();
        }
        if let Some(cb) = on_change {
            cb.run(val);
        }
    };

    let on_blur_handler = move |_| {
        if let Some(cb) = on_blur {
            cb.run(());
        }
    };

    let is_placeholder_selected = {
        let val = value.clone();
        Signal::derive(move || val.get().is_empty())
    };

    view! {
        <select
            id=select_id
            name=name_attr
            prop:value=move || value.get()
            on:change=on_change_handler
            on:blur=on_blur_handler
            disabled=move || is_disabled.get()
            required=required
            multiple=multiple
            aria-invalid=move || if is_invalid.get() { "true" } else { "false" }
            aria-describedby=aria_describedby_signal
            class=select_class
        >
            {placeholder.map(|ph| view! {
                <option value="" selected=move || is_placeholder_selected.get()>{ph}</option>
            })}
            {options.into_iter().map(|opt| {
                let opt_val = opt.value.clone();
                let is_selected = {
                    let opt_v = opt_val.clone();
                    let val = value.clone();
                    Signal::derive(move || val.get() == opt_v)
                };
                view! {
                    <option
                        value=opt_val
                        selected=move || is_selected.get()
                        disabled=opt.disabled
                    >
                        {opt.label}
                    </option>
                }
            }).collect_view()}
        </select>
    }
}

// ─── FormCheckbox ──────────────────────────────────────────────────────────

/// Styled boolean checkbox.
#[component]
pub fn FormCheckbox(
    /// Checked signal.
    #[prop(into)]
    checked: Signal<bool>,
    /// Callback on toggle.
    #[prop(optional, into)]
    on_change: Option<Callback<bool>>,
    /// Callback on blur.
    #[prop(optional, into)]
    on_blur: Option<Callback<()>>,
    /// Field name. Defaults to the enclosing `FormField` name if omitted.
    #[prop(optional, into)]
    name: Option<String>,
    /// Input ID. Defaults to enclosing `FormField` ID if omitted.
    #[prop(optional, into)]
    id: Option<String>,
    /// Label text rendered next to checkbox.
    #[prop(optional, into)]
    label: Option<String>,
    /// Subdued description text below the label.
    #[prop(optional, into)]
    description: Option<String>,
    /// Disabled state.
    #[prop(optional, into)]
    disabled: Option<Signal<bool>>,
    /// Required attribute.
    #[prop(optional)]
    required: bool,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let field = use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.as_ref().map(|f| f.name.clone()));
    let input_id = id.or_else(|| field.as_ref().map(|f| f.id.clone()).or_else(|| name_attr.clone()));

    let field_for_disabled = field.clone();
    let is_disabled = Signal::derive(move || {
        if let Some(sig) = disabled {
            sig.get()
        } else if let Some(ref f) = field_for_disabled {
            f.is_submitting()
        } else {
            false
        }
    });

    let field_for_invalid = field.clone();
    let is_invalid = Signal::derive(move || {
        field_for_invalid.as_ref().map(|f| f.is_invalid()).unwrap_or(false)
    });

    let id_for_desc = input_id.clone();
    let aria_describedby_signal = move || {
        id_for_desc.as_ref().map(|n| {
            if is_invalid.get() {
                format!("{n}-message")
            } else {
                format!("{n}-description")
            }
        })
    };

    let field_for_dirty = field;
    let on_change_handler = move |ev: Event| {
        let is_chk = event_target_checked(&ev);
        if let Some(ref f) = field_for_dirty {
            f.mark_dirty();
        }
        if let Some(cb) = on_change {
            cb.run(is_chk);
        }
    };

    let on_blur_handler = move |_| {
        if let Some(cb) = on_blur {
            cb.run(());
        }
    };

    let checkbox_class = move || {
        let base = "w-4 h-4 rounded text-primary focus:ring-primary/20 accent-primary cursor-pointer disabled:cursor-not-allowed disabled:opacity-50 transition";
        let state_border = if is_invalid.get() {
            "border-destructive ring-1 ring-destructive/30"
        } else {
            "border-border"
        };
        format!("{base} {state_border}")
    };

    view! {
        <label class=format!("inline-flex items-start gap-2.5 cursor-pointer select-none text-xs text-foreground {class}")>
            <input
                id=input_id
                type="checkbox"
                name=name_attr
                checked=move || checked.get()
                prop:checked=move || checked.get()
                on:change=on_change_handler
                on:blur=on_blur_handler
                disabled=move || is_disabled.get()
                required=required
                aria-invalid=move || if is_invalid.get() { "true" } else { "false" }
                aria-describedby=aria_describedby_signal
                class=checkbox_class
            />
            <div class="flex flex-col">
                {label.map(|lbl| view! { <span class="font-medium leading-none">{lbl}</span> })}
                {description.map(|desc| view! { <span class="text-muted-foreground text-[11px] mt-0.5">{desc}</span> })}
            </div>
        </label>
    }
}

// ─── FormSwitch ────────────────────────────────────────────────────────────

/// Modern iOS-style toggle switch.
#[component]
pub fn FormSwitch(
    /// Checked signal.
    #[prop(into)]
    checked: Signal<bool>,
    /// Callback on toggle.
    #[prop(optional, into)]
    on_change: Option<Callback<bool>>,
    /// Field name. Defaults to the enclosing `FormField` name if omitted.
    #[prop(optional, into)]
    name: Option<String>,
    /// Switch button ID. Defaults to enclosing `FormField` ID if omitted.
    #[prop(optional, into)]
    id: Option<String>,
    /// Label text rendered next to the switch.
    #[prop(optional, into)]
    label: Option<String>,
    /// Subdued description text below the label.
    #[prop(optional, into)]
    description: Option<String>,
    /// Disabled state.
    #[prop(optional, into)]
    disabled: Option<Signal<bool>>,
    /// Required attribute.
    #[prop(optional)]
    required: bool,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let field = use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.as_ref().map(|f| f.name.clone()));
    let switch_id = id.or_else(|| field.as_ref().map(|f| f.id.clone()).or_else(|| name_attr.clone()));

    let field_for_disabled = field.clone();
    let is_disabled = Signal::derive(move || {
        if let Some(sig) = disabled {
            sig.get()
        } else if let Some(ref f) = field_for_disabled {
            f.is_submitting()
        } else {
            false
        }
    });

    let field_for_invalid = field.clone();
    let is_invalid = Signal::derive(move || {
        field_for_invalid.as_ref().map(|f| f.is_invalid()).unwrap_or(false)
    });

    let id_for_desc = switch_id.clone();
    let aria_describedby_signal = move || {
        id_for_desc.as_ref().map(|n| {
            if is_invalid.get() {
                format!("{n}-message")
            } else {
                format!("{n}-description")
            }
        })
    };

    let field_for_dirty = field;
    let on_toggle = move |_| {
        if !is_disabled.get() {
            let next = !checked.get();
            if let Some(ref f) = field_for_dirty {
                f.mark_dirty();
            }
            if let Some(cb) = on_change {
                cb.run(next);
            }
        }
    };

    let on_label_toggle = on_toggle;

    view! {
        <div class=format!("inline-flex items-start gap-2.5 {class}")>
            <button
                id=switch_id
                type="button"
                role="switch"
                name=name_attr
                aria-checked=move || if checked.get() { "true" } else { "false" }
                aria-required=if required { "true" } else { "false" }
                aria-invalid=move || if is_invalid.get() { "true" } else { "false" }
                aria-describedby=aria_describedby_signal
                disabled=move || is_disabled.get()
                on:click=on_toggle
                class=move || {
                    let base = "relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-50";
                    let state_color = if checked.get() {
                        "bg-primary"
                    } else if is_invalid.get() {
                        "bg-destructive/30"
                    } else {
                        "bg-muted"
                    };
                    format!("{base} {state_color}")
                }
            >
                <span
                    class=move || {
                        let base = "pointer-events-none inline-block h-3.5 w-3.5 transform rounded-full bg-background shadow-sm ring-0 transition duration-200 ease-in-out";
                        if checked.get() {
                            format!("{base} translate-x-4")
                        } else {
                            format!("{base} translate-x-0.5")
                        }
                    }
                />
            </button>
            <div class="flex flex-col cursor-pointer select-none" on:click=on_label_toggle>
                {label.map(|lbl| view! { <span class="text-xs font-medium text-foreground leading-none">{lbl}</span> })}
                {description.map(|desc| view! { <span class="text-muted-foreground text-[11px] mt-0.5">{desc}</span> })}
            </div>
        </div>
    }
}

// ─── FormRadioGroup ────────────────────────────────────────────────────────

/// Radio group for choosing one option out of multiple choices.
#[component]
pub fn FormRadioGroup(
    /// Current selected value signal.
    #[prop(into)]
    value: Signal<String>,
    /// Radio options.
    options: Vec<FieldOption>,
    /// Callback on change.
    #[prop(optional, into)]
    on_change: Option<Callback<String>>,
    /// Radio group name. Defaults to the enclosing `FormField` name if omitted.
    #[prop(optional, into)]
    name: Option<String>,
    /// Layout orientation: horizontal or vertical. Defaults to vertical.
    #[prop(optional)]
    horizontal: bool,
    /// Disabled state.
    #[prop(optional, into)]
    disabled: Option<Signal<bool>>,
    /// Required attribute.
    #[prop(optional)]
    required: bool,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let field = use_context::<FieldContext>();

    let group_name = name
        .or_else(|| field.as_ref().map(|f| f.name.clone()))
        .unwrap_or_else(|| "radio_group".to_string());

    let field_for_disabled = field.clone();
    let is_disabled = Signal::derive(move || {
        if let Some(sig) = disabled {
            sig.get()
        } else if let Some(ref f) = field_for_disabled {
            f.is_submitting()
        } else {
            false
        }
    });

    let field_for_invalid = field.clone();
    let is_invalid = Signal::derive(move || {
        field_for_invalid.as_ref().map(|f| f.is_invalid()).unwrap_or(false)
    });

    let group_name_for_desc = group_name.clone();
    let aria_describedby_signal = move || {
        if is_invalid.get() {
            Some(format!("{group_name_for_desc}-message"))
        } else {
            Some(format!("{group_name_for_desc}-description"))
        }
    };

    let layout_class = if horizontal {
        "flex flex-wrap items-center gap-4"
    } else {
        "flex flex-col gap-2"
    };

    let field_for_dirty = field;

    view! {
        <div
            role="radiogroup"
            aria-invalid=move || if is_invalid.get() { "true" } else { "false" }
            aria-describedby=aria_describedby_signal
            class=format!("{layout_class} {class}")
        >
            {options.into_iter().map(|opt| {
                let opt_val = opt.value.clone();
                let opt_val_for_click = opt.value.clone();
                let opt_id = format!("{}-{}", group_name, opt.value);
                let is_checked = {
                    let opt_v = opt_val.clone();
                    let val = value.clone();
                    Signal::derive(move || val.get() == opt_v)
                };
                let opt_disabled = opt.disabled;

                let field_dirty = field_for_dirty.clone();
                let on_click = move |_| {
                    if !opt_disabled && !is_disabled.get() {
                        if let Some(ref f) = field_dirty {
                            f.mark_dirty();
                        }
                        if let Some(cb) = on_change {
                            cb.run(opt_val_for_click.clone());
                        }
                    }
                };

                view! {
                    <label
                        for=opt_id.clone()
                        class="inline-flex items-center gap-2 cursor-pointer select-none text-xs text-foreground"
                    >
                        <input
                            id=opt_id
                            type="radio"
                            name=group_name.clone()
                            value=opt_val
                            checked=move || is_checked.get()
                            prop:checked=move || is_checked.get()
                            on:change=on_click
                            required=required
                            disabled=move || is_disabled.get() || opt_disabled
                            class="w-4 h-4 border-border text-primary focus:ring-primary/20 accent-primary cursor-pointer disabled:cursor-not-allowed disabled:opacity-50"
                        />
                        <span>{opt.label}</span>
                    </label>
                }
            }).collect_view()}
        </div>
    }
}

// ─── FormFileInput ─────────────────────────────────────────────────────────

/// Safe file input component.
///
/// Unlike standard inputs, `FormFileInput` does not bind `prop:value` which would
/// throw an `InvalidStateError` in browsers for security reasons.
#[component]
pub fn FormFileInput(
    /// Callback executed when files are selected.
    #[prop(optional, into)]
    on_change: Option<Callback<Event>>,
    /// Callback on blur.
    #[prop(optional, into)]
    on_blur: Option<Callback<()>>,
    /// Accepted file types/extensions (e.g. `image/*`, `.pdf,.docx`).
    #[prop(optional, into)]
    accept: Option<String>,
    /// Whether multiple files can be selected.
    #[prop(optional)]
    multiple: bool,
    /// Field name. Defaults to the enclosing `FormField` name if omitted.
    #[prop(optional, into)]
    name: Option<String>,
    /// Input ID. Defaults to enclosing `FormField` ID if omitted.
    #[prop(optional, into)]
    id: Option<String>,
    /// Disabled state.
    #[prop(optional, into)]
    disabled: Option<Signal<bool>>,
    /// Required attribute.
    #[prop(optional)]
    required: bool,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let field = use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.as_ref().map(|f| f.name.clone()));
    let input_id = id.or_else(|| field.as_ref().map(|f| f.id.clone()).or_else(|| name_attr.clone()));

    let field_for_disabled = field.clone();
    let is_disabled = Signal::derive(move || {
        if let Some(sig) = disabled {
            sig.get()
        } else if let Some(ref f) = field_for_disabled {
            f.is_submitting()
        } else {
            false
        }
    });

    let field_for_invalid = field.clone();
    let is_invalid = Signal::derive(move || {
        field_for_invalid.as_ref().map(|f| f.is_invalid()).unwrap_or(false)
    });

    let id_for_desc = input_id.clone();
    let aria_describedby = move || {
        id_for_desc.as_ref().map(|n| {
            if is_invalid.get() {
                format!("{n}-message")
            } else {
                format!("{n}-description")
            }
        })
    };

    let file_class = move || {
        let base = "w-full text-xs text-foreground file:mr-3 file:py-1.5 file:px-3 file:rounded-lg file:border-0 file:text-xs file:font-medium file:bg-primary/10 file:text-primary hover:file:bg-primary/20 file:cursor-pointer cursor-pointer border rounded-xl px-3 py-1.5 transition disabled:cursor-not-allowed disabled:opacity-50";
        let state_border = if is_invalid.get() {
            "border-destructive text-destructive"
        } else {
            "border-border"
        };
        if class.is_empty() {
            format!("{base} {state_border}")
        } else {
            format!("{base} {state_border} {class}")
        }
    };

    let field_for_dirty = field;
    let on_change_handler = move |ev: Event| {
        if let Some(ref f) = field_for_dirty {
            f.mark_dirty();
        }
        if let Some(cb) = on_change {
            cb.run(ev);
        }
    };

    let on_blur_handler = move |_| {
        if let Some(cb) = on_blur {
            cb.run(());
        }
    };

    view! {
        <input
            id=input_id
            type="file"
            name=name_attr
            accept=accept
            multiple=multiple
            required=required
            disabled=move || is_disabled.get()
            on:change=on_change_handler
            on:blur=on_blur_handler
            aria-invalid=move || if is_invalid.get() { "true" } else { "false" }
            aria-describedby=aria_describedby
            class=file_class
        />
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::FormContext;
    use rustok_forms::FormState;

    #[test]
    fn test_form_input_renders_attributes_and_aria() {
        let state = FormState::idle();
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext::new(state_signal);

        let val = Signal::derive(|| "Initial".to_string());
        let html = view! {
            <div>
                {provide_context(form_ctx)}
                <FormInput
                    value=val
                    name="username"
                    placeholder="Enter username"
                    class="custom-input"
                    required=true
                />
            </div>
        }
        .to_html();

        assert!(html.contains("id=\"username\""));
        assert!(html.contains("name=\"username\""));
        assert!(html.contains("placeholder=\"Enter username\""));
        assert!(html.contains("aria-invalid=\"false\""));
        assert!(html.contains("aria-describedby=\"username-description\""));
        assert!(html.contains("required"));
        assert!(html.contains("custom-input"));
    }

    #[test]
    fn test_form_password_input_renders() {
        let state = FormState::idle();
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext::new(state_signal);

        let val = Signal::derive(|| "secret".to_string());
        let html = view! {
            <div>
                {provide_context(form_ctx)}
                <FormPasswordInput
                    value=val
                    name="password"
                    placeholder="Enter password"
                />
            </div>
        }
        .to_html();

        assert!(html.contains("type=\"password\""));
        assert!(html.contains("id=\"password\""));
        assert!(html.contains("aria-label=\"Show password\""));
    }

    #[test]
    fn test_form_number_and_search_inputs() {
        let state = FormState::idle();
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext::new(state_signal);

        let num_val = Signal::derive(|| "42".to_string());
        let q_val = Signal::derive(|| "query".to_string());

        let html = view! {
            <div>
                {provide_context(form_ctx)}
                <FormNumberInput
                    value=num_val
                    name="quantity"
                    min=1.0
                    max=100.0
                    step=1.0
                />
                <FormSearchInput
                    value=q_val
                    name="q"
                />
            </div>
        }
        .to_html();

        assert!(html.contains("type=\"number\""));
        assert!(html.contains("min=\"1\""));
        assert!(html.contains("max=\"100\""));
        assert!(html.contains("type=\"search\""));
        assert!(html.contains("name=\"q\""));
    }

    #[test]
    fn test_form_select_renders_selected_option_in_ssr() {
        let state = FormState::idle();
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext::new(state_signal);

        let val = Signal::derive(|| "ru".to_string());
        let opts = vec![
            FieldOption::new("en", "English"),
            FieldOption::new("ru", "Русский"),
        ];

        let html = view! {
            <div>
                {provide_context(form_ctx)}
                <FormSelect
                    value=val
                    name="locale"
                    options=opts
                    placeholder="Choose language"
                />
            </div>
        }
        .to_html();

        assert!(html.contains("id=\"locale\""));
        assert!(html.contains("name=\"locale\""));
        assert!(html.contains("value=\"ru\""));
        assert!(html.contains("value=\"ru\" selected"));
        assert!(html.contains("Русский"));
    }

    #[test]
    fn test_form_checkbox_and_switch_render_checked() {
        let state = FormState::idle();
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext::new(state_signal);

        let checked = Signal::derive(|| true);
        let html = view! {
            <div>
                {provide_context(form_ctx)}
                <FormCheckbox
                    checked=checked
                    name="agree"
                    label="I agree to terms"
                />
                <FormSwitch
                    checked=checked
                    name="notifications"
                    label="Enable notifications"
                />
            </div>
        }
        .to_html();

        assert!(html.contains("type=\"checkbox\""));
        assert!(html.contains("name=\"agree\""));
        assert!(html.contains("checked"));
        assert!(html.contains("I agree to terms"));

        assert!(html.contains("role=\"switch\""));
        assert!(html.contains("aria-checked=\"true\""));
        assert!(html.contains("Enable notifications"));
    }

    #[test]
    fn test_form_radio_group_renders_active_item() {
        let state = FormState::idle();
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext::new(state_signal);

        let val = Signal::derive(|| "dark".to_string());
        let opts = vec![
            FieldOption::new("light", "Light"),
            FieldOption::new("dark", "Dark"),
        ];

        let html = view! {
            <div>
                {provide_context(form_ctx)}
                <FormRadioGroup
                    value=val
                    name="theme"
                    options=opts
                />
            </div>
        }
        .to_html();

        assert!(html.contains("role=\"radiogroup\""));
        assert!(html.contains("name=\"theme\""));
        assert!(html.contains("id=\"theme-dark\""));
        assert!(html.contains("value=\"dark\" checked"));
        assert!(html.contains("Dark"));
    }

    #[test]
    fn test_form_file_input_renders_safely() {
        let state = FormState::idle();
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext::new(state_signal);

        let html = view! {
            <div>
                {provide_context(form_ctx)}
                <FormFileInput
                    name="avatar"
                    accept="image/*"
                    multiple=true
                />
            </div>
        }
        .to_html();

        assert!(html.contains("type=\"file\""));
        assert!(html.contains("name=\"avatar\""));
        assert!(html.contains("accept=\"image/*\""));
        assert!(html.contains("multiple"));
    }
}
