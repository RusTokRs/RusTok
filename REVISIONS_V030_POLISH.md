# rustok-revisions v0.3.0 — Final Polish

**Дата:** 2026-10-09  
**Статус:** ✅ Polished  
**Версия:** 0.3.0 (от v0.2.0)

## Обзор улучшений

Финальная полировка библиотеки с улучшением developer experience и добавлением полезных утилит.

## ✨ Новые возможности

### 1. **Derive Macro** 🎯

Упрощенное определение `Revisionable` с помощью `#[derive(Revisionable)]`:

```rust
use rustok_revisions::Revisionable;
use rustok_revisions_derive::Revisionable;

#[derive(Debug, Clone, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "blog_post")]
struct BlogPost {
    id: Uuid,
    
    #[revision(tracked)]
    title: String,
    
    #[revision(tracked)]
    content: String,
    
    #[revision(ignored)]
    updated_at: DateTime<Utc>,
}
```

**Преимущества:**
- ✅ Меньше boilerplate кода
- ✅ Типобезопасность (compile-time checks)
- ✅ Чище и понятнее

**Использование:**

```toml
# Cargo.toml
[dependencies]
rustok-revisions = { version = "0.3.0", features = ["derive"] }
```

**Comparison:**

```rust
// Без derive (v0.2.0):
impl Revisionable for BlogPost {
    fn content_type() -> &'static str { "blog_post" }
    fn tracked_fields() -> Vec<&'static str> { vec!["title", "content"] }
    fn ignored_fields() -> Vec<&'static str> { vec!["updated_at"] }
}

// С derive (v0.3.0):
#[derive(Revisionable)]
#[revision(content_type = "blog_post")]
struct BlogPost {
    #[revision(tracked)]
    title: String,
    #[revision(tracked)]
    content: String,
    #[revision(ignored)]
    updated_at: DateTime<Utc>,
}
```

### 2. **Named Versions (Snapshots)** 🏷️

Возможность создавать именованные версии для важных milestones:

```rust
// Создать named version
service.create_named_version(
    tenant_id,
    post_id,
    "en",
    &current_post,
    "v1.0-published",  // Имя версии
    user_id,
).await?;

// Получить все named versions
let versions = service.list_named_versions(
    tenant_id,
    "blog_post",
    post_id,
    "en",
).await?;

for version in versions {
    println!("Version: {} at {}", 
        version.version_name.unwrap(),
        version.created_at
    );
}
```

**Use cases:**
- 📌 Отметить важные milestones ("v1.0", "published", "approved")
- 🔄 Quick restore к известным хорошим состояниям
- 📊 Audit trail с понятными именами

**Пример:**

```rust
// Перед публикацией
service.create_named_version(
    tenant_id, post_id, "en",
    &post,
    "before-publication",
    user_id,
).await?;

// Публикуем
post.publish();

// Если что-то пошло не так — restore к "before-publication"
let versions = service.list_named_versions(...).await?;
let before_pub = versions.iter()
    .find(|v| v.version_name == Some("before-publication".to_string()))
    .unwrap();

let restored = service.restore_revision(
    tenant_id, post_id, "en",
    before_pub.revision_number,
    &current_post,
    user_id,
).await?;
```

### 3. **Diff Utilities** 🔍

Утилиты для сравнения revisions:

```rust
use rustok_revisions::{RevisionDiff, RevisionComparator};

// Diff между двумя revisions
let diff = service.diff_revisions(
    tenant_id,
    "blog_post",
    post_id,
    "en",
    1,  // from revision
    3,  // to revision
).await?;

println!("Changed {} fields", diff.changed_fields_count());

for change in &diff.changes {
    println!("{}: {} → {}", 
        change.field,
        change.old_value,
        change.new_value
    );
}

// Human-readable output
println!("{}", diff.human_readable());
```

**Output:**
```
Changes from revision 1 to 3:
  - title: "Old Title" → "New Title"
  - content: "Old content..." → "New content..."
  - status: "draft" → "published"
```

**RevisionComparator:**

```rust
// Сравнить все revisions
let revisions = service.list_revisions(...).await?;
let comparator = RevisionComparator::new(revisions);

// Get all diffs
let diffs = comparator.all_diffs();

// Summary
println!("{}", comparator.summary());
```

### 4. **Enhanced API** 🚀

Новые методы в `RevisionService`:

