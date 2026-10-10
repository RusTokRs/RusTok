//! Import/export services.

use crate::{CsvFormatHandler, JsonFormatHandler, read_file, write_file};
use rustok_content_portability_api::{
    BatchImportResult, ExportContext, ExportResult, Format, FormatOptions, ImportContext,
    PortabilityError,
};
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;
use tracing::instrument;

/// Service for importing content from files.
pub struct ImportService;

impl ImportService {
    pub fn new() -> Self {
        Self
    }

    /// Import items from a JSON file.
    #[instrument(skip(context), fields(tenant_id = %context.tenant_id, format = %context.format))]
    pub async fn import_json_file<T: DeserializeOwned>(
        path: impl AsRef<Path>,
        context: ImportContext,
    ) -> Result<Vec<T>, PortabilityError> {
        if context.format != Format::Json {
            return Err(PortabilityError::format_not_supported(format!(
                "expected JSON, got {}",
                context.format
            )));
        }

        let data = read_file(path).await?;
        JsonFormatHandler::parse_array(&data)
    }

    /// Import items from a CSV file.
    #[instrument(skip(context), fields(tenant_id = %context.tenant_id, format = %context.format))]
    pub async fn import_csv_file<T: DeserializeOwned>(
        path: impl AsRef<Path>,
        context: ImportContext,
        options: &FormatOptions,
    ) -> Result<Vec<T>, PortabilityError> {
        if context.format != Format::Csv {
            return Err(PortabilityError::format_not_supported(format!(
                "expected CSV, got {}",
                context.format
            )));
        }

        let data = read_file(path).await?;
        CsvFormatHandler::parse(&data, options)
    }

    /// Import items from bytes (JSON or CSV).
    pub fn import_from_bytes<T: DeserializeOwned>(
        data: &[u8],
        context: &ImportContext,
        options: &FormatOptions,
    ) -> Result<Vec<T>, PortabilityError> {
        match context.format {
            Format::Json => JsonFormatHandler::parse_array(data),
            Format::Csv => CsvFormatHandler::parse(data, options),
            other => Err(PortabilityError::format_not_supported(other.to_string())),
        }
    }
}

impl Default for ImportService {
    fn default() -> Self {
        Self::new()
    }
}

/// Service for exporting content to files.
pub struct ExportService;

impl ExportService {
    pub fn new() -> Self {
        Self
    }

    /// Export items to a JSON file.
    #[instrument(skip(items, context), fields(tenant_id = %context.tenant_id, format = %context.format, count = items.len()))]
    pub async fn export_json_file<T: Serialize>(
        path: impl AsRef<Path>,
        items: &[T],
        context: ExportContext,
        options: &FormatOptions,
    ) -> Result<ExportResult<()>, PortabilityError> {
        if context.format != Format::Json {
            return Err(PortabilityError::format_not_supported(format!(
                "expected JSON, got {}",
                context.format
            )));
        }

        let start = std::time::Instant::now();
        let data = JsonFormatHandler::serialize_array(items, options)?;
        write_file(path, &data).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ExportResult {
            entities: vec![(); items.len()],
            total_count: items.len(),
            format: context.format,
            exported_at: chrono::Utc::now(),
            duration_ms,
        })
    }

    /// Export items to a CSV file.
    #[instrument(skip(items, context), fields(tenant_id = %context.tenant_id, format = %context.format, count = items.len()))]
    pub async fn export_csv_file<T: Serialize>(
        path: impl AsRef<Path>,
        items: &[T],
        context: ExportContext,
        options: &FormatOptions,
    ) -> Result<ExportResult<()>, PortabilityError> {
        if context.format != Format::Csv {
            return Err(PortabilityError::format_not_supported(format!(
                "expected CSV, got {}",
                context.format
            )));
        }

        let start = std::time::Instant::now();
        let data = CsvFormatHandler::serialize(items, options)?;
        write_file(path, &data).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ExportResult {
            entities: vec![(); items.len()],
            total_count: items.len(),
            format: context.format,
            exported_at: chrono::Utc::now(),
            duration_ms,
        })
    }

    /// Export items to bytes (JSON or CSV).
    pub fn export_to_bytes<T: Serialize>(
        items: &[T],
        context: &ExportContext,
        options: &FormatOptions,
    ) -> Result<Vec<u8>, PortabilityError> {
        match context.format {
            Format::Json => JsonFormatHandler::serialize_array(items, options),
            Format::Csv => CsvFormatHandler::serialize(items, options),
            other => Err(PortabilityError::format_not_supported(other.to_string())),
        }
    }
}

impl Default for ExportService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_content_portability_api::Format;
    use serde::{Deserialize, Serialize};
    use tempfile::NamedTempFile;
    use uuid::Uuid;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestItem {
        id: u32,
        name: String,
    }

    fn test_context(format: Format) -> ImportContext {
        ImportContext::new(Uuid::new_v4(), Uuid::new_v4(), format)
    }

    fn test_export_context(format: Format) -> ExportContext {
        ExportContext::new(Uuid::new_v4(), Uuid::new_v4(), format)
    }

    #[tokio::test]
    async fn import_json_file() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        let items = vec![
            TestItem { id: 1, name: "a".to_string() },
            TestItem { id: 2, name: "b".to_string() },
        ];
        let json = serde_json::to_vec(&items).unwrap();
        tokio::fs::write(path, json).await.unwrap();

        let context = test_context(Format::Json);
        let imported: Vec<TestItem> = ImportService::import_json_file(path, context).await.unwrap();
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[0].id, 1);
    }

    #[tokio::test]
    async fn import_csv_file() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        let csv = "id,name\n1,alice\n2,bob";
        tokio::fs::write(path, csv).await.unwrap();

        let context = test_context(Format::Csv);
        let options = FormatOptions::new();
        let imported: Vec<TestItem> =
            ImportService::import_csv_file(path, context, &options).await.unwrap();
        assert_eq!(imported.len(), 2);
    }

    #[tokio::test]
    async fn export_json_file() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        let items = vec![
            TestItem { id: 1, name: "a".to_string() },
            TestItem { id: 2, name: "b".to_string() },
        ];

        let context = test_export_context(Format::Json);
        let options = FormatOptions::new();
        let result = ExportService::export_json_file(path, &items, context, &options)
            .await
            .unwrap();
        assert_eq!(result.total_count, 2);

        let data = tokio::fs::read(path).await.unwrap();
        let imported: Vec<TestItem> = serde_json::from_slice(&data).unwrap();
        assert_eq!(imported.len(), 2);
    }

    #[tokio::test]
    async fn export_csv_file() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        let items = vec![TestItem { id: 1, name: "alice".to_string() }];

        let context = test_export_context(Format::Csv);
        let options = FormatOptions::new();
        let result = ExportService::export_csv_file(path, &items, context, &options)
            .await
            .unwrap();
        assert_eq!(result.total_count, 1);
    }
}
