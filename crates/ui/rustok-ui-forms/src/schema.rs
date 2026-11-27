//! Declarative schema definition for dynamic, form-wide schemas.
//!
//! `FormSchema` provides an aggregated, declarative representation of an entire
//! form's fields, types, constraints, default values, and validations. It enables
//! dynamic schema reflection, automated map/JSON form payload validation, and
//! form serialization.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::FieldError;
use crate::field::{FieldDescriptor, FieldKind};
use crate::validation::rules;

/// Values submitted by schema-driven forms. Scalar and multiple-selection values are distinct;
/// native file handles remain adapter-specific and are not yet supported by this payload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicFormPayload {
    /// Text, numeric and boolean values, keyed by field name.
    pub values: HashMap<String, String>,
    /// Ordered selected option values, keyed by multiple-select field name.
    pub selections: HashMap<String, Vec<String>>,
}

/// Declarative schema holding the collection of all fields in a form.
///
/// # Example
///
/// ```rust
/// use rustok_ui_forms::schema::FormSchema;
/// use rustok_ui_forms::field::{FieldDescriptor, FieldKind};
/// use std::collections::HashMap;
///
/// let mut schema = FormSchema::new("user_registration")
///     .with_title("Create Account")
///     .with_field(
///         FieldDescriptor::new("username", FieldKind::Text)
///             .label("Username")
///             .required()
///             .min_length(3)
///     )
///     .with_field(
///         FieldDescriptor::new("email", FieldKind::Email)
///             .label("Email Address")
///             .required()
///     );
///
/// let mut payload = HashMap::new();
/// payload.insert("username".to_string(), "al".to_string());
/// payload.insert("email".to_string(), "invalid-email".to_string());
///
/// let errors = schema.validate_map(&payload);
/// assert_eq!(errors.len(), 2);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FormSchema {
    /// Unique identifier for this schema.
    pub id: String,
    /// Human-readable title of the form.
    pub title: Option<String>,
    /// Optional explanatory description for the form.
    pub description: Option<String>,
    /// Field descriptors in this form.
    pub fields: Vec<FieldDescriptor>,
}

