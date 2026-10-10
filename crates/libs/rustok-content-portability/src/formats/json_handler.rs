//! JSON format handler.

use async_trait::async_trait;
use rustok_content_portability_api::{Format, FormatOptions, PortabilityError};
use serde::{de::DeserializeOwned, Serialize};

/// JSON format handler for import/export operations.
pub struct JsonFormatHandler;

impl JsonFormatHandler {
    pub fn new() -> Self {
        Self
    }

    /// Parse JSON from bytes.
    pub fn parse<T: DeserializeOwned>(data: &[u8]) -> Result<T, PortabilityError> {
        serde_json::from_slice(data).map_err(|e| PortabilityError::format(e.to_string()))
    }

    /// Parse JSON array from bytes.
    pub fn parse_array<T: DeserializeOwned>(data: &[u8]) -> Result<Vec<T>, PortabilityError> {
        serde_json::from_slice(data).map_err(|e| PortabilityError::format(e.to_string()))
    }

    /// Serialize to JSON bytes.
    pub fn serialize<T: Serialize>(
        value: &T,
        options: &FormatOptions,
    ) -> Result<Vec<u8>, PortabilityError> {
        let pretty = options.json_pretty.unwrap_or(false);
        if pretty {
            serde_json::to_vec_pretty(value).map_err(|e| PortabilityError::serialization(e.to_string()))
        } else {
            serde_json::to_vec(value).map_err(|e| PortabilityError::serialization(e.to_string()))
        }
    }

    /// Serialize array to JSON bytes.
    pub fn serialize_array<T: Serialize>(
        values: &[T],
        options: &FormatOptions,
    ) -> Result<Vec<u8>, PortabilityError> {
        Self::serialize(values, options)
    }
}

impl Default for JsonFormatHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestItem {
        id: u32,
        name: String,
    }

    #[test]
    fn parse_single_item() {
        let json = r#"{"id": 1, "name": "test"}"#;
        let item: TestItem = JsonFormatHandler::parse(json.as_bytes()).unwrap();
        assert_eq!(item.id, 1);
        assert_eq!(item.name, "test");
    }

    #[test]
    fn parse_array() {
        let json = r#"[{"id": 1, "name": "a"}, {"id": 2, "name": "b"}]"#;
        let items: Vec<TestItem> = JsonFormatHandler::parse_array(json.as_bytes()).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, 1);
        assert_eq!(items[1].id, 2);
    }

    #[test]
    fn serialize_compact() {
        let item = TestItem { id: 1, name: "test".to_string() };
        let options = FormatOptions::new();
        let bytes = JsonFormatHandler::serialize(&item, &options).unwrap();
        let json = String::from_utf8(bytes).unwrap();
        assert!(!json.contains('\n'));
    }

    #[test]
    fn serialize_pretty() {
        let item = TestItem { id: 1, name: "test".to_string() };
        let options = FormatOptions::new().with_json_pretty(true);
        let bytes = JsonFormatHandler::serialize(&item, &options).unwrap();
        let json = String::from_utf8(bytes).unwrap();
        assert!(json.contains('\n'));
    }
}
