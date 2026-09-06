use crate::validator::Validator;
use leptos::prelude::*;
use std::collections::HashMap;

#[derive(Clone)]
pub struct FormContext {
    fields: RwSignal<HashMap<String, String>>,
    validators: RwSignal<HashMap<String, Validator>>,
    field_errors: RwSignal<HashMap<String, String>>,
    form_error: RwSignal<Option<String>>,
    is_submitting: RwSignal<bool>,
}

impl FormContext {
    pub fn new() -> Self {
        Self {
            fields: RwSignal::new(HashMap::new()),
            validators: RwSignal::new(HashMap::new()),
            field_errors: RwSignal::new(HashMap::new()),
            form_error: RwSignal::new(None),
            is_submitting: RwSignal::new(false),
        }
    }

    pub fn register(&self, name: impl Into<String>) {
        let name = name.into();
        self.fields.update(|fields| {
            fields.entry(name).or_insert_with(String::new);
        });
    }

    pub fn set_validator(&self, name: impl Into<String>, validator: Validator) {
        let name = name.into();
        self.validators.update(|validators| {
            validators.insert(name, validator);
        });
    }

    pub fn set_value(&self, name: impl Into<String>, value: String) {
        let name = name.into();
        self.fields.update(|fields| {
            fields.insert(name, value);
        });
    }

    pub fn get_value(&self, name: &str) -> String {
        self.fields
            .with(|fields| fields.get(name).cloned().unwrap_or_default())
    }

    pub fn validate_field(&self, name: &str) -> Result<(), String> {
        let value = self.get_value(name);
        let validator = self
            .validators
            .with(|validators| validators.get(name).cloned());

        if let Some(validator) = validator {
            match validator.validate(&value) {
                Ok(()) => {
                    self.field_errors.update(|errors| {
                        errors.remove(name);
                    });
                    Ok(())
                }
                Err(err) => {
                    self.field_errors.update(|errors| {
                        errors.insert(name.to_string(), err.clone());
                    });
                    Err(err)
                }
            }
        } else {
            Ok(())
        }
    }

    pub fn validate_all(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        self.validators.with(|validators| {
            for name in validators.keys() {
                if let Err(err) = self.validate_field(name) {
                    errors.push(err);
                }
            }
        });

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn get_field_error(&self, name: &str) -> Option<String> {
        self.field_errors.with(|errors| errors.get(name).cloned())
    }

    pub fn set_form_error(&self, error: Option<String>) {
        self.form_error.set(error);
    }

    pub fn get_form_error(&self) -> Option<String> {
        self.form_error.get()
    }

    pub fn is_submitting(&self) -> bool {
        self.is_submitting.get()
    }

    pub fn set_submitting(&self, submitting: bool) {
        self.is_submitting.set(submitting);
    }

    pub fn reset(&self) {
        self.fields.update(|fields| {
            for value in fields.values_mut() {
                value.clear();
            }
        });
        self.field_errors.update(|errors| errors.clear());
        self.form_error.set(None);
        self.is_submitting.set(false);
    }
}

impl Default for FormContext {
    fn default() -> Self {
        Self::new()
    }
}
