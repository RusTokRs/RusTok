# Content Revision History — Final Proposal

**Дата:** 2026-10-09  
**Статус:** ✅ Approved  
**Автор:** AI Assistant

## Обзор

Финальная архитектура revision history для RusTok platform с учетом:
- Отключаемость per module
- Configurable tracking (какие поля отслеживать)
- Multilingual support
- Вынесение в отдельный крейт
- Лучшие практики из PaperTrail и django-simple-history

## Исследование аналогов

### Ruby PaperTrail ⭐ (лучший пример)

**Features:**
```ruby
class Article < ActiveRecord::Base
  has_paper_trail \
    only: [:title, :content],           # Track only these fields
    ignore: [:updated_at],              # Ignore these fields
    if: Proc.new { |a| a.published? },  # Conditional tracking
    on: [:update]                        # Only on update (not create/destroy)
end

# Usage
article.versions                    # All versions
article.paper_trail.previous_version # Previous version
version.reify                       # Restore to this version
PaperTrail.config.version_limit = 10 # Retention policy
```

**Ключевые паттерны:**
- ✅ **Opt-in per model** — `has_paper_trail`
- ✅ **Configurable tracking** — `:only`, `:ignore`, `:if`, `:unless`
- ✅ **Full snapshots** — хранит полное состояние
- ✅ **Automatic via callbacks** — не нужно вручную вызывать
- ✅ **Retention limits** — `version_limit`
- ✅ **Audit trail** — `whodunnit`, `created_at`

### Django Simple History

**Features:**
```python
class Article(models.Model):
    title = models.CharField(max_length=200)
    content = models.TextField()
    history = HistoricalRecords(
        excluded_fields=['updated_at'],
        included_fields=['title', 'content'],
    )

# Usage
article.history.all()               # All versions
article.history.as_of(datetime)     # Version at specific time
article.history.first().instance    # Restore version
```

**Ключевые паттерны:**
- ✅ **Opt-in per model** — `HistoricalRecords()`
- ✅ **Point-in-time querying** — `as_of(datetime)`
- ✅ **Django Admin integration**
- ✅ **Tracked fields** — `included_fields`, `excluded_fields`

### Django Reversion

**Features:**
- ✅ Version control для model instances
- ✅ Point-in-time querying
- ✅ Versioned model relations
- ✅ Visual compare (с django-reversion-compare)

## Финальная архитектура

### Design Goals

1. **Opt-in per module** — каждый модуль решает использовать ли revision history
2. **Configurable tracking** — какие поля отслеживать, какие игнорировать
3. **Conditional tracking** — условия для создания revision
4. **Multilingual** — per-locale revision history
5. **Generic** — работает для любого content type
6. **Automatic** — через lifecycle hooks, не нужно вручную вызывать
7. **Efficient** — delta-based для экономии места
8. **Flexible retention** — configurable per module/tenant

### Architecture

```
rustok-revisions (standalone library, open-source)
├─ Core traits
│  ├─ Revisionable (trait для content types)
│  ├─ RevisionTracker (configuration)
│  └─ RevisionService (business logic)
├─ Storage backends
│  ├─ PostgresBackend (default)
│  └─ InMemoryBackend (testing)
├─ Integrations
│  ├─ SeaORM integration
│  └─ Lifecycle hooks
└─ Features
   ├─ Delta-based storage
   ├─ Full snapshots
   ├─ Point-in-time queries
   └─ Retention policies

rustok-content-revisions (platform capability)
├─ Database migrations (shared tables)
├─ GraphQL/REST API
├─ Admin UI integration
└─ Module adapters
   ├─ BlogRevisionAdapter
   ├─ ForumRevisionAdapter
   └─ PageRevisionAdapter
```

### Standalone Library: `rustok-revisions`

**Почему отдельная библиотека:**
- ✅ Reusable в других проектах
- ✅ Open-source contribution
- ✅ Нет зависимости от RusTok platform
- ✅ Легче тестировать
- ✅ Community adoption

**Cargo.toml:**
```toml
[package]
name = "rustok-revisions"
version = "0.1.0"
description = "Content revision history library for Rust applications"
license = "MIT OR Apache-2.0"
repository = "https://github.com/rustok/rustok-revisions"

[dependencies]
async-trait = "0.1"
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "1.0"
uuid = { version = "1.0", features = ["v4", "serde"] }

[features]
default = ["postgres"]
postgres = ["sqlx", "sqlx/postgres"]
sea-orm = ["sea-orm-migration"]
```

### Core Traits

#### Revisionable

