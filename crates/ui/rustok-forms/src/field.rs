//! Framework-agnostic field descriptor contracts.
//!
//! These types describe what a form field *is* (kind, constraints, options)
//! without depending on any UI framework. They enable shared schema
//! construction, dynamic form generation, and cross-framework parity checks.

use std::hash::Hash;
use serde::{Deserialize, Serialize};

use crate::FieldError;

/// The kind of a form field, driving rendering hints and input behavior.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FieldKind {
    /// Standard single-line text input.
    Text,
    /// Email input with email keyboard and format hints.
    Email,
    /// Password input with masked characters.
    Password,
    /// Numeric input with number keyboard and step increments.
    Number,
    /// Web URL input.
    Url,
    /// Telephone number input.
    Tel,
    /// Search input with search icon and clear button.
    Search,
    /// Multi-line text area.
    Textarea,
    /// Dropdown selection input.
    Select,
    /// Boolean checkbox.
    Checkbox,
    /// Boolean toggle switch.
    Switch,
    /// Single choice radio button group.
    Radio,
    /// File upload input.
    File,
    /// Calendar date input (`YYYY-MM-DD`).
    Date,
    /// Date and time input (`YYYY-MM-DDTHH:MM`).
    DateTime,
    /// Time input (`HH:MM`).
    Time,
    /// Month picker input (`YYYY-MM`).
    Month,
    /// Week picker input (`YYYY-Www`).
    Week,
    /// Numerical range slider.
    Range,
    /// Color picker input.
    Color,
    /// Hidden field input for metadata/tokens.
    Hidden,
    /// Rich text WYSIWYG editor.
    RichText,
    /// Extension point for module-specific field kinds.
    Custom {
        /// Custom type discriminator string.
        type_name: String,
    },
}

impl FieldKind {
    /// Return the corresponding standard HTML5 `<input type="...">` string attribute if applicable.
    pub fn html_input_type(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Email => "email",
            Self::Password => "password",
            Self::Number => "number",
            Self::Url => "url",
            Self::Tel => "tel",
            Self::Search => "search",
            Self::Date => "date",
            Self::DateTime => "datetime-local",
            Self::Time => "time",
            Self::Month => "month",
            Self::Week => "week",
            Self::Range => "range",
            Self::Color => "color",
            Self::Checkbox => "checkbox",
            Self::Radio => "radio",
            Self::File => "file",
            Self::Hidden => "hidden",
            Self::Textarea | Self::Select | Self::Switch | Self::RichText | Self::Custom { .. } => "text",
        }
    }
}

/// A selectable option for Select, Radio, CheckboxGroup, and similar fields.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldOption {
    /// Option value submitted with the form.
    pub value: String,
    /// Human-readable label displayed in UI.
    pub label: String,
    /// Whether this option is disabled for selection.
    #[serde(default)]
    pub disabled: bool,
}

impl FieldOption {
    /// Create a new enabled `FieldOption`.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }

    /// Mark this option as disabled (builder pattern).
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// Create a new `FieldOption` specifying disabled explicitly.
    pub fn with_disabled(value: impl Into<String>, label: impl Into<String>, disabled: bool) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled,
        }
    }
}

impl<V: Into<String>, L: Into<String>> From<(V, L)> for FieldOption {
    fn from((value, label): (V, L)) -> Self {
        Self::new(value, label)
    }
}

/// Constraints on a field value, evaluated by validation logic.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FieldConstraints {
    /// Whether the field is mandatory (non-empty).
    pub required: bool,
    /// Minimum string length in unicode characters.
    pub min_length: Option<usize>,
    /// Maximum string length in unicode characters.
    pub max_length: Option<usize>,
    /// Minimum numerical value.
    pub min: Option<f64>,
    /// Maximum numerical value.
    pub max: Option<f64>,
    /// Step interval for numbers.
    pub step: Option<f64>,
    /// Regex pattern for client-side validation hints.
    pub pattern: Option<String>,
    /// `accept` attribute for file inputs.
    pub accept: Option<String>,
    /// Whether the field accepts multiple values (file, select).
    #[serde(default)]
    pub multiple: bool,
}

