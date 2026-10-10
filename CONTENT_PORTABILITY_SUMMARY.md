# Content Portability — Implementation Summary

**Дата:** 2026-10-09  
**Статус:** ✅ Завершено (Phase 1)  
**Тип:** Platform capability

## Обзор

Создана унифицированная платформа для импорта/экспорта контента across all modules платформы RusTok.

## Архитектурное решение

**Ключевой вопрос:** Где разместить функционал import/export?

**Решение:** Platform capability (support crate), НЕ отдельный модуль.

**Обоснование:**
1. **Cross-cutting concern** — используется всеми модулями (blog, forum, commerce, newsletter)
2. **Infrastructure layer** — предоставляет инструменты, а не бизнес-логику
3. **Always available** — не требует включения/выключения на уровне tenant
4. **No own tables** — не владеет persistent data
5. **Follows platform pattern** — как `rustok-api`, `rustok-core`, `rustok-runtime`

## Созданные артефакты

### Crates

#### `rustok-content-portability-api` (contracts)

**Путь:** `crates/libs/rustok-content-portability-api/`

**Структура:**
```
src/
  lib.rs           — crate facade, re-exports
  contracts.rs     — ContentImporter, ContentExporter traits
  formats.rs       — Format enum, FormatOptions
  errors.rs        — PortabilityError enum
  progress.rs      — ProgressCallback, ProgressReporter
  validation.rs    — ValidationResult, FieldValidation, validate_fields! macro
```

**Ключевые контракты:**

1. **ContentImporter<Source, Target>**
   - `import(source, context)` — import single item
   - `import_batch(sources, context)` — import multiple items (default impl)
   - `validate(source, context)` — validate without importing

2. **ContentExporter<Source, Target>**
   - `export(source, context)` — export single item
   - `export_batch(sources, context)` — export multiple items (default impl)

3. **Format enum**
   - `Json` — JSON format
   - `Csv` — CSV format
   - `WordPressXml` — WordPress XML (placeholder)
   - `Markdown` — Markdown with frontmatter (placeholder)
   - `Custom` — custom formats

4. **Context types**
   - `ImportContext` — tenant_id, user_id, format, continue_on_error, progress_callback
   - `ExportContext` — tenant_id, user_id, format, filters, progress_callback

5. **Result types**
   - `ImportResult<T>` — single item import result (success, errors, warnings)
   - `BatchImportResult<T>` — batch import result (succeeded, failed, statistics)
   - `ExportResult<T>` — export result (entities, total_count, duration)

6. **Validation framework**
   - `ValidationResult` — collection of field validations
   - `FieldValidation` — single field validation (valid/invalid)
   - `validate_fields!` — macro for building validation results

**Тесты:** ✅ Unit tests для всех компонентов

#### `rustok-content-portability` (implementation)

**Путь:** `crates/libs/rustok-content-portability/`

**Структура:**
```
src/
  lib.rs                    — crate facade, re-exports
  formats/
    mod.rs
    json_handler.rs        — JSON parse/serialize
    csv_handler.rs         — CSV parse/serialize
  io.rs                    — file I/O helpers
  services.rs              — ImportService, ExportService
```

**Ключевые компоненты:**

1. **JsonFormatHandler**
   - `parse<T>(data)` — parse single item
   - `parse_array<T>(data)` — parse array
   - `serialize<T>(value, options)` — serialize to bytes
   - `serialize_array<T>(values, options)` — serialize array
   - Options: `json_pretty` (pretty-print)

2. **CsvFormatHandler**
   - `parse<T>(data, options)` — parse CSV
   - `serialize<T>(values, options)` — serialize to CSV
   - Options: `csv_delimiter`, `csv_quote`, `csv_include_headers`

3. **ImportService**
   - `import_json_file<T>(path, context)` — import from JSON file
   - `import_csv_file<T>(path, context, options)` — import from CSV file
   - `import_from_bytes<T>(data, context, options)` — import from bytes

4. **ExportService**
   - `export_json_file<T>(path, items, context, options)` — export to JSON file
   - `export_csv_file<T>(path, items, context, options)` — export to CSV file
   - `export_to_bytes<T>(items, context, options)` — export to bytes

5. **File I/O helpers**
   - `read_file(path)` — read file as bytes
   - `write_file(path, data)` — write bytes to file
   - `file_exists(path)` — check file existence
   - `file_size(path)` — get file size

**Тесты:** ✅ Unit tests для всех компонентов + integration tests с tempfile

### Документация

1. **README.md** для каждого crate:
   - `rustok-content-portability-api/README.md` — contracts, examples, use cases
   - `rustok-content-portability/README.md` — implementation details, examples

2. **Architecture document:**
   - `docs/architecture/content-portability.md` — architectural decision, rationale, examples

### Workspace интеграция

✅ Добавлено в `Cargo.toml`:
```toml
rustok-content-portability-api = { path = "crates/libs/rustok-content-portability-api" }
rustok-content-portability = { path = "crates/libs/rustok-content-portability" }
```

✅ Автоматически включены в workspace через `crates/libs/*`

## Примеры использования

### 1. Blog post import из WordPress

