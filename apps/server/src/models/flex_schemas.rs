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

impl flex::StandaloneSchemaViewSource for Model {
    fn schema_id(&self) -> uuid::Uuid {
        self.id
    }

    fn slug(&self) -> &str {
        &self.slug
    }

    fn fields_config_json(&self) -> serde_json::Value {
        self.fields_config.clone()
    }

    fn settings_json(&self) -> serde_json::Value {
        self.settings.clone()
    }

    fn is_active(&self) -> bool {
        self.is_active
    }

    fn created_at_rfc3339(&self) -> String {
        self.created_at.to_rfc3339()
    }

    fn updated_at_rfc3339(&self) -> String {
        self.updated_at.to_rfc3339()
    }
}
