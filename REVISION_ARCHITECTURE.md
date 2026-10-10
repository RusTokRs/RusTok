# Revision History Architecture — Обоснование

## Проблема

Библиотека `rustok-revisions` создана вне RusTok, и я пытался интегрировать её напрямую в модули. Это нарушает архитектуру платформы.

## Правильная архитектура

### Два уровня абстракции

```
┌─────────────────────────────────────────────────────────┐
│  RusTok Platform                                         │
│                                                          │
│  ┌────────────────────────────────────────────────┐    │
│  │  crates/modules/rustok-content-revisions/      │    │
│  │  (Platform Module)                             │    │
│  │                                                 │    │
│  │  - Database migrations                         │    │
│  │  - GraphQL/REST API                            │    │
│  │  - Admin UI integration                        │    │
│  │  - Module-specific configurations              │    │
│  │  - Lifecycle hooks                             │    │
│  └────────────────────────────────────────────────┘    │
│                         │                                │
│                         │ depends on                     │
│                         ▼                                │
│  ┌────────────────────────────────────────────────┐    │
│  │  crates/libs/rustok-revisions/                 │    │
│  │  (Standalone Library)                          │    │
│  │                                                 │    │
│  │  - Core traits (Revisionable)                  │    │
│  │  - RevisionService                             │    │
│  │  - Backends (InMemory, SeaORM)                 │    │
│  │  - No platform dependencies                    │    │
│  │  - Can be used outside RusTok                  │    │
│  └────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
```

## Почему два уровня?

### 1. **Standalone Library** (`rustok-revisions`)

**Цель:** Generic библиотека для любого Rust проекта

**Что включает:**
- Core traits (`Revisionable`, `RevisionBackend`)
- Business logic (`RevisionService`)
- Backends (InMemory, SeaORM)
- Utilities (diff, tracker, retention)

**Что НЕ включает:**
- ❌ Platform-specific код
- ❌ Database migrations (это responsibility of platform module)
- ❌ GraphQL/REST API
- ❌ Admin UI
- ❌ Dependencies на RusTok modules

**Преимущества:**
- ✅ **Reusable** — можно использовать в других проектах
- ✅ **Open-source** — publish to crates.io
- ✅ **Testable** — нет platform dependencies
- ✅ **Focused** — одна responsibility: revision tracking

**Пример использования вне RusTok:**
```rust
// Любой Rust проект
use rustok_revisions::{RevisionService, SeaOrmBackend};

let backend = SeaOrmBackend::new(db);
let service = RevisionService::new(Box::new(backend));
service.create_revision(...).await?;
```

### 2. **Platform Module** (`rustok-content-revisions`)

**Цель:** Интеграция библиотеки в RusTok platform

**Что включает:**
- Database migrations для RusTok
- GraphQL API для revision history
- REST API для revision history
- Admin UI components
- Module-specific configurations (blog, forum, commerce)
- Lifecycle hooks integration
- Tenant isolation

**Что НЕ включает:**
- ❌ Core revision logic (это в библиотеке)
- ❌ Backend implementations

**Преимущества:**
- ✅ **Platform-specific** — знает о RusTok architecture
- ✅ **Centralized** — один module для всех revision features
- ✅ **Consistent API** — unified GraphQL/REST endpoints
- ✅ **Easy integration** — другие modules просто depend on this

**Пример использования в RusTok:**
```rust
// В blog module
use rustok_content_revisions::RevisionApi;

impl BlogService {
    async fn update_post(&self, ...) {
        // Update post
        let updated = self.db_update(...).await?;
        
        // Track revision (через platform module)
        self.revision_api.track_update(
            tenant_id,
            "blog_post",
            post_id,
            &old_post,
            &updated,
            user_id,
        ).await?;
        
        Ok(updated)
    }
}
```

## Comparison: Direct vs Module Integration

### ❌ Direct Integration (неправильно)

```rust
// В blog module напрямую
use rustok_revisions::{RevisionService, SeaOrmBackend, RevisionTracker};

impl BlogService {
    fn new(db: DatabaseConnection) -> Self {
        // Каждый module создает свой service
        let backend = SeaOrmBackend::new(db.clone());
        let revision_service = RevisionService::new(Box::new(backend));
        
        Self { revision_service, ... }
    }
    
    async fn update_post(&self, ...) {
        // Каждый module настраивает tracker
        let tracker = RevisionTracker::builder()
            .enabled(true)
            .retention(RetentionPolicy::KeepLast(50))
            .build();
        
        self.revision_service.create_revision_with_tracker(...).await?;
    }
}
```