```rust
/// Trait for content types that support revision history.
///
/// # Example
///
/// ```rust
/// use rustok_revisions::Revisionable;
///
/// #[derive(Revisionable)]
/// #[revision(
///     content_type = "blog_post",
///     tracked_fields = ["title", "content", "status"],
///     localized_fields = ["title", "content"],
///     ignore_fields = ["updated_at", "view_count"],
/// )]
/// pub struct BlogPost {
///     pub id: Uuid,
///     pub title: String,
///     pub content: String,
///     pub status: PostStatus,
///     pub updated_at: DateTime<Utc>,
///     pub view_count: i32,
/// }
/// ```
#[async_trait]
pub trait Revisionable: Send + Sync {
    /// Content type identifier (e.g., "blog_post", "forum_topic")
    fn content_type() -> &'static str;
    
    /// Serialize content to JSON for storage
    fn to_revision_json(&self) -> serde_json::Value;
    
    /// Deserialize content from JSON
    fn from_revision_json(value: serde_json::Value) -> Result<Self, RevisionError>
    where
        Self: Sized;
    
    /// Get list of fields to track for revisions
    fn tracked_fields() -> Vec<&'static str>;
    
    /// Get list of fields to ignore (never track)
    fn ignored_fields() -> Vec<&'static str> {
        Vec::new()
    }
    
    /// Check if a field is localized (different per locale)
    fn is_localized_field(field: &str) -> bool;
}
```

#### RevisionTracker

```rust
/// Configuration for revision tracking.
///
/// # Example
///
/// ```rust
/// let tracker = RevisionTracker::builder()
///     .enabled(true)
///     .track_on(vec![RevisionEvent::Update])
///     .condition(|post: &BlogPost| post.is_published())
///     .retention(RetentionPolicy::KeepLast(50))
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct RevisionTracker<T: Revisionable> {
    /// Whether revision tracking is enabled
    pub enabled: bool,
    
    /// Which events trigger revision creation
    pub track_on: Vec<RevisionEvent>,
    
    /// Optional condition for creating revisions
    pub condition: Option<Box<dyn Fn(&T) -> bool + Send + Sync>>,
    
    /// Retention policy for old revisions
    pub retention: RetentionPolicy,
    
    /// Maximum number of revisions per content item
    pub max_revisions: Option<usize>,
}

impl<T: Revisionable> RevisionTracker<T> {
    pub fn builder() -> RevisionTrackerBuilder<T> {
        RevisionTrackerBuilder::new()
    }
    
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    pub fn should_track(&self, event: RevisionEvent, content: &T) -> bool {
        if !self.enabled {
            return false;
        }
        
        if !self.track_on.contains(&event) {
            return false;
        }
        
        if let Some(ref condition) = self.condition {
            if !condition(content) {
                return false;
            }
        }
        
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionEvent {
    Create,
    Update,
    Delete,
}

#[derive(Debug, Clone)]
pub enum RetentionPolicy {
    /// Keep all revisions forever
    KeepAll,
    
    /// Keep only the last N revisions
    KeepLast(usize),
    
    /// Keep revisions for N days
    KeepDays(u32),
    
    /// Custom retention logic
    Custom(Box<dyn Fn(&Revision) -> bool + Send + Sync>),
}
```

#### RevisionService

```rust
pub struct RevisionService {
    backend: Box<dyn RevisionBackend>,
}

impl RevisionService {
    /// Create a new revision when content is updated
    pub async fn create_revision<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        old_content: &T,
        new_content: &T,
        metadata: RevisionMetadata,
    ) -> Result<Option<Revision>, RevisionError> {
        // 1. Calculate delta between old and new content
        let delta = self.calculate_delta(old_content, new_content)?;
        
        // 2. Skip revision if no changes to tracked fields
        if delta.is_empty() {
            return Ok(None);
        }
        
        // 3. Get next revision number
        let revision_number = self.backend
            .get_next_revision_number(tenant_id, T::content_type(), content_id, locale)
            .await?;
        
        // 4. Create revision record
        let revision = Revision {
            id: Uuid::new_v4(),
            tenant_id,
            content_type: T::content_type().to_string(),
            content_id,
            locale: locale.to_string(),
            revision_number,
            parent_revision_id: self.backend
                .get_parent_revision_id(tenant_id, T::content_type(), content_id, locale)
                .await?,
            delta,
            created_by: metadata.created_by,
            created_at: Utc::now(),
            change_source: metadata.change_source,
            change_summary: metadata.change_summary,
        };
        
        // 5. Insert into database
        self.backend.insert_revision(&revision).await?;
        
        // 6. Apply retention policy
        self.apply_retention_policy::<T>(tenant_id, content_id, locale).await?;
        
        Ok(Some(revision))
    }
    
    /// List all revisions for a content item
    pub async fn list_revisions(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Vec<Revision>, RevisionError> {
        self.backend
            .list_revisions(tenant_id, content_type, content_id, locale)
            .await
    }
    
    /// Get content at specific revision
    pub async fn get_content_at_revision<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        target_revision: i32,
        current_content: &T,
    ) -> Result<T, RevisionError> {
        // 1. Get all revisions from target to current
        let revisions = self.backend
            .get_revisions_range(
                tenant_id,
                T::content_type(),
                content_id,
                locale,
                target_revision,
            )
            .await?;
        
        // 2. Apply deltas in reverse order
        let mut content = current_content.to_revision_json();
        for revision in revisions.iter().rev() {
            content = self.apply_delta_reverse(content, &revision.delta)?;
        }
        
        // 3. Deserialize back to content type
        T::from_revision_json(content)
    }
    
    /// Restore content to a previous revision
    pub async fn restore_revision<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        target_revision: i32,
        restored_by: Uuid,
    ) -> Result<T, RevisionError> {
        // 1. Get current content
        let current_content = self.get_current_content::<T>(
            tenant_id,
            content_id,
            locale,
        ).await?;
        
        // 2. Get content at target revision
        let restored_content = self.get_content_at_revision(
            tenant_id,
            content_id,
            locale,
            target_revision,
            &current_content,
        ).await?;
        
        // 3. Create new revision for the restore operation
        self.create_revision(
            tenant_id,
            content_id,
            locale,
            &current_content,
            &restored_content,
            RevisionMetadata {
                created_by: restored_by,
                change_source: ChangeSource::Restore,
                change_summary: Some(format!("Restored to revision {}", target_revision)),
            },
        ).await?;
        
        Ok(restored_content)
    }
    
