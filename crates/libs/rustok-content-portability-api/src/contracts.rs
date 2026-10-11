//! Core import/export contracts.

use crate::{Format, PortabilityError, ProgressCallback};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Context for import operations.
/// Context for import operations.
#[derive(Clone)]
pub struct ImportContext {
    /// Tenant ID for multi-tenant isolation.
    pub tenant_id: Uuid,
    /// User ID performing the import.
    pub user_id: Uuid,
    /// Source format descriptor.
    pub format: Format,
    /// Whether to skip validation errors and continue.
    pub continue_on_error: bool,
    /// Optional progress callback for batch operations.
    pub progress_callback: Option<ProgressCallback>,
    /// Timestamp when import started.
    pub started_at: DateTime<Utc>,
}

impl std::fmt::Debug for ImportContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportContext")
            .field("tenant_id", &self.tenant_id)
            .field("user_id", &self.user_id)
            .field("format", &self.format)
            .field("continue_on_error", &self.continue_on_error)
            .field(
                "progress_callback",
                &self.progress_callback.as_ref().map(|_| "<callback>"),
            )
            .field("started_at", &self.started_at)
            .finish()
    }
}

impl ImportContext {
    pub fn new(tenant_id: Uuid, user_id: Uuid, format: Format) -> Self {
        Self {
            tenant_id,
            user_id,
            format,
            continue_on_error: false,
            progress_callback: None,
            started_at: Utc::now(),
        }
    }

    pub fn with_continue_on_error(mut self, value: bool) -> Self {
        self.continue_on_error = value;
        self
    }

    pub fn with_progress_callback(mut self, callback: ProgressCallback) -> Self {
        self.progress_callback = Some(callback);
        self
    }
}

/// Context for export operations.
#[derive(Clone)]
pub struct ExportContext {
    /// Tenant ID for multi-tenant isolation.
    pub tenant_id: Uuid,
    /// User ID performing the export.
    pub user_id: Uuid,
    /// Target format descriptor.
    pub format: Format,
    /// Optional filters (e.g., date range, status).
    pub filters: ExportFilters,
    /// Optional progress callback for batch operations.
    pub progress_callback: Option<ProgressCallback>,
    /// Timestamp when export started.
    pub started_at: DateTime<Utc>,
}

impl std::fmt::Debug for ExportContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportContext")
            .field("tenant_id", &self.tenant_id)
            .field("user_id", &self.user_id)
            .field("format", &self.format)
            .field("filters", &self.filters)
            .field(
                "progress_callback",
                &self.progress_callback.as_ref().map(|_| "<callback>"),
            )
            .field("started_at", &self.started_at)
            .finish()
    }
}

impl ExportContext {
    pub fn new(tenant_id: Uuid, user_id: Uuid, format: Format) -> Self {
        Self {
            tenant_id,
            user_id,
            format,
            filters: ExportFilters::default(),
            progress_callback: None,
            started_at: Utc::now(),
        }
    }

    pub fn with_filters(mut self, filters: ExportFilters) -> Self {
        self.filters = filters;
        self
    }

    pub fn with_progress_callback(mut self, callback: ProgressCallback) -> Self {
        self.progress_callback = Some(callback);
        self
    }
}

/// Filters for export operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExportFilters {
    /// Filter by creation date (from).
    pub created_after: Option<DateTime<Utc>>,
    /// Filter by creation date (to).
    pub created_before: Option<DateTime<Utc>>,
    /// Filter by status (e.g., "published", "draft").
    pub status: Option<String>,
    /// Custom filters as key-value pairs.
    pub custom: std::collections::HashMap<String, String>,
}

/// Result of a single item import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult<T> {
    /// Imported entity (if successful).
    pub entity: Option<T>,
    /// Source identifier (e.g., original ID, row number).
    pub source_id: String,
    /// Whether the import succeeded.
    pub success: bool,
    /// Validation errors (if any).
    pub errors: Vec<String>,
    /// Warnings (non-fatal issues).
    pub warnings: Vec<String>,
    /// Timestamp of import attempt.
    pub imported_at: DateTime<Utc>,
}

impl<T> ImportResult<T> {
    pub fn success(entity: T, source_id: String) -> Self {
        Self {
            entity: Some(entity),
            source_id,
            success: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            imported_at: Utc::now(),
        }
    }

    pub fn failure(source_id: String, errors: Vec<String>) -> Self {
        Self {
            entity: None,
            source_id,
            success: false,
            errors,
            warnings: Vec::new(),
            imported_at: Utc::now(),
        }
    }

    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }
}

/// Result of a batch import operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchImportResult<T> {
    /// Successfully imported entities.
    pub succeeded: Vec<ImportResult<T>>,
    /// Failed imports with errors.
    pub failed: Vec<ImportResult<T>>,
    /// Total items processed.
    pub total: usize,
    /// Duration of the batch operation.
    pub duration_ms: u64,
}

impl<T> BatchImportResult<T> {
    pub fn new() -> Self {
        Self {
            succeeded: Vec::new(),
            failed: Vec::new(),
            total: 0,
            duration_ms: 0,
        }
    }

