//! Decoding of raw field definitions from storage and provider DTOs.

use std::collections::HashMap;

use serde_json::Value as JsonValue;

use super::{FieldDefinition, FieldType, ValidationRule};

pub use rustok_api::field_schema::FieldDefinitionSource;

pub fn field_definition_from_source<T: FieldDefinitionSource>(
    source: &T,
) -> Result<FieldDefinition, String> {
    let field_type: FieldType = serde_json::from_value(JsonValue::String(
        source.field_type().to_string(),
    ))
    .map_err(|error| {
        format!(
            "unknown persisted field type `{}`: {error}",
            source.field_type()
        )
    })?;

    let label: HashMap<String, String> = serde_json::from_value(source.label().clone())
        .map_err(|error| format!("persisted field label is invalid: {error}"))?;

    let description: Option<HashMap<String, String>> = source
        .description()
        .map(|value| {
            serde_json::from_value(value.clone())
                .map_err(|error| format!("persisted field description is invalid: {error}"))
        })
        .transpose()?;

    let validation: Option<ValidationRule> = source
        .validation()
        .map(|value| {
            serde_json::from_value(value.clone())
                .map_err(|error| format!("persisted field validation is invalid: {error}"))
        })
        .transpose()?;

    Ok(FieldDefinition {
        field_key: source.field_key().to_string(),
        field_type,
        label,
        description,
        is_localized: source.is_localized(),
        is_required: source.is_required(),
        default_value: source.default_value().cloned(),
        validation,
        position: source.position(),
        is_active: source.is_active(),
    })
}