impl FormSchema {
    /// Create a new `FormSchema` with the specified ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: None,
            description: None,
            fields: Vec::new(),
        }
    }

    /// Set the human-readable title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the form description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add a field descriptor to the schema.
    pub fn with_field(mut self, field: FieldDescriptor) -> Self {
        self.fields.push(field);
        self
    }

    /// Add multiple field descriptors to the schema.
    pub fn with_fields<I: IntoIterator<Item = FieldDescriptor>>(mut self, fields: I) -> Self {
        self.fields.extend(fields);
        self
    }

    /// Find a field descriptor by name.
    pub fn get_field(&self, name: &str) -> Option<&FieldDescriptor> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Check if a field exists in the schema.
    pub fn has_field(&self, name: &str) -> bool {
        self.fields.iter().any(|f| f.name == name)
    }

    /// Validate a key-value map of string inputs against this schema.
    ///
    /// Evaluates:
    /// - Required constraints (must be present and non-empty).
    /// - Minimum and maximum string length.
    /// - Regex patterns.
    /// - Numeric range constraints (if kind is Number/Range/etc.).
    /// - Email format constraints (if kind is Email).
    /// - URL format constraints (if kind is Url).
    /// - Select/Radio valid options.
    pub fn validate_map(&self, data: &HashMap<String, String>) -> Vec<FieldError> {
        let mut errors = Vec::new();

        for field in &self.fields {
            let value = data.get(&field.name).map(String::as_str);

            // 1. Check required
            if field.constraints.required {
                let val_str = value.unwrap_or("");
                let required_result =
                    if matches!(field.kind, FieldKind::Checkbox | FieldKind::Switch) {
                        if matches!(val_str.trim(), "true" | "1") {
                            Ok(())
                        } else {
                            Err(FieldError::new(
                                &field.name,
                                format!("{} is required", field.name),
                            ))
                        }
                    } else {
                        rules::required(&field.name, val_str, format!("{} is required", field.name))
                    };
                if let Err(e) = required_result {
                    errors.push(e);
                    // Skip further checks if missing and required
                    continue;
                }
            }

            let val_str = match value {
                Some(v) if !v.trim().is_empty() => v,
                _ => continue, // empty optional field: valid
            };

            // 2. Length checks
            if let Some(min) = field.constraints.min_length {
                if let Err(e) = rules::min_length(
                    &field.name,
                    val_str,
                    min,
                    format!("{} must be at least {} characters", field.name, min),
                ) {
                    errors.push(e);
                }
            }
            if let Some(max) = field.constraints.max_length {
                if let Err(e) = rules::max_length(
                    &field.name,
                    val_str,
                    max,
                    format!("{} must be at most {} characters", field.name, max),
                ) {
                    errors.push(e);
                }
            }

            // 3. Pattern checks
            if let Some(pattern) = &field.constraints.pattern {
                if let Err(e) = rules::pattern(
                    &field.name,
                    val_str,
                    pattern,
                    format!("{} format is invalid", field.name),
                ) {
                    errors.push(e);
                }
            }

            // 4. Kind-specific format checks
            match field.kind {
                FieldKind::Email => {
                    if let Err(e) =
                        rules::email(&field.name, val_str, "Invalid email address format")
                    {
                        errors.push(e);
                    }
                }
                FieldKind::Url => {
                    if let Err(e) = rules::url(&field.name, val_str, "Invalid URL format") {
                        errors.push(e);
                    }
                }
                FieldKind::Number | FieldKind::Range => {
                    if let Ok(num) = val_str.trim().parse::<f64>() {
                        if !num.is_finite() {
                            errors.push(FieldError::new(
                                &field.name,
                                "Must be a valid numeric value",
                            ));
                            continue;
                        }
                        if let Some(min) = field.constraints.min {
                            if let Err(e) = rules::min(
                                &field.name,
                                num,
                                min,
                                format!("{} must be at least {}", field.name, min),
                            ) {
                                errors.push(e);
                            }
                        }
                        if let Some(max) = field.constraints.max {
                            if let Err(e) = rules::max(
                                &field.name,
                                num,
                                max,
                                format!("{} must be at most {}", field.name, max),
                            ) {
                                errors.push(e);
                            }
                        }
                    } else {
                        errors.push(FieldError::new(
                            &field.name,
                            "Must be a valid numeric value",
                        ));
                    }
                }
                FieldKind::Select | FieldKind::Radio => {
                    if !field.options.is_empty() {
                        let allowed_values: Vec<&str> =
                            field.options.iter().map(|o| o.value.as_str()).collect();
                        if let Err(e) = rules::one_of(
                            &field.name,
                            val_str,
                            &allowed_values,
                            format!("{} must be one of allowed options", field.name),
                        ) {
                            errors.push(e);
                        }
                    }
                }
                _ => {}
            }
        }

        errors
    }

    /// Validate a dynamic-form payload, including every multiple-selection value.
    /// File fields are rejected until adapters can transport native file handles.
    pub fn validate_dynamic_form(&self, data: &DynamicFormPayload) -> Vec<FieldError> {
        let mut errors = self.validate_map(&data.values);
        for field in &self.fields {
            if field.is_multiple_select() {
                errors.retain(|error| error.field != field.name);
                if data.values.contains_key(&field.name) {
                    errors.push(FieldError::new(
                        &field.name,
                        "Multiple selections must use the selections payload",
                    ));
                    continue;
                }
                let selected = data
                    .selections
                    .get(&field.name)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                if field.constraints.required && selected.is_empty() {
                    errors.push(FieldError::new(
                        &field.name,
                        format!("{} is required", field.name),
                    ));
                    continue;
                }
                let mut seen = std::collections::HashSet::new();
                if selected.iter().any(|value| {
                    !seen.insert(value)
                        || !field
                            .options
                            .iter()
                            .any(|option| !option.disabled && option.value == *value)
                }) {
                    errors.push(FieldError::new(
                        &field.name,
                        "Invalid multiple-selection value",
                    ));
                }
            } else if !field.supports_dynamic_form() {
                errors.retain(|error| error.field != field.name);
                errors.push(FieldError::new(
                    &field.name,
                    "This dynamic form does not support file fields",
                ));
            }
        }
        for name in data.selections.keys() {
            if !self
                .get_field(name)
                .is_some_and(FieldDescriptor::is_multiple_select)
            {
                errors.push(FieldError::new(name, "Unexpected multiple-selection field"));
            }
        }
        errors
    }

    /// Validate a JSON object (`serde_json::Value`) against this schema.
    /// Non-object inputs produce a form-level error.
    pub fn validate_json(&self, json: &serde_json::Value) -> Vec<FieldError> {
        let mut map = HashMap::new();

        let Some(obj) = json.as_object() else {
            return vec![FieldError::form("Expected a JSON object")];
        };
        for (k, v) in obj {
            let str_val = match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Null => String::new(),
                other => other.to_string(),
            };
            map.insert(k.clone(), str_val);
        }

        self.validate_map(&map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::FieldOption;

    #[test]
    fn dynamic_schema_rejects_unrepresentable_payloads_and_invalid_values() {
        let schema = FormSchema::new("example")
            .with_field(FieldDescriptor::new("consent", FieldKind::Checkbox).required())
            .with_field(FieldDescriptor::new("enabled", FieldKind::Switch).required())
            .with_field(FieldDescriptor::new("number", FieldKind::Number));
        let mut data = HashMap::from([
            ("consent".to_string(), "false".to_string()),
            ("enabled".to_string(), "0".to_string()),
            ("number".to_string(), "NaN".to_string()),
        ]);
        assert_eq!(schema.validate_map(&data).len(), 3);
        data.insert("consent".into(), "true".into());
        data.insert("enabled".into(), "1".into());
        for number in ["inf", "-inf", "1e9999"] {
            data.insert("number".into(), number.into());
            assert_eq!(schema.validate_map(&data).len(), 1);
        }
        data.insert("number".into(), "42".into());
        assert!(schema.validate_map(&data).is_empty());

        let unsupported = FormSchema::new("unsupported")
            .with_field(FieldDescriptor::new("attachment", FieldKind::File))
            .with_field(
                FieldDescriptor::new("tags", FieldKind::Select)
                    .constraints(crate::field::FieldConstraints::new().multiple(true)),
            );
        let errors = unsupported.validate_dynamic_form(&DynamicFormPayload::default());
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].field, "attachment");
    }

    #[test]
    fn dynamic_multiple_select_validates_required_options_and_payload_shape() {
        let schema = FormSchema::new("test").with_field(
            FieldDescriptor::new("tags", FieldKind::Select)
                .multiple(true)
                .required()
                .options(vec![
                    FieldOption::new("a", "A"),
                    FieldOption::new("b", "B"),
                    FieldOption::new("disabled", "Disabled").disabled(),
                ]),
        );
        let mut payload = DynamicFormPayload::default();
        assert_eq!(schema.validate_dynamic_form(&payload).len(), 1);
        payload
            .selections
            .insert("tags".into(), vec!["a".into(), "b".into()]);
        assert!(schema.validate_dynamic_form(&payload).is_empty());
        for values in [vec!["a", "a"], vec!["missing"], vec!["disabled"]] {
            payload.selections.insert(
                "tags".into(),
                values.into_iter().map(str::to_string).collect(),
            );
            assert_eq!(schema.validate_dynamic_form(&payload).len(), 1);
        }
        payload.selections.insert("tags".into(), vec!["a".into()]);
        payload.values.insert("tags".into(), "b".into());
        assert_eq!(schema.validate_dynamic_form(&payload).len(), 1);
        payload.values.clear();
        payload.selections.insert("other".into(), vec!["a".into()]);
        assert_eq!(schema.validate_dynamic_form(&payload).len(), 1);
    }

    #[test]
    fn json_validation_rejects_non_objects_even_for_optional_fields() {
        let schema =
            FormSchema::new("example").with_field(FieldDescriptor::new("name", FieldKind::Text));
        for input in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!(42),
        ] {
            let errors = schema.validate_json(&input);
            assert_eq!(errors.len(), 1);
            assert!(errors[0].is_form_level());
        }
        assert!(schema.validate_json(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn test_schema_builder_and_validation() {
        let schema = FormSchema::new("test_form")
            .with_title("Test Form")
            .with_field(
                FieldDescriptor::new("name", FieldKind::Text)
                    .required()
                    .min_length(2)
                    .max_length(50),
            )
            .with_field(
                FieldDescriptor::new("age", FieldKind::Number)
                    .required()
                    .range(18.0, 120.0),
            )
            .with_field(
                FieldDescriptor::new("role", FieldKind::Select)
                    .required()
                    .options(vec![
                        FieldOption::new("admin", "Admin"),
                        FieldOption::new("editor", "Editor"),
                    ]),
            );

        assert!(schema.has_field("name"));
        assert!(schema.has_field("age"));
        assert!(!schema.has_field("unknown"));

        let mut valid_data = HashMap::new();
        valid_data.insert("name".to_string(), "Alice".to_string());
        valid_data.insert("age".to_string(), "25".to_string());
        valid_data.insert("role".to_string(), "admin".to_string());

        let errors = schema.validate_map(&valid_data);
        assert!(errors.is_empty(), "expected valid, got: {:?}", errors);

        let mut invalid_data = HashMap::new();
        invalid_data.insert("name".to_string(), "A".to_string());
        invalid_data.insert("age".to_string(), "15".to_string());
        invalid_data.insert("role".to_string(), "superadmin".to_string());

        let errors = schema.validate_map(&invalid_data);
        assert_eq!(errors.len(), 3);
    }

    #[test]
    fn test_validate_json() {
        let schema = FormSchema::new("contact_form")
            .with_field(FieldDescriptor::new("email", FieldKind::Email).required());

        let json = serde_json::json!({
            "email": "not-an-email"
        });

        let errors = schema.validate_json(&json);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].field, "email");
    }
}
