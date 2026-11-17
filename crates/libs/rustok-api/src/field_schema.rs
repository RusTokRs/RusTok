//! Raw definition shape shared by attached-field wire DTOs and model adapters.

use serde_json::Value as JsonValue;

/// Source shape for converting persisted rows into portable core field definitions.
pub trait FieldDefinitionSource {
    fn field_key(&self) -> &str;
    fn field_type(&self) -> &str;
    fn label(&self) -> &JsonValue;
    fn description(&self) -> Option<&JsonValue>;
    fn is_localized(&self) -> bool;
    fn is_required(&self) -> bool;
    fn default_value(&self) -> Option<&JsonValue>;
    fn validation(&self) -> Option<&JsonValue>;
    fn position(&self) -> i32;
    fn is_active(&self) -> bool;
}

#[macro_export]
macro_rules! impl_field_definition_source {
    ($model:ty) => {
        impl $crate::field_schema::FieldDefinitionSource for $model {
            fn field_key(&self) -> &str {
                &self.field_key
            }

            fn field_type(&self) -> &str {
                &self.field_type
            }

            fn label(&self) -> &serde_json::Value {
                &self.label
            }

            fn description(&self) -> Option<&serde_json::Value> {
                self.description.as_ref()
            }

            fn is_localized(&self) -> bool {
                self.is_localized
            }

            fn is_required(&self) -> bool {
                self.is_required
            }

            fn default_value(&self) -> Option<&serde_json::Value> {
                self.default_value.as_ref()
            }

            fn validation(&self) -> Option<&serde_json::Value> {
                self.validation.as_ref()
            }

            fn position(&self) -> i32 {
                self.position
            }

            fn is_active(&self) -> bool {
                self.is_active
            }
        }
    };
}