    pub fn add_success(&mut self, result: ImportResult<T>) {
        self.total += 1;
        self.succeeded.push(result);
    }

    pub fn add_failure(&mut self, result: ImportResult<T>) {
        self.total += 1;
        self.failed.push(result);
    }

    pub fn success_count(&self) -> usize {
        self.succeeded.len()
    }

    pub fn failure_count(&self) -> usize {
        self.failed.len()
    }

    pub fn success_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.succeeded.len() as f64) / (self.total as f64) * 100.0
        }
    }
}

impl<T> Default for BatchImportResult<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of an export operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult<T> {
    /// Exported entities.
    pub entities: Vec<T>,
    /// Total count (may differ from entities.len() if paginated).
    pub total_count: usize,
    /// Export format.
    pub format: Format,
    /// Timestamp of export.
    pub exported_at: DateTime<Utc>,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

impl<T> ExportResult<T> {
    pub fn new(entities: Vec<T>, format: Format, duration_ms: u64) -> Self {
        let total_count = entities.len();
        Self {
            entities,
            total_count,
            format,
            exported_at: Utc::now(),
            duration_ms,
        }
    }
}

/// Trait for importing content from external sources.
///
/// # Type Parameters
///
/// * `Source` — Input format (e.g., `serde_json::Value`, CSV row)
/// * `Target` — Domain entity (e.g., `BlogPost`, `Product`)
///
/// # Example
///
/// ```rust,ignore
/// struct BlogPostImporter;
///
/// #[async_trait]
/// impl ContentImporter<serde_json::Value, BlogPost> for BlogPostImporter {
///     async fn import(
///         &self,
///         source: serde_json::Value,
///         context: ImportContext,
///     ) -> Result<ImportResult<BlogPost>, PortabilityError> {
///         // 1. Validate source
///         // 2. Transform to domain entity
///         // 3. Persist
///         // 4. Return result
///         todo!()
///     }
/// }
/// ```
#[async_trait]
pub trait ContentImporter<Source: Send + 'static, Target: Send + 'static>: Send + Sync {
    /// Import a single item.
    async fn import(
        &self,
        source: Source,
        context: ImportContext,
    ) -> Result<ImportResult<Target>, PortabilityError>;

    /// Import a batch of items.
    async fn import_batch(
        &self,
        sources: Vec<Source>,
        context: ImportContext,
    ) -> Result<BatchImportResult<Target>, PortabilityError> {
        let start = std::time::Instant::now();
        let mut result = BatchImportResult::new();

        for (index, source) in sources.into_iter().enumerate() {
            match self.import(source, context.clone()).await {
                Ok(import_result) => {
                    if import_result.success {
                        result.add_success(import_result);
                    } else {
                        result.add_failure(import_result);
                    }
                }
                Err(e) => {
                    let failure =
                        ImportResult::failure(format!("item_{}", index), vec![e.to_string()]);
                    result.add_failure(failure);

                    if !context.continue_on_error {
                        result.duration_ms = start.elapsed().as_millis() as u64;
                        return Err(e);
                    }
                }
            }

            if let Some(ref callback) = context.progress_callback {
                callback(index + 1, result.total);
            }
        }

        result.duration_ms = start.elapsed().as_millis() as u64;
        Ok(result)
    }

    /// Validate source without importing.
    async fn validate(
        &self,
        source: Source,
        context: ImportContext,
    ) -> Result<Vec<String>, PortabilityError>;
}

/// Trait for exporting content to external formats.
///
/// # Type Parameters
///
/// * `Source` — Domain entity (e.g., `BlogPost`, `Product`)
/// * `Target` — Output format (e.g., `serde_json::Value`, CSV row)
///
/// # Example
///
/// ```rust,ignore
/// struct BlogPostExporter;
///
/// #[async_trait]
/// impl ContentExporter<BlogPost, serde_json::Value> for BlogPostExporter {
///     async fn export(
///         &self,
///         source: BlogPost,
///         context: ExportContext,
///     ) -> Result<serde_json::Value, PortabilityError> {
///         // Transform domain entity to target format
///         todo!()
///     }
/// }
/// ```
#[async_trait]
pub trait ContentExporter<Source: Send + 'static, Target: Send + 'static>: Send + Sync {
    /// Export a single item.
    async fn export(
        &self,
        source: Source,
        context: ExportContext,
    ) -> Result<Target, PortabilityError>;

    /// Export a batch of items.
    async fn export_batch(
        &self,
        sources: Vec<Source>,
        context: ExportContext,
    ) -> Result<ExportResult<Target>, PortabilityError> {
        let start = std::time::Instant::now();
        let total = sources.len();
        let mut exported = Vec::with_capacity(total);

        for (index, source) in sources.into_iter().enumerate() {
            match self.export(source, context.clone()).await {
                Ok(target) => exported.push(target),
                Err(e) => {
                    if !context
                        .filters
                        .custom
                        .get("continue_on_error")
                        .map(|v| v == "true")
                        .unwrap_or(false)
                    {
                        return Err(e);
                    }
                    // Log error and continue
                }
            }

            if let Some(ref callback) = context.progress_callback {
                callback(index + 1, total);
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ExportResult::new(exported, context.format, duration_ms))
    }
}
