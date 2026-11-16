//! `<DynamicForm>` — Schema-driven reactive form renderer for Leptos 0.8.
//!
//! Automatically maps a [`FormSchema`] and its [`FieldDescriptor`] definitions to
//! typed RusToK form controls (`FormInput`, `FormPasswordInput`, `FormNumberInput`,
//! `FormTextarea`, `FormSelect`, `FormCheckbox`, `FormSwitch`, `FormRadioGroup`, etc.),
//! handling validation, accessible labels, helper texts, and submission coordination.

use std::collections::HashMap;

use leptos::prelude::*;
use rustok_ui_forms::field::{FieldDescriptor, FieldKind};
use rustok_ui_forms::schema::FormSchema;
use rustok_ui_forms::{DirtyTracker, FormState};

use crate::button::{ResetButton, SubmitButton};
use crate::context::FormContext;
use crate::field::{FormDescription, FormError, FormField, FormLabel, FormMessage};
use crate::form::Form;
use crate::inputs::{
    FormCheckbox, FormColorInput, FormFileInput, FormHiddenInput, FormInput, FormNumberInput,
    FormPasswordInput, FormRadioGroup, FormRangeInput, FormSearchInput, FormSelect, FormSwitch,
    FormTextarea,
};

/// Individual dynamic field renderer mapping a [`FieldDescriptor`] to the appropriate input control.
#[component]
fn DynamicFieldItem(
    field: FieldDescriptor,
    values: RwSignal<HashMap<String, String>>,
) -> impl IntoView {
    let name = field.name.clone();
    let name_for_val = name.clone();
    let value_sig = Signal::derive(move || {
        values.with(|m| m.get(&name_for_val).cloned().unwrap_or_default())
    });

    let name_for_input = name.clone();
    let on_input_cb = Callback::new(move |val: String| {
        values.update(|m| {
            m.insert(name_for_input.clone(), val);
        });
    });

    let name_for_change = name.clone();
    let on_change_cb = Callback::new(move |val: String| {
        values.update(|m| {
            m.insert(name_for_change.clone(), val);
        });
    });

    let form_ctx = use_context::<FormContext>();
    let field_disabled = field.disabled;
    let disabled_sig = Signal::derive(move || {
        if field_disabled {
            true
        } else if let Some(ref ctx) = form_ctx {
            ctx.state.get().is_submitting
        } else {
            false
        }
    });

    let required = field.constraints.required;
    let placeholder = field.placeholder.clone().unwrap_or_default();

    let control = match &field.kind {
        FieldKind::Password => view! {
            <FormPasswordInput
                value=value_sig
                on_input=on_input_cb
                placeholder=placeholder
                disabled=disabled_sig
                required=required
            />
        }
        .into_any(),

        FieldKind::Number => view! {
            <FormNumberInput
                value=value_sig
                on_input=on_input_cb
                placeholder=placeholder
                min=field.constraints.min
                max=field.constraints.max
                step=field.constraints.step
                disabled=disabled_sig
                required=required
            />
        }
        .into_any(),

        FieldKind::Search => view! {
            <FormSearchInput
                value=value_sig
                on_input=on_input_cb
                placeholder=placeholder
            />
        }
        .into_any(),

        FieldKind::Textarea | FieldKind::RichText => view! {
            <FormTextarea
                value=value_sig
                on_input=on_input_cb
                placeholder=placeholder
                disabled=disabled_sig
                required=required
            />
        }
        .into_any(),

        FieldKind::Select => view! {
            <FormSelect
                value=value_sig
                options=field.options.clone()
                on_change=on_change_cb
                placeholder=placeholder
                disabled=disabled_sig
                required=required
                multiple=field.constraints.multiple
            />
        }
        .into_any(),

        FieldKind::Checkbox => {
            let name_bool = name.clone();
            let bool_sig = Signal::derive(move || {
                values.with(|m| {
                    m.get(&name_bool)
                        .map(|s| s == "true" || s == "1")
                        .unwrap_or(false)
                })
            });
            let name_bool_change = name.clone();
            let on_bool_change = Callback::new(move |checked: bool| {
                values.update(|m| {
                    m.insert(name_bool_change.clone(), checked.to_string());
                });
            });

            match (field.label.clone(), field.description.clone()) {
                (Some(lbl), Some(desc)) => view! {
                    <FormCheckbox
                        checked=bool_sig
                        on_change=on_bool_change
                        label=lbl
                        description=desc
                        disabled=disabled_sig
                        required=required
                    />
                }
                .into_any(),
                (Some(lbl), None) => view! {
                    <FormCheckbox
                        checked=bool_sig
                        on_change=on_bool_change
                        label=lbl
                        disabled=disabled_sig
                        required=required
                    />
                }
                .into_any(),
                (None, Some(desc)) => view! {
                    <FormCheckbox
                        checked=bool_sig
                        on_change=on_bool_change
                        description=desc
                        disabled=disabled_sig
                        required=required
                    />
                }
                .into_any(),
                (None, None) => view! {
                    <FormCheckbox
                        checked=bool_sig
                        on_change=on_bool_change
                        disabled=disabled_sig
                        required=required
                    />
                }
                .into_any(),
            }
        }

        FieldKind::Switch => {
            let name_bool = name.clone();
            let bool_sig = Signal::derive(move || {
                values.with(|m| {
                    m.get(&name_bool)
                        .map(|s| s == "true" || s == "1")
                        .unwrap_or(false)
                })
            });
            let name_bool_change = name.clone();
            let on_bool_change = Callback::new(move |checked: bool| {
                values.update(|m| {
                    m.insert(name_bool_change.clone(), checked.to_string());
                });
            });

            match (field.label.clone(), field.description.clone()) {
                (Some(lbl), Some(desc)) => view! {
                    <FormSwitch
                        checked=bool_sig
                        on_change=on_bool_change
                        label=lbl
                        description=desc
                        disabled=disabled_sig
                        required=required
                    />
                }
                .into_any(),
                (Some(lbl), None) => view! {
                    <FormSwitch
                        checked=bool_sig
                        on_change=on_bool_change
                        label=lbl
                        disabled=disabled_sig
                        required=required
                    />
                }
                .into_any(),
                (None, Some(desc)) => view! {
                    <FormSwitch
                        checked=bool_sig
                        on_change=on_bool_change
                        description=desc
                        disabled=disabled_sig
                        required=required
                    />
                }
                .into_any(),
                (None, None) => view! {
                    <FormSwitch
                        checked=bool_sig
                        on_change=on_bool_change
                        disabled=disabled_sig
                        required=required
                    />
                }
                .into_any(),
            }
        }

        FieldKind::Radio => view! {
            <FormRadioGroup
                value=value_sig
                options=field.options.clone()
                on_change=on_change_cb
                disabled=disabled_sig
                required=required
            />
        }
        .into_any(),

        FieldKind::Color => {
            if let Some(lbl) = field.label.clone() {
                view! {
                    <FormColorInput
                        value=value_sig
                        on_change=on_change_cb
                        label=lbl
                        disabled=disabled_sig
                    />
                }
                .into_any()
            } else {
                view! {
                    <FormColorInput
                        value=value_sig
                        on_change=on_change_cb
                        disabled=disabled_sig
                    />
                }
                .into_any()
            }
        }

        FieldKind::Range => {
            let min = field.constraints.min.unwrap_or(0.0);
            let max = field.constraints.max.unwrap_or(100.0);
            let step = field.constraints.step.unwrap_or(1.0);
            view! {
                <FormRangeInput
                    value=value_sig
                    on_input=on_input_cb
                    min=min
                    max=max
                    step=step
                    disabled=disabled_sig
                />
            }
            .into_any()
        }

        FieldKind::Hidden => view! {
            <FormHiddenInput
                value=value_sig
                name=name.clone()
            />
        }
        .into_any(),

        FieldKind::File => {
            let accept = field.constraints.accept.clone().unwrap_or_default();
            view! {
                <FormFileInput
                    accept=accept
                    multiple=field.constraints.multiple
                    disabled=disabled_sig
                    required=required
                />
            }
            .into_any()
        }

        _ => {
            let input_type = field.kind.html_input_type();
            view! {
                <FormInput
                    input_type=input_type
                    value=value_sig
                    on_input=on_input_cb
                    placeholder=placeholder
                    disabled=disabled_sig
                    required=required
                />
            }
            .into_any()
        }
    };

    if matches!(field.kind, FieldKind::Hidden) {
        return control;
    }

    if matches!(field.kind, FieldKind::Checkbox | FieldKind::Switch) {
        return view! {
            <FormField name=name>
                {control}
                <FormMessage />
            </FormField>
        }
        .into_any();
    }

    let field_label_view = field.label.clone().map(|label| {
        view! {
            <FormLabel required=required>{label}</FormLabel>
        }
    });

    let field_desc_view = field.description.clone().map(|desc| {
        view! {
            <FormDescription>{desc}</FormDescription>
        }
    });

    view! {
        <FormField name=name>
            {field_label_view}
            {control}
            {field_desc_view}
            <FormMessage />
        </FormField>
    }
    .into_any()
}

