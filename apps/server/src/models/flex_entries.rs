pub use super::_entities::flex_entries::{ActiveModel, Column, Entity, Model, Relation};

impl flex::StandaloneEntryViewSource for Model {
    fn entry_id(&self) -> uuid::Uuid {
        self.id
    }

    fn schema_id(&self) -> uuid::Uuid {
        self.schema_id
    }

    fn entity_type(&self) -> Option<&str> {
        self.entity_type.as_deref()
    }

    fn entity_id(&self) -> Option<uuid::Uuid> {
        self.entity_id
    }

    fn data_json(&self) -> &serde_json::Value {
        &self.data
    }

    fn status(&self) -> &str {
        &self.status
    }

    fn created_at_rfc3339(&self) -> String {
        self.created_at.to_rfc3339()
    }

    fn updated_at_rfc3339(&self) -> String {
        self.updated_at.to_rfc3339()
    }
}
