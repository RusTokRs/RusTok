//! `<DynamicForm>` — Schema-driven reactive form renderer for Dioxus 0.7.
//!
//! Automatically maps a [`FormSchema`] and its [`FieldDescriptor`] definitions to
//! typed RusToK form controls (`FormInput`, `FormPasswordInput`, `FormNumberInput`,
//! `FormTextarea`, `FormSelect`, `FormCheckbox`, `FormSwitch`, `FormRadioGroup`, etc.),
//! handling validation, accessible labels, helper texts, and submission coordination.

use std::collections::HashMap;

use dioxus::prelude::*;
use rustok_ui_forms::field::{FieldDescriptor, FieldKind, FieldOption};
use rustok_ui_forms::schema::FormSchema;
use rustok_ui_forms::{DirtyTracker, DynamicFormPayload, FormState};

use crate::button::{ResetButton, SubmitButton};
use crate::field::{FormDescription, FormError, FormField, FormLabel, FormMessage};
use crate::form::Form;
use crate::inputs::{
    FormCheckbox, FormColorInput, FormFileInput, FormHiddenInput, FormInput, FormNumberInput,
    FormPasswordInput, FormRadioGroup, FormRangeInput, FormSelect, FormSwitch, FormTextarea,
};

/// One option in a multiple-selection field, with independent stable identity.
#[component]
fn DynamicMultiOption(
    name: String,
    index: usize,
    option: FieldOption,
    mut selections: Signal<HashMap<String, Vec<String>>>,
    disabled: bool,
) -> Element {
    let checked = selections
        .read()
        .get(&name)
        .is_some_and(|values| values.contains(&option.value));
    let id = format!("{}-option-{}", name, index);
    let change_name = name.clone();
    let change_value = option.value.clone();
    rsx! {
        FormCheckbox {
            id: Some(id),
            name: Some(name),
            label: Some(option.label),
            checked: checked,
            disabled: disabled || option.disabled,
            on_change: move |checked: bool| {
                let mut map = selections.write();
                let values = map.entry(change_name.clone()).or_default();
                if checked && !values.contains(&change_value) {
                    values.push(change_value.clone());
                } else if !checked {
                    values.retain(|value| value != &change_value);
                }
            },
        }
    }
}

