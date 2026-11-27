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
use rustok_ui_forms::{DirtyTracker, DynamicFormPayload, FormState};

use crate::button::{ResetButton, SubmitButton};
use crate::context::FormContext;
use crate::field::{FormDescription, FormError, FormField, FormLabel, FormMessage};
use crate::form::Form;
use crate::inputs::{
    FormCheckbox, FormColorInput, FormFileInput, FormHiddenInput, FormInput, FormNumberInput,
    FormPasswordInput, FormRadioGroup, FormRangeInput, FormSearchInput, FormSelect, FormSwitch,
    FormTextarea,
};

/// Give each control its field context before constructing it.
#[component]
fn DynamicFieldItem(
    field: FieldDescriptor,
    values: RwSignal<HashMap<String, String>>,
    selections: RwSignal<HashMap<String, Vec<String>>>,
) -> impl IntoView {
    if matches!(field.kind, FieldKind::Hidden) {
        return view! { <DynamicFieldControl field=field values=values selections=selections /> }
            .into_any();
    }

    let name = field.name.clone();
    let is_boolean = matches!(field.kind, FieldKind::Checkbox | FieldKind::Switch);
    let is_multi = field.is_multiple_select();
    let required = field.constraints.required;
    let label = field.label.clone();
    let description = if field.supports_dynamic_form() {
        field.description.clone()
    } else {
        let warning = "This form does not support file fields.";
        Some(match &field.description {
            Some(description) => format!("{description} {warning}"),
            None => warning.to_string(),
        })
    };
    view! {
        <FormField name=name>
            {(!is_boolean && !is_multi).then(|| label.map(|label| view! {
                <FormLabel required=required>{label}</FormLabel>
            })).flatten()}
            <DynamicFieldControl field=field values=values selections=selections />
            {(!is_boolean).then(|| description.map(|description| view! {
                <FormDescription>{description}</FormDescription>
            })).flatten()}
            <FormMessage />
        </FormField>
    }
    .into_any()
}

