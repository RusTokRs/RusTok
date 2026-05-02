//! Validation helpers for Flex standalone mode (Phase 5).
//!
//! This adapter-layer service converts persisted standalone schema definitions
//! (`flex_schemas.fields_config`) into [`rustok_core::field_schema::CustomFieldsSchema`] and validates
//! entry payloads in a transport-agnostic way.

use serde_json::{Map, Value as JsonValue};

use rustok_core::field_schema::FlexError;

use crate::models::flex_schemas;

/// Adapter-level validation for standalone Flex entries.
pub struct FlexStandaloneValidationService;

impl FlexStandaloneValidationService {
    /// Build a core [`rustok_core::field_schema::CustomFieldsSchema`] from
    /// persisted `flex_schemas.fields_config` JSON.
    pub fn build_schema(
        schema: &flex_schemas::Model,
    ) -> Result<rustok_core::field_schema::CustomFieldsSchema, FlexError> {
        schema.build_custom_fields_schema()
    }

    /// Normalize and validate entry payload against a schema.
    ///
    /// Flow:
    /// 1. Ensure object payload (or coerce `null` to `{}`)
    /// 2. Apply defaults from schema
    /// 3. Strip unknown keys
    /// 4. Validate required/type/range constraints
    pub fn normalize_and_validate_entry(
        schema: &rustok_core::field_schema::CustomFieldsSchema,
        mut data: JsonValue,
    ) -> Result<JsonValue, FlexError> {
        if data.is_null() {
            data = JsonValue::Object(Map::new());
        }

        if !data.is_object() {
            return Err(FlexError::InvalidFieldKey(
                "entry data must be a JSON object".to_string(),
            ));
        }

        schema.apply_defaults(&mut data);
        schema.strip_unknown(&mut data);

        let errors = schema.validate(&data);
        if !errors.is_empty() {
            return Err(FlexError::ValidationFailed(errors));
        }

        Ok(data)
    }

    /// Full validation pipeline for standalone entry payload and persisted schema model.
    pub fn validate_entry_against_schema(
        schema_model: &flex_schemas::Model,
        data: JsonValue,
    ) -> Result<JsonValue, FlexError> {
        let schema = Self::build_schema(schema_model)?;
        Self::normalize_and_validate_entry(&schema, data)
    }
}

#[cfg(test)]
mod tests {
    use super::FlexStandaloneValidationService;
    use crate::models::flex_schemas::Model as FlexSchemaModel;
    use rustok_core::field_schema::{FieldDefinition, FieldType, FlexError};
    use sea_orm::entity::prelude::DateTimeWithTimeZone;
    use serde_json::json;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn ts() -> DateTimeWithTimeZone {
        "2026-03-17T00:00:00Z".parse().expect("valid timestamp")
    }

    fn schema_model(definitions: serde_json::Value) -> FlexSchemaModel {
        FlexSchemaModel {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            slug: "landing_form".to_string(),
            fields_config: definitions,
            settings: json!({}),
            is_active: true,
            created_at: ts(),
            updated_at: ts(),
        }
    }

    fn field(key: &str, required: bool, default: Option<serde_json::Value>) -> FieldDefinition {
        FieldDefinition {
            field_key: key.to_string(),
            field_type: FieldType::Text,
            label: HashMap::from([("en".to_string(), key.to_string())]),
            description: None,
            is_localized: false,
            is_required: required,
            default_value: default,
            validation: None,
            position: 0,
            is_active: true,
        }
    }

    #[test]
    fn validate_entry_against_schema_applies_defaults_and_strips_unknown() {
        let model = schema_model(json!([
            field("name", true, None),
            field("source", false, Some(json!("organic")))
        ]));

        let out = FlexStandaloneValidationService::validate_entry_against_schema(
            &model,
            json!({"name": "Alice", "extra": 1}),
        )
        .expect("valid entry");

        assert_eq!(out, json!({"name": "Alice", "source": "organic"}));
    }

    #[test]
    fn validate_entry_against_schema_rejects_missing_required_fields() {
        let model = schema_model(json!([field("name", true, None)]));

        let err = FlexStandaloneValidationService::validate_entry_against_schema(&model, json!({}))
            .expect_err("required field must fail");

        match err {
            FlexError::ValidationFailed(errors) => {
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].field_key, "name");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn validate_entry_against_schema_rejects_non_object_data() {
        let model = schema_model(json!([field("name", false, None)]));

        let err = FlexStandaloneValidationService::validate_entry_against_schema(&model, json!(42))
            .expect_err("non-object must fail");

        match err {
            FlexError::InvalidFieldKey(msg) => {
                assert!(msg.contains("JSON object"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn build_schema_rejects_invalid_fields_config_json_shape() {
        let model = schema_model(json!({"invalid": "shape"}));

        let err = match FlexStandaloneValidationService::build_schema(&model) {
            Ok(_) => panic!("invalid schema json should fail"),
            Err(err) => err,
        };

        match err {
            FlexError::Database(message) => {
                assert!(message.contains("invalid flex_schemas.fields_config"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
