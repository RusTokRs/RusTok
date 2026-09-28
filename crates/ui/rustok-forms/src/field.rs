//! Framework-agnostic field descriptor contracts.
//!
//! These types describe what a form field *is* (kind, constraints, options)
//! without depending on any UI framework. They enable shared schema
//! construction, dynamic form generation, and cross-framework parity checks.

use serde::{Deserialize, Serialize};

/// The kind of a form field, driving rendering hints and input behavior.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Email,
    Password,
    Number,
    Url,
    Tel,
    Textarea,
    Select,
    Checkbox,
    Switch,
    Radio,
    File,
    Date,
    DateTime,
    Time,
    Color,
    Hidden,
    RichText,
    /// Extension point for module-specific field kinds.
    Custom { type_name: String },
}

/// A selectable option for Select, Radio, CheckboxGroup, and similar fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldOption {
    pub value: String,
    pub label: String,
    #[serde(default)]
    pub disabled: bool,
}

impl FieldOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

/// Constraints on a field value, evaluated by validation logic.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FieldConstraints {
    pub required: bool,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    /// Regex pattern for client-side validation hints.
    pub pattern: Option<String>,
    /// `accept` attribute for file inputs.
    pub accept: Option<String>,
    /// Whether the field accepts multiple values (file, select).
    #[serde(default)]
    pub multiple: bool,
}

/// Complete descriptor for a form field.
///
/// Used for schema-driven form rendering and cross-framework parity.
/// Not required for basic form usage — modules can compose form components
/// directly without declaring descriptors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FieldDescriptor {
    pub name: String,
    pub kind: FieldKind,
    #[serde(default)]
    pub constraints: FieldConstraints,
    pub label: Option<String>,
    pub description: Option<String>,
    pub placeholder: Option<String>,
    #[serde(default)]
    pub disabled: bool,
    pub default_value: Option<String>,
    #[serde(default)]
    pub options: Vec<FieldOption>,
}

impl FieldDescriptor {
    /// Create a minimal field descriptor.
    pub fn new(name: impl Into<String>, kind: FieldKind) -> Self {
        Self {
            name: name.into(),
            kind,
            constraints: FieldConstraints::default(),
            label: None,
            description: None,
            placeholder: None,
            disabled: false,
            default_value: None,
            options: Vec::new(),
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn placeholder(mut self, ph: impl Into<String>) -> Self {
        self.placeholder = Some(ph.into());
        self
    }

    pub fn required(mut self) -> Self {
        self.constraints.required = true;
        self
    }

    pub fn min_length(mut self, min: usize) -> Self {
        self.constraints.min_length = Some(min);
        self
    }

    pub fn max_length(mut self, max: usize) -> Self {
        self.constraints.max_length = Some(max);
        self
    }

    pub fn min(mut self, min: f64) -> Self {
        self.constraints.min = Some(min);
        self
    }

    pub fn max(mut self, max: f64) -> Self {
        self.constraints.max = Some(max);
        self
    }

    pub fn pattern(mut self, pat: impl Into<String>) -> Self {
        self.constraints.pattern = Some(pat.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn default_value(mut self, val: impl Into<String>) -> Self {
        self.default_value = Some(val.into());
        self
    }

    pub fn options(mut self, opts: Vec<FieldOption>) -> Self {
        self.options = opts;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_builder() {
        let field = FieldDescriptor::new("email", FieldKind::Email)
            .label("Email address")
            .placeholder("user@example.com")
            .default_value("admin@example.com")
            .min_length(5)
            .max_length(100)
            .pattern(r"^.+@.+$")
            .disabled(true)
            .required();

        assert_eq!(field.name, "email");
        assert_eq!(field.kind, FieldKind::Email);
        assert!(field.constraints.required);
        assert_eq!(field.constraints.min_length, Some(5));
        assert_eq!(field.constraints.max_length, Some(100));
        assert_eq!(field.constraints.pattern.as_deref(), Some(r"^.+@.+$"));
        assert!(field.disabled);
        assert_eq!(field.default_value.as_deref(), Some("admin@example.com"));
        assert_eq!(field.label.as_deref(), Some("Email address"));
    }

    #[test]
    fn select_with_options() {
        let field = FieldDescriptor::new("locale", FieldKind::Select)
            .label("Locale")
            .options(vec![
                FieldOption::new("en", "English"),
                FieldOption::new("ru", "Русский").disabled(),
            ]);

        assert_eq!(field.options.len(), 2);
        assert!(!field.options[0].disabled);
        assert!(field.options[1].disabled);
    }
}
