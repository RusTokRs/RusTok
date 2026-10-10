# Best Practices Guide

Руководство по лучшим практикам использования rustok-revisions.

## Содержание

1. [Modeling Best Practices](#modeling-best-practices)
2. [Performance Best Practices](#performance-best-practices)
3. [Error Handling Best Practices](#error-handling-best-practices)
4. [Testing Best Practices](#testing-best-practices)
5. [Security Best Practices](#security-best-practices)
6. [Migration Best Practices](#migration-best-practices)
7. [Monitoring Best Practices](#monitoring-best-practices)
8. [Operational Best Practices](#operational-best-practices)

## Modeling Best Practices

### 1. Правильно определяйте tracked поля

**✅ Хорошо:**

```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]
    title: String,           // Важно для истории
    
    #[revision(tracked)]
    content: String,         // Важно для истории
    
    #[revision(tracked)]
    status: PostStatus,      // Важно для истории
    
    #[revision(ignored)]
    updated_at: DateTime<Utc>,  // Не важно для истории
    
    #[revision(ignored)]
    view_count: i32,         // Часто меняется, не важно
}
```

**❌ Плохо:**

```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]
    title: String,
    
    #[revision(tracked)]
    updated_at: DateTime<Utc>,  // ❌ Создает много ревизий
    
    #[revision(tracked)]
    view_count: i32,            // ❌ Создает много ревизий
    
    #[revision(tracked)]
    last_viewed_at: DateTime<Utc>,  // ❌ Создает много ревизий
}
```

**Правило:** Отслеживайте только те поля, которые важны для бизнес-логики и которые меняются нечасто.

### 2. Используйте правильные типы данных

**✅ Хорошо:**

```rust
#[derive(Revisionable)]
struct Product {
    #[revision(tracked)]
    name: String,
    
    #[revision(tracked)]
    price: Decimal,           // ✅ Точность для денег
    
    #[revision(tracked)]
    tags: Vec<String>,        // ✅ JSON-совместимый тип
    
    #[revision(tracked)]
    metadata: serde_json::Value,  // ✅ Гибкий JSON
}
```

**❌ Плохо:**

```rust
#[derive(Revisionable)]
struct Product {
    #[revision(tracked)]
    name: String,
    
    #[revision(tracked)]
    price: f64,               // ❌ Неточность для денег
    
    #[revision(tracked)]
    created_at: DateTime<Utc>,  // ❌ Не нужно отслеживать
}
```

### 3. Группируйте связанные изменения

**✅ Хорошо:**

```rust
async fn update_post(
    service: &RevisionService,
    post: &mut Post,
    new_title: String,
    new_content: String,
) -> Result<(), RevisionError> {
    // Обновляем все поля
    post.title = new_title;
    post.content = new_content;
    post.updated_at = Utc::now();
    
    // Создаем ОДНУ ревизию для всех изменений
    service.create_revision::<Post>(
        post,
        &post.id,
        None,
        Some("user-123"),
        Some("web"),
        Some("Updated title and content"),
    ).await?;
    
    Ok(())
}
```

**❌ Плохо:**

```rust
async fn update_post(
    service: &RevisionService,
    post: &mut Post,
    new_title: String,
    new_content: String,
) -> Result<(), RevisionError> {
    post.title = new_title;
    post.updated_at = Utc::now();
    
    // ❌ Создаем ревизию для каждого изменения
    service.create_revision::<Post>(post, &post.id, ...).await?;
    
    post.content = new_content;
    post.updated_at = Utc::now();
    
    // ❌ Еще одна ревизия
    service.create_revision::<Post>(post, &post.id, ...).await?;
    
    Ok(())
}
```

### 4. Используйте осмысленные change summaries

**✅ Хорошо:**

```rust
service.create_revision::<Post>(
    &post,
    &post.id,
    None,
    Some("user-123"),
    Some("web"),
    Some("Fixed typo in introduction paragraph"),  // ✅ Конкретно
).await?;

service.create_revision::<Post>(
    &post,
    &post.id,
    None,
    Some("user-123"),
    Some("web"),
    Some("Added new section about performance optimization"),  // ✅ Описательно
).await?;
```

**❌ Плохо:**

```rust
service.create_revision::<Post>(
    &post,
    &post.id,
    None,
    Some("user-123"),
    Some("web"),
    Some("Updated"),  // ❌ Неинформативно
).await?;

service.create_revision::<Post>(
    &post,
    &post.id,
    None,
    Some("user-123"),
    Some("web"),
    None,  // ❌ Нет информации
).await?;
```

### 5. Используйте tracker для batch операций

**✅ Хорошо:**

```rust
async fn bulk_update_posts(
    service: &RevisionService,
    posts: &mut [Post],
) -> Result<(), RevisionError> {
    // Создаем tracker для всех операций
    let tracker = RevisionTracker::new("user-123")
        .with_source("bulk-update")
        .with_summary("Bulk update: added tags to all posts");
    
    for post in posts.iter_mut() {
        post.tags.push("updated".to_string());
        
        // Используем tracker
        service.create_revision_with_tracker::<Post>(
            post,
            &post.id,
            None,
            &tracker,
        ).await?;
    }
    
    // Очищаем tracker
    tracker.clear();
    
    Ok(())
}
```

**❌ Плохо:**

```rust
async fn bulk_update_posts(
    service: &RevisionService,
    posts: &mut [Post],
) -> Result<(), RevisionError> {
    for post in posts.iter_mut() {
        post.tags.push("updated".to_string());
        
        // ❌ Повторяем параметры для каждой ревизии
        service.create_revision::<Post>(
            post,
            &post.id,
            None,
            Some("user-123"),
            Some("bulk-update"),
            Some("Bulk update: added tags to all posts"),
        ).await?;
    }
    
    Ok(())
}
```

## Performance Best Practices

### 1. Используйте connection pooling

**✅ Хорошо:**

```rust
use sqlx::postgres::PgPoolOptions;

let pool = PgPoolOptions::new()
    .max_connections(20)
    .min_connections(5)
    .connect_timeout(Duration::from_secs(30))
    .idle_timeout(Duration::from_secs(300))
    .max_lifetime(Duration::from_secs(1800))
    .connect(&database_url)
    .await?;

let backend = SeaORMBackend::from_pool(pool);
let service = RevisionService::new(backend);
```

**❌ Плохо:**

```rust
// Создаем новое подключение для каждого запроса
let backend = SeaORMBackend::connect(&database_url).await?;
let service = RevisionService::new(backend);
```

### 2. Используйте batch операции

**✅ Хорошо:**

```rust
// Batch create
let items = vec![
    (post1, "post-1".to_string(), Some("en".to_string())),
    (post2, "post-2".to_string(), Some("en".to_string())),
    (post3, "post-3".to_string(), Some("en".to_string())),
];

let revision_ids = service.batch_create_revisions::<Post>(items).await?;
```

**❌ Плохо:**

```rust
// Создаем по одной ревизии
let id1 = service.create_revision::<Post>(&post1, "post-1", ...).await?;
let id2 = service.create_revision::<Post>(&post2, "post-2", ...).await?;
let id3 = service.create_revision::<Post>(&post3, "post-3", ...).await?;
```

### 3. Ограничивайте размер выборки

**✅ Хорошо:**

```rust
// Загружаем только нужное количество
let revisions = service.list_revisions::<Post>(
    "post-123",
    Some("en"),
    Some(100),  // ✅ Limit
    None,
    None,
    Some(SortOrder::Descending),
).await?;
```

**❌ Плохо:**

```rust
// Загружаем ВСЕ ревизии
let revisions = service.list_revisions::<Post>(
    "post-123",
    Some("en"),
    None,  // ❌ No limit - может загрузить тысячи ревизий
    None,
    None,
    Some(SortOrder::Descending),
).await?;
```

### 4. Используйте пагинацию

**✅ Хорошо:**

```rust
// Cursor-based pagination
let mut cursor = None;
let mut all_revisions = Vec::new();

loop {
    let batch = service.list_revisions::<Post>(
        "post-123",
        Some("en"),
        Some(100),
        cursor.as_deref(),
        None,
        Some(SortOrder::Descending),
    ).await?;
    
    if batch.is_empty() {
        break;
    }
    
    cursor = batch.last().map(|r| r.id.to_string());
    all_revisions.extend(batch);
    
    // Ограничиваем общее количество
    if all_revisions.len() >= 1000 {
        break;
    }
}
```

### 5. Кэшируйте часто запрашиваемые данные

**✅ Хорошо:**

```rust
use dashmap::DashMap;
use std::time::{Duration, Instant};

struct RevisionCache {
    cache: DashMap<String, (Vec<Revision>, Instant)>,
    ttl: Duration,
}

impl RevisionCache {
    async fn get_or_fetch<F, Fut>(
        &self,
        key: &str,
        fetcher: F,
    ) -> Result<Vec<Revision>, RevisionError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Vec<Revision>, RevisionError>>,
    {
        // Проверяем кэш
        if let Some(entry) = self.cache.get(key) {
            let (revisions, timestamp) = entry.value();
            if timestamp.elapsed() < self.ttl {
                return Ok(revisions.clone());
            }
        }
        
        // Загружаем из базы
        let revisions = fetcher().await?;
        
        // Сохраняем в кэш
        self.cache.insert(
            key.to_string(),
            (revisions.clone(), Instant::now()),
        );
        
        Ok(revisions)
    }
}

// Использование
let cache = RevisionCache::new(Duration::from_secs(300));

let revisions = cache.get_or_fetch("post-123:en", || async {
    service.list_revisions::<Post>("post-123", Some("en"), Some(100), ...).await
}).await?;
```

### 6. Используйте индексы

**✅ Хорошо:**

```sql
-- Создайте индексы для частых запросов
CREATE INDEX idx_revisions_lookup 
  ON content_revisions(tenant_id, content_type, content_id, locale, revision_number DESC);

CREATE INDEX idx_revisions_created_at 
  ON content_revisions(created_at DESC);

CREATE INDEX idx_revisions_version_name 
  ON content_revisions(version_name) 
  WHERE version_name IS NOT NULL;
```

### 7. Избегайте N+1 queries

**✅ Хорошо:**

```rust
// Загружаем все ревизии одним запросом
let revisions = service.batch_get_revisions::<Post>(
    vec![
        ("post-1".to_string(), Some("en".to_string()), 5),
        ("post-2".to_string(), Some("en".to_string()), 3),
        ("post-3".to_string(), Some("en".to_string()), 7),
    ]
).await?;
```

**❌ Плохо:**

```rust
// N+1 проблема
let rev1 = service.get_revision::<Post>("post-1", Some("en"), 5).await?;
let rev2 = service.get_revision::<Post>("post-2", Some("en"), 3).await?;
let rev3 = service.get_revision::<Post>("post-3", Some("en"), 7).await?;
```

## Error Handling Best Practices

### 1. Обрабатывайте все ошибки

**✅ Хорошо:**

```rust
async fn create_revision_safely(
    service: &RevisionService,
    post: &Post,
) -> Result<Uuid, AppError> {
    match service.create_revision::<Post>(
        post,
        &post.id,
        None,
        None,
        None,
        None,
    ).await {
        Ok(revision_id) => {
            info!("Created revision: {}", revision_id);
            Ok(revision_id)
        }
        Err(RevisionError::DatabaseError(e)) => {
            error!("Database error: {}", e);
            Err(AppError::DatabaseError(e))
        }
        Err(RevisionError::ValidationError(msg)) => {
            warn!("Validation error: {}", msg);
            Err(AppError::ValidationError(msg))
        }
        Err(e) => {
            error!("Unexpected error: {}", e);
            Err(AppError::InternalError(e.to_string()))
        }
    }
}
```

**❌ Плохо:**

```rust
async fn create_revision_safely(
    service: &RevisionService,
    post: &Post,
) -> Result<Uuid, AppError> {
    // ❌ Unwrap может panic
    let revision_id = service.create_revision::<Post>(
        post,
        &post.id,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    Ok(revision_id)
}
```

### 2. Используйте ? operator

**✅ Хорошо:**

```rust
async fn update_post_with_revision(
    service: &RevisionService,
    post: &mut Post,
    new_title: String,
) -> Result<(), RevisionError> {
    post.title = new_title;
    
    let revision_id = service.create_revision::<Post>(
        post,
        &post.id,
        None,
        None,
        None,
        None,
    ).await?;
    
    info!("Created revision: {}", revision_id);
    
    Ok(())
}
```

### 3. Добавляйте контекст к ошибкам

**✅ Хорошо:**

```rust
use anyhow::{Context, Result};

async fn restore_post(
    service: &RevisionService,
    post_id: &str,
    revision_number: i32,
) -> Result<()> {
    let revision = service.get_revision::<Post>(
        post_id,
        None,
        revision_number,
    ).await
    .context(format!("Failed to get revision {} for post {}", revision_number, post_id))?;
    
    let revision = revision.ok_or_else(|| {
        anyhow::anyhow!("Revision {} not found for post {}", revision_number, post_id)
    })?;
    
    // Restore logic...
    
    Ok(())
}
```

### 4. Логируйте ошибки

**✅ Хорошо:**

```rust
async fn create_revision_with_logging(
    service: &RevisionService,
    post: &Post,
) -> Result<Uuid, RevisionError> {
    let start = Instant::now();
    
    let result = service.create_revision::<Post>(
        post,
        &post.id,
        None,
        None,
        None,
        None,
    ).await;
    
    let duration = start.elapsed();
    
    match &result {
        Ok(revision_id) => {
            info!(
                revision_id = %revision_id,
                post_id = %post.id,
                duration_ms = duration.as_millis(),
                "Revision created successfully"
            );
        }
        Err(e) => {
            error!(
                post_id = %post.id,
                error = %e,
                duration_ms = duration.as_millis(),
                "Failed to create revision"
            );
        }
    }
    
    result
}
```

## Testing Best Practices

### 1. Используйте InMemoryBackend для unit tests

**✅ Хорошо:**

```rust
#[tokio::test]
async fn test_create_revision() {
    let backend = InMemoryBackend::new();
    let service = RevisionService::new(backend);
    
    let post = Post {
        id: "post-1".to_string(),
        title: "Test".to_string(),
        content: "Content".to_string(),
    };
    
    let revision_id = service.create_revision::<Post>(
        &post,
        &post.id,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    assert!(revision_id != Uuid::nil());
}
```

### 2. Тестируйте edge cases

**✅ Хорошо:**

```rust
#[tokio::test]
async fn test_empty_content() {
    let backend = InMemoryBackend::new();
    let service = RevisionService::new(backend);
    
    let post = Post {
        id: "post-1".to_string(),
        title: "".to_string(),  // Empty title
        content: "".to_string(), // Empty content
    };
    
    let result = service.create_revision::<Post>(
        &post,
        &post.id,
        None,
        None,
        None,
        None,
    ).await;
    
    // Должно работать даже с пустыми полями
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_large_content() {
    let backend = InMemoryBackend::new();
    let service = RevisionService::new(backend);
    
    let post = Post {
        id: "post-1".to_string(),
        title: "Test".to_string(),
        content: "x".repeat(1_000_000), // 1 MB
    };
    
    let result = service.create_revision::<Post>(
        &post,
        &post.id,
        None,
        None,
        None,
        None,
    ).await;
    
    assert!(result.is_ok());
}
```

### 3. Используйте integration tests с реальной базой

**✅ Хорошо:**

```rust
#[tokio::test]
async fn test_database_integration() {
    // Setup test database
    let pool = setup_test_database().await;
    let backend = SeaORMBackend::from_pool(pool);
    let service = RevisionService::new(backend);
    
    // Test
    let post = create_test_post();
    let revision_id = service.create_revision::<Post>(
        &post,
        &post.id,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    // Verify
    let revision = service.get_revision::<Post>(
        &post.id,
        None,
        1,
    ).await.unwrap();
    
    assert!(revision.is_some());
    assert_eq!(revision.unwrap().id, revision_id);
}
```

### 4. Тестируйте retention policy

**✅ Хорошо:**

```rust
#[tokio::test]
async fn test_retention_policy() {
    let backend = InMemoryBackend::new();
    let service = RevisionService::new(backend);
    
    // Создаем 150 ревизий
    for i in 0..150 {
        let post = Post {
            id: "post-1".to_string(),
            title: format!("Title {}", i),
            content: "Content".to_string(),
        };
        
        service.create_revision::<Post>(
            &post,
            &post.id,
            None,
            None,
            None,
            None,
        ).await.unwrap();
    }
    
    // Применяем retention policy (keep last 100)
    let deleted = service.apply_retention_policy_for_type::<Post>().await.unwrap();
    
    assert_eq!(deleted, 50); // 150 - 100 = 50 deleted
    
    // Проверяем что осталось 100 ревизий
    let revisions = service.list_revisions::<Post>(
        "post-1",
        None,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    assert_eq!(revisions.len(), 100);
}
```

## Security Best Practices

### 1. Всегда проверяйте tenant isolation

**✅ Хорошо:**

```rust
async fn get_revision_secure(
    service: &RevisionService,
    claims: &Claims,
    content_id: &str,
    revision_number: i32,
) -> Result<Revision, RevisionError> {
    let revision = service.get_revision::<Post>(
        content_id,
        None,
        revision_number,
    ).await?
    .ok_or(RevisionError::NotFound)?;
    
    // ✅ Проверяем tenant isolation
    if revision.tenant_id != claims.tenant_id {
        return Err(RevisionError::Forbidden);
    }
    
    Ok(revision)
}
```

**❌ Плохо:**

```rust
async fn get_revision_insecure(
    service: &RevisionService,
    content_id: &str,
    revision_number: i32,
) -> Result<Revision, RevisionError> {
    // ❌ Нет проверки tenant isolation
    let revision = service.get_revision::<Post>(
        content_id,
        None,
        revision_number,
    ).await?
    .ok_or(RevisionError::NotFound)?;
    
    Ok(revision)
}
```

### 2. Валидируйте все входные данные

**✅ Хорошо:**

```rust
use validator::Validate;

#[derive(Validate)]
struct CreateRevisionInput {
    #[validate(length(min = 1, max = 255))]
    content_id: String,
    
    #[validate(length(min = 1, max = 255))]
    content_type: String,
    
    #[validate(length(min = 2, max = 10))]
    locale: Option<String>,
}

async fn create_revision_validated(
    service: &RevisionService,
    input: CreateRevisionInput,
) -> Result<Uuid, RevisionError> {
    // ✅ Валидация
    input.validate()
        .map_err(|e| RevisionError::ValidationError(e.to_string()))?;
    
    // Создание ревизии...
}
```

### 3. Используйте parameterized queries

**✅ Хорошо:**

```rust
// ✅ Parameterized query
let revision = sqlx::query_as::<_, Revision>(
    "SELECT * FROM content_revisions WHERE id = $1"
)
.bind(id)
.fetch_one(&pool)
.await?;
```

**❌ Плохо:**

```rust
// ❌ SQL injection vulnerability
let query = format!("SELECT * FROM content_revisions WHERE id = '{}'", id);
let revision = sqlx::query_as::<_, Revision>(&query)
    .fetch_one(&pool)
    .await?;
```

## Migration Best Practices

### 1. Создавайте идемпотентные миграции

**✅ Хорошо:**

```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ✅ Проверяем существует ли таблица
        if !manager.has_table("content_revisions").await? {
            manager
                .create_table(
                    Table::create()
                        .table(ContentRevisions::Table)
                        .col(ColumnDef::new(ContentRevisions::Id).uuid().not_null().primary_key())
                        // ...
                        .to_owned(),
                )
                .await?;
        }
        
        Ok(())
    }
    
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ✅ Проверяем существует ли таблица
        if manager.has_table("content_revisions").await? {
            manager
                .drop_table(Table::drop().table(ContentRevisions::Table).to_owned())
                .await?;
        }
        
        Ok(())
    }
}
```

### 2. Тестируйте миграции

**✅ Хорошо:**

```bash
# Тестируем up
rustok-revisions migrate up

# Проверяем схему
psql $DATABASE_URL -c "\d content_revisions"

# Тестируем down
rustok-revisions migrate down

# Проверяем что таблица удалена
psql $DATABASE_URL -c "\dt"
```

### 3. Создавайте backup перед миграцией

**✅ Хорошо:**

```bash
# Backup перед миграцией
pg_dump -U rustok -d rustok_revisions -F c -f backup_before_migration.sql

# Применяем миграцию
rustok-revisions migrate up

# Если что-то пошло не так, восстанавливаем
pg_restore -U rustok -d rustok_revisions -c backup_before_migration.sql
```

## Monitoring Best Practices

### 1. Собирайте ключевые метрики

**✅ Хорошо:**

```rust
use prometheus::{HistogramVec, IntCounterVec, Opts, Registry};

pub struct Metrics {
    pub requests_total: IntCounterVec,
    pub request_duration: HistogramVec,
    pub revisions_created: IntCounterVec,
}

impl Metrics {
    pub fn new(registry: &Registry) -> Self {
        let requests_total = IntCounterVec::new(
            Opts::new("http_requests_total", "Total HTTP requests"),
            &["method", "path", "status"],
        ).unwrap();
        
        let request_duration = HistogramVec::new(
            HistogramOpts::new("http_request_duration_seconds", "Request duration")
                .buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0]),
            &["method", "path"],
        ).unwrap();
        
        let revisions_created = IntCounterVec::new(
            Opts::new("revisions_created_total", "Total revisions created"),
            &["content_type"],
        ).unwrap();
        
        registry.register(Box::new(requests_total.clone())).unwrap();
        registry.register(Box::new(request_duration.clone())).unwrap();
        registry.register(Box::new(revisions_created.clone())).unwrap();
        
        Self {
            requests_total,
            request_duration,
            revisions_created,
        }
    }
}
```

### 2. Настраивайте alerting

**✅ Хорошо:**

```yaml
# Prometheus alerting rules
groups:
  - name: rustok-revisions
    rules:
      - alert: HighErrorRate
        expr: rate(http_requests_total{status=~"5.."}[5m]) > 0.1
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High error rate"
          
      - alert: HighLatency
        expr: histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m])) > 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High latency"
```

## Operational Best Practices

### 1. Регулярно запускайте cleanup

**✅ Хорошо:**

```bash
# Cron job для cleanup
0 2 * * * /usr/local/bin/rustok-revisions cleanup >> /var/log/revisions-cleanup.log 2>&1
```

### 2. Мониторьте размер базы данных

**✅ Хорошо:**

```sql
-- Проверяем размер таблицы
SELECT 
    pg_size_pretty(pg_total_relation_size('content_revisions')) AS total_size,
    pg_size_pretty(pg_relation_size('content_revisions')) AS table_size,
    pg_size_pretty(pg_indexes_size('content_revisions')) AS index_size;

-- Проверяем количество ревизий
SELECT content_type, count(*) 
FROM content_revisions 
GROUP BY content_type 
ORDER BY count(*) DESC;
```

### 3. Делайте регулярные backups

**✅ Хорошо:**

```bash
#!/bin/bash
# backup-revisions.sh

BACKUP_DIR="/backups/revisions"
DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="$BACKUP_DIR/revisions_$DATE.sql"

mkdir -p $BACKUP_DIR

pg_dump -U rustok -d rustok_revisions -F c -f $BACKUP_FILE

gzip $BACKUP_FILE

# Удаляем backups старше 30 дней
find $BACKUP_DIR -name "revisions_*.sql.gz" -mtime +30 -delete

echo "Backup created: $BACKUP_FILE.gz"
```

## Заключение

Эти best practices помогут вам:

✅ **Modeling** — правильно проектировать модели  
✅ **Performance** — достичь максимальной производительности  
✅ **Error Handling** — правильно обрабатывать ошибки  
✅ **Testing** — писать надежные тесты  
✅ **Security** — обеспечить безопасность  
✅ **Migration** — безопасно мигрировать  
✅ **Monitoring** — эффективно мониторить  
✅ **Operations** — правильно эксплуатировать  

Следуя этим практикам, вы сможете:

- Избежать распространенных ошибок
- Достичь отличной производительности
- Обеспечить надежность системы
- Упростить поддержку и развитие

**Удачи в использовании rustok-revisions!** 🚀
