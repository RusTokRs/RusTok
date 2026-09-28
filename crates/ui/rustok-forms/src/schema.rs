//! Schema-driven form definitions and collective validation.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::error::FieldError;
use crate::field::FieldDescriptor;

/// A collection of field descriptors representing a complete form schema.
///
/// Enables server-driven form generation, configuration forms, and schema validation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FormSchema {
    pub name: Option<String>,
    pub fields: Vec<FieldDescriptor>,
}

impl FormSchema {
    /// Create a new empty `FormSchema`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new named `FormSchema`.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            fields: Vec::new(),
        }
    }

    /// Append a field descriptor to the schema.
    pub fn field(mut self, descriptor: FieldDescriptor) -> Self {
        self.fields.push(descriptor);
        self
    }

    /// Replace or set all field descriptors.
    pub fn fields(mut self, descriptors: Vec<FieldDescriptor>) -> Self {
        self.fields = descriptors;
        self
    }

    /// Look up a field descriptor by name.
    pub fn get_field(&self, name: &str) -> Option<&FieldDescriptor> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Check whether a field name exists in the schema.
    pub fn has_field(&self, name: &str) -> bool {
        self.fields.iter().any(|f| f.name == name)
    }

    /// List all field names in the schema in declaration order.
    pub fn field_names(&self) -> Vec<&str> {
        self.fields.iter().map(|f| f.name.as_str()).collect()
    }

    /// List all field names that are marked required.
    pub fn required_fields(&self) -> Vec<&str> {
        self.fields
            .iter()
            .filter(|f| f.constraints.required)
            .map(|f| f.name.as_str())
            .collect()
    }

    /// Generate a map of default values defined across fields in the schema.
    pub fn default_values(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for f in &self.fields {
            if let Some(ref val) = f.default_value {
                map.insert(f.name.clone(), val.clone());
            }
        }
        map
    }

    /// Validate a map of field name -> string value against the schema.
    pub fn validate_map(&self, values: &HashMap<String, String>) -> Result<(), Vec<FieldError>> {
        let mut all_errors = Vec::new();

        for descriptor in &self.fields {
            let val = values.get(&descriptor.name).map(String::as_str).unwrap_or("");
            if let Err(errs) = descriptor.validate(val) {
                all_errors.extend(errs);
            }
        }

        if all_errors.is_empty() {
            Ok(())
        } else {
            Err(all_errors)
        }
    }

    /// Validate a JSON object against the schema.
    pub fn validate_json(&self, json: &serde_json::Value) -> Result<(), Vec<FieldError>> {
        let mut all_errors = Vec::new();

        if let Some(obj) = json.as_object() {
            for descriptor in &self.fields {
                let val_str = match obj.get(&descriptor.name) {
                    Some(serde_json::Value::String(s)) => s.clone(),
                    Some(serde_json::Value::Number(n)) => n.to_string(),
                    Some(serde_json::Value::Bool(b)) => b.to_string(),
                    Some(serde_json::Value::Null) | None => String::new(),
                    Some(other) => other.to_string(),
                };

                if let Err(errs) = descriptor.validate(&val_str) {
                    all_errors.extend(errs);
                }
            }
        } else {
            all_errors.push(FieldError::form("Expected a JSON object"));
        }

        if all_errors.is_empty() {
            Ok(())
        } else {
            Err(all_errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{FieldDescriptor, FieldKind};

    #[test]
    fn schema_builder_and_query() {
        let schema = FormSchema::named("user_registration")
            .field(FieldDescriptor::new("username", FieldKind::Text).required().min_length(3))
            .field(FieldDescriptor::new("email", FieldKind::Email).required())
            .field(FieldDescriptor::new("bio", FieldKind::Textarea).default_value("About me..."));

        assert_eq!(schema.name.as_deref(), Some("user_registration"));
        assert_eq!(schema.field_names(), vec!["username", "email", "bio"]);
        assert_eq!(schema.required_fields(), vec!["username", "email"]);

        let defaults = schema.default_values();
        assert_eq!(defaults.get("bio").map(String::as_str), Some("About me..."));
    }

    #[test]
    fn schema_validate_map() {
        let schema = FormSchema::new()
            .field(FieldDescriptor::new("title", FieldKind::Text).required().min_length(5))
            .field(FieldDescriptor::new("price", FieldKind::Number).required().min(0.0));

        let mut values = HashMap::new();
        values.insert("title".to_string(), "Hi".to_string());
        values.insert("price".to_string(), "-10".to_string());

        let errs = schema.validate_map(&values).unwrap_err();
        assert_eq!(errs.len(), 2);
    }

    #[test]
    fn schema_validate_json() {
        let schema = FormSchema::new()
            .field(FieldDescriptor::new("slug", FieldKind::Text).required())
            .field(FieldDescriptor::new("count", FieldKind::Number).min(1.0));

        let json = serde_json::json!({
            "slug": "my-post",
            "count": 5
        });

        assert!(schema.validate_json(&json).is_ok());

        let bad_json = serde_json::json!({
            "slug": "",
            "count": 0
        });

        assert!(schema.validate_json(&bad_json).is_err());
    }
}
