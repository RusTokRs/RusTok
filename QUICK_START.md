# Quick Start Guide

Быстрый старт с rustok-revisions за 5 минут.

## Установка

Добавьте в `Cargo.toml`:

```toml
[dependencies]
rustok-revisions = "0.1.0"
rustok-revisions-derive = "0.1.0"
sea-orm = { version = "0.12", features = ["sqlx-postgres", "runtime-tokio-rustls"] }
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
```

## Настройка базы данных

```bash
# Создайте базу данных
createdb rustok_revisions

# Примените миграции
rustok-revisions migrate up
```

## Первый пример

```rust
use rustok_revisions::{
    RevisionService, SeaORMBackend, Revisionable, ContentRevisionConfig, RetentionPolicy
};
use rustok_revisions_derive::Revisionable;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// 1. Определите модель
#[derive(Debug, Clone, Serialize, Deserialize, Revisionable)]
pub struct Post {
    #[revision(tracked)]
    pub title: String,
    
    #[revision(tracked)]
    pub content: String,
    
    #[revision(tracked)]
    pub status: String,
    
    #[revision(ignored)]
    pub updated_at: DateTime<Utc>,
}

// 2. Настройте конфигурацию
impl ContentRevisionConfig for Post {
    fn content_type() -> &'static str {
        "blog_post"
    }
    
    fn tracked_fields() -> &'static [&'static str] {
        &["title", "content", "status"]
    }
    
    fn ignored_fields() -> &'static [&'static str] {
        &["updated_at"]
    }
    
    fn default_retention_policy() -> Option<RetentionPolicy> {
        Some(RetentionPolicy::KeepLast { count: 100 })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 3. Подключитесь к базе данных
    let backend = SeaORMBackend::connect(
        "postgres://rustok:password@localhost:5432/rustok_revisions"
    ).await?;
    
    let service = RevisionService::new(backend);
    
    // 4. Создайте пост
    let mut post = Post {
        title: "Hello World".to_string(),
        content: "This is my first post".to_string(),
        status: "draft".to_string(),
        updated_at: Utc::now(),
    };
    
    // 5. Создайте первую ревизию
    let rev1 = service.create_revision::<Post>(
        &post,
        "post-1",
        Some("en"),
        Some("user-123"),
        Some("web"),
        Some("Created initial draft"),
    ).await?;
    
    println!("Created revision: {}", rev1);
    
    // 6. Обновите пост
    post.title = "Hello Rust".to_string();
    post.status = "published".to_string();
    post.updated_at = Utc::now();
    
    // 7. Создайте вторую ревизию
    let rev2 = service.create_revision::<Post>(
        &post,
        "post-1",
        Some("en"),
        Some("user-123"),
        Some("web"),
        Some("Updated title and published"),
    ).await?;
    
    println!("Created revision: {}", rev2);
    
    // 8. Получите историю
    let history = service.list_revisions::<Post>(
        "post-1",
        Some("en"),
        Some(10),
        None,
        None,
        Some(rustok_revisions::SortOrder::Descending),
    ).await?;
    
    println!("History ({} revisions):", history.len());
    for rev in history {
        println!("  #{}: {} at {}", 
            rev.revision_number, 
            rev.change_summary.unwrap_or_default(),
            rev.created_at
        );
    }
    
    // 9. Сравните версии
    let diff = service.compare_revisions::<Post>(
        "post-1",
        Some("en"),
        1,  // from
        2,  // to
    ).await?;
    
    println!("\nChanges from v1 to v2:");
    for change in diff.changes {
        println!("  {}: {:?} → {:?}", 
            change.field, 
            change.old_value, 
            change.new_value
        );
    }
    
    // 10. Восстановите к первой версии
    let restored = service.restore_revision::<Post>(
        "post-1",
        Some("en"),
        1,              // target revision
        Some("user-123"),
        Some("Restored to initial draft"),
        true,           // create snapshot
    ).await?;
    
    println!("\nRestored to revision: {}", restored);
    
    Ok(())
}
```

## Запуск

```bash
cargo run
```

**Ожидаемый вывод:**

```
Created revision: 550e8400-e29b-41d4-a716-446655440000
Created revision: 550e8400-e29b-41d4-a716-446655440001
History (2 revisions):
  #2: Updated title and published at 2024-01-15T10:30:00Z
  #1: Created initial draft at 2024-01-15T10:00:00Z

Changes from v1 to v2:
  title: "Hello World" → "Hello Rust"
  status: "draft" → "published"

Restored to revision: 550e8400-e29b-41d4-a716-446655440002
```

## Что дальше?

- 📖 Читайте [Best Practices](BEST_PRACTICES.md)
- 🔧 Изучите [API Reference](API_REFERENCE.md)
- 🚀 Посмотрите [Examples](examples/)
- ❓ Проверьте [FAQ](FAQ.md)
- 🐛 Если проблемы — [Troubleshooting](TROUBLESHOOTING.md)

## Быстрые рецепты

### Создать named version

```rust
service.create_named_version::<Post>(
    "post-1",
    Some("en"),
    "v1.0-published",
    Some("First published version"),
    Some("user-123"),
).await?;
```

### Получить все named versions

```rust
let versions = service.list_named_versions::<Post>("post-1", Some("en")).await?;
```

### Применить retention policy

```rust
let deleted = service.apply_retention_policy_for_type::<Post>().await?;
println!("Deleted {} old revisions", deleted);
```

### Использовать tracker для batch операций

```rust
let tracker = RevisionTracker::new("user-123")
    .with_source("bulk-update")
    .with_summary("Updated all posts");

for post in posts {
    service.create_revision_with_tracker::<Post>(
        &post,
        &post.id,
        None,
        &tracker,
    ).await?;
}
```

### Point-in-time recovery

```rust
let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
let revision = service.get_revision_at_time::<Post>(
    "post-1",
    Some("en"),
    timestamp,
).await?;
```

## Готово! 🎉

Теперь вы знаете основы rustok-revisions. Удачи в разработке!