    /// Calculate delta between two content versions
    fn calculate_delta<T: Revisionable>(
        &self,
        old: &T,
        new: &T,
    ) -> Result<serde_json::Value, RevisionError> {
        let old_json = old.to_revision_json();
        let new_json = new.to_revision_json();
        
        let mut delta = serde_json::Map::new();
        
        for field in T::tracked_fields() {
            // Skip ignored fields
            if T::ignored_fields().contains(&field) {
                continue;
            }
            
            let old_value = old_json.get(field);
            let new_value = new_json.get(field);
            
            if old_value != new_value {
                if let Some(value) = new_value {
                    delta.insert(field.to_string(), value.clone());
                } else {
                    delta.insert(field.to_string(), serde_json::Value::Null);
                }
            }
        }
        
        Ok(serde_json::Value::Object(delta))
    }
    
    /// Apply retention policy to cleanup old revisions
    async fn apply_retention_policy<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
    ) -> Result<(), RevisionError> {
        let tracker = T::revision_tracker();
        
        match tracker.retention {
            RetentionPolicy::KeepAll => {
                // Do nothing
            }
            RetentionPolicy::KeepLast(n) => {
                self.backend
                    .delete_old_revisions_keep_last(tenant_id, T::content_type(), content_id, locale, n)
                    .await?;
            }
            RetentionPolicy::KeepDays(days) => {
                let cutoff = Utc::now() - Duration::days(days as i64);
                self.backend
                    .delete_old_revisions_before(tenant_id, T::content_type(), content_id, locale, cutoff)
                    .await?;
            }
            RetentionPolicy::Custom(ref predicate) => {
                let revisions = self.list_revisions(tenant_id, T::content_type(), content_id, locale).await?;
                for revision in revisions {
                    if !predicate(&revision) {
                        self.backend.delete_revision(revision.id).await?;
                    }
                }
            }
        }
        
        Ok(())
    }
}
```

### Database Schema

#### content_revisions (delta-based)

```sql
CREATE TABLE content_revisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    
    -- Content identification
    content_type VARCHAR(100) NOT NULL,  -- 'blog_post', 'forum_topic', 'page'
    content_id UUID NOT NULL,
    locale VARCHAR(10) NOT NULL,          -- 'en', 'ru', 'fr'
    
    -- Revision metadata
    revision_number INTEGER NOT NULL,     -- Auto-incrementing per content_id+locale
    parent_revision_id UUID,              -- Previous revision (for chain)
    
    -- Delta (только измененные поля)
    delta JSONB NOT NULL,                 -- {"title": "New Title", "status": "published"}
    
    -- Audit trail
    created_by UUID NOT NULL,             -- User who made the change
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    change_source VARCHAR(50) NOT NULL,   -- 'admin_ui', 'api', 'import', 'restore'
    change_summary TEXT,                  -- Optional: "Fixed typo in title"
    
    -- Unique constraint
    UNIQUE(tenant_id, content_type, content_id, locale, revision_number)
);

