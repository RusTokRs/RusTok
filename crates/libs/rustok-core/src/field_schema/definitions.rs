//! Decoding of raw field definitions from storage and provider DTOs.

use std::collections::HashMap;

use serde_json::Value as JsonValue;

use super::{FieldDefinition, FieldType, ValidationRule};

pub use rustok_api::field_schema::FieldDefinitionSource;

/// Error decoding a field definition from raw source DTO/storage
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FieldDefinitionError {
    #[error("unknown persisted field type `{field_type}`: {message}")]
    UnknownFieldType { field_type: String, message: String },

    #[error("persisted field label is invalid: {message}")]
    InvalidLabel { message: String },

    #[error("persisted field description is invalid: {message}")]
    InvalidDescription { message: String },

    #[error("persisted field validation is invalid: {message}")]
    InvalidValidation { message: String },
}

impl From<FieldDefinitionError> for String {
    fn from(err: FieldDefinitionError) -> Self {
        err.to_string()
    }
}

pub fn field_definition_from_source<T: FieldDefinitionSource>(
    source: &T,
) -> Result<FieldDefinition, FieldDefinitionError> {
    let field_type: FieldType = serde_json::from_value(JsonValue::String(
        source.field_type().to_string(),
    ))
    .map_err(|error| FieldDefinitionError::UnknownFieldType {
        field_type: source.field_type().to_string(),
        message: error.to_string(),
    })?;

    let label: HashMap<String, String> = serde_json::from_value(source.label().clone())
        .map_err(|error| FieldDefinitionError::InvalidLabel {
            message: error.to_string(),
        })?;

    let description: Option<HashMap<String, String>> = source
        .description()
        .map(|value| {
            serde_json::from_value(value.clone()).map_err(|error| {
                FieldDefinitionError::InvalidDescription {
                    message: error.to_string(),
                }
            })
        })
        .transpose()?;

    let validation: Option<ValidationRule> = source
        .validation()
        .map(|value| {
            serde_json::from_value(value.clone()).map_err(|error| {
                FieldDefinitionError::InvalidValidation {
                    message: error.to_string(),
                }
            })
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
