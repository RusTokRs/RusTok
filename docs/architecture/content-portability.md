# Content Portability Platform Architecture

**Date:** 2026-10-09  
**Status:** ✅ Implemented  
**Type:** Platform capability (support crate)

## Overview

Content Portability is a platform capability providing unified content import and export across all modules.

## Architectural Decision

### Why not a separate module?

Content Portability is a **support crate**, not a tenant-toggled module:

1. **Cross-cutting concern** — Used across domain modules (blog, forum, commerce, newsletter).
2. **Infrastructure layer** — Provides utilities and format serialization, not business rules.
3. **Always available** — Does not require tenant-level activation/deactivation.
4. **No own tables** — Does not own persistent database tables (domain modules own their schemas).

### Architectural Pattern

```
rustok-content-portability-api  ← contracts (traits, types)
  ├─ ContentImporter<Source, Target>
  ├─ ContentExporter<Source, Target>
  ├─ Format descriptors
  └─ Batch/progress/validation contracts

rustok-content-portability      ← implementation
  ├─ ImportService, ExportService
  ├─ JsonFormatHandler, CsvFormatHandler
  └─ File I/O helpers

Domain modules                  ← domain-specific implementations
  ├─ rustok-blog       → BlogPostImporter, BlogPostExporter
  ├─ rustok-forum      → ForumTopicImporter (NodeBB migration)
  ├─ rustok-commerce   → ProductImporter, ProductExporter
  └─ rustok-newsletter → SubscriberImporter, SubscriberExporter
```

### Analogues in the Platform

Content Portability follows the exact pattern established by other foundation crates:

- `rustok-api` — stable contracts (PortContext, PortError)
- `rustok-core` — core traits and types
- `rustok-runtime` — runtime helpers
- `rustok-web` — Axum helpers
- `rustok-fba` — FBA metadata

All of these:
- Are not tenant-toggled modules.
- Have no entry in `modules.toml`.
- Provide infrastructure rather than sovereign business logic.
- Are consumed by domain modules as direct dependencies.

## Key Components

### 1. ContentImporter Trait

```rust
#[async_trait]
pub trait ContentImporter<Source, Target>: Send + Sync {
    async fn import(
        &self,
        source: Source,
        context: ImportContext,
    ) -> Result<ImportResult<Target>, PortabilityError>;

    async fn import_batch(
        &self,
        sources: Vec<Source>,
        context: ImportContext,
    ) -> Result<BatchImportResult<Target>, PortabilityError>;

    async fn validate(
        &self,
        source: Source,
        context: ImportContext,
    ) -> Result<Vec<String>, PortabilityError>;
}
```

**Generic parameters:**
- `Source` — input format representation (e.g., `serde_json::Value`, CSV row)
- `Target` — domain entity (e.g., `BlogPost`, `Product`)

**Default implementation:**
- `import_batch` — Iteratively invokes `import` with progress tracking.
- `continue_on_error` — Configuration flag to skip invalid rows and collect partial successes.

### 2. ContentExporter Trait

```rust
#[async_trait]
pub trait ContentExporter<Source, Target>: Send + Sync {
    async fn export(
        &self,
        source: Source,
        context: ExportContext,
    ) -> Result<Target, PortabilityError>;

    async fn export_batch(
        &self,
        sources: Vec<Source>,
        context: ExportContext,
    ) -> Result<ExportResult<Target>, PortabilityError>;
}
```

**Generic parameters:**
- `Source` — domain entity
- `Target` — output format

### 3. Format Descriptors

```rust
pub enum Format {
    Json,
    Csv,
    WordPressXml,
    Markdown,
    Custom,
}
```

**Methods:**
- `extension()` — file extension (.json, .csv, etc.)
- `mime_type()` — MIME type
- `supports_batch()` — native batch support
- `is_human_readable()` — human-readable format flag

### 4. Context Types

**ImportContext:**
```rust
pub struct ImportContext {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub format: Format,
    pub continue_on_error: bool,
    pub progress_callback: Option<ProgressCallback>,
    pub started_at: DateTime<Utc>,
}
```

**ExportContext:**
```rust
pub struct ExportContext {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub format: Format,
    pub filters: ExportFilters,
    pub progress_callback: Option<ProgressCallback>,
    pub started_at: DateTime<Utc>,
}
```

### 5. Result Types

**ImportResult<T>:**
```rust
pub struct ImportResult<T> {
    pub entity: Option<T>,
    pub source_id: String,
    pub success: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub imported_at: DateTime<Utc>,
}
```

**BatchImportResult<T>:**
```rust
pub struct BatchImportResult<T> {
    pub succeeded: Vec<ImportResult<T>>,
    pub failed: Vec<ImportResult<T>>,
    pub total: usize,
    pub duration_ms: u64,
}
```

**ExportResult<T>:**
```rust
pub struct ExportResult<T> {
    pub entities: Vec<T>,
    pub total_count: usize,
    pub format: Format,
    pub exported_at: DateTime<Utc>,
    pub duration_ms: u64,
}
```

## Use Cases

