# Content Portability

Реализация сервисов импорта/экспорта контента для платформы RusTok.

## Назначение

Этот crate предоставляет конкретные реализации для импорта и экспорта контента в различных форматах (JSON, CSV).

## Компоненты

### Format Handlers

#### JsonFormatHandler

Обработчик JSON формата:

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

Обработчик CSV формата:

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

Сервис для импорта из файлов:

```rust
use rustok_content_portability::ImportService;
use rustok_content_portability_api::{ImportContext, Format};

let context = ImportContext::new(tenant_id, user_id, Format::Json);

// Import from JSON file
let posts: Vec<BlogPost> = ImportService::import_json_file(
    "posts.json",
    context,
).await?;

// Import from CSV file
let options = FormatOptions::new();
let products: Vec<Product> = ImportService::import_csv_file(
    "products.csv",
    context,
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

Сервис для экспорта в файлы:

```rust
use rustok_content_portability::ExportService;
use rustok_content_portability_api::{ExportContext, Format, FormatOptions};

let context = ExportContext::new(tenant_id, user_id, Format::Json);
let options = FormatOptions::new().with_json_pretty(true);

// Export to JSON file
let result = ExportService::export_json_file(
    "export.json",
    &posts,
    context,
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

Утилиты для работы с файлами:

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

## Пример: Полный цикл импорта

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

## Интеграция с модулями

Каждый доменный модуль реализует свои importer/exporter:

### Blog

```rust
// rustok-blog/src/import_export/mod.rs
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
// rustok-commerce/src/import_export/mod.rs
pub struct ProductImporter { /* ... */ }
pub struct ProductExporter { /* ... */ }
```

### Newsletter

```rust
// rustok-newsletter/src/import_export/mod.rs
pub struct SubscriberImporter { /* ... */ }
pub struct SubscriberExporter { /* ... */ }
```

## Тестирование

Все компоненты покрыты unit тестами:

```bash
cargo test -p rustok-content-portability-api
cargo test -p rustok-content-portability
```

## Производительность

- **Batch operations** — обработка больших объемов данных с progress tracking
- **Streaming** — поддержка потоковой обработки (планируется)
- **Async I/O** — неблокирующие операции с файлами
- **Memory efficient** — минимальное использование памяти

## Будущие улучшения

- [ ] WordPress XML format handler
- [ ] Markdown with frontmatter handler
- [ ] Streaming import/export для больших файлов
- [ ] Compression support (gzip, zip)
- [ ] Encryption support для sensitive data
- [ ] Batch size configuration
- [ ] Retry logic для failed imports
- [ ] Import/export job queue для background processing

## Лицензия

Workspace license applies.