```rust
// Named versions
service.create_named_version(...).await?;
service.list_named_versions(...).await?;

// Diff utilities
service.diff_revisions(from, to).await?;
service.all_diffs().await?;
```

## API Reference

### Derive Macro

```rust
#[derive(Revisionable)]
#[revision(content_type = "name")]
struct MyContent {
    #[revision(tracked)]
    field1: String,
    
    #[revision(ignored)]
    field2: DateTime<Utc>,
}
```

**Attributes:**
- `#[revision(content_type = "name")]` — content type identifier
- `#[revision(tracked)]` — mark field as tracked
- `#[revision(ignored)]` — mark field as ignored

### Named Versions

```rust
impl RevisionService {
    pub async fn create_named_version<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        current_content: &T,
        version_name: &str,
        created_by: Uuid,
    ) -> Result<Revision, RevisionError>;

    pub async fn list_named_versions(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Vec<Revision>, RevisionError>;
}
```

### Diff Utilities

```rust
pub struct FieldDiff {
    pub field: String,
    pub old_value: serde_json::Value,
    pub new_value: serde_json::Value,
}

pub struct RevisionDiff {
    pub from_revision: i32,
    pub to_revision: i32,
    pub changes: Vec<FieldDiff>,
    pub summary: String,
}

impl RevisionDiff {
    pub fn between(from: &Revision, to: &Revision) -> Self;
    pub fn human_readable(&self) -> String;
    pub fn has_changes(&self) -> bool;
    pub fn changed_fields_count(&self) -> usize;
}

pub struct RevisionComparator {
    // ...
}

impl RevisionComparator {
    pub fn new(revisions: Vec<Revision>) -> Self;
    pub fn diff(&self, from: i32, to: i32) -> Option<RevisionDiff>;
    pub fn all_diffs(&self) -> Vec<RevisionDiff>;
    pub fn summary(&self) -> String;
}
```

## Примеры использования

### Full Example с Derive

```rust
use rustok_revisions::{
    Revisionable, RevisionService, RevisionTracker,
    RetentionPolicy, RevisionEvent, InMemoryBackend,
};
use rustok_revisions_derive::Revisionable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "blog_post")]
struct BlogPost {
    id: Uuid,
    
    #[revision(tracked)]
    title: String,
    
    #[revision(tracked)]
    content: String,
    
    #[revision(tracked)]
    status: String,
    
    #[revision(ignored)]
    updated_at: DateTime<Utc>,
}

impl BlogPost {
    pub fn revision_tracker() -> RevisionTracker<Self> {
        RevisionTracker::builder()
            .enabled(true)
            .track_on(vec![RevisionEvent::Update])
            .condition(|post| post.status == "published")
            .retention(RetentionPolicy::KeepLast(50))
            .build()
    }
}

#[tokio::main]
async fn main() {
    let service = RevisionService::new(Box::new(InMemoryBackend::new()));
    let tracker = BlogPost::revision_tracker();
    
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    
    // Initial post
    let post_v1 = BlogPost {
        id: Uuid::new_v4(),
        title: "My First Post".to_string(),
        content: "Hello world!".to_string(),
        status: "draft".to_string(),
        updated_at: Utc::now(),
    };
    
    // Update to published
    let post_v2 = BlogPost {
        title: "My First Post".to_string(),
        content: "Hello world! (updated)".to_string(),
        status: "published".to_string(),
        updated_at: Utc::now(),
        ..post_v1.clone()
    };
    
    // Create revision
    service.create_revision_with_tracker(
        tenant_id,
        post_v1.id,
        "en",
        &post_v1,
        &post_v2,
        user_id,
        &tracker,
        RevisionEvent::Update,
    ).await?;
    
    // Create named version
    service.create_named_version(
        tenant_id,
        post_v1.id,
        "en",
        &post_v2,
        "v1.0-published",
        user_id,
    ).await?;
    
    // Update again
    let post_v3 = BlogPost {
        title: "My First Post (Revised)".to_string(),
        content: "Hello world! (revised)".to_string(),
        status: "published".to_string(),
        updated_at: Utc::now(),
        ..post_v2.clone()
    };
    
    service.create_revision(
        tenant_id,
        post_v1.id,
        "en",
        &post_v2,
        &post_v3,
        user_id,
    ).await?;
    
    // List all revisions
    let revisions = service.list_revisions(
        tenant_id,
        "blog_post",
        post_v1.id,
        "en",
    ).await?;
    
    println!("Total revisions: {}", revisions.len());
    
    // List named versions
    let versions = service.list_named_versions(
        tenant_id,
        "blog_post",
        post_v1.id,
        "en",
    ).await?;
    
    println!("Named versions: {}", versions.len());
    for v in versions {
        println!("  - {}", v.version_name.unwrap());
    }
    
    // Get diff between revisions
    let diff = service.diff_revisions(
        tenant_id,
        "blog_post",
        post_v1.id,
        "en",
        1,
        3,
    ).await?;
    
    println!("\n{}", diff.human_readable());
}
```

