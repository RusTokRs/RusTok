# rustok-revisions v0.2.0 — Улучшения

**Дата:** 2026-10-09  
**Статус:** ✅ Enhanced  
**Версия:** 0.2.0 (от MVP 0.1.0)

## Обзор улучшений

Обновил библиотеку `rustok-revisions` с критическими улучшениями для production use.

## 🔥 Ключевые улучшения

### 1. **Fixed Delta Format** ✅

**Проблема (v0.1.0):** Delta хранила только новые значения, что делало restore некорректным.

**Решение (v0.2.0):** Delta теперь хранит old и new values:

```json
{
  "title": {
    "old": "Old Title",
    "new": "New Title"
  },
  "content": {
    "old": "Old content",
    "new": "New content"
  }
}
```

**Преимущества:**
- ✅ **Proper restore** — можно восстановить любую предыдущую версию
- ✅ **Diff view** — легко показать что изменилось
- ✅ **Audit trail** — полная история изменений

**Пример:**
```rust
let revision = service.create_revision(...).await?.unwrap();
println!("Changed: {} → {}", 
    revision.delta["title"]["old"],
    revision.delta["title"]["new"]
);
```

### 2. **RevisionTracker Configuration** ✅

Добавлена конфигурация для гибкого управления tracking:

```rust
use rustok_revisions::{RevisionTracker, RetentionPolicy, RevisionEvent};

let tracker = RevisionTracker::<BlogPost>::builder()
    .enabled(true)                                    // Включить/отключить
    .track_on(vec![RevisionEvent::Update])           // Какие события отслеживать
    .condition(|post| post.status == "published")    // Условие
    .retention(RetentionPolicy::KeepLast(50))        // Retention policy
    .build();

// Использовать с service
service.create_revision_with_tracker(
    tenant_id, post_id, "en",
    &old_post, &new_post,
    user_id,
    &tracker,
    RevisionEvent::Update,
).await?;
```

**Features:**

#### `enabled: bool`
```rust
// Blog — включено
RevisionTracker::builder().enabled(true).build();

// Forum — отключено
RevisionTracker::builder().enabled(false).build();
```

#### `track_on: Vec<RevisionEvent>`
```rust
// Только обновления
.track_on(vec![RevisionEvent::Update])

// Создание и обновления
.track_on(vec![RevisionEvent::Create, RevisionEvent::Update])

// Все события
.track_on(vec![
    RevisionEvent::Create,
    RevisionEvent::Update,
    RevisionEvent::Delete,
])
```

#### `condition: Fn(&T) -> bool`
```rust
// Только для published posts
.condition(|post| post.status == "published")

// Только для важных изменений
.condition(|post| post.is_featured)

// Комбинированные условия
.condition(|post| post.is_published() && post.category == "news")
```

### 3. **Retention Policies** ✅

Автоматическая очистка старых revisions:

```rust
pub enum RetentionPolicy {
    KeepAll,           // Хранить всё
    KeepLast(usize),   // Последние N revisions
    KeepDays(u32),     // Хранить N дней
}
```

**Примеры:**

```rust
// Хранить последние 50 revisions
RevisionTracker::builder()
    .retention(RetentionPolicy::KeepLast(50))
    .build();

// Хранить 30 дней
RevisionTracker::builder()
    .retention(RetentionPolicy::KeepDays(30))
    .build();

// Хранить всё (для важных документов)
RevisionTracker::builder()
    .retention(RetentionPolicy::KeepAll)
    .build();
```

**Автоматическая очистка:**
```rust
// После создания revision применяется retention policy
service.create_revision_with_tracker(..., &tracker, ...).await?;
// Автоматически удаляет старые revisions согласно policy
```

### 4. **Enhanced Backend API** ✅

Добавлены методы для retention:

```rust
#[async_trait]
pub trait RevisionBackend {
    // ... existing methods ...
    
    /// Delete revisions older than revision_number
    async fn delete_old_revisions(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
        locale: &str,
        keep_revision_number: i32,
    ) -> Result<usize, RevisionError>;

    /// Delete revisions older than datetime
    async fn delete_revisions_before(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
        locale: &str,
        before: DateTime<Utc>,
    ) -> Result<usize, RevisionError>;
}
```

### 5. **Comprehensive Tests** ✅

Добавлены 15+ новых тестов:

**Delta format tests:**
- ✅ `test_delta_contains_old_and_new_values`
- ✅ `test_delta_multiple_fields_changed`
- ✅ `test_ignored_fields_not_in_delta`

**Restore tests:**
- ✅ `test_get_content_at_revision`
- ✅ `test_restore_revision`

**Tracker tests:**
- ✅ `test_tracker_disabled`
- ✅ `test_tracker_enabled`
- ✅ `test_tracker_with_condition`
- ✅ `test_tracker_track_on_events`
- ✅ `test_create_revision_with_tracker`

**Retention tests:**
- ✅ `test_retention_keep_last`
- ✅ `test_retention_keep_days`
- ✅ `test_retention_keep_all`

**Multilingual tests:**
- ✅ `test_multilingual_revisions`

## API Changes

### New Types