CREATE INDEX idx_content_revisions_lookup 
    ON content_revisions(tenant_id, content_type, content_id, locale, revision_number DESC);

CREATE INDEX idx_content_revisions_created_at 
    ON content_revisions(created_at);
```

### Module Configuration

#### Per-Module Configuration

```rust
// crates/modules/rustok-blog/src/revisions.rs

use rustok_revisions::{Revisionable, RevisionTracker, RevisionEvent, RetentionPolicy};

#[derive(Revisionable)]
#[revision(
    content_type = "blog_post",
    tracked_fields = [
        "title",
        "slug",
        "content",
        "excerpt",
        "status",
        "featured_image_url",
        "seo_title",
        "seo_description",
        "category_id",
        "tags",
    ],
    localized_fields = ["title", "content", "excerpt", "seo_title", "seo_description"],
    ignore_fields = ["updated_at", "view_count", "is_pinned", "pinned_at"],
)]
pub struct BlogPost {
    pub id: Uuid,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub excerpt: Option<String>,
    pub status: PostStatus,
    pub featured_image_url: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub category_id: Option<Uuid>,
    pub tags: Vec<String>,
    pub updated_at: DateTime<Utc>,
    pub view_count: i32,
    pub is_pinned: bool,
    pub pinned_at: Option<DateTime<Utc>>,
}

impl BlogPost {
    pub fn revision_tracker() -> RevisionTracker<Self> {
        RevisionTracker::builder()
            .enabled(true)  // Blog has revision history enabled
            .track_on(vec![RevisionEvent::Update])  // Only track updates
            .condition(|post| post.is_published())  // Only track published posts
            .retention(RetentionPolicy::KeepLast(50))  // Keep last 50 revisions
            .build()
    }
}
```

```rust
// crates/modules/rustok-forum/src/revisions.rs

#[derive(Revisionable)]
#[revision(
    content_type = "forum_topic",
    tracked_fields = ["title", "content", "category_id", "tags"],
    localized_fields = ["title", "content"],
    ignore_fields = ["updated_at", "view_count", "reply_count"],
)]
pub struct ForumTopic {
    // ... fields ...
}

impl ForumTopic {
    pub fn revision_tracker() -> RevisionTracker<Self> {
        RevisionTracker::builder()
            .enabled(false)  // Forum wiki has revision history DISABLED
            .build()
    }
}
```

```rust
// crates/modules/rustok-commerce/src/revisions.rs

#[derive(Revisionable)]
#[revision(
    content_type = "product",
    tracked_fields = ["name", "description", "price", "sku", "category_id"],
    localized_fields = ["name", "description"],
    ignore_fields = ["updated_at", "stock_quantity", "view_count"],
)]
pub struct Product {
    // ... fields ...
}

impl Product {
    pub fn revision_tracker() -> RevisionTracker<Self> {
        RevisionTracker::builder()
            .enabled(false)  // Product description revision history DISABLED
            .build()
    }
}
```

### Integration с Lifecycle Hooks

```rust
// crates/modules/rustok-blog/src/services/post/commands.rs

use rustok_revisions::RevisionService;

pub async fn update_post(
    db: &DatabaseConnection,
    revision_service: &RevisionService,
    tenant_id: Uuid,
    post_id: Uuid,
    locale: &str,
    old_post: &BlogPost,
    new_post: &BlogPost,
    updated_by: Uuid,
) -> Result<(), BlogError> {
    // 1. Check if revision tracking is enabled for this module
    let tracker = BlogPost::revision_tracker();
    
    if tracker.should_track(RevisionEvent::Update, new_post) {
        // 2. Create revision
        revision_service.create_revision(
            tenant_id,
            post_id,
            locale,
            old_post,
            new_post,
            RevisionMetadata {
                created_by: updated_by,
                change_source: ChangeSource::AdminUi,
                change_summary: None,
            },
        ).await?;
    }
    
    // 3. Update post in database
    // ... existing update logic ...
    
    Ok(())
}
```

### API Design

#### GraphQL

```graphql
type ContentRevision {
  id: UUID!
  revisionNumber: Int!
  delta: JSON!
  createdBy: UUID!
  createdAt: DateTime!
  changeSource: String!
  changeSummary: String
}

