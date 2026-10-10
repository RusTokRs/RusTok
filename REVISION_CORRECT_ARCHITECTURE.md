# Revision History — Правильная архитектура

**Дата:** 2026-10-09  
**Статус:** ✅ Архитектура утверждена

## Архитектура

```
┌─────────────────────────────────────────────────────────┐
│  Business Modules                                        │
│  (blog, forum, commerce)                                 │
│                                                          │
│  - Используют ContentRevisionApi                        │
│  - Не зависят напрямую от rustok-revisions              │
│  - Dependency injection                                  │
└─────────────────────────────────────────────────────────┘
                    │
                    │ uses
                    ▼
┌─────────────────────────────────────────────────────────┐
│  Межмодульный крейт                                      │
│  crates/integration/rustok-content-revisions/            │
│                                                          │
│  - ContentRevisionService                               │
│  - Configuration per content type                       │
│  - Migrations                                           │
│  - Unified API                                          │
│  - Platform-specific logic                              │
└─────────────────────────────────────────────────────────┘
                    │
                    │ depends on
                    ▼
┌─────────────────────────────────────────────────────────┐
│  Standalone Library                                      │
│  rustok-revisions/                                       │
│                                                          │
│  - Core traits (Revisionable)                           │
│  - RevisionService                                      │
│  - Backends (InMemory, SeaORM)                          │
│  - No platform dependencies                             │
│  - Reusable outside RusTok                              │
└─────────────────────────────────────────────────────────┘
```

## Компоненты

### 1. **Standalone Library** (`rustok-revisions`)

**Расположение:** `/home/user/rustok-revisions/`

**Назначение:** Generic библиотека для любого Rust проекта

**Что включает:**
- Core traits (`Revisionable`, `RevisionBackend`)
- Business logic (`RevisionService`)
- Backends (InMemory, SeaORM)
- Utilities (diff, tracker, retention)
- Derive macro

**Не включает:**
- ❌ Platform-specific код
- ❌ Migrations
- ❌ GraphQL/REST API

### 2. **Межмодульный крейт** (`rustok-content-revisions`)

**Расположение:** `crates/integration/rustok-content-revisions/`

**Назначение:** Интеграция библиотеки в RusTok platform

**Что включает:**
- `ContentRevisionService` — основной сервис
- `ContentRevisionApi` — public API trait
- `ContentRevisionConfig` — configuration per content type
- Migrations для RusTok
- Platform-specific logic

**Структура:**
```
crates/integration/rustok-content-revisions/
├─ Cargo.toml
└─ src/
   ├─ lib.rs          — public API
   ├─ api.rs          — ContentRevisionApi trait
   ├─ service.rs      — ContentRevisionService
   ├─ config.rs       — Configuration
   ├─ error.rs        — Errors
   └─ migrations.rs   — Database migrations
```

### 3. **Business Modules** (blog, forum, commerce)

**Назначение:** Использовать revision tracking через межмодульный крейт

**Как используют:**
```rust
use rustok_content_revisions::ContentRevisionService;

pub struct BlogService {
    revision_service: Arc<ContentRevisionService>,
    // ...
}

impl BlogService {
    async fn update_post(&self, ...) {
        let updated = self.db_update(...).await?;
        
        // Track через межмодульный крейт
        self.revision_service.track_update(
            tenant_id,
            post_id,
            "en",
            &old_post,
            &updated,
            user_id,
        ).await?;
        
        Ok(updated)
    }
}
```

## API

### ContentRevisionApi

```rust
#[async_trait]
pub trait ContentRevisionApi: Send + Sync {
    async fn track_update<T: Revisionable>(...);
    async fn track_create<T: Revisionable>(...);
    async fn list_revisions(...);
    async fn get_content_at_revision<T: Revisionable>(...);
    async fn restore_revision<T: Revisionable>(...);
    async fn create_named_version<T: Revisionable>(...);
    async fn list_named_versions(...);
    async fn diff_revisions(...);
    fn is_enabled(&self, content_type: &str) -> bool;
}
```

### Configuration

```rust
pub struct ContentRevisionConfig {
    pub enabled: bool,
    pub content_types: HashMap<String, ContentTypeConfig>,
    pub default_retention: RetentionPolicy,
    pub default_track_on: Vec<RevisionEvent>,
}

pub struct ContentTypeConfig {
    pub enabled: bool,
    pub track_on: Vec<RevisionEvent>,
    pub condition_field: Option<String>,
    pub condition_value: Option<String>,
    pub retention: RetentionPolicy,
    pub tracked_fields: Vec<String>,
    pub ignored_fields: Vec<String>,
}
```

**Pre-configured:**
- ✅ Blog posts — enabled, track published only
- ✅ Blog categories — disabled
- ✅ Forum topics — disabled (can enable for wiki)
- ✅ Forum posts — disabled
- ✅ Products — disabled

## Использование

### Шаг 1: Добавить dependency

