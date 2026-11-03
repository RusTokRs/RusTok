# Content Portability

Implementation of content import/export services for the RusToK platform.

See also [CRATE_API.md](CRATE_API.md) for formal description of the public API and contract invariants.

## Purpose

This crate provides concrete implementations for importing and exporting content in various formats (JSON, CSV).

## Components

### Format Handlers

#### JsonFormatHandler

JSON format handler:

```rust
use rustok_content_portability::JsonFormatHandler;
use rustok_content_portability_api::FormatOptions;

// Parse single item
let item: BlogPost = JsonFormatHandler::parse(&json_bytes)?;

// Parse array
let items: Vec<BlogPost> = JsonFormatHandler::parse_array(&json_bytes)?;

// Serialize with options
let options = FormatOptions::new().with_json_pretty(true);
let bytes = JsonFormatHandler::serialize(&items, &options)?;
```

#### CsvFormatHandler

CSV format handler:

```rust
use rustok_content_portability::CsvFormatHandler;
use rustok_content_portability_api::FormatOptions;

// Parse with custom delimiter
let options = FormatOptions::new()
    .with_csv_delimiter(b';')
    .with_csv_headers(true);
let items: Vec<Product> = CsvFormatHandler::parse(&csv_bytes, &options)?;

// Serialize
let bytes = CsvFormatHandler::serialize(&items, &options)?;
```

### Services

#### ImportService

Service for file imports:

```rust
use rustok_content_portability::ImportService;
use rustok_content_portability_api::{ImportContext, Format, FormatOptions};

let context = ImportContext::new(tenant_id, user_id, Format::Json);

// Import from JSON file
let posts: Vec<BlogPost> = ImportService::import_json_file(
    "posts.json",
    context.clone(),
).await?;

// Import from CSV file
let options = FormatOptions::new();
let products: Vec<Product> = ImportService::import_csv_file(
    "products.csv",
    context.clone(),
    &options,
).await?;

// Import from bytes
let items: Vec<BlogPost> = ImportService::import_from_bytes(
    &data,
    &context,
    &options,
)?;
```

#### ExportService

Service for file exports:

```rust
use rustok_content_portability::ExportService;
use rustok_content_portability_api::{ExportContext, Format, FormatOptions};

let context = ExportContext::new(tenant_id, user_id, Format::Json);
let options = FormatOptions::new().with_json_pretty(true);

// Export to JSON file
let result = ExportService::export_json_file(
    "export.json",
    &posts,
    context.clone(),
    &options,
).await?;

println!("Exported {} items in {}ms", result.total_count, result.duration_ms);

// Export to CSV file
let csv_options = FormatOptions::new().with_csv_headers(true);
let result = ExportService::export_csv_file(
    "export.csv",
    &products,
    context,
    &csv_options,
).await?;
```

### File I/O Helpers

File utility helpers:

```rust
use rustok_content_portability::{read_file, write_file, file_exists, file_size};

// Read file
let data = read_file("data.json").await?;

// Write file
write_file("output.json", &data).await?;

// Check existence
if file_exists("config.json").await {
    println!("Config exists");
}

// Get size
let size = file_size("data.json").await?;
println!("File size: {} bytes", size);
```

## Example: Full Import/Export Lifecycle

```rust
use rustok_content_portability::{ImportService, ExportService};
use rustok_content_portability_api::{
    ImportContext, ExportContext, Format, FormatOptions,
    ContentImporter, BatchImportResult,
};

// 1. Import from external source
let import_context = ImportContext::new(tenant_id, user_id, Format::Json);
let raw_posts: Vec<RawBlogPost> = ImportService::import_json_file(
    "wordpress_export.json",
    import_context,
).await?;

// 2. Transform and validate using domain importer
let importer = BlogPostImporter;
let transform_context = ImportContext::new(tenant_id, user_id, Format::Json)
    .with_continue_on_error(true);

let result: BatchImportResult<BlogPost> = importer
    .import_batch(raw_posts.into_iter().map(|p| p.to_json()).collect(), transform_context)
    .await?;

println!(
    "Import complete: {}/{} succeeded ({:.1}%)",
    result.success_count(),
    result.total,
    result.success_rate()
);

// 3. Export processed data
let export_context = ExportContext::new(tenant_id, user_id, Format::Csv);
let csv_options = FormatOptions::new().with_csv_headers(true);

let exported_posts: Vec<BlogPost> = result
    .succeeded
    .into_iter()
    .filter_map(|r| r.entity)
    .collect();

ExportService::export_csv_file(
    "processed_posts.csv",
    &exported_posts,
    export_context,
    &csv_options,
).await?;
```

## Integration with Domain Modules

Each domain module implements its own importer/exporter:

### Blog

```rust
pub struct BlogPostImporter {
    repository: Arc<BlogPostRepository>,
}

#[async_trait]
impl ContentImporter<Value, BlogPost> for BlogPostImporter {
    async fn import(
        &self,
        source: Value,
        context: ImportContext,
    ) -> Result<ImportResult<BlogPost>, PortabilityError> {
        // Validate, transform, persist
    }
}

pub struct BlogPostExporter {
    repository: Arc<BlogPostRepository>,
}

#[async_trait]
impl ContentExporter<BlogPost, Value> for BlogPostExporter {
    async fn export(
        &self,
        source: BlogPost,
        context: ExportContext,
    ) -> Result<Value, PortabilityError> {
        // Transform to export format
    }
}
```

### Commerce

```rust
pub struct ProductImporter { /* ... */ }
pub struct ProductExporter { /* ... */ }
```

### Newsletter

```rust
pub struct SubscriberImporter { /* ... */ }
pub struct SubscriberExporter { /* ... */ }
```

## Testing

All components are covered by unit tests:

```bash
cargo test -p rustok-content-portability-api
cargo test -p rustok-content-portability
```

## Performance

- **Batch operations** — High-throughput batch processing with progress tracking
- **Streaming** — Streaming processing planned for multi-gigabyte exports
- **Async I/O** — Non-blocking asynchronous file operations
- **Memory efficient** — Minimal allocation overhead

## Future Enhancements

- [ ] WordPress XML format handler
- [ ] Markdown with frontmatter handler
- [ ] Streaming import/export for large datasets
- [ ] Compression support (gzip, zip)
- [ ] Encryption support for sensitive data
- [ ] Batch size configuration
- [ ] Retry logic for transient import failures
- [ ] Import/export job queue for background workers

## License

Workspace license applies.
