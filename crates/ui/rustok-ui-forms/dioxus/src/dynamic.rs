//! `<DynamicForm>` — Schema-driven reactive form renderer for Dioxus 0.7.
//!
//! Automatically maps a [`FormSchema`] and its [`FieldDescriptor`] definitions to
//! typed RusToK form controls (`FormInput`, `FormPasswordInput`, `FormNumberInput`,
//! `FormTextarea`, `FormSelect`, `FormCheckbox`, `FormSwitch`, `FormRadioGroup`, etc.),
//! handling validation, accessible labels, helper texts, and submission coordination.

use std::collections::HashMap;

use dioxus::prelude::*;
use rustok_ui_forms::field::{FieldDescriptor, FieldKind};
use rustok_ui_forms::schema::FormSchema;
use rustok_ui_forms::{DirtyTracker, FormState};

use crate::button::{ResetButton, SubmitButton};
use crate::field::{FormDescription, FormError, FormField, FormLabel, FormMessage};
use crate::form::Form;
use crate::inputs::{
    FormCheckbox, FormColorInput, FormFileInput, FormHiddenInput, FormInput, FormNumberInput,
    FormPasswordInput, FormRadioGroup, FormRangeInput, FormSelect, FormSwitch, FormTextarea,
};

/// Individual dynamic field renderer mapping a [`FieldDescriptor`] to the appropriate input control.
#[component]
fn DynamicFieldItem(field: FieldDescriptor, mut values: Signal<HashMap<String, String>>) -> Element {
    let name = field.name.clone();
    let cur_val = values.read().get(&name).cloned().unwrap_or_default();
    let cur_bool = cur_val == "true" || cur_val == "1";
    let required = field.constraints.required;
    let placeholder = field.placeholder.clone();
    let disabled = field.disabled;

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
            if let Some(ref label) = field.label {
                FormLabel { required: required, "{label}" }
            }
            {control}
            if let Some(ref desc) = field.description {
                FormDescription { "{desc}" }
            }
            FormMessage {}
        }
    }
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
    on_submit: EventHandler<HashMap<String, String>>,
    /// Optional externally controlled values map.
    #[props(default)]
    values: Option<HashMap<String, String>>,
    /// Optional externally controlled form state.
    #[props(default)]
    state: Option<Signal<FormState>>,
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

    let initial_values = values.unwrap_or(default_map.clone());
    let mut local_values = use_signal(move || initial_values);
    let local_state = use_signal(FormState::idle);
    let mut actual_state = state.unwrap_or(local_state);

    let handle_submit = {
        let schema = schema.clone();
        move |_| {
            let current = local_values.read().clone();
            let errors = schema.validate_map(&current);
            if errors.is_empty() {
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