```rust
use rustok_content_portability::ImportService;
use rustok_content_portability_api::{ImportContext, Format, ContentImporter};

// 1. Load WordPress XML (future handler)
let context = ImportContext::new(tenant_id, user_id, Format::WordPressXml);
let wp_posts = load_wordpress_export("wordpress.xml")?;

// 2. Transform using domain importer
let importer = BlogPostImporter::new(blog_repo);
let result = importer.import_batch(wp_posts, context).await?;

println!("Imported {}/{} posts ({:.1}% success rate)",
    result.success_count(),
    result.total,
    result.success_rate()
);
```

### 2. CSV product upload

```rust
use rustok_content_portability::{ImportService, CsvFormatHandler};
use rustok_content_portability_api::{ImportContext, Format, FormatOptions};

let context = ImportContext::new(tenant_id, user_id, Format::Csv);
let options = FormatOptions::new()
    .with_csv_delimiter(b',')
    .with_csv_headers(true);

// Parse CSV
let raw_products: Vec<RawProduct> = ImportService::import_csv_file(
    "products.csv",
    context.clone(),
    &options,
).await?;

// Transform and persist
let importer = ProductImporter::new(product_service);
let result = importer.import_batch(raw_products, context).await?;
```

### 3. Demo data seeding

```rust
use rustok_content_portability::ImportService;
use rustok_content_portability_api::{ImportContext, Format};

let demo_context = ImportContext::new(tenant_id, system_user_id, Format::Json);

// Seed blog posts
let demo_posts = load_demo_data("seeds/blog_posts.json")?;
let blog_importer = BlogPostImporter::new(blog_repo);
blog_importer.import_batch(demo_posts, demo_context.clone()).await?;

// Seed products
let demo_products = load_demo_data("seeds/products.json")?;
let product_importer = ProductImporter::new(product_service);
product_importer.import_batch(demo_products, demo_context.clone()).await?;

// Seed newsletter subscribers
let demo_subscribers = load_demo_data("seeds/subscribers.json")?;
let subscriber_importer = SubscriberImporter::new(newsletter_service);
subscriber_importer.import_batch(demo_subscribers, demo_context).await?;
```

### 4. Data export для backup

```rust
use rustok_content_portability::ExportService;
use rustok_content_portability_api::{ExportContext, Format, FormatOptions};

let export_context = ExportContext::new(tenant_id, admin_user_id, Format::Json);
let options = FormatOptions::new().with_json_pretty(true);

// Export all blog posts
let posts = blog_repo.find_all(tenant_id).await?;
ExportService::export_json_file(
    "backup/posts.json",
    &posts,
    export_context.clone(),
    &options,
).await?;

// Export all products
let products = product_service.list_all(tenant_id).await?;
ExportService::export_json_file(
    "backup/products.json",
    &products,
    export_context,
    &options,
).await?;
```

## Интеграция с существующими системами

### rustok-modules::data::export

Content Portability работает **поверх** низкоуровневого artifact data export:

- `rustok-modules::data::export` — экспорт module artifacts (metadata, config)
- `rustok-content-portability` — экспорт business entities (posts, products, subscribers)

### rustok-forum import/export

Forum уже имеет полную import/export систему для NodeBB migration. Content Portability может **обернуть** существующую логику через adapter pattern:

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
        // Delegate to existing forum import logic
        self.inner.import_topic(source, context.tenant_id).await
    }
}
```

## Преимущества архитектуры

### 1. Единый API
- Одинаковые context types для всех модулей
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

## Ограничения (Phase 1)

### 1. Форматы
✅ JSON — реализован  
✅ CSV — реализован  
⏳ WordPress XML — placeholder  
⏳ Markdown — placeholder  

### 2. Streaming
Batch processing загружает все данные в память. Streaming для больших файлов планируется в Phase 3.

### 3. Transaction management
Import operations не оборачиваются в глобальную транзакцию. Каждый item импортируется независимо (позволяет `continue_on_error`, но означает partial failures).

### 4. Deduplication
Content Portability не предоставляет built-in deduplication. Domain modules должны сами решать, как обрабатывать дубликаты.

## Roadmap

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

## Статистика

**Создано файлов:** 14
- API crate: 6 файлов (lib.rs, contracts.rs, formats.rs, errors.rs, progress.rs, validation.rs)
- Implementation crate: 6 файлов (lib.rs, formats/mod.rs, json_handler.rs, csv_handler.rs, io.rs, services.rs)
- Documentation: 3 файла (2 README.md, 1 architecture doc)

**Строки кода:** ~2,500
- API crate: ~1,200 строк
- Implementation crate: ~1,300 строк

**Тесты:** ✅ Все компоненты покрыты unit tests

## Заключение

Content Portability предоставляет унифицированную платформу для импорта и экспорта контента across all modules. Архитектура следует established patterns платформы (support crate, traits, generic implementations) и обеспечивает:

- ✅ Единый API для всех модулей
- ✅ Переиспользование кода
- ✅ Расширяемость
- ✅ Тестируемость
- ✅ Производительность

**Phase 1 завершен.** Система готова к использованию и может быть расширена новыми форматами и features в будущем.

## Следующие шаги

1. **Domain module implementations:**
   - Blog: `BlogPostImporter`, `BlogPostExporter`
   - Commerce: `ProductImporter`, `ProductExporter`
   - Newsletter: `SubscriberImporter`, `SubscriberExporter`
   - Forum: adapter для существующей NodeBB import logic

2. **Demo data integration:**
   - Создать seed data файлы для каждого модуля
   - Интегрировать с tenant provisioning workflow

3. **Phase 2 formats:**
   - WordPress XML handler
   - Markdown with frontmatter handler
