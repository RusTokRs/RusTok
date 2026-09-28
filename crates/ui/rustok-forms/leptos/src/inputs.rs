//! Interactive form control components styled with RusToK design tokens.
//!
//! All controls automatically detect when rendered inside a [`FormField`](crate::FormField)
//! via [`FieldContext`](crate::context::FieldContext), inheriting the field name,
//! accessibility attributes (`aria-invalid`), and error state styling (`border-destructive`).

use leptos::ev::Event;
use leptos::prelude::*;
use rustok_forms::FieldOption;

use crate::context::FieldContext;

// ─── FormInput ─────────────────────────────────────────────────────────────

/// Text-like input component (text, email, password, number, search, url).
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
    let name_attr = field.as_ref().map(|f| f.name.clone());

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
            type=t
            name=name_attr
            placeholder=placeholder
            prop:value=move || value.get()
            on:input=on_input_handler
            disabled=move || is_disabled.get()
            readonly=readonly
            autocomplete=autocomplete
            aria-invalid=move || if is_invalid.get() { "true" } else { "false" }
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

    let name_attr = field.as_ref().map(|f| f.name.clone());
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
            name=name_attr
            rows=r
            placeholder=placeholder
            prop:value=move || value.get()
            on:input=on_input_handler
            disabled=move || is_disabled.get()
            aria-invalid=move || if is_invalid.get() { "true" } else { "false" }
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

    let name_attr = field.as_ref().map(|f| f.name.clone());

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

    view! {
        <select
            name=name_attr
            prop:value=move || value.get()
            on:change=on_change_handler
            disabled=move || is_disabled.get()
            aria-invalid=move || if is_invalid.get() { "true" } else { "false" }
            class=select_class
        >
            {placeholder.map(|ph| view! {
                <option value="">{ph}</option>
            })}
            {options.into_iter().map(|opt| {
                view! {
                    <option value=opt.value.clone() disabled=opt.disabled>
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

    let name_attr = field.as_ref().map(|f| f.name.clone());

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
                type="checkbox"
                name=name_attr
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

    let name_attr = field.as_ref().map(|f| f.name.clone());

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

    let group_name = field.as_ref().map(|f| f.name.clone()).unwrap_or_else(|| "radio_group".to_string());

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
                    Signal::derive(move || value.get() == opt_v)
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