/// Dynamic form component that renders an entire interactive form from a [`FormSchema`].
///
/// Features:
/// - Generates typed inputs for all [`FieldKind`] variants.
/// - Validates payloads against [`FormSchema::validate_map`] on submission before calling `on_submit`.
/// - Displays field errors and form errors automatically.
/// - Supports controlled and uncontrolled values/state.
/// - Auto-disables inputs and buttons during asynchronous submission.
/// - Supports reset button with automatic restore of default schema values.
#[component]
pub fn DynamicForm(
    /// Declarative schema defining the form fields, labels, constraints, and kinds.
    schema: FormSchema,
    /// Submission handler receiving validated form data (key-value map).
    #[prop(into)]
    on_submit: Callback<HashMap<String, String>>,
    /// Optional externally controlled values map. If omitted, internal reactive state is maintained.
    #[prop(optional)]
    values: Option<RwSignal<HashMap<String, String>>>,
    /// Optional externally controlled form state. If omitted, internal state is maintained.
    #[prop(optional)]
    state: Option<RwSignal<FormState>>,
    /// Optional dirty field tracker.
    #[prop(optional)]
    dirty_tracker: Option<RwSignal<DirtyTracker>>,
    /// Submit button label. Defaults to "Submit".
    #[prop(optional, into)]
    submit_label: Option<String>,
    /// Submit button text while submitting. Defaults to "Submitting...".
    #[prop(optional, into)]
    submitting_label: Option<String>,
    /// Whether to render a reset button. Defaults to false.
    #[prop(default = false)]
    show_reset: bool,
    /// Reset button label. Defaults to "Reset".
    #[prop(optional, into)]
    reset_label: Option<String>,
    /// Optional callback fired on form reset.
    #[prop(optional, into)]
    on_reset: Option<Callback<()>>,
    /// Additional CSS classes on the `<form>` element.
    #[prop(optional, into)]
    class: String,
) -> impl IntoView {
    let default_map = {
        let mut map = HashMap::new();
        for f in &schema.fields {
            if let Some(ref def) = f.default_value {
                map.insert(f.name.clone(), def.clone());
            } else if matches!(f.kind, FieldKind::Checkbox | FieldKind::Switch) {
                map.insert(f.name.clone(), "false".to_string());
            } else {
                map.insert(f.name.clone(), String::new());
            }
        }
        map
    };

    let local_values = if let Some(sig) = values {
        sig.update(|m| {
            for (k, v) in &default_map {
                m.entry(k.clone()).or_insert_with(|| v.clone());
            }
        });
        sig
    } else {
        RwSignal::new(default_map.clone())
    };
    let form_state = state.unwrap_or_else(|| RwSignal::new(FormState::idle()));

    let handle_submit = {
        let schema = schema.clone();
        Callback::new(move |()| {
            let current = local_values.get_untracked();
            let errors = schema.validate_map(&current);
            if errors.is_empty() {
                form_state.update(|s| {
                    s.clear_errors();
                    s.set_submitting();
                });
                on_submit.run(current);
            } else {
                form_state.update(|s| s.set_field_errors(errors));
            }
        })
    };

    let initial_for_reset = default_map.clone();
    let on_reset_btn = Callback::new(move |()| {
        local_values.set(initial_for_reset.clone());
        if let Some(cb) = on_reset {
            cb.run(());
        }
    });

    let submit_txt = submit_label.unwrap_or_else(|| "Submit".to_string());
    let submitting_txt = submitting_label.unwrap_or_else(|| "Submitting...".to_string());
    let reset_txt = reset_label.unwrap_or_else(|| "Reset".to_string());

    let form_class = if class.is_empty() {
        "space-y-6".to_string()
    } else {
        format!("space-y-6 {class}")
    };

    let fields_view = schema
        .fields
        .iter()
        .map(|f| {
            view! {
                <DynamicFieldItem field=f.clone() values=local_values />
            }
        })
        .collect_view();

    let header_view = schema.title.clone().map(|title| {
        let desc_view = schema.description.clone().map(|desc| {
            view! {
                <p class="text-sm text-muted-foreground">{desc}</p>
            }
        });
        view! {
            <div class="space-y-1 mb-6">
                <h3 class="text-lg font-semibold text-foreground tracking-tight">{title}</h3>
                {desc_view}
            </div>
        }
    });

    view! {
        <Form
            state=form_state
            dirty_tracker=dirty_tracker
            on_submit=handle_submit
            auto_submitting=false
            class=form_class
        >
            {header_view}
            <FormError />
            <div class="space-y-4">
                {fields_view}
            </div>
            <div class="flex items-center gap-3 pt-4">
                <SubmitButton submitting_text=submitting_txt>{submit_txt}</SubmitButton>
                {show_reset.then(|| view! {
                    <ResetButton on_reset=on_reset_btn>{reset_txt}</ResetButton>
                })}
            </div>
        </Form>
    }
}
