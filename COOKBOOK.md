# Cookbook

Сборник рецептов для типичных задач с rustok-revisions.

## Содержание

1. [Базовые операции](#базовые-операции)
2. [Продвинутые сценарии](#продвинутые-сценарии)
3. [Интеграция с веб-фреймворками](#интеграция-с-веб-фреймворками)
4. [Работа с большими данными](#работа-с-большими-данными)
5. [Multilingual сценарии](#multilingual-сценарии)
6. [Мониторинг и отладка](#мониторинг-и-отладка)

## Базовые операции

### Рецепт 1: Создание ревизии с метаданными

**Задача:** Создать ревизию с полной информацией о пользователе, источнике и описании изменений.

**Решение:**

```rust
let revision_id = service.create_revision::<Post>(
    &post,
    "post-123",                    // content_id
    Some("en"),                    // locale
    Some("user-456"),              // created_by
    Some("web"),                   // change_source
    Some("Fixed typo in title"),   // change_summary
).await?;

println!("Created revision: {}", revision_id);
```

---

### Рецепт 2: Группировка связанных изменений

**Задача:** Создать несколько ревизий с общими метаданными (например, при bulk update).

**Решение:**

```rust
use rustok_revisions::RevisionTracker;

// Создайте tracker
let tracker = RevisionTracker::new("user-456")
    .with_source("bulk-update")
    .with_summary("Added tags to all posts");

// Используйте tracker для всех операций
for post in posts.iter_mut() {
    post.tags.push("updated".to_string());
    
    service.create_revision_with_tracker::<Post>(
        post,
        &post.id,
        None,
        &tracker,
    ).await?;
}

// Очистите tracker
tracker.clear();
```

---

### Рецепт 3: Получение истории с пагинацией

**Задача:** Получить историю ревизий с пагинацией (cursor-based).

**Решение:**

```rust
// Первая страница
let first_page = service.list_revisions::<Post>(
    "post-123",
    Some("en"),
    Some(10),  // limit
    None,      // before
    None,      // after
    Some(SortOrder::Descending),
).await?;

// Получите cursor для следующей страницы
let cursor = first_page.last().map(|r| r.id.to_string());

// Следующая страница
let second_page = service.list_revisions::<Post>(
    "post-123",
    Some("en"),
    Some(10),
    None,
    cursor.as_deref(),  // after cursor
    Some(SortOrder::Descending),
).await?;
```

---

### Рецепт 4: Сравнение двух версий

**Задача:** Показать различия между двумя версиями контента.

**Решение:**

```rust
let diff = service.compare_revisions::<Post>(
    "post-123",
    Some("en"),
    3,  // from_revision
    5,  // to_revision
).await?;

println!("Changes from v{} to v{}:", diff.from_revision, diff.to_revision);

for change in diff.changes {
    match (change.old_value, change.new_value) {
        (Some(old), Some(new)) => {
            println!("  {} changed: {:?} → {:?}", change.field, old, new);
        }
        (None, Some(new)) => {
            println!("  {} added: {:?}", change.field, new);
        }
        (Some(old), None) => {
            println!("  {} removed: {:?}", change.field, old);
        }
        _ => {}
    }
}
```

---

### Рецепт 5: Восстановление к предыдущей версии

**Задача:** Восстановить контент к определенной ревизии с созданием snapshot.

**Решение:**

```rust
let restored_id = service.restore_revision::<Post>(
    "post-123",
    Some("en"),
    3,                          // target_revision
    Some("user-456"),           // restored_by
    Some("Restored to v3"),     // change_summary
    true,                       // create_snapshot
).await?;

println!("Restored! New revision: {}", restored_id);
```

---

### Рецепт 6: Создание именованной версии

**Задача:** Создать named version для важного момента (например, публикация).

**Решение:**

```rust
let version_id = service.create_named_version::<Post>(
    "post-123",
    Some("en"),
    "v1.0-published",
    Some("First published version"),
    Some("user-456"),
).await?;

println!("Created named version: {}", version_id);
```

---

### Рецепт 7: Получение всех named versions

**Задача:** Получить список всех именованных версий.

**Решение:**

```rust
let versions = service.list_named_versions::<Post>(
    "post-123",
    Some("en"),
).await?;

println!("Named versions:");
for version in versions {
    println!("  {} (revision #{}) - created at {}", 
        version.version_name.unwrap_or_default(),
        version.revision_number,
        version.created_at
    );
}
```

---

### Рецепт 8: Point-in-time recovery

**Задача:** Получить состояние контента на определенный момент времени.

**Решение:**

```rust
use chrono::{TimeZone, Utc};

let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

let revision = service.get_revision_at_time::<Post>(
    "post-123",
    Some("en"),
    timestamp,
).await?;

if let Some(rev) = revision {
    println!("State at {}: revision #{}", timestamp, rev.revision_number);
    println!("Title: {}", rev.delta["title"]["new"]);
} else {
    println!("No revision found at that time");
}
```

---

### Рецепт 9: Применение retention policy

**Задача:** Очистить старые ревизии согласно политике хранения.

**Решение:**

```rust
// Применить retention policy для типа
let deleted = service.apply_retention_policy_for_type::<Post>().await?;

println!("Deleted {} old revisions", deleted);
```

---

### Рецепт 10: Подсчет ревизий

**Задача:** Подсчитать количество ревизий для контента.

**Решение:**

```rust
use rustok_revisions::RevisionFilter;

let filter = RevisionFilter {
    content_type: Some("blog_post".to_string()),
    content_id: Some("post-123".to_string()),
    locale: Some("en".to_string()),
    ..Default::default()
};

let count = service.backend().count_revisions(filter).await?;

println!("Total revisions: {}", count);
```

## Продвинутые сценарии

### Рецепт 11: Транзакционное создание ревизии

**Задача:** Создать ревизию в рамках транзакции с другими операциями.

**Решение:**

```rust
use sea_orm::TransactionTrait;

let txn = pool.begin().await?;

// Создайте ревизию
let revision_id = service.create_revision_in_transaction::<Post>(
    &txn,
    &post,
    "post-123",
    Some("en"),
    Some("user-456"),
    Some("web"),
    Some("Updated post"),
).await?;

// Выполните другие операции
sqlx::query("UPDATE posts SET updated_at = NOW() WHERE id = $1")
    .bind("post-123")
    .execute(&mut txn)
    .await?;

// Зафиксируйте транзакцию
txn.commit().await?;
```

---

### Рецепт 12: Batch создание ревизий

**Задача:** Создать ревизии для множества объектов за одну операцию.

**Решение:**

```rust
let items: Vec<(Post, String, Option<String>)> = posts
    .into_iter()
    .map(|p| (p, p.id.clone(), None))
    .collect();

let revision_ids = service.batch_create_revisions::<Post>(items).await?;

println!("Created {} revisions", revision_ids.len());
```

---

### Рецепт 13: Streaming больших историй

**Задача:** Обработать большую историю ревизий без загрузки всего в память.

**Решение:**

```rust
use futures::stream::StreamExt;

let mut stream = service.stream_revisions::<Post>(
    "post-123",
    Some("en"),
).await?;

while let Some(result) = stream.next().await {
    let revision = result?;
    process_revision(revision);
}
```

---

### Рецепт 14: Кэширование ревизий

**Задача:** Кэшировать часто запрашиваемые ревизии.

**Решение:**

```rust
use dashmap::DashMap;
use std::time::{Duration, Instant};

struct RevisionCache {
    cache: DashMap<String, (Revision, Instant)>,
    ttl: Duration,
}

impl RevisionCache {
    fn new(ttl: Duration) -> Self {
        Self {
            cache: DashMap::new(),
            ttl,
        }
    }
    
    async fn get_or_fetch<F, Fut>(
        &self,
        key: &str,
        fetcher: F,
    ) -> Result<Revision, RevisionError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Revision, RevisionError>>,
    {
        // Проверьте кэш
        if let Some(entry) = self.cache.get(key) {
            let (revision, timestamp) = entry.value();
            if timestamp.elapsed() < self.ttl {
                return Ok(revision.clone());
            }
        }
        
        // Загрузите из базы
        let revision = fetcher().await?;
        
        // Сохраните в кэш
        self.cache.insert(key.to_string(), (revision.clone(), Instant::now()));
        
        Ok(revision)
    }
}

// Использование
let cache = RevisionCache::new(Duration::from_secs(300));

let revision = cache.get_or_fetch("post-123:en:5", || async {
    service.get_revision::<Post>("post-123", Some("en"), 5).await?
        .ok_or(RevisionError::NotFound)
}).await?;
```

---

### Рецепт 15: Условное создание ревизии

**Задача:** Создать ревизию только если контент действительно изменился.

**Решение:**

```rust
async fn create_revision_if_changed<T: Revisionable>(
    service: &RevisionService,
    item: &T,
    content_id: &str,
    locale: Option<&str>,
) -> Result<Option<Uuid>, RevisionError> {
    // Получите последнюю ревизию
    let last_revision = service.list_revisions::<T>(
        content_id,
        locale,
        Some(1),
        None,
        None,
        Some(SortOrder::Descending),
    ).await?;
    
    if let Some(last) = last_revision.first() {
        // Сравните с текущим состоянием
        let current_value = item.to_revision_value()?;
        
        if last.delta == current_value {
            // Не изменилось
            return Ok(None);
        }
    }
    
    // Создайте ревизию
    let revision_id = service.create_revision::<T>(
        item,
        content_id,
        locale,
        None,
        None,
        None,
    ).await?;
    
    Ok(Some(revision_id))
}

// Использование
if let Some(revision_id) = create_revision_if_changed(&service, &post, "post-123", None).await? {
    println!("Created revision: {}", revision_id);
} else {
    println!("No changes detected");
}
```

## Интеграция с веб-фреймворками

### Рецепт 16: Интеграция с Axum

**Задача:** Интегрировать revision history в Axum приложение.

**Решение:**

```rust
use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::Json,
    Router,
    routing::{get, post},
};
use std::sync::Arc;

// State
struct AppState {
    revision_service: RevisionService,
}

// Handler для создания ревизии
async fn create_revision_handler(
    Extension(state): Extension<Arc<AppState>>,
    Path(post_id): Path<String>,
    Json(post): Json<Post>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let revision_id = state.revision_service
        .create_revision::<Post>(
            &post,
            &post_id,
            None,
            None,
            Some("api"),
            None,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(json!({
        "revision_id": revision_id,
        "status": "created"
    })))
}

// Handler для получения истории
async fn get_history_handler(
    Extension(state): Extension<Arc<AppState>>,
    Path(post_id): Path<String>,
) -> Result<Json<Vec<Revision>>, StatusCode> {
    let revisions = state.revision_service
        .list_revisions::<Post>(
            &post_id,
            None,
            Some(100),
            None,
            None,
            Some(SortOrder::Descending),
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(revisions))
}

// Router
let app = Router::new()
    .route("/posts/:id/revisions", post(create_revision_handler))
    .route("/posts/:id/revisions", get(get_history_handler))
    .layer(Extension(Arc::new(AppState {
        revision_service: service,
    })));
```

---

### Рецепт 17: GraphQL resolver

**Задача:** Создать GraphQL resolver для revision history.

**Решение:**

```rust
use async_graphql::*;

pub struct RevisionQuery;

#[Object]
impl RevisionQuery {
    async fn revisions(
        &self,
        ctx: &Context<'_>,
        content_id: String,
        content_type: String,
        locale: Option<String>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<Vec<Revision>> {
        let service = ctx.data::<RevisionService>()?;
        
        let revisions = service.list_revisions::<Post>(
            &content_id,
            locale.as_deref(),
            first.map(|f| f as usize),
            None,
            after.as_deref(),
            Some(SortOrder::Descending),
        ).await?;
        
        Ok(revisions)
    }
    
    async fn revision_diff(
        &self,
        ctx: &Context<'_>,
        content_id: String,
        from_revision: i32,
        to_revision: i32,
    ) -> Result<RevisionDiff> {
        let service = ctx.data::<RevisionService>()?;
        
        let diff = service.compare_revisions::<Post>(
            &content_id,
            None,
            from_revision,
            to_revision,
        ).await?;
        
        Ok(diff)
    }
}

pub struct RevisionMutation;

#[Object]
impl RevisionMutation {
    async fn restore_revision(
        &self,
        ctx: &Context<'_>,
        content_id: String,
        target_revision: i32,
        create_snapshot: Option<bool>,
    ) -> Result<Uuid> {
        let service = ctx.data::<RevisionService>()?;
        
        let revision_id = service.restore_revision::<Post>(
            &content_id,
            None,
            target_revision,
            None,
            None,
            create_snapshot.unwrap_or(true),
        ).await?;
        
        Ok(revision_id)
    }
}
```

## Работа с большими данными

### Рецепт 18: Экспорт всей истории

**Задача:** Экспортировать всю историю ревизий в CSV.

**Решение:**

```rust
use csv::Writer;
use std::fs::File;

async fn export_history(
    service: &RevisionService,
    content_id: &str,
    output_file: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut writer = Writer::from_path(output_file)?;
    
    // Write header
    writer.write_record(&[
        "revision_number",
        "created_at",
        "created_by",
        "change_summary",
        "delta",
    ])?;
    
    // Stream revisions
    let mut stream = service.stream_revisions::<Post>(content_id, None).await?;
    let mut count = 0;
    
    while let Some(result) = stream.next().await {
        let revision = result?;
        
        writer.write_record(&[
            revision.revision_number.to_string(),
            revision.created_at.to_rfc3339(),
            revision.created_by.unwrap_or_default(),
            revision.change_summary.unwrap_or_default(),
            serde_json::to_string(&revision.delta)?,
        ])?;
        
        count += 1;
    }
    
    writer.flush()?;
    
    Ok(count)
}

// Использование
let count = export_history(&service, "post-123", "history.csv").await?;
println!("Exported {} revisions", count);
```

---

### Рецепт 19: Архивация старых ревизий

**Задача:** Переместить старые ревизии в архивную таблицу.

**Решение:**

```rust
async fn archive_old_revisions(
    pool: &PgPool,
    older_than_days: i64,
) -> Result<u64, sqlx::Error> {
    let cutoff_date = Utc::now() - Duration::days(older_than_days);
    
    // Переместите в архив
    let result = sqlx::query(
        "WITH to_archive AS (
            SELECT id FROM content_revisions 
            WHERE created_at < $1
        )
        INSERT INTO content_revisions_archive
        SELECT * FROM content_revisions 
        WHERE id IN (SELECT id FROM to_archive)"
    )
    .bind(cutoff_date)
    .execute(pool)
    .await?;
    
    // Удалите из основной таблицы
    let deleted = sqlx::query(
        "DELETE FROM content_revisions 
         WHERE created_at < $1"
    )
    .bind(cutoff_date)
    .execute(pool)
    .await?;
    
    Ok(deleted.rows_affected())
}

// Использование
let archived = archive_old_revisions(&pool, 365).await?;
println!("Archived {} revisions", archived);
```

## Multilingual сценарии

### Рецепт 20: Создание ревизий для разных локалей

**Задача:** Версионировать контент на разных языках независимо.

**Решение:**

```rust
// Создайте ревизию для English
let en_revision = service.create_revision::<Post>(
    &post_en,
    "post-123",
    Some("en"),
    Some("user-456"),
    Some("web"),
    Some("Updated English version"),
).await?;

// Создайте ревизию для Russian
let ru_revision = service.create_revision::<Post>(
    &post_ru,
    "post-123",
    Some("ru"),
    Some("user-456"),
    Some("web"),
    Some("Updated Russian version"),
).await?;

// Получите историю для каждой локали отдельно
let en_history = service.list_revisions::<Post>(
    "post-123",
    Some("en"),
    Some(100),
    None,
    None,
    Some(SortOrder::Descending),
).await?;

let ru_history = service.list_revisions::<Post>(
    "post-123",
    Some("ru"),
    Some(100),
    None,
    None,
    Some(SortOrder::Descending),
).await?;

println!("English revisions: {}", en_history.len());
println!("Russian revisions: {}", ru_history.len());
```

---

### Рецепт 21: Сравнение версий между локалями

**Задача:** Сравнить контент на разных языках.

**Решение:**

```rust
// Получите последнюю версию для каждой локали
let en_revision = service.list_revisions::<Post>(
    "post-123",
    Some("en"),
    Some(1),
    None,
    None,
    Some(SortOrder::Descending),
).await?.pop();

let ru_revision = service.list_revisions::<Post>(
    "post-123",
    Some("ru"),
    Some(1),
    None,
    None,
    Some(SortOrder::Descending),
).await?.pop();

// Сравните контент
if let (Some(en), Some(ru)) = (en_revision, ru_revision) {
    println!("English title: {}", en.delta["title"]["new"]);
    println!("Russian title: {}", ru.delta["title"]["new"]);
    
    // Проверьте синхронизацию
    if en.created_at > ru.created_at {
        println!("⚠️ Russian version is outdated");
    }
}
```

## Мониторинг и отладка

### Рецепт 22: Логирование всех операций

**Задача:** Логировать все операции с ревизиями.

**Решение:**

```rust
use tracing::{info, warn, error};

async fn create_revision_with_logging<T: Revisionable>(
    service: &RevisionService,
    item: &T,
    content_id: &str,
    locale: Option<&str>,
) -> Result<Uuid, RevisionError> {
    let start = std::time::Instant::now();
    
    info!(
        content_id = content_id,
        locale = locale,
        "Creating revision"
    );
    
    let result = service.create_revision::<T>(
        item,
        content_id,
        locale,
        None,
        None,
        None,
    ).await;
    
    let duration = start.elapsed();
    
    match &result {
        Ok(revision_id) => {
            info!(
                revision_id = %revision_id,
                content_id = content_id,
                duration_ms = duration.as_millis(),
                "Revision created successfully"
            );
        }
        Err(e) => {
            error!(
                content_id = content_id,
                error = %e,
                duration_ms = duration.as_millis(),
                "Failed to create revision"
            );
        }
    }
    
    result
}
```

---

### Рецепт 23: Метрики для Prometheus

**Задача:** Собрать метрики для мониторинга.

**Решение:**

```rust
use prometheus::{IntCounterVec, HistogramVec, Opts, Registry};

struct Metrics {
    revisions_created: IntCounterVec,
    revision_duration: HistogramVec,
}

impl Metrics {
    fn new(registry: &Registry) -> Self {
        let revisions_created = IntCounterVec::new(
            Opts::new("revisions_created_total", "Total revisions created"),
            &["content_type"],
        ).unwrap();
        
        let revision_duration = HistogramVec::new(
            HistogramOpts::new("revision_creation_duration_seconds", "Revision creation duration")
                .buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0]),
            &["content_type"],
        ).unwrap();
        
        registry.register(Box::new(revisions_created.clone())).unwrap();
        registry.register(Box::new(revision_duration.clone())).unwrap();
        
        Self {
            revisions_created,
            revision_duration,
        }
    }
}

// Использование
let metrics = Metrics::new(&registry);

let start = Instant::now();
let revision_id = service.create_revision::<Post>(&post, "post-123", ...).await?;
let duration = start.elapsed();

metrics.revisions_created
    .with_label_values(&["blog_post"])
    .inc();

metrics.revision_duration
    .with_label_values(&["blog_post"])
    .observe(duration.as_secs_f64());
```

---

### Рецепт 24: Health check endpoint

**Задача:** Создать health check endpoint для мониторинга.

**Решение:**

```rust
use axum::{Json, http::StatusCode};
use serde::Serialize;

#[derive(Serialize)]
struct HealthStatus {
    status: String,
    database: bool,
    uptime_seconds: u64,
    revision_count: i64,
}

async fn health_check(
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<HealthStatus>, StatusCode> {
    // Проверьте базу данных
    let db_healthy = sqlx::query("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();
    
    // Получите количество ревизий
    let revision_count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM content_revisions"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);
    
    let status = if db_healthy { "healthy" } else { "unhealthy" };
    
    Ok(Json(HealthStatus {
        status: status.to_string(),
        database: db_healthy,
        uptime_seconds: state.start_time.elapsed().as_secs(),
        revision_count,
    }))
}

// Router
let app = Router::new()
    .route("/health", get(health_check));
```

## Заключение

Этот cookbook содержит **24 готовых рецепта** для:

✅ **Базовых операций** — создание, получение, сравнение, восстановление  
✅ **Продвинутых сценариев** — транзакции, batch, streaming, кэширование  
✅ **Интеграции** — Axum, GraphQL  
✅ **Работы с большими данными** — экспорт, архивация  
✅ **Multilingual** — работа с разными локалями  
✅ **Мониторинга** — логирование, метрики, health check  

Используйте эти рецепты как:

- Готовые решения для типичных задач
- Примеры best practices
- Starting point для вашей реализации

**Удачи в разработке!** 🍳