impl FieldConstraints {
    /// Create a new empty `FieldConstraints`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set required constraint.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Set minimum string length.
    pub fn min_length(mut self, min: usize) -> Self {
        self.min_length = Some(min);
        self
    }

    /// Set maximum string length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_length = Some(max);
        self
    }

    /// Set both minimum and maximum string length.
    pub fn length_range(mut self, min: usize, max: usize) -> Self {
        self.min_length = Some(min);
        self.max_length = Some(max);
        self
    }

    /// Set minimum numerical value.
    pub fn min(mut self, min: f64) -> Self {
        self.min = Some(min);
        self
    }

    /// Set maximum numerical value.
    pub fn max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }

    /// Set both minimum and maximum numerical value.
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    /// Set numeric step interval.
    pub fn step(mut self, step: f64) -> Self {
        self.step = Some(step);
        self
    }

    /// Set regex pattern constraint.
    pub fn pattern(mut self, pattern: impl Into<String>) -> Self {
        self.pattern = Some(pattern.into());
        self
    }

    /// Set file accept constraint.
    pub fn accept(mut self, accept: impl Into<String>) -> Self {
        self.accept = Some(accept.into());
        self
    }

    /// Set multiple selection / upload support.
    pub fn multiple(mut self, multiple: bool) -> Self {
        self.multiple = multiple;
        self
    }

    /// Validate a value against these constraints.
    pub fn validate(&self, field: &str, value: &str) -> Result<(), Vec<FieldError>> {
        let mut errors = Vec::new();
        let trimmed = value.trim();

        if self.required && trimmed.is_empty() {
            errors.push(FieldError::new(field, format!("{field} is required")));
            return Err(errors);
        }

        if !trimmed.is_empty() {
            let char_count = trimmed.chars().count();
            if let Some(min_len) = self.min_length {
                if char_count < min_len {
                    errors.push(FieldError::new(
                        field,
                        format!("{field} must be at least {min_len} characters"),
                    ));
                }
            }
            if let Some(max_len) = self.max_length {
                if char_count > max_len {
                    errors.push(FieldError::new(
                        field,
                        format!("{field} must be at most {max_len} characters"),
                    ));
                }
            }

            if self.min.is_some() || self.max.is_some() {
                if let Ok(num) = trimmed.parse::<f64>() {
                    if let Some(min_val) = self.min {
                        if num < min_val {
                            errors.push(FieldError::new(
                                field,
                                format!("{field} must be at least {min_val}"),
                            ));
                        }
                    }
                    if let Some(max_val) = self.max {
                        if num > max_val {
                            errors.push(FieldError::new(
                                field,
                                format!("{field} must be at most {max_val}"),
                            ));
                        }
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Complete descriptor for a form field.
///
/// Used for schema-driven form rendering and cross-framework parity.
/// Not required for basic form usage — modules can compose form components
/// directly without declaring descriptors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FieldDescriptor {
    /// Field name / submission key.
    pub name: String,
    /// Field kind driving rendering and interaction.
    pub kind: FieldKind,
    /// Validation and input constraints.
    #[serde(default)]
    pub constraints: FieldConstraints,
    /// Optional field label.
    pub label: Option<String>,
    /// Optional field description or help text.
    pub description: Option<String>,
    /// Optional placeholder text.
    pub placeholder: Option<String>,
    /// Whether the field is disabled.
    #[serde(default)]
    pub disabled: bool,
    /// Default string value.
    pub default_value: Option<String>,
    /// List of selectable options for dropdowns/radios.
    #[serde(default)]
    pub options: Vec<FieldOption>,
}

impl FieldDescriptor {
    /// Create a minimal field descriptor with name and kind.
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

    /// Set field label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set field description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set field placeholder.
    pub fn placeholder(mut self, ph: impl Into<String>) -> Self {
        self.placeholder = Some(ph.into());
        self
    }

    /// Replace field constraints.
    pub fn constraints(mut self, constraints: FieldConstraints) -> Self {
        self.constraints = constraints;
        self
    }

    /// Mark field as required.
    pub fn required(mut self) -> Self {
        self.constraints.required = true;
        self
    }

    /// Set required state explicitly.
    pub fn set_required(mut self, required: bool) -> Self {
        self.constraints.required = required;
        self
    }

    /// Set minimum string length.
    pub fn min_length(mut self, min: usize) -> Self {
        self.constraints.min_length = Some(min);
        self
    }

    /// Set maximum string length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.constraints.max_length = Some(max);
        self
    }

    /// Set length range.
    pub fn length_range(mut self, min: usize, max: usize) -> Self {
        self.constraints.min_length = Some(min);
        self.constraints.max_length = Some(max);
        self
    }

    /// Set minimum numerical value.
    pub fn min(mut self, min: f64) -> Self {
        self.constraints.min = Some(min);
        self
    }

    /// Set maximum numerical value.
    pub fn max(mut self, max: f64) -> Self {
        self.constraints.max = Some(max);
        self
    }

    /// Set numerical range.
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.constraints.min = Some(min);
        self.constraints.max = Some(max);
        self
    }

    /// Set numeric step.
    pub fn step(mut self, step: f64) -> Self {
        self.constraints.step = Some(step);
        self
    }

    /// Set regex pattern.
    pub fn pattern(mut self, pat: impl Into<String>) -> Self {
        self.constraints.pattern = Some(pat.into());
        self
    }

    /// Set file accept filter.
    pub fn accept(mut self, accept: impl Into<String>) -> Self {
        self.constraints.accept = Some(accept.into());
        self
    }

    /// Set multiple selection flag.
    pub fn multiple(mut self, multiple: bool) -> Self {
        self.constraints.multiple = multiple;
        self
    }

    /// Set disabled state.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set default value.
    pub fn default_value(mut self, val: impl Into<String>) -> Self {
        self.default_value = Some(val.into());
        self
    }

    /// Set options list.
    pub fn options(mut self, opts: Vec<FieldOption>) -> Self {
        self.options = opts;
        self
    }

    /// Append a single selectable option.
    pub fn add_option(mut self, opt: impl Into<FieldOption>) -> Self {
        self.options.push(opt.into());
        self
    }

    /// Validate a value using this descriptor's constraints.
    pub fn validate(&self, value: &str) -> Result<(), Vec<FieldError>> {
        self.constraints.validate(&self.name, value)
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
        assert_eq!(field.kind.html_input_type(), "email");
        assert!(field.constraints.required);
        assert_eq!(field.constraints.min_length, Some(5));
        assert_eq!(field.constraints.max_length, Some(100));
        assert_eq!(field.constraints.pattern.as_deref(), Some(r"^.+@.+$"));
        assert!(field.disabled);
        assert_eq!(field.default_value.as_deref(), Some("admin@example.com"));
        assert_eq!(field.label.as_deref(), Some("Email address"));
    }

    #[test]
    fn constraints_builder_and_validate() {
        let constraints = FieldConstraints::new()
            .required(true)
            .length_range(3, 10)
            .range(0.0, 100.0);

        assert!(constraints.validate("age", "").is_err());
        assert!(constraints.validate("age", "ab").is_err());
        assert!(constraints.validate("age", "abc").is_ok());

        let descriptor = FieldDescriptor::new("score", FieldKind::Number)
            .constraints(constraints);
        assert!(descriptor.validate("50").is_ok());
    }

    #[test]
    fn select_with_options() {
        let field = FieldDescriptor::new("locale", FieldKind::Select)
            .label("Locale")
            .options(vec![
                FieldOption::new("en", "English"),
                FieldOption::new("ru", "Русский").disabled(),
            ])
            .add_option(("fr", "Français"));

        assert_eq!(field.options.len(), 3);
        assert!(!field.options[0].disabled);
        assert!(field.options[1].disabled);
        assert_eq!(field.options[2].value, "fr");
        assert_eq!(field.options[2].label, "Français");
    }
}