/// Individual dynamic field renderer mapping a [`FieldDescriptor`] to the appropriate input control.
#[component]
fn DynamicFieldItem(
    field: FieldDescriptor,
    mut values: Signal<HashMap<String, String>>,
    selections: Signal<HashMap<String, Vec<String>>>,
) -> Element {
    let name = field.name.clone();
    let cur_val = values.read().get(&name).cloned().unwrap_or_default();
    let cur_bool = cur_val == "true" || cur_val == "1";
    let required = field.constraints.required;
    let placeholder = field.placeholder.clone();
    let disabled = field.disabled || !field.supports_dynamic_form();

    let control = match &field.kind {
        FieldKind::Password => {
            let name_input = name.clone();
            rsx! {
                FormPasswordInput {
                    value: cur_val,
                    on_input: move |v| {
                        values.write().insert(name_input.clone(), v);
                    },
                    placeholder: placeholder,
                    disabled: disabled,
                    required: required,
                }
            }
        }
        FieldKind::Number => {
            let name_input = name.clone();
            rsx! {
                FormNumberInput {
                    value: cur_val,
                    on_input: move |v| {
                        values.write().insert(name_input.clone(), v);
                    },
                    placeholder: placeholder,
                    min: field.constraints.min,
                    max: field.constraints.max,
                    step: field.constraints.step,
                    disabled: disabled,
                    required: required,
                }
            }
        }
        FieldKind::Search => {
            let name_input = name.clone();
            rsx! {
                FormInput {
                    input_type: "search",
                    value: cur_val,
                    on_input: move |v| {
                        values.write().insert(name_input.clone(), v);
                    },
                    placeholder: placeholder,
                    disabled: disabled,
                    required: required,
                }
            }
        }
        FieldKind::Textarea | FieldKind::RichText => {
            let name_input = name.clone();
            rsx! {
                FormTextarea {
                    value: cur_val,
                    on_input: move |v| {
                        values.write().insert(name_input.clone(), v);
                    },
                    placeholder: placeholder,
                    disabled: disabled,
                    required: required,
                }
            }
        }
        FieldKind::Select if field.is_multiple_select() => {
            let group_label = field.label.clone().unwrap_or_else(|| name.clone());
            rsx! {
                fieldset {
                    "aria-required": if required { "true" } else { "false" },
                    legend { class: "text-sm font-medium", "{group_label}" }
                    for (index, option) in field.options.iter().enumerate() {
                        DynamicMultiOption {
                            key: "{index}",
                            name: name.clone(),
                            index: index,
                            option: option.clone(),
                            selections: selections,
                            disabled: disabled,
                        }
                    }
                }
            }
        }
        FieldKind::Select => {
            let name_change = name.clone();
            rsx! {
                FormSelect {
                    value: cur_val,
                    options: field.options.clone(),
                    on_change: move |v| {
                        values.write().insert(name_change.clone(), v);
                    },
                    placeholder: placeholder,
                    disabled: disabled,
                    required: required,
                }
            }
        }
        FieldKind::Checkbox => {
            let name_change = name.clone();
            rsx! {
                FormCheckbox {
                    checked: cur_bool,
                    on_change: move |c: bool| {
                        values.write().insert(name_change.clone(), c.to_string());
                    },
                    label: field.label.clone(),
                    description: field.description.clone(),
                    disabled: disabled,
                    required: required,
                }
            }
        }
        FieldKind::Switch => {
            let name_change = name.clone();
            rsx! {
                FormSwitch {
                    checked: cur_bool,
                    on_change: move |c: bool| {
                        values.write().insert(name_change.clone(), c.to_string());
                    },
                    label: field.label.clone(),
                    description: field.description.clone(),
                    disabled: disabled,
                }
            }
        }
        FieldKind::Radio => {
            let name_change = name.clone();
            rsx! {
                FormRadioGroup {
                    value: cur_val,
                    options: field.options.clone(),
                    on_change: move |v| {
                        values.write().insert(name_change.clone(), v);
                    },
                    disabled: disabled,
                }
            }
        }
        FieldKind::Color => {
            let name_change = name.clone();
            rsx! {
                FormColorInput {
                    value: cur_val,
                    on_change: move |v| {
                        values.write().insert(name_change.clone(), v);
                    },
                    label: field.label.clone(),
                    disabled: disabled,
                }
            }
        }
        FieldKind::Range => {
            let name_change = name.clone();
            rsx! {
                FormRangeInput {
                    value: cur_val,
                    on_change: move |v| {
                        values.write().insert(name_change.clone(), v);
                    },
                    min: field.constraints.min.unwrap_or(0.0),
                    max: field.constraints.max.unwrap_or(100.0),
                    step: field.constraints.step.unwrap_or(1.0),
                    disabled: disabled,
                }
            }
        }
        FieldKind::Hidden => {
            rsx! {
                FormHiddenInput {
                    value: cur_val,
                    name: name.clone(),
                }
            }
        }
        FieldKind::File => {
            rsx! {
                FormFileInput {
                    accept: field.constraints.accept.clone(),
                    multiple: field.constraints.multiple,
                    disabled: disabled,
                    required: required,
                }
            }
        }
        _ => {
            let name_input = name.clone();
            rsx! {
                FormInput {
                    input_type: field.kind.html_input_type(),
                    value: cur_val,
                    on_input: move |v| {
                        values.write().insert(name_input.clone(), v);
                    },
                    placeholder: placeholder,
                    disabled: disabled,
                    required: required,
                }
            }
        }
    };

    if matches!(field.kind, FieldKind::Hidden) {
        return control;
    }

    if matches!(field.kind, FieldKind::Checkbox | FieldKind::Switch) {
        return rsx! {
            FormField {
                name: "{name}",
                {control}
                FormMessage {}
            }
        };
    }

    rsx! {
        FormField {
            name: "{name}",
            if !field.is_multiple_select() {
                if let Some(ref label) = field.label {
                    FormLabel { required: required, "{label}" }
                }
            }
            {control}
            if !field.supports_dynamic_form() {
                FormDescription {
                    if let Some(ref desc) = field.description { "{desc} " }
                    "This form does not support file fields."
                }
            } else if let Some(ref desc) = field.description {
                FormDescription { "{desc}" }
            }
            FormMessage {}
        }
    }
}

