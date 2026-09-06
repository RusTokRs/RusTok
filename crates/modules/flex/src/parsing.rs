use rustok_core::field_schema::FieldDefinition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDefinitionsConfigParseError {
    message: String,
}

impl FieldDefinitionsConfigParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

pub fn parse_field_definitions_config(
    value: serde_json::Value,
) -> Result<Vec<FieldDefinition>, FieldDefinitionsConfigParseError> {
    serde_json::from_value(value).map_err(|_| {
        FieldDefinitionsConfigParseError::new(
            "fields_config must be a valid JSON array of FieldDefinition-compatible objects",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::parse_field_definitions_config;
    use serde_json::json;

    #[test]
    fn parses_valid_field_definitions_array() {
        let fields = parse_field_definitions_config(json!([
            {
                "field_key": "title",
                "field_type": "text",
                "label": { "en": "Title" },
                "is_localized": false,
                "is_required": true
            }
        ]))
        .expect("field definitions should parse");

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field_key, "title");
    }

    #[test]
    fn rejects_non_array_payload() {
        let error = parse_field_definitions_config(json!({ "not": "an array" }))
            .expect_err("non-array payload must be rejected");

        assert_eq!(
            error.message(),
            "fields_config must be a valid JSON array of FieldDefinition-compatible objects"
        );
    }
}