### 1. WordPress Migration

```rust
// Import posts from WordPress XML
let context = ImportContext::new(tenant_id, user_id, Format::WordPressXml);
let importer = BlogPostImporter::new(repository);

let wp_posts = read_wordpress_export("wordpress.xml")?;
let result = importer.import_batch(wp_posts, context).await?;

println!("Imported {}/{} posts", result.success_count(), result.total);
```

### 2. CSV Product Upload

```rust
// Import catalog products from CSV
let context = ImportContext::new(tenant_id, user_id, Format::Csv);
let options = FormatOptions::new()
    .with_csv_delimiter(b',')
    .with_csv_headers(true);

let raw_products: Vec<RawProduct> = ImportService::import_csv_file(
    "products.csv",
    context.clone(),
    &options,
).await?;

let importer = ProductImporter::new(product_service);
let result = importer.import_batch(raw_products, context).await?;
```

### 3. Demo Data Seeding

```rust
// Seed initial dataset for a newly created tenant
let demo_context = ImportContext::new(tenant_id, system_user_id, Format::Json);

// Demo posts
let demo_posts = load_demo_data("seeds/blog_posts.json")?;
let blog_importer = BlogPostImporter::new(blog_repo);
blog_importer.import_batch(demo_posts, demo_context.clone()).await?;

// Demo products
let demo_products = load_demo_data("seeds/products.json")?;
let product_importer = ProductImporter::new(product_service);
product_importer.import_batch(demo_products, demo_context.clone()).await?;

// Demo subscribers
let demo_subscribers = load_demo_data("seeds/subscribers.json")?;
let subscriber_importer = SubscriberImporter::new(newsletter_service);
subscriber_importer.import_batch(demo_subscribers, demo_context).await?;
```

### 4. Data Export for Backup

```rust
// Export complete tenant dataset for offline backup
let export_context = ExportContext::new(tenant_id, admin_user_id, Format::Json);
let options = FormatOptions::new().with_json_pretty(true);

// Export posts
let posts = blog_repo.find_all(tenant_id).await?;
ExportService::export_json_file(
    "backup/posts.json",
    &posts,
    export_context.clone(),
    &options,
).await?;

// Export products
let products = product_service.list_all(tenant_id).await?;
ExportService::export_json_file(
    "backup/products.json",
    &products,
    export_context.clone(),
    &options,
).await?;
```

## Integration with Existing Systems

### rustok-modules::data::export

Content Portability operates on top of low-level module artifact export:

```
rustok-modules::data::export  ← artifact data (structured JSON)
  ↓
rustok-content-portability    ← business entities (posts, products)
  ↓
Domain modules                ← domain-specific logic
```

- `rustok-modules::data::export` — Exports module configuration and metadata artifacts.
- `rustok-content-portability` — Exports business domain entities (posts, products, subscribers).

### rustok-forum Import/Export

The Forum module maintains its own specialized import/export pipeline for NodeBB migrations. Content Portability wraps that existing domain logic:

```rust
pub struct ForumTopicImporterAdapter {
    inner: ForumImportService,
}

#[async_trait]
impl ContentImporter<Value, ForumTopic> for ForumTopicImporterAdapter {
    async fn import(
        &self,
        source: Value,
        context: ImportContext,
    ) -> Result<ImportResult<ForumTopic>, PortabilityError> {
        self.inner.import_topic(source, context.tenant_id).await
    }
}
```

## Architectural Advantages

1. **Unified API** — All modules share identical context types, result types, error handling, and progress reporting.
2. **Code Reuse** — Format parsers (JSON, CSV), file I/O utilities, batch processing, and validation frameworks are authored once.
3. **Extensibility** — New formats (WordPress XML, Markdown) and domain consumers can be registered additively without breaking changes.
4. **Testability** — Clean generic traits allow simple unit-level mocking without file system overhead.
5. **Performance** — Non-blocking async I/O with bounded memory footprint during batch execution.

## Current Limitations

1. **Formats** — Initial support is restricted to JSON and CSV. WordPress XML and Markdown are planned for subsequent phases.
2. **Streaming** — In-memory batch processing is standard; streaming for multi-gigabyte exports is scheduled for Phase 3.
3. **Transaction Management** — Import operations are per-item rather than globally transactional, supporting partial success via `continue_on_error`.
4. **Deduplication** — Built-in deduplication is domain-owned: each module decides how to resolve key conflicts (by slug, external ID, etc.).

## Future Roadmap

- **Phase 2: Additional Formats** — WordPress XML, Markdown with frontmatter, RSS/Atom feeds.
- **Phase 3: High-throughput Features** — Streaming I/O, gzip/zip compression, envelope encryption for sensitive fields.
- **Phase 4: Background Processing** — Asynchronous job queues, durable progress checkpointing, cancellation tokens.
- **Phase 5: Administration UI** — Upload/download panels, execution visualization, error inspection.

## Conclusion

Content Portability provides a unified, production-grade foundation for importing and exporting content across all RusToK domain modules while respecting sovereign module boundaries and platform foundation layering.