## File Structure

```
rustok-revisions/
├─ Cargo.toml (updated: version 0.3.0, derive feature)
├─ README.md (updated)
└─ src/
   ├─ lib.rs (updated: export diff, derive)
   ├─ error.rs
   ├─ revision.rs (updated: version_name field)
   ├─ traits.rs
   ├─ tracker.rs
   ├─ backend.rs
   ├─ diff.rs            — NEW: Diff utilities
   ├─ service.rs         — updated: named versions, diff
   └─ tests.rs

rustok-revisions-derive/  — NEW: Derive macro crate
├─ Cargo.toml
└─ src/
   └─ lib.rs            — Revisionable derive macro
```

## Migration Guide

### From v0.2.0 to v0.3.0

**1. Optional: Use derive macro**

```rust
// v0.2.0 (manual):
impl Revisionable for BlogPost {
    fn content_type() -> &'static str { "blog_post" }
    fn tracked_fields() -> Vec<&'static str> { vec!["title", "content"] }
    fn ignored_fields() -> Vec<&'static str> { vec!["updated_at"] }
}

// v0.3.0 (derive):
#[derive(Revisionable)]
#[revision(content_type = "blog_post")]
struct BlogPost {
    #[revision(tracked)]
    title: String,
    #[revision(tracked)]
    content: String,
    #[revision(ignored)]
    updated_at: DateTime<Utc>,
}
```

**2. Backward compatible**

Old API still works:
```rust
// Manual impl still works in v0.3.0
impl Revisionable for BlogPost { ... }
```

**3. New features are optional**

```rust
// Named versions — optional
service.create_named_version(...).await?;

// Diff — optional
service.diff_revisions(...).await?;
```

## Comparison

| Feature | v0.2.0 | v0.3.0 |
|---------|--------|--------|
| Manual impl | ✅ | ✅ |
| Derive macro | ❌ | ✅ **NEW** |
| Named versions | ❌ | ✅ **NEW** |
| Diff utilities | ❌ | ✅ **NEW** |
| Human-readable diff | ❌ | ✅ **NEW** |
| RevisionComparator | ❌ | ✅ **NEW** |
| Retention policies | ✅ | ✅ |
| Conditional tracking | ✅ | ✅ |

## What's Next?

### Phase 3: PostgreSQL Backend

```rust
pub struct PostgresBackend {
    pool: sqlx::PgPool,
}

impl PostgresBackend {
    pub async fn new(database_url: &str) -> Result<Self, RevisionError> {
        let pool = sqlx::PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }
}
```

**Database schema:**
```sql
CREATE TABLE content_revisions (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    content_type VARCHAR(100) NOT NULL,
    content_id UUID NOT NULL,
    locale VARCHAR(10) NOT NULL,
    revision_number INTEGER NOT NULL,
    parent_revision_id UUID,
    delta JSONB NOT NULL,
    created_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    change_source VARCHAR(50) NOT NULL,
    change_summary TEXT,
    version_name VARCHAR(100),
    UNIQUE(tenant_id, content_type, content_id, locale, revision_number)
);
```

### Phase 4: Platform Integration

- `rustok-content-revisions` module
- GraphQL API
- REST API
- Admin UI

## Conclusion

**v0.3.0 — Polished and production-ready!** ✅

- ✅ **Derive macro** — better DX
- ✅ **Named versions** — mark important milestones
- ✅ **Diff utilities** — easy comparison
- ✅ **Human-readable output** — great for UI
- ✅ **Backward compatible** — no breaking changes

**Библиотека готова к использованию в production!**

Следующий шаг: PostgreSQL backend для real-world использования.