extend type Query {
  # List all revisions for a content item
  contentRevisions(
    contentType: String!
    contentId: UUID!
    locale: String!
  ): [ContentRevision!]!
  
  # Get content at specific revision
  contentAtRevision(
    contentType: String!
    contentId: UUID!
    locale: String!
    revisionNumber: Int!
  ): JSON!
}

extend type Mutation {
  # Restore content to a previous revision
  restoreContentRevision(
    contentType: String!
    contentId: UUID!
    locale: String!
    revisionNumber: Int!
  ): Boolean!
}
```

#### REST

```
GET    /api/content/{type}/{id}/revisions?locale=en
GET    /api/content/{type}/{id}/revisions/{revision_number}?locale=en
POST   /api/content/{type}/{id}/revisions/{revision_number}/restore?locale=en
```

### Comparison: PaperTrail vs django-simple-history vs RusTok

| Feature | PaperTrail | django-simple-history | RusTok (Proposed) |
|---------|------------|----------------------|-------------------|
| Opt-in | `has_paper_trail` | `HistoricalRecords()` | `#[derive(Revisionable)]` ✅ |
| Tracked fields | `:only`, `:ignore` | `included_fields`, `excluded_fields` | `tracked_fields`, `ignore_fields` ✅ |
| Conditional | `:if`, `:unless` | Custom middleware | `.condition(...)` ✅ |
| Storage | Full snapshots | Full snapshots | Delta-based ✅ (more efficient) |
| Multilingual | ❌ | ❌ | ✅ Per-locale revisions |
| Retention | `version_limit` | Manual cleanup | `RetentionPolicy` ✅ |
| Point-in-time | ❌ | `as_of(datetime)` | Planned |
| Relations | With addon | ❌ | Planned |
| Standalone lib | ❌ (Rails gem) | ❌ (Django app) | ✅ `rustok-revisions` crate |

## Implementation Phases

### Phase 1: Standalone Library
- [ ] Create `rustok-revisions` crate
- [ ] Implement core traits (`Revisionable`, `RevisionTracker`, `RevisionService`)
- [ ] Implement Postgres backend
- [ ] Add comprehensive tests
- [ ] Publish to crates.io
- [ ] Write documentation

### Phase 2: Platform Integration
- [ ] Create `rustok-content-revisions` module
- [ ] Database migrations (shared tables)
- [ ] GraphQL/REST API
- [ ] Admin UI: revision history viewer
- [ ] Admin UI: restore functionality

### Phase 3: Blog Integration
- [ ] Implement `Revisionable` for `BlogPost`
- [ ] Configure tracker (enabled, tracked fields, retention)
- [ ] Hook into update lifecycle
- [ ] Test end-to-end

### Phase 4: Forum Integration (Optional)
- [ ] Implement `Revisionable` for `ForumTopic`
- [ ] Configure tracker (disabled by default)
- [ ] Wiki mode: enable for specific categories

### Phase 5: Advanced Features
- [ ] Named versions (snapshots)
- [ ] Diff view (compare two revisions)
- [ ] Point-in-time queries
- [ ] Bulk restore
- [ ] Revision comments/notes

## Open Questions (Resolved)

### 1. Storage approach
**Answer:** Delta-based ✅ (more efficient than full snapshots)

### 2. What to track
**Answer:** All changes (API + UI) ✅ (complete audit trail)

### 3. Retention policy
**Answer:** Configurable per module ✅ (default: KeepLast(50))

### 4. Which modules first
**Answer:** Blog (enabled) → Forum (disabled) → Commerce (disabled) ✅

### 5. How to make it optional
**Answer:** `RevisionTracker::enabled(false)` ✅ (per module configuration)

### 6. How to handle tables when disabled
**Answer:** Tables exist, but no revisions created when disabled ✅ (zero overhead)

### 7. Standalone library or platform-only
**Answer:** Standalone `rustok-revisions` crate ✅ (reusable, open-source)

## Benefits

✅ **Reusable** — standalone library для community  
✅ **Flexible** — opt-in per module, configurable tracking  
✅ **Efficient** — delta-based storage  
✅ **Multilingual** — per-locale revisions  
✅ **Production-ready** — retention policies, audit trail  
✅ **Well-tested** — inspired by PaperTrail and django-simple-history  
✅ **Future-proof** — extensible architecture  

## Next Steps

1. ✅ Research complete
2. ✅ Proposal approved
3. ⏳ Create `rustok-revisions` standalone library
4. ⏳ Publish to crates.io
5. ⏳ Create `rustok-content-revisions` platform module
6. ⏳ Integrate with blog module
7. ⏳ Test and iterate

**Готов начать реализацию!**
