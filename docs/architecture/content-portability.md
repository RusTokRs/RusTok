# Content Portability Platform Architecture

**Дата:** 2026-10-09  
**Статус:** ✅ Реализовано  
**Тип:** Platform capability (support crate)

## Обзор

Content Portability — это платформенная capability для унифицированного импорта и экспорта контента across all modules.

## Архитектурное решение

### Почему не отдельный модуль?

Content Portability — это **support crate**, а не tenant-toggled module:

1. **Cross-cutting concern** — используется всеми модулями (blog, forum, commerce, newsletter)
2. **Infrastructure layer** — предоставляет инструменты, а не бизнес-логику
3. **Always available** — не требует включения/выключения на уровне tenant
4. **No own tables** — не владеет persistent data (модули используют свои таблицы)

### Архитектурный паттерн

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

### Аналоги в платформе

Content Portability следует тому же паттерну, что и другие support crates:

- `rustok-api` — stable contracts (PortContext, PortError)
- `rustok-core` — core traits and types
- `rustok-runtime` — runtime helpers
- `rustok-web` — Axum helpers
- `rustok-fba` — FBA metadata

Все они:
- Не являются tenant-toggled modules
- Не имеют записи в `modules.toml`
- Предоставляют infrastructure, а не business logic
- Используются domain modules как зависимости

## Ключевые компоненты

### 1. ContentImporter trait

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
- `Source` — input format (e.g., `serde_json::Value`, CSV row)
- `Target` — domain entity (e.g., `BlogPost`, `Product`)

**Default implementation:**
- `import_batch` — итеративно вызывает `import` с progress tracking
- `continue_on_error` — опция продолжать при ошибках

### 2. ContentExporter trait

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

### 3. Format descriptors

```rust
pub enum Format {
    Json,
    Csv,
    WordPressXml,
    Markdown,
    Custom,
}
```

**Методы:**
- `extension()` — file extension (.json, .csv, etc.)
- `mime_type()` — MIME type
- `supports_batch()` — native batch support
- `is_human_readable()` — human-readable format

### 4. Context types

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

### 5. Result types

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

## Use cases

### 1. WordPress migration

```rust
// Импорт постов из WordPress XML
let context = ImportContext::new(tenant_id, user_id, Format::WordPressXml);
let importer = BlogPostImporter::new(repository);

let wp_posts = read_wordpress_export("wordpress.xml")?;
let result = importer.import_batch(wp_posts, context).await?;

println!("Imported {}/{} posts", result.success_count(), result.total);
```

### 2. CSV product upload

```rust
// Импорт товаров из CSV
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

### 3. Demo data seeding

```rust
// Загрузка демо-данных для нового tenant
let demo_context = ImportContext::new(tenant_id, system_user_id, Format::Json);

// Демо-посты
let demo_posts = load_demo_data("seeds/blog_posts.json")?;
let blog_importer = BlogPostImporter::new(blog_repo);
blog_importer.import_batch(demo_posts, demo_context.clone()).await?;

// Демо-товары
let demo_products = load_demo_data("seeds/products.json")?;
let product_importer = ProductImporter::new(product_service);
product_importer.import_batch(demo_products, demo_context.clone()).await?;

// Демо-подписчики
let demo_subscribers = load_demo_data("seeds/subscribers.json")?;
let subscriber_importer = SubscriberImporter::new(newsletter_service);
subscriber_importer.import_batch(demo_subscribers, demo_context).await?;
```

### 4. Data export for backup

```rust
// Экспорт всех данных tenant для backup
let export_context = ExportContext::new(tenant_id, admin_user_id, Format::Json);
let options = FormatOptions::new().with_json_pretty(true);

// Экспорт постов
let posts = blog_repo.find_all(tenant_id).await?;
ExportService::export_json_file(
    "backup/posts.json",
    &posts,
    export_context.clone(),
    &options,
).await?;

// Экспорт товаров
let products = product_service.list_all(tenant_id).await?;
ExportService::export_json_file(
    "backup/products.json",
    &products,
    export_context.clone(),
    &options,
).await?;
```

## Интеграция с существующими системами

### rustok-modules::data::export

Content Portability работает **поверх** низкоуровневого artifact data export:

```
rustok-modules::data::export  ← artifact data (structured JSON)
  ↓
rustok-content-portability    ← business entities (posts, products)
  ↓
Domain modules                ← domain-specific logic
```

- `rustok-modules::data::export` — экспорт module artifacts (metadata, config)
- `rustok-content-portability` — экспорт business entities (posts, products, subscribers)

### rustok-forum import/export

Forum уже имеет полную import/export систему для NodeBB migration. Content Portability может **обернуть** существующую логику:

```rust
// Wrapper для существующего forum importer
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
        // Delegate to existing forum import logic
        self.inner.import_topic(source, context.tenant_id).await
    }
}
```

## Преимущества архитектуры

### 1. Единый API

Все модули используют одинаковый интерфейс:
- Одинаковые context types
- Одинаковые result types
- Одинаковые error handling
- Одинаковые progress tracking

### 2. Переиспользование кода

- Format handlers (JSON, CSV) написаны один раз
- File I/O helpers общие для всех
- Batch processing logic общая
- Validation framework общий

### 3. Расширяемость

- Легко добавить новые форматы (WordPress XML, Markdown)
- Легко добавить новые модули (blog, commerce, newsletter)
- Легко добавить новые features (compression, encryption)

### 4. Тестируемость

- Traits легко mock'аются
- Format handlers изолированы
- Services можно тестировать независимо

### 5. Производительность

- Async I/O для файловых операций
- Batch processing для больших объемов
- Progress tracking для long-running operations
- Memory-efficient streaming (планируется)

## Ограничения

### 1. Форматы

Сейчас реализованы только JSON и CSV. WordPress XML и Markdown планируются.

### 2. Streaming

Batch processing загружает все данные в память. Streaming для больших файлов планируется.

### 3. Transaction management

Import operations не оборачиваются в глобальную транзакцию. Каждый item импортируется независимо. Это позволяет `continue_on_error`, но означает partial failures.

### 4. Deduplication

Content Portability не предоставляет built-in deduplication. Domain modules должны сами решать, как обрабатывать дубликаты (по slug, external_id, etc.).

## Будущие улучшения

### Phase 2: Additional formats
- [ ] WordPress XML handler
- [ ] Markdown with frontmatter handler
- [ ] RSS/Atom feed handler

### Phase 3: Advanced features
- [ ] Streaming import/export для больших файлов
- [ ] Compression support (gzip, zip)
- [ ] Encryption support для sensitive data
- [ ] Batch size configuration
- [ ] Retry logic для failed imports

### Phase 4: Background processing
- [ ] Import/export job queue
- [ ] Background workers
- [ ] Progress persistence
- [ ] Job cancellation
- [ ] Job history

### Phase 5: UI
- [ ] Admin UI для import/export
- [ ] File upload/download
- [ ] Progress visualization
- [ ] Error reporting
- [ ] Job management

## Заключение

Content Portability предоставляет унифицированную платформу для импорта и экспорта контента across all modules. Архитектура следует established patterns платформы (support crate, traits, generic implementations) и обеспечивает:

- ✅ Единый API для всех модулей
- ✅ Переиспользование кода
- ✅ Расширяемость
- ✅ Тестируемость
- ✅ Производительность

Система готова к использованию и может быть расширена новыми форматами и features в будущем.
