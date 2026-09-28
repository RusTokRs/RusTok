//! Interactive form control components styled with RusToK design tokens.
//!
//! All controls automatically detect when rendered inside a [`FormField`](crate::FormField)
//! via [`FieldContext`](crate::context::FieldContext), inheriting the field name,
//! accessibility attributes (`aria-invalid`, `aria-describedby`), and error state styling (`border-destructive`).

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
    /// Input type (e.g. "text", "email", "password", "number", "search", "url"). Defaults to "text".
    #[prop(optional)]
    input_type: Option<&'static str>,
    /// Field name. Defaults to the enclosing `FormField` name if omitted.
    #[prop(optional, into)]
    name: Option<String>,
    /// Input ID. Defaults to `name` if omitted.
    #[prop(optional, into)]
    id: Option<String>,
    /// Placeholder text.
    #[prop(optional, into)]
    placeholder: Option<String>,
    /// Disabled state. If omitted, disables automatically during form submission.
    #[prop(optional, into)]
    disabled: Option<Signal<bool>>,
    /// Read-only state.
    #[prop(optional)]
    readonly: bool,
    /// Autocomplete attribute.
    #[prop(optional, into)]
    autocomplete: Option<String>,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let field = use_context::<FieldContext>();

    let t = input_type.unwrap_or("text");
    let name_attr = name.or_else(|| field.as_ref().map(|f| f.name.clone()));
    let input_id = id.or_else(|| name_attr.clone());

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

    let field_for_invalid = field;
    let is_invalid = Signal::derive(move || {
        field_for_invalid.as_ref().map(|f| f.is_invalid()).unwrap_or(false)
    });

    let name_for_desc = name_attr.clone();
    let aria_describedby = move || {
        name_for_desc.as_ref().map(|n| {
            if is_invalid.get() {
                format!("{n}-message")
            } else {
                format!("{n}-description")
            }
        })
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

    let on_input_handler = move |ev| {
        if let Some(cb) = on_input {
            cb.run(event_target_value(&ev));
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
            disabled=move || is_disabled.get()
            readonly=readonly
            autocomplete=autocomplete
            aria-invalid=move || if is_invalid.get() { "true" } else { "false" }
            aria-describedby=aria_describedby
            class=input_class
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
    /// Field name. Defaults to the enclosing `FormField` name if omitted.
    #[prop(optional, into)]
    name: Option<String>,
    /// Textarea ID. Defaults to `name` if omitted.
    #[prop(optional, into)]
    id: Option<String>,
    /// Placeholder text.
    #[prop(optional, into)]
    placeholder: Option<String>,
    /// Number of rows. Defaults to 3.
    #[prop(optional)]
    rows: Option<u32>,
    /// Disabled state.
    #[prop(optional, into)]
    disabled: Option<Signal<bool>>,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let field = use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.as_ref().map(|f| f.name.clone()));
    let textarea_id = id.or_else(|| name_attr.clone());
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

    let field_for_invalid = field;
    let is_invalid = Signal::derive(move || {
        field_for_invalid.as_ref().map(|f| f.is_invalid()).unwrap_or(false)
    });

    let name_for_desc = name_attr.clone();
    let aria_describedby = move || {
        name_for_desc.as_ref().map(|n| {
            if is_invalid.get() {
                format!("{n}-message")
            } else {
                format!("{n}-description")
            }
        })
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

    let on_input_handler = move |ev| {
        if let Some(cb) = on_input {
            cb.run(event_target_value(&ev));
        }
    };

    view! {
        <textarea
            id=textarea_id
            name=name_attr
            rows=r
            placeholder=placeholder
            prop:value=move || value.get()
            on:input=on_input_handler
            disabled=move || is_disabled.get()
            aria-invalid=move || if is_invalid.get() { "true" } else { "false" }
            aria-describedby=aria_describedby
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
    /// Field name. Defaults to the enclosing `FormField` name if omitted.
    #[prop(optional, into)]
    name: Option<String>,
    /// Select ID. Defaults to `name` if omitted.
    #[prop(optional, into)]
    id: Option<String>,
    /// Optional placeholder / empty label (e.g. "Select an option...").
    #[prop(optional, into)]
    placeholder: Option<String>,
    /// Disabled state.
    #[prop(optional, into)]
    disabled: Option<Signal<bool>>,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let field = use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.as_ref().map(|f| f.name.clone()));
    let select_id = id.or_else(|| name_attr.clone());

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

    let field_for_invalid = field;
    let is_invalid = Signal::derive(move || {
        field_for_invalid.as_ref().map(|f| f.is_invalid()).unwrap_or(false)
    });

    let name_for_desc = name_attr.clone();
    let aria_describedby = move || {
        name_for_desc.as_ref().map(|n| {
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

    let on_change_handler = move |ev: Event| {
        if let Some(cb) = on_change {
            cb.run(event_target_value(&ev));
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
            disabled=move || is_disabled.get()
            aria-invalid=move || if is_invalid.get() { "true" } else { "false" }
            aria-describedby=aria_describedby
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
    /// Field name. Defaults to the enclosing `FormField` name if omitted.
    #[prop(optional, into)]
    name: Option<String>,
    /// Input ID. Defaults to `name` if omitted.
    #[prop(optional, into)]
    id: Option<String>,
    /// Label text rendered next to checkbox.
    #[prop(optional, into)]
    label: Option<String>,
    /// Disabled state.
    #[prop(optional, into)]
    disabled: Option<Signal<bool>>,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let field = use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.as_ref().map(|f| f.name.clone()));
    let input_id = id.or_else(|| name_attr.clone());

    let field_for_disabled = field;
    let is_disabled = Signal::derive(move || {
        if let Some(sig) = disabled {
            sig.get()
        } else if let Some(ref f) = field_for_disabled {
            f.is_submitting()
        } else {
            false
        }
    });

    let on_change_handler = move |ev: Event| {
        let is_chk = event_target_checked(&ev);
        if let Some(cb) = on_change {
            cb.run(is_chk);
        }
    };

    view! {
        <label class=format!("inline-flex items-center gap-2 cursor-pointer select-none text-xs text-foreground {class}")>
            <input
                id=input_id
                type="checkbox"
                name=name_attr
                checked=move || checked.get()
                prop:checked=move || checked.get()
                on:change=on_change_handler
                disabled=move || is_disabled.get()
                class="w-4 h-4 rounded border-border text-primary focus:ring-primary/20 accent-primary cursor-pointer disabled:cursor-not-allowed disabled:opacity-50"
            />
            {label.map(|lbl| view! { <span>{lbl}</span> })}
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
    /// Switch button ID. Defaults to `name` if omitted.
    #[prop(optional, into)]
    id: Option<String>,
    /// Label text rendered next to the switch.
    #[prop(optional, into)]
    label: Option<String>,
    /// Disabled state.
    #[prop(optional, into)]
    disabled: Option<Signal<bool>>,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let field = use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.as_ref().map(|f| f.name.clone()));
    let switch_id = id.or_else(|| name_attr.clone());

    let field_for_disabled = field;
    let is_disabled = Signal::derive(move || {
        if let Some(sig) = disabled {
            sig.get()
        } else if let Some(ref f) = field_for_disabled {
            f.is_submitting()
        } else {
            false
        }
    });

    let on_toggle = move |_| {
        if !is_disabled.get() {
            let next = !checked.get();
            if let Some(cb) = on_change {
                cb.run(next);
            }
        }
    };

    let on_label_toggle = on_toggle;

    view! {
        <div class=format!("inline-flex items-center gap-2.5 {class}")>
            <button
                id=switch_id
                type="button"
                role="switch"
                name=name_attr
                aria-checked=move || if checked.get() { "true" } else { "false" }
                disabled=move || is_disabled.get()
                on:click=on_toggle
                class=move || {
                    let base = "relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-50";
                    if checked.get() {
                        format!("{base} bg-primary")
                    } else {
                        format!("{base} bg-muted")
                    }
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
            {label.map(|lbl| view! {
                <span class="text-xs text-foreground cursor-pointer select-none" on:click=on_label_toggle>
                    {lbl}
                </span>
            })}
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
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let field = use_context::<FieldContext>();

    let group_name = name
        .or_else(|| field.as_ref().map(|f| f.name.clone()))
        .unwrap_or_else(|| "radio_group".to_string());

    let field_for_disabled = field;
    let is_disabled = Signal::derive(move || {
        if let Some(sig) = disabled {
            sig.get()
        } else if let Some(ref f) = field_for_disabled {
            f.is_submitting()
        } else {
            false
        }
    });

    let layout_class = if horizontal {
        "flex flex-wrap items-center gap-4"
    } else {
        "flex flex-col gap-2"
    };

    view! {
        <div class=format!("{layout_class} {class}")>
            {options.into_iter().map(|opt| {
                let opt_val = opt.value.clone();
                let opt_val_for_click = opt.value.clone();
                let is_checked = {
                    let opt_v = opt_val.clone();
                    let val = value.clone();
                    Signal::derive(move || val.get() == opt_v)
                };
                let opt_disabled = opt.disabled;

                let on_click = move |_| {
                    if !opt_disabled && !is_disabled.get() {
                        if let Some(cb) = on_change {
                            cb.run(opt_val_for_click.clone());
                        }
                    }
                };

                view! {
                    <label class="inline-flex items-center gap-2 cursor-pointer select-none text-xs text-foreground">
                        <input
                            type="radio"
                            name=group_name.clone()
                            value=opt_val
                            checked=move || is_checked.get()
                            prop:checked=move || is_checked.get()
                            on:change=on_click
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
    /// Accepted file types/extensions (e.g. `image/*`, `.pdf,.docx`).
    #[prop(optional, into)]
    accept: Option<String>,
    /// Whether multiple files can be selected.
    #[prop(optional)]
    multiple: bool,
    /// Field name. Defaults to the enclosing `FormField` name if omitted.
    #[prop(optional, into)]
    name: Option<String>,
    /// Input ID. Defaults to `name` if omitted.
    #[prop(optional, into)]
    id: Option<String>,
    /// Disabled state.
    #[prop(optional, into)]
    disabled: Option<Signal<bool>>,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let field = use_context::<FieldContext>();

    let name_attr = name.or_else(|| field.as_ref().map(|f| f.name.clone()));
    let input_id = id.or_else(|| name_attr.clone());

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

    let field_for_invalid = field;
    let is_invalid = Signal::derive(move || {
        field_for_invalid.as_ref().map(|f| f.is_invalid()).unwrap_or(false)
    });

    let name_for_desc = name_attr.clone();
    let aria_describedby = move || {
        name_for_desc.as_ref().map(|n| {
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

    let on_change_handler = move |ev: Event| {
        if let Some(cb) = on_change {
            cb.run(ev);
        }
    };

    view! {
        <input
            id=input_id
            type="file"
            name=name_attr
            accept=accept
            multiple=multiple
            disabled=move || is_disabled.get()
            on:change=on_change_handler
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
        let form_ctx = FormContext { state: state_signal };

        let val = Signal::derive(|| "Initial".to_string());
        let html = view! {
            <div>
                {provide_context(form_ctx)}
                <FormInput
                    value=val
                    name="username"
                    placeholder="Enter username"
                    class="custom-input"
                />
            </div>
        }
        .to_html();

        assert!(html.contains("id=\"username\""));
        assert!(html.contains("name=\"username\""));
        assert!(html.contains("placeholder=\"Enter username\""));
        assert!(html.contains("aria-invalid=\"false\""));
        assert!(html.contains("aria-describedby=\"username-description\""));
        assert!(html.contains("custom-input"));
    }

    #[test]
    fn test_form_select_renders_selected_option_in_ssr() {
        let state = FormState::idle();
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext { state: state_signal };

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
        // Check that Russian option has selected attribute in SSR
        assert!(html.contains("value=\"ru\" selected"));
        assert!(html.contains("Русский"));
    }

    #[test]
    fn test_form_checkbox_and_switch_render_checked() {
        let state = FormState::idle();
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext { state: state_signal };

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
        let form_ctx = FormContext { state: state_signal };

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

        assert!(html.contains("name=\"theme\""));
        assert!(html.contains("value=\"dark\" checked"));
        assert!(html.contains("Dark"));
    }

    #[test]
    fn test_form_file_input_renders_safely() {
        let state = FormState::idle();
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext { state: state_signal };

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