/// Construct a control only after the enclosing FormField has provided its context.
#[component]
fn DynamicFieldControl(
    field: FieldDescriptor,
    values: RwSignal<HashMap<String, String>>,
    selections: RwSignal<HashMap<String, Vec<String>>>,
) -> impl IntoView {
    let name = field.name.clone();
    let name_for_val = name.clone();
    let value_sig =
        Signal::derive(move || values.with(|m| m.get(&name_for_val).cloned().unwrap_or_default()));

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
    let field_disabled = field.disabled || !field.supports_dynamic_form();
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
                disabled=disabled_sig
                required=required
                on_clear=Callback::new(move |()| on_input_cb.run(String::new()))
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

        FieldKind::Select if field.is_multiple_select() => {
            let group_label = field.label.clone().unwrap_or_else(|| name.clone());
            let group_name = name.clone();
            let options = field.options.clone();
            view! {
                <fieldset aria-required=if required { "true" } else { "false" }>
                    <legend class="text-sm font-medium">{group_label}</legend>
                    {options.into_iter().enumerate().map(|(index, option)| {
                        let selected_name = group_name.clone();
                        let selected_value = option.value.clone();
                        let checked = Signal::derive(move || selections.with(|map| {
                            map.get(&selected_name).is_some_and(|values| values.contains(&selected_value))
                        }));
                        let change_name = group_name.clone();
                        let change_value = option.value.clone();
                        let on_change = Callback::new(move |checked: bool| selections.update(|map| {
                            let values = map.entry(change_name.clone()).or_default();
                            if checked && !values.contains(&change_value) { values.push(change_value.clone()); }
                            else if !checked { values.retain(|value| value != &change_value); }
                        }));
                        let option_disabled = option.disabled;
                        let disabled = Signal::derive(move || option_disabled || disabled_sig.get());
                        let id = format!("{}-option-{}", group_name, index);
                        view! {
                            <FormCheckbox id=id name=group_name.clone() label=option.label
                                checked=checked on_change=on_change disabled=disabled />
                        }
                    }).collect_view()}
                </fieldset>
            }.into_any()
        }

        FieldKind::Select => view! {
            <FormSelect
                value=value_sig
                options=field.options.clone()
                on_change=on_change_cb
                placeholder=placeholder
                disabled=disabled_sig
                required=required
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

    control
}

/// Dynamic form component that renders an entire interactive form from a [`FormSchema`].
///
/// Features:
/// - Renders scalar and multiple-selection fields and disables unsupported file fields.
/// - Validates payloads against [`FormSchema::validate_dynamic_form`] on submission before calling `on_submit`.
/// - Displays field errors and form errors automatically.
/// - Supports controlled or internal values and requires caller-owned submission state.
/// - Auto-disables inputs and buttons during asynchronous submission.
/// - Supports reset button with automatic restore of default schema values.
#[component]
pub fn DynamicForm(
    /// Declarative schema defining the form fields, labels, constraints, and kinds.
    schema: FormSchema,
    /// Submission handler receiving validated scalar and multiple-selection data.
    #[prop(into)]
    on_submit: Callback<DynamicFormPayload>,
    /// Optional externally controlled values map. If omitted, internal reactive state is maintained.
    #[prop(optional)]
    values: Option<RwSignal<HashMap<String, String>>>,
    /// Optional controlled multiple-selection values. Omitted selections start empty.
    #[prop(optional)]
    selections: Option<RwSignal<HashMap<String, Vec<String>>>>,
    /// Externally owned state; the submit handler must complete the submission on this signal.
    state: RwSignal<FormState>,
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
            if f.is_multiple_select() {
                continue;
            }
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
    let local_selections = selections.unwrap_or_else(|| RwSignal::new(HashMap::new()));
    let form_state = state;

    let handle_submit = {
        let schema = schema.clone();
        Callback::new(move |()| {
            let current = DynamicFormPayload {
                values: local_values.get_untracked(),
                selections: local_selections.get_untracked(),
            };
            let errors = schema.validate_dynamic_form(&current);
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
        local_selections.set(HashMap::new());
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

    let fields = schema.fields.clone();

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
                {fields.into_iter().map(|field| view! {
                    <DynamicFieldItem field=field values=local_values selections=local_selections />
                }).collect_view()}
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_fields_are_disabled_and_explained() {
        let state = RwSignal::new(FormState::idle());
        let schema = FormSchema::new("test")
            .with_field(FieldDescriptor::new("attachment", FieldKind::File).label("Attachment"))
            .with_field(
                FieldDescriptor::new("tags", FieldKind::Select)
                    .multiple(true)
                    .label("Tags")
                    .options(vec![rustok_ui_forms::FieldOption::new("a", "Alpha")]),
            );
        let html = view! {
            <DynamicForm schema=schema state=state on_submit=Callback::new(|_| ()) />
        }
        .to_html();
        assert!(html.contains("This form does not support file fields."));
        let file = html.split("type=\"file\"").nth(1).expect("file input");
        assert!(
            file.split('>')
                .next()
                .unwrap_or_default()
                .contains("disabled")
        );
        assert!(html.contains("<fieldset"));
        assert!(html.contains("Tags"));
        assert!(html.contains("Alpha"));
        assert!(html.contains("id=\"tags-option-0\""));
    }

    #[test]
    fn multiple_selection_renders_initial_checked_options() {
        let state = RwSignal::new(FormState::idle());
        let selections =
            RwSignal::new(HashMap::from([("tags".to_string(), vec!["b".to_string()])]));
        let schema = FormSchema::new("test").with_field(
            FieldDescriptor::new("tags", FieldKind::Select)
                .multiple(true)
                .label("Tags")
                .options(vec![
                    rustok_ui_forms::FieldOption::new("a", "Alpha"),
                    rustok_ui_forms::FieldOption::new("b", "Beta"),
                ]),
        );
        let html = view! {
            <DynamicForm schema=schema state=state selections=selections on_submit=Callback::new(|_| ()) />
        }.to_html();
        let first = html
            .split("id=\"tags-option-0\"")
            .nth(1)
            .expect("first checkbox");
        let second = html
            .split("id=\"tags-option-1\"")
            .nth(1)
            .expect("second checkbox");
        assert!(
            !first
                .split('>')
                .next()
                .unwrap_or_default()
                .contains("checked")
        );
        assert!(
            second
                .split('>')
                .next()
                .unwrap_or_default()
                .contains("checked")
        );
    }

    #[test]
    fn multiple_options_reference_shared_field_error() {
        let state = RwSignal::new(FormState::idle().with_field_error("tags", "Choose a tag"));
        let schema = FormSchema::new("test").with_field(
            FieldDescriptor::new("tags", FieldKind::Select)
                .multiple(true)
                .label("Tags")
                .options(vec![rustok_ui_forms::FieldOption::new("a", "Alpha")]),
        );
        let html = view! {
            <DynamicForm schema=schema state=state on_submit=Callback::new(|_| ()) />
        }
        .to_html();
        let checkbox = html.split("id=\"tags-option-0\"").nth(1).expect("checkbox");
        assert!(
            checkbox
                .split('>')
                .next()
                .unwrap_or_default()
                .contains("aria-describedby=\"tags-message\"")
        );
        assert!(html.contains("id=\"tags-message\""));
    }

    #[test]
    fn dynamic_fields_inherit_form_and_field_context() {
        let state = RwSignal::new(FormState::idle().with_field_error("title", "Title is invalid"));
        let schema = FormSchema::new("test").with_field(
            FieldDescriptor::new("title", FieldKind::Text)
                .label("Title")
                .description("Enter a title"),
        );
        let html = view! {
            <DynamicForm schema=schema state=state on_submit=Callback::new(|_| ()) />
        }
        .to_html();
        assert!(html.contains("for=\"title\""));
        assert!(html.contains("id=\"title\""));
        assert!(html.contains("aria-invalid=\"true\""));
        assert!(html.contains("aria-describedby=\"title-message\""));
        assert!(html.contains("id=\"title-message\""));
        assert!(html.contains("Title is invalid"));
        assert!(html.contains("id=\"title-description\""));
    }
}