**Проблемы:**
- ❌ **Duplication** — каждый module создает свой service
- ❌ **Inconsistency** — разные configurations в разных modules
- ❌ **No centralized API** — нет unified GraphQL/REST endpoints
- ❌ **Hard to maintain** — изменения нужно делать во всех modules
- ❌ **No admin UI** — нет unified way to view/manage revisions

### ✅ Module Integration (правильно)

```rust
// В content-revisions module
pub struct ContentRevisionModule {
    service: RevisionService,
    db: DatabaseConnection,
}

impl ContentRevisionModule {
    pub fn new(db: DatabaseConnection) -> Self {
        let backend = SeaOrmBackend::new(db.clone());
        let service = RevisionService::new(Box::new(backend));
        Self { service, db }
    }
    
    pub async fn track_update<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
        old: &T,
        new: &T,
        user_id: Uuid,
    ) -> Result<Option<Revision>, RevisionError> {
        // Get tracker configuration for this content type
        let tracker = self.get_tracker_for_type(content_type);
        
        self.service.create_revision_with_tracker(
            tenant_id,
            content_id,
            "en",
            old,
            new,
            user_id,
            &tracker,
            RevisionEvent::Update,
        ).await
    }
    
    // GraphQL API
    pub async fn list_revisions(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
    ) -> Result<Vec<Revision>, RevisionError> {
        self.service.list_revisions(tenant_id, content_type, content_id, "en").await
    }
}

// В blog module
use rustok_content_revisions::ContentRevisionModule;

impl BlogService {
    fn new(
        db: DatabaseConnection,
        revision_module: Arc<ContentRevisionModule>,
    ) -> Self {
        Self {
            db,
            revision_module,  // Injected dependency
            ...
        }
    }
    
    async fn update_post(&self, ...) {
        let updated = self.db_update(...).await?;
        
        // Track через platform module
        self.revision_module.track_update(
            tenant_id,
            "blog_post",
            post_id,
            &old_post,
            &updated,
            user_id,
        ).await?;
        
        Ok(updated)
    }
}
```

**Преимущества:**
- ✅ **Single source of truth** — один service для всех modules
- ✅ **Consistent configuration** — centralized tracker configs
- ✅ **Unified API** — GraphQL/REST endpoints для всех
- ✅ **Easy to maintain** — изменения в одном месте
- ✅ **Admin UI** — unified way to view/manage revisions
- ✅ **Dependency injection** — modules depend on interface, not implementation

## Dependency Graph

```
rustok-content-revisions (platform module)
    ├── depends on: rustok-revisions (library)
    ├── depends on: rustok-core (platform core)
    └── provides: RevisionApi для других modules

rustok-blog (business module)
    ├── depends on: rustok-content-revisions
    └── uses: RevisionApi для tracking

rustok-forum (business module)
    ├── depends on: rustok-content-revisions
    └── uses: RevisionApi для tracking (если enabled)

rustok-commerce (business module)
    ├── depends on: rustok-content-revisions
    └── uses: RevisionApi для tracking (если enabled)
```

## Module Responsibilities

### `rustok-revisions` (Library)

**Responsibility:** Generic revision tracking

**Public API:**
```rust
pub trait Revisionable { ... }
pub trait RevisionBackend { ... }
pub struct RevisionService { ... }
pub struct RevisionTracker<T> { ... }
pub enum RetentionPolicy { ... }
pub struct InMemoryBackend { ... }
pub struct SeaOrmBackend { ... }
```

**No dependencies on:**
- RusTok platform
- Other RusTok modules
- Platform-specific types

### `rustok-content-revisions` (Platform Module)

**Responsibility:** Platform integration

