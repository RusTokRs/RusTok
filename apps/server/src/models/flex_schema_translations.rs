pub use super::_entities::flex_schema_translations::{
    ActiveModel, Column, Entity, Model, PrimaryKey, Relation,
};

impl flex::StandaloneSchemaTranslationSource for Model {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}
