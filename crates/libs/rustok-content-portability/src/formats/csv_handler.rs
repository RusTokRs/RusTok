//! CSV format handler.

use rustok_content_portability_api::{FormatOptions, PortabilityError};
use serde::{de::DeserializeOwned, Serialize};

/// CSV format handler for import/export operations.
pub struct CsvFormatHandler;

impl CsvFormatHandler {
    pub fn new() -> Self {
        Self
    }

    /// Parse CSV from bytes.
    pub fn parse<T: DeserializeOwned>(
        data: &[u8],
        options: &FormatOptions,
    ) -> Result<Vec<T>, PortabilityError> {
        let delimiter = options.csv_delimiter.unwrap_or(b',');
        let has_headers = options.csv_include_headers.unwrap_or(true);

        let mut reader = csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .has_headers(has_headers)
            .from_reader(data);

        let mut items = Vec::new();
        for result in reader.deserialize() {
            let item: T = result.map_err(|e| PortabilityError::format(e.to_string()))?;
            items.push(item);
        }

        Ok(items)
    }

    /// Serialize to CSV bytes.
    pub fn serialize<T: Serialize>(
        values: &[T],
        options: &FormatOptions,
    ) -> Result<Vec<u8>, PortabilityError> {
        let delimiter = options.csv_delimiter.unwrap_or(b',');
        let has_headers = options.csv_include_headers.unwrap_or(true);

        let mut writer = csv::WriterBuilder::new()
            .delimiter(delimiter)
            .has_headers(has_headers)
            .from_writer(Vec::new());

        for value in values {
            writer
                .serialize(value)
                .map_err(|e| PortabilityError::serialization(e.to_string()))?;
        }

        writer
            .into_inner()
            .map_err(|e| PortabilityError::serialization(e.to_string()))
    }
}

impl Default for CsvFormatHandler {
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
    fn parse_csv_with_headers() {
        let csv = "id,name\n1,alice\n2,bob";
        let options = FormatOptions::new();
        let items: Vec<TestItem> = CsvFormatHandler::parse(csv.as_bytes(), &options).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, 1);
        assert_eq!(items[0].name, "alice");
    }

    #[test]
    fn parse_csv_custom_delimiter() {
        let csv = "id;name\n1;alice\n2;bob";
        let options = FormatOptions::new().with_csv_delimiter(b';');
        let items: Vec<TestItem> = CsvFormatHandler::parse(csv.as_bytes(), &options).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn serialize_csv_with_headers() {
        let items = vec![
            TestItem { id: 1, name: "alice".to_string() },
            TestItem { id: 2, name: "bob".to_string() },
        ];
        let options = FormatOptions::new();
        let bytes = CsvFormatHandler::serialize(&items, &options).unwrap();
        let csv = String::from_utf8(bytes).unwrap();
        assert!(csv.contains("id,name"));
        assert!(csv.contains("1,alice"));
    }

    #[test]
    fn serialize_csv_without_headers() {
        let items = vec![TestItem { id: 1, name: "alice".to_string() }];
        let options = FormatOptions::new().with_csv_headers(false);
        let bytes = CsvFormatHandler::serialize(&items, &options).unwrap();
        let csv = String::from_utf8(bytes).unwrap();
        assert!(!csv.contains("id,name"));
        assert!(csv.contains("1,alice"));
    }
}