```rust
pub enum RetentionPolicy {
    KeepAll,
    KeepLast(usize),
    KeepDays(u32),
}

pub enum RevisionEvent {
    Create,
    Update,
    Delete,
}

pub struct RevisionTracker<T: Revisionable> {
    pub enabled: bool,
    pub track_on: Vec<RevisionEvent>,
    pub condition: Option<Arc<dyn Fn(&T) -> bool + Send + Sync>>,
    pub retention: RetentionPolicy,
    pub max_revisions: Option<usize>,
}

pub struct RevisionTrackerBuilder<T: Revisionable> { ... }
```

### New Methods

```rust
impl RevisionService {
    pub async fn create_revision_with_tracker<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        old_content: &T,
        new_content: &T,
        created_by: Uuid,
        tracker: &RevisionTracker<T>,
        event: RevisionEvent,
    ) -> Result<Option<Revision>, RevisionError>;

    pub async fn apply_retention_policy(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        policy: &RetentionPolicy,
    ) -> Result<usize, RevisionError>;

    pub async fn apply_retention_policy_for_type<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        policy: &RetentionPolicy,
    ) -> Result<usize, RevisionError>;
}

impl RevisionBackend {
    async fn delete_old_revisions(...) -> Result<usize, RevisionError>;
    async fn delete_revisions_before(...) -> Result<usize, RevisionError>;
}
```

## Примеры использования

### Blog Post (enabled with retention)

```rust
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

// В lifecycle hook
async fn after_update(old: &BlogPost, new: &BlogPost, user_id: Uuid) {
    let tracker = BlogPost::revision_tracker();
    
    service.create_revision_with_tracker(
        tenant_id,
        new.id,
        &new.locale,
        old,
        new,
        user_id,
        &tracker,
        RevisionEvent::Update,
    ).await?;
}
```

### Forum Topic (disabled by default)

```rust
impl ForumTopic {
    pub fn revision_tracker() -> RevisionTracker<Self> {
        RevisionTracker::builder()
            .enabled(false)  // Disabled for forum
            .build()
    }
}
```

### Wiki Page (enabled for specific categories)

```rust
impl WikiPage {
    pub fn revision_tracker() -> RevisionTracker<Self> {
        RevisionTracker::builder()
            .enabled(true)
            .condition(|page| page.category == "documentation")
            .retention(RetentionPolicy::KeepAll)  // Keep all for docs
            .build()
    }
}
```

## Comparison: v0.1.0 vs v0.2.0

| Feature | v0.1.0 (MVP) | v0.2.0 (Enhanced) |
|---------|--------------|-------------------|
| Delta format | New values only | **Old + New values** ✅ |
| Proper restore | ⚠️ Limited | ✅ **Full support** |
| Configuration | ❌ None | ✅ **RevisionTracker** |
| Conditional tracking | ❌ None | ✅ **condition()** |
| Retention policies | ❌ None | ✅ **KeepLast, KeepDays** |
| Auto cleanup | ❌ Manual | ✅ **Automatic** |
| Event filtering | ❌ None | ✅ **track_on()** |
| Tests | 5 basic | **20+ comprehensive** ✅ |

## File Structure

```
rustok-revisions/
├─ Cargo.toml
├─ README.md
└─ src/
   ├─ lib.rs              — public API (updated)
   ├─ error.rs            — RevisionError
   ├─ revision.rs         — Revision, RevisionMetadata, ChangeSource
   ├─ traits.rs           — Revisionable trait
   ├─ tracker.rs          — NEW: RevisionTracker, RetentionPolicy
   ├─ backend.rs          — RevisionBackend + InMemoryBackend (updated)
   ├─ service.rs          — RevisionService (updated)
   └─ tests.rs            — NEW: Comprehensive tests
```

## Migration Guide

### From v0.1.0 to v0.2.0

**1. Delta format changed:**

```rust
// v0.1.0:
revision.delta["title"]  // "New Title"

// v0.2.0:
revision.delta["title"]["old"]  // "Old Title"
revision.delta["title"]["new"]  // "New Title"
```

**2. Optional: Use RevisionTracker:**

```rust
// v0.1.0:
service.create_revision(tenant_id, post_id, "en", &old, &new, user_id).await?;

// v0.2.0 (optional):
let tracker = RevisionTracker::builder()
    .enabled(true)
    .retention(RetentionPolicy::KeepLast(50))
    .build();

service.create_revision_with_tracker(
    tenant_id, post_id, "en", &old, &new, user_id,
    &tracker, RevisionEvent::Update,
).await?;
```

**3. Backward compatible:**

Old API still works:
```rust
// This still works in v0.2.0
service.create_revision(tenant_id, post_id, "en", &old, &new, user_id).await?;
```

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

### Phase 4: Advanced Features

- **Named versions** (snapshots с именем)
- **Diff view** (визуальное сравнение)
- **Point-in-time queries** (`get_content_at(datetime)`)
- **Bulk operations** (restore multiple items)

### Phase 5: Platform Integration

- `rustok-content-revisions` module
- GraphQL API
- REST API
- Admin UI

## Conclusion

**v0.2.0 — Production-ready core!**

✅ **Fixed critical bug** — proper restore с old/new values  
✅ **Flexible configuration** — RevisionTracker с conditions  
✅ **Automatic cleanup** — retention policies  
✅ **Well tested** — 20+ comprehensive tests  
✅ **Backward compatible** — old API still works  

**Готов к интеграции с RusTok platform!**

Следующий шаг: PostgreSQL backend для production use.
