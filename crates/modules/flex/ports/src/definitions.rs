use serde_json::Value as JsonValue;
use uuid::Uuid;

/// Service-layer representation of a field definition.
///
/// Kept outside GraphQL types so registry contracts stay transport-agnostic.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FieldDefinitionView {
    pub id: Uuid,
    pub field_key: String,
    pub field_type: String,
    pub label: JsonValue,
    pub description: Option<JsonValue>,
    pub is_localized: bool,
    pub is_required: bool,
    pub default_value: Option<JsonValue>,
    pub validation: Option<JsonValue>,
    pub position: i32,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

rustok_api::impl_field_definition_source!(FieldDefinitionView);

/// Source shape for converting persisted field-definition rows into Flex views.
pub trait FieldDefinitionViewSource {
    fn id(&self) -> Uuid;
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
    fn created_at(&self) -> String;
    fn updated_at(&self) -> String;
}

impl FieldDefinitionView {
    pub fn from_source<T: FieldDefinitionViewSource>(source: &T) -> Self {
        Self {
            id: source.id(),
            field_key: source.field_key().to_string(),
            field_type: source.field_type().to_string(),
            label: source.label().clone(),
            description: source.description().cloned(),
            is_localized: source.is_localized(),
            is_required: source.is_required(),
            default_value: source.default_value().cloned(),
            validation: source.validation().cloned(),
            position: source.position(),
            is_active: source.is_active(),
            created_at: source.created_at(),
            updated_at: source.updated_at(),
        }
    }
}