**Public API:**
```rust
pub struct ContentRevisionModule { ... }

impl ContentRevisionModule {
    pub fn new(db: DatabaseConnection) -> Self;
    
    pub async fn track_update<T: Revisionable>(...);
    pub async fn track_create<T: Revisionable>(...);
    pub async fn list_revisions(...);
    pub async fn restore_revision(...);
    pub async fn diff_revisions(...);
    
    // GraphQL resolvers
    pub fn graphql_resolvers() -> ...;
    
    // REST endpoints
    pub fn rest_routes() -> ...;
}
```

**Dependencies:**
- `rustok-revisions` (library)
- `rustok-core` (platform types)
- SeaORM (для migrations)

### `rustok-blog` (Business Module)

**Responsibility:** Blog functionality

**Uses:**
```rust
use rustok_content_revisions::ContentRevisionModule;

pub struct BlogService {
    revision_module: Arc<ContentRevisionModule>,
    // ...
}
```

**No direct dependency on:**
- `rustok-revisions` (использует через platform module)

## Benefits of This Architecture

### 1. **Separation of Concerns**

- Library: generic revision tracking
- Platform module: RusTok integration
- Business modules: use platform module

### 2. **Reusability**

Library can be used outside RusTok:
```rust
// В другом проекте
use rustok_revisions::{RevisionService, SeaOrmBackend};
```

### 3. **Testability**

Library can be tested independently:
```rust
#[cfg(test)]
mod tests {
    use rustok_revisions::{InMemoryBackend, RevisionService};
    
    #[tokio::test]
    async fn test_revision_tracking() {
        let backend = InMemoryBackend::new();
        let service = RevisionService::new(Box::new(backend));
        // Test without platform dependencies
    }
}
```

### 4. **Maintainability**

Changes to revision logic in one place:
```rust
// В content-revisions module
impl ContentRevisionModule {
    // Изменить логику tracking здесь
    // Все modules автоматически получат изменения
}
```

### 5. **Consistency**

Unified API для всех modules:
```rust
// Blog
revision_module.track_update(..., "blog_post", ...).await?;

// Forum
revision_module.track_update(..., "forum_topic", ...).await?;

// Commerce
revision_module.track_update(..., "product", ...).await?;
```

### 6. **Centralized Configuration**

```rust
impl ContentRevisionModule {
    fn get_tracker_for_type(&self, content_type: &str) -> RevisionTracker<...> {
        match content_type {
            "blog_post" => RevisionTracker::builder()
                .enabled(true)
                .retention(RetentionPolicy::KeepLast(50))
                .build(),
            
            "forum_topic" => RevisionTracker::builder()
                .enabled(false)  // Disabled by default
                .build(),
            
            "product" => RevisionTracker::builder()
                .enabled(false)
                .build(),
            
            _ => RevisionTracker::builder()
                .enabled(false)
                .build(),
        }
    }
}
```

## Implementation Plan

### Phase 1: Move Library to RusTok

```bash
# Move library to proper location
mv /home/user/rustok-revisions /home/user/RusTok/crates/libs/rustok-revisions

# Update workspace Cargo.toml
# Добавить в members: "crates/libs/rustok-revisions"
```

### Phase 2: Create Platform Module

```bash
# Create platform module
mkdir -p /home/user/RusTok/crates/modules/rustok-content-revisions/src

# Structure:
# rustok-content-revisions/
# ├── Cargo.toml
# ├── migrations/
# │   └── m0001_create_content_revisions.rs
# └── src/
#     ├── lib.rs
#     ├── module.rs
#     ├── api.rs
#     ├── graphql.rs
#     └── rest.rs
```

### Phase 3: Integrate with Blog

```rust
// В blog module
use rustok_content_revisions::ContentRevisionModule;

pub struct BlogService {
    revision_module: Arc<ContentRevisionModule>,
}
```

### Phase 4: Add GraphQL API

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

## Conclusion

**Правильная архитектура:**

1. ✅ **Standalone library** (`crates/libs/rustok-revisions`)
   - Generic, reusable, no platform dependencies
   
2. ✅ **Platform module** (`crates/modules/rustok-content-revisions`)
   - RusTok integration, migrations, API, admin UI
   
3. ✅ **Business modules** (blog, forum, commerce)
   - Use platform module, not library directly

**Преимущества:**
- Separation of concerns
- Reusability
- Testability
- Maintainability
- Consistency
- Centralized configuration

**Следующий шаг:** Переместить библиотеку в `crates/libs/` и создать platform module.