```toml
# crates/modules/rustok-blog/Cargo.toml
[dependencies]
rustok-content-revisions = { path = "../../integration/rustok-content-revisions" }
```

### Шаг 2: Dependency injection

```rust
use rustok_content_revisions::ContentRevisionService;

pub struct BlogService {
    db: DatabaseConnection,
    revision_service: Arc<ContentRevisionService>,
}

impl BlogService {
    pub fn new(
        db: DatabaseConnection,
        revision_service: Arc<ContentRevisionService>,
    ) -> Self {
        Self {
            db,
            revision_service,
        }
    }
}
```

### Шаг 3: Track updates

```rust
impl BlogService {
    pub async fn update_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        new_post: BlogPost,
        user_id: Uuid,
    ) -> Result<BlogPost, Error> {
        // Get old post
        let old_post = self.get_post(tenant_id, post_id).await?;
        
        // Update in database
        let updated = self.db_update_post(&new_post).await?;
        
        // Track revision
        self.revision_service.track_update(
            tenant_id,
            post_id,
            "en",
            &old_post,
            &updated,
            user_id,
        ).await?;
        
        Ok(updated)
    }
}
```

### Шаг 4: Implement Revisionable

```rust
use rustok_revisions_derive::Revisionable;

#[derive(Debug, Clone, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "blog_post")]
pub struct BlogPost {
    pub id: Uuid,
    pub tenant_id: Uuid,
    
    #[revision(tracked)]
    pub title: String,
    
    #[revision(tracked)]
    pub content: String,
    
    #[revision(tracked)]
    pub status: PostStatus,
    
    #[revision(ignored)]
    pub updated_at: DateTime<Utc>,
    
    #[revision(ignored)]
    pub view_count: i32,
}
```

## Преимущества

### 1. **Separation of Concerns**

- Library: generic revision tracking
- Integration crate: RusTok-specific logic
- Business modules: use integration crate

### 2. **Reusability**

Library можно использовать вне RusTok:
```rust
// В другом проекте
use rustok_revisions::{RevisionService, SeaOrmBackend};
```

### 3. **Centralized Configuration**

Все configurations в одном месте:
```rust
// В content-revisions crate
impl ContentRevisionConfig {
    fn configure_blog(&mut self) { ... }
    fn configure_forum(&mut self) { ... }
    fn configure_commerce(&mut self) { ... }
}
```

### 4. **Consistent API**

Все modules используют одинаковый API:
```rust
// Blog
revision_service.track_update(..., "blog_post", ...).await?;

// Forum
revision_service.track_update(..., "forum_topic", ...).await?;

// Commerce
revision_service.track_update(..., "product", ...).await?;
```

### 5. **Easy Testing**

```rust
#[cfg(test)]
mod tests {
    use rustok_content_revisions::{ContentRevisionService, ContentRevisionConfig};
    use rustok_revisions::InMemoryBackend;

    #[tokio::test]
    async fn test_blog_revision_tracking() {
        let backend = InMemoryBackend::new();
        let service = ContentRevisionService::new(backend, ContentRevisionConfig::default());
        
        // Test without database
    }
}
```

## Dependency Graph

```
rustok-content-revisions (integration crate)
    ├── depends on: rustok-revisions (library)
    ├── depends on: rustok-core (platform types)
    ├── depends on: sea-orm (для migrations)
    └── provides: ContentRevisionApi

rustok-blog
    ├── depends on: rustok-content-revisions
    └── uses: ContentRevisionService

rustok-forum
    ├── depends on: rustok-content-revisions
    └── uses: ContentRevisionService (если enabled)

rustok-commerce
    ├── depends on: rustok-content-revisions
    └── uses: ContentRevisionService (если enabled)
```

## Что дальше?

### Шаг 1: Интеграция с blog module

1. Добавить dependency в `rustok-blog/Cargo.toml`
2. Implement `Revisionable` для `BlogPost`
3. Inject `ContentRevisionService` в `BlogService`
4. Track updates в lifecycle hooks

### Шаг 2: GraphQL API

```graphql
type Revision {
    id: ID!
    revisionNumber: Int!
    delta: JSON!
    createdBy: User!
    createdAt: DateTime!
    changeSource: String!
    changeSummary: String
    versionName: String
}

extend type BlogPost {
    revisions: [Revision!]!
    restoreRevision(revisionNumber: Int!): BlogPost!
    diffRevisions(from: Int!, to: Int!): RevisionDiff!
}
```

### Шаг 3: Admin UI

- List revisions для content
- View diff
- Restore to previous version
- Create named versions

## Conclusion

**Архитектура готова!** ✅

- ✅ **Standalone library** — generic, reusable
- ✅ **Integration crate** — RusTok-specific
- ✅ **Business modules** — use integration crate
- ✅ **Separation of concerns**
- ✅ **Centralized configuration**
- ✅ **Consistent API**

**Следующий шаг:** Интегрировать с blog module.
