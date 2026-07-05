use rustok_core::field_schema::{CustomFieldsSchema, FieldDefinition, FlexError};

pub use super::_entities::flex_schemas::{ActiveModel, Column, Entity, Model, Relation};

impl Model {
    /// Deserialize `fields_config` JSONB into field definitions.
    pub fn parse_field_definitions(&self) -> Result<Vec<FieldDefinition>, FlexError> {
        flex::parse_standalone_fields_config(self.fields_config.clone())
    }

    /// Build a `CustomFieldsSchema` directly from persisted `fields_config`.
    pub fn build_custom_fields_schema(&self) -> Result<CustomFieldsSchema, FlexError> {
        flex::build_standalone_custom_fields_schema(self.fields_config.clone())
    }
}