/// Dynamic form component that renders an entire interactive form from a [`FormSchema`].
///
/// Features:
/// - Renders scalar and multiple-selection fields and disables unsupported file fields.
/// - Validates payloads against [`FormSchema::validate_dynamic_form`] on submission before calling `on_submit`.
/// - Displays field errors and form errors automatically.
/// - Accepts initial values and requires externally owned submission state.
/// - Auto-disables inputs and buttons during asynchronous submission.
/// - Supports reset button with automatic restore of default schema values.
#[component]
pub fn DynamicForm(
    /// Declarative schema defining the form fields, labels, constraints, and kinds.
    schema: FormSchema,
    /// Submission handler receiving validated scalar and multiple-selection data.
    on_submit: EventHandler<DynamicFormPayload>,
    /// Optional initial values, copied once when the component mounts.
    #[props(default)]
    initial_values: Option<HashMap<String, String>>,
    /// Optional initial multiple-selection values, copied once on mount.
    #[props(default)]
    initial_selections: Option<HashMap<String, Vec<String>>>,
    /// Externally owned state; the submit handler must complete the submission on this signal.
    state: Signal<FormState>,
    /// Optional dirty field tracker.
    #[props(default)]
    dirty_tracker: Option<Signal<DirtyTracker>>,
    /// Submit button label. Defaults to "Submit".
    #[props(default = "Submit".to_string())]
    submit_label: String,
    /// Submit button text while submitting.
    #[props(default)]
    submitting_label: Option<String>,
    /// Whether to render a reset button. Defaults to false.
    #[props(default = false)]
    show_reset: bool,
    /// Reset button label. Defaults to "Reset".
    #[props(default = "Reset".to_string())]
    reset_label: String,
    /// Optional callback fired on form reset.
    #[props(default)]
    on_reset: Option<EventHandler<()>>,
    /// Additional CSS classes on the form element.
    #[props(default)]
    class: Option<String>,
) -> Element {
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

    let initial_values = {
        let mut map = default_map.clone();
        if let Some(passed) = initial_values {
            for (k, v) in passed {
                map.insert(k, v);
            }
        }
        map
    };
    let mut local_values = use_signal(move || initial_values);
    let mut local_selections = use_signal(move || initial_selections.unwrap_or_default());
    let mut actual_state = state;

    let handle_submit = {
        let schema = schema.clone();
        move |_| {
            let current = DynamicFormPayload {
                values: local_values.read().clone(),
                selections: local_selections.read().clone(),
            };
            let errors = schema.validate_dynamic_form(&current);
            if errors.is_empty() {
                actual_state.write().clear_errors();
                actual_state.write().set_submitting();
                on_submit.call(current);
            } else {
                actual_state.write().set_field_errors(errors);
            }
        }
    };

    let handle_reset = {
        let initial_for_reset = default_map.clone();
        move |_| {
            local_values.set(initial_for_reset.clone());
            local_selections.set(HashMap::new());
            actual_state.write().reset();
            if let Some(ref cb) = on_reset {
                cb.call(());
            }
        }
    };

    let form_class = format!("space-y-6 {}", class.unwrap_or_default());

    rsx! {
        Form {
            state: actual_state,
            dirty_tracker: dirty_tracker,
            on_submit: handle_submit,
            auto_submitting: false,
            class: "{form_class}",

            if let Some(ref title) = schema.title {
                div {
                    class: "space-y-1 mb-6",
                    h3 { class: "text-lg font-semibold text-foreground tracking-tight", "{title}" }
                    if let Some(ref desc) = schema.description {
                        p { class: "text-sm text-muted-foreground", "{desc}" }
                    }
                }
            }

            FormError {}

            div {
                class: "space-y-4",
                for field in schema.fields.iter() {
                    DynamicFieldItem {
                        key: "{field.name}",
                        field: field.clone(),
                        values: local_values,
                        selections: local_selections,
                    }
                }
            }

            div {
                class: "flex items-center gap-3 pt-4",
                SubmitButton {
                    submitting_text: submitting_label,
                    "{submit_label}"
                }
                if show_reset {
                    ResetButton {
                        on_reset: handle_reset,
                        "{reset_label}"
                    }
                }
            }
        }
    }
}
