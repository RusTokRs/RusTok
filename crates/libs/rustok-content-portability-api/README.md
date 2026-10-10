# Content Portability API

Унифицированные контракты для импорта/экспорта контента в платформе RusTok.

## Назначение

Этот crate предоставляет domain-agnostic контракты для импорта и экспорта бизнес-сущностей (посты, товары, подписчики и т.д.) в различных форматах (JSON, CSV, WordPress XML, Markdown).

## Архитектура

```
rustok-content-portability-api  ← контракты (этот crate)
  ├─ ContentImporter trait
  ├─ ContentExporter trait
  ├─ Format descriptors (JSON, CSV, WordPress XML, Markdown)
  ├─ Batch/progress contracts
  └─ Validation framework

rustok-content-portability      ← реализация
  ├─ Services (ImportService, ExportService)
  ├─ Format handlers (JSON, CSV)
  └─ File I/O helpers

Domain modules реализуют:
  rustok-blog       → BlogPostImporter, BlogPostExporter
  rustok-forum      → ForumTopicImporter (уже есть для NodeBB)
  rustok-commerce   → ProductImporter, ProductExporter
  rustok-newsletter → SubscriberImporter, SubscriberExporter
```

## Основные концепции

### ContentImporter

Трейт для импорта контента из внешних источников:

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

### ContentExporter

Трейт для экспорта контента во внешние форматы:

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

### Форматы

Поддерживаемые форматы:

- **JSON** — универсальный формат
- **CSV** — табличные данные
- **WordPress XML** — импорт из WordPress (планируется)
- **Markdown** — с frontmatter (планируется)
- **Custom** — пользовательские форматы

### Контекст операций

- **ImportContext** — tenant_id, user_id, format, continue_on_error, progress_callback
- **ExportContext** — tenant_id, user_id, format, filters, progress_callback

### Результаты

- **ImportResult<T>** — результат импорта одного элемента (success, errors, warnings)
- **BatchImportResult<T>** — результат пакетного импорта (succeeded, failed, statistics)
- **ExportResult<T>** — результат экспорта (entities, total_count, duration)

### Валидация

Фреймворк для валидации импортируемых данных:

```rust
let result = validate_fields! {
    "title" => title.is_empty() => "title is required",
    "email" => !email.contains('@') => "invalid email format",
    "age" => age < 0 => "age cannot be negative"
};

result.into_portability_error()?;
```

### Progress tracking

Callback для отслеживания прогресса batch операций:

```rust
let callback = Arc::new(|current: usize, total: usize| {
    println!("Progress: {}/{} ({:.1}%)", current, total, (current as f64 / total as f64) * 100.0);
});

let context = ImportContext::new(tenant_id, user_id, Format::Json)
    .with_progress_callback(callback);
```

## Пример использования

```rust
use rustok_content_portability_api::{
    ContentImporter, ImportContext, ImportResult, Format,
};

struct BlogPostImporter;

#[async_trait]
impl ContentImporter<serde_json::Value, BlogPost> for BlogPostImporter {
    async fn import(
        &self,
        source: serde_json::Value,
        context: ImportContext,
    ) -> Result<ImportResult<BlogPost>, PortabilityError> {
        // 1. Validate source
        let errors = self.validate(source.clone(), context.clone()).await?;
        if !errors.is_empty() {
            return Ok(ImportResult::failure(
                source["id"].to_string(),
                errors,
            ));
        }

        // 2. Transform to domain entity
        let post = BlogPost {
            title: source["title"].as_str().unwrap().to_string(),
            content: source["content"].as_str().unwrap().to_string(),
            // ...
        };

        // 3. Persist
        // self.repository.save(&post).await?;

        // 4. Return result
        Ok(ImportResult::success(post, source["id"].to_string()))
    }

    async fn validate(
        &self,
        source: serde_json::Value,
        _context: ImportContext,
    ) -> Result<Vec<String>, PortabilityError> {
        let mut errors = Vec::new();
        if source["title"].is_null() {
            errors.push("title is required".to_string());
        }
        Ok(errors)
    }
}
```

## Интеграция с демо-данными

Content portability API используется для загрузки seed data:

```rust
// Загрузка демо-постов из JSON
let context = ImportContext::new(tenant_id, system_user_id, Format::Json);
let importer = BlogPostImporter;
let demo_posts = load_demo_data("blog_posts.json")?;
let result = importer.import_batch(demo_posts, context).await?;

println!("Imported {}/{} demo posts", result.success_count(), result.total);
```

## Лицензия

Workspace license applies.
