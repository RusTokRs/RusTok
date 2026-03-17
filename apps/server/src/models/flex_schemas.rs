use rustok_core::field_schema::{CustomFieldsSchema, FieldDefinition, FlexError};

pub use super::_entities::flex_schemas::{ActiveModel, Column, Entity, Model, Relation};

impl Model {
    /// Deserialize `fields_config` JSONB into field definitions.
    pub fn parse_field_definitions(&self) -> Result<Vec<FieldDefinition>, FlexError> {
        serde_json::from_value(self.fields_config.clone()).map_err(|e| {
            FlexError::Database(format!("invalid flex_schemas.fields_config JSON: {e}"))
        })
    }

    /// Build a `CustomFieldsSchema` directly from persisted `fields_config`.
    pub fn build_custom_fields_schema(&self) -> Result<CustomFieldsSchema, FlexError> {
        Ok(CustomFieldsSchema::new(self.parse_field_definitions()?))
    }
}
