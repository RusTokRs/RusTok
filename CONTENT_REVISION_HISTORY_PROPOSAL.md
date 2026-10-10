# Content Revision History — Architecture Proposal

**Дата:** 2026-10-09  
**Статус:** 📋 Proposal  
**Автор:** AI Assistant

## Обзор

Предложение по реализации revision history для контента в RusTok platform. Решение должно работать для:
- Blog posts
- Forum topics/wiki pages
- Любого другого content type
- С учетом многоязычности

## Исследование

### Rust библиотеки

**Найденные:**
- `revision` (0.28.0) — schema evolution, revision-tolerant serialization
- `type-history` — versioning types с backfill
- `thisversion` — schema versioning

**Проблема:** Все эти библиотеки решают **schema evolution** (как менять структуру данных), а не **content revision history** (как хранить историю изменений контента).

**Вывод:** Готовой Rust библиотеки для content revision history нет. Нужно реализовывать самостоятельно.

### Strapi v5

**Подход:**
- Built-in Content History feature
- **Full snapshots** — хранит полные копии контента
- Retention period: 14 дней (Growth), 30 дней (Enterprise)
- Автоматическая очистка старых версий
- Restore previous versions
- Draft/Publish workflow

**Multilingual:**
- Unique fields (общие для всех locales)
- Localized fields (разные для каждой locale)
- При restore unique field → восстанавливается для всех locales
- При restore localized field → только для текущей locale

**Ограничения:**
- Versions создаются только через Content Manager
- API changes не отслеживаются
- Не является полным audit log

### Directus

**Подход:**
- **Delta-based** — хранит только измененные поля
- Revisions в `directus_revisions` collection
- Versions в `directus_versions` collection
- Draft version автоматически доступна
- API endpoints для доступа к revisions

**Multilingual:**
- Translations хранятся отдельно
- Revisions отслеживают изменения в translations

**Оптимизация:**
- Delta field в `directus_versions` объединяет все revisions
- Можно prune `directus_revisions` без потери versions

## Предлагаемая архитектура

### Design Goals

1. **Generic** — работает для любого content type (blog, forum, wiki, etc.)
2. **Multilingual** — поддерживает локализованный контент
3. **Efficient** — delta-based для экономии места
4. **Flexible** — configurable retention policy
5. **Observable** — кто, когда, что изменил
6. **Recoverable** — restore to any previous version

### Architecture

```
Content Revision System
├─ rustok-content-revisions (platform capability)
│  ├─ Entities
│  │  ├─ content_revisions (revisions table)
│  │  └─ content_versions (named versions/snapshots)
│  ├─ Services
│  │  ├─ RevisionService (create, list, restore)
│  │  ├─ VersionService (create, promote, delete)
│  │  └─ RetentionService (cleanup old revisions)
│  ├─ Ports
│  │  └─ RevisionableContent trait
│  └─ Integrations
│     └─ Lifecycle hooks для auto-tracking
│
└─ Domain modules implement RevisionableContent
   ├─ rustok-blog → BlogPostRevisionAdapter
   ├─ rustok-forum → ForumTopicRevisionAdapter
   └─ rustok-pages → PageRevisionAdapter
```

### Database Schema

#### 1. content_revisions (delta-based)

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

#### 2. content_versions (named snapshots)

```sql
CREATE TABLE content_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    
    -- Content identification
    content_type VARCHAR(100) NOT NULL,
    content_id UUID NOT NULL,
    locale VARCHAR(10) NOT NULL,
    
    -- Version metadata
    version_name VARCHAR(255) NOT NULL,   -- "v1.0", "Before major rewrite", etc.
    version_label VARCHAR(50),            -- Optional: "published", "approved"
    
    -- Full snapshot (полная копия на момент создания version)
    snapshot JSONB NOT NULL,
    
    -- Link to revision
    revision_id UUID NOT NULL REFERENCES content_revisions(id),
    
    -- Audit trail
    created_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Unique constraint
    UNIQUE(tenant_id, content_type, content_id, locale, version_name)
);

CREATE INDEX idx_content_versions_lookup 
    ON content_versions(tenant_id, content_type, content_id, locale);
```

### Core Traits

#### RevisionableContent

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Trait for content types that support revision history.
///
/// Each module implements this trait to enable automatic revision tracking.
#[async_trait]
pub trait RevisionableContent: Send + Sync {
    /// Content type identifier (e.g., "blog_post", "forum_topic")
    fn content_type(&self) -> &'static str;
    
    /// Serialize content to JSON for storage
    fn to_revision_json(&self) -> serde_json::Value;
    
    /// Deserialize content from JSON
    fn from_revision_json(value: serde_json::Value) -> Result<Self, RevisionError>
    where
        Self: Sized;
    
    /// Get list of fields that should be tracked for revisions
    fn tracked_fields() -> Vec<&'static str>;
    
    /// Check if a field is localized (different per locale)
    fn is_localized_field(field: &str) -> bool;
}
```

### Core Services

#### RevisionService

```rust
pub struct RevisionService {
    db: DatabaseConnection,
    retention_days: u32,
}

impl RevisionService {
    /// Create a new revision when content is updated
    pub async fn create_revision<T: RevisionableContent>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        old_content: &T,
        new_content: &T,
        created_by: Uuid,
        change_source: ChangeSource,
        change_summary: Option<String>,
    ) -> Result<Revision, RevisionError> {
        // 1. Calculate delta between old and new content
        let delta = self.calculate_delta(old_content, new_content)?;
        
        // 2. Skip revision if no changes
        if delta.is_empty() {
            return Ok(None);
        }
        
        // 3. Get next revision number
        let revision_number = self.get_next_revision_number(
            tenant_id,
            T::content_type(),
            content_id,
            locale,
        ).await?;
        
        // 4. Create revision record
        let revision = Revision {
            id: Uuid::new_v4(),
            tenant_id,
            content_type: T::content_type().to_string(),
            content_id,
            locale: locale.to_string(),
            revision_number,
            parent_revision_id: self.get_parent_revision_id(...).await?,
            delta,
            created_by,
            created_at: Utc::now(),
            change_source,
            change_summary,
        };
        
        // 5. Insert into database
        self.insert_revision(&revision).await?;
        
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
        // Query revisions ordered by revision_number DESC
    }
    
    /// Get specific revision
    pub async fn get_revision(
        &self,
        tenant_id: Uuid,
        revision_id: Uuid,
    ) -> Result<Revision, RevisionError> {
        // Query single revision
    }
    
    /// Reconstruct content at specific revision
    pub async fn get_content_at_revision<T: RevisionableContent>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        target_revision: i32,
        current_content: &T,
    ) -> Result<T, RevisionError> {
        // 1. Get all revisions from target to current
        let revisions = self.get_revisions_range(
            tenant_id,
            T::content_type(),
            content_id,
            locale,
            target_revision,
        ).await?;
        
        // 2. Apply deltas in reverse order
        let mut content = current_content.to_revision_json();
        for revision in revisions.iter().rev() {
            content = self.apply_delta_reverse(content, &revision.delta)?;
        }
        
        // 3. Deserialize back to content type
        T::from_revision_json(content)
    }
    
    /// Restore content to a previous revision
    pub async fn restore_revision<T: RevisionableContent>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        target_revision: i32,
        restored_by: Uuid,
    ) -> Result<T, RevisionError> {
        // 1. Get content at target revision
        let current_content = self.get_current_content(...).await?;
        let restored_content = self.get_content_at_revision(
            tenant_id,
            content_id,
            locale,
            target_revision,
            &current_content,
        ).await?;
        
        // 2. Create new revision for the restore operation
        self.create_revision(
            tenant_id,
            content_id,
            locale,
            &current_content,
            &restored_content,
            restored_by,
            ChangeSource::Restore,
            Some(format!("Restored to revision {}", target_revision)),
        ).await?;
        
        Ok(restored_content)
    }
    
    /// Calculate delta between two content versions
    fn calculate_delta<T: RevisionableContent>(
        &self,
        old: &T,
        new: &T,
    ) -> Result<serde_json::Value, RevisionError> {
        let old_json = old.to_revision_json();
        let new_json = new.to_revision_json();
        
        let mut delta = serde_json::Map::new();
        
        for field in T::tracked_fields() {
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
    
    /// Cleanup old revisions based on retention policy
    pub async fn cleanup_old_revisions(&self) -> Result<usize, RevisionError> {
        let cutoff_date = Utc::now() - Duration::days(self.retention_days as i64);
        
        let deleted = sqlx::query!(
            "DELETE FROM content_revisions WHERE created_at < $1",
            cutoff_date
        )
        .execute(&self.db)
        .await?;
        
        Ok(deleted.rows_affected() as usize)
    }
}
```

#### VersionService

```rust
pub struct VersionService {
    db: DatabaseConnection,
}

impl VersionService {
    /// Create a named version (snapshot) at current state
    pub async fn create_version<T: RevisionableContent>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        version_name: &str,
        version_label: Option<&str>,
        content: &T,
        created_by: Uuid,
    ) -> Result<Version, VersionError> {
        // 1. Get current revision
        let current_revision = self.get_current_revision(...).await?;
        
        // 2. Create full snapshot
        let snapshot = content.to_revision_json();
        
        // 3. Create version record
        let version = Version {
            id: Uuid::new_v4(),
            tenant_id,
            content_type: T::content_type().to_string(),
            content_id,
            locale: locale.to_string(),
            version_name: version_name.to_string(),
            version_label: version_label.map(String::from),
            snapshot,
            revision_id: current_revision.id,
            created_by,
            created_at: Utc::now(),
        };
        
        // 4. Insert into database
        self.insert_version(&version).await?;
        
        Ok(version)
    }
    
    /// Promote a revision to a named version
    pub async fn promote_revision(
        &self,
        tenant_id: Uuid,
        revision_id: Uuid,
        version_name: &str,
        version_label: Option<&str>,
        created_by: Uuid,
    ) -> Result<Version, VersionError> {
        // 1. Get revision
        let revision = self.get_revision(tenant_id, revision_id).await?;
        
        // 2. Reconstruct content at that revision
        let content = self.reconstruct_content_at_revision(...).await?;
        
        // 3. Create version with snapshot
        self.create_version(
            tenant_id,
            revision.content_id,
            &revision.locale,
            version_name,
            version_label,
            &content,
            created_by,
        ).await
    }
    
    /// List all versions for a content item
    pub async fn list_versions(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Vec<Version>, VersionError> {
        // Query versions ordered by created_at DESC
    }
    
    /// Delete a version
    pub async fn delete_version(
        &self,
        tenant_id: Uuid,
        version_id: Uuid,
    ) -> Result<(), VersionError> {
        // Delete version record
    }
}
```

### Integration с domain modules

#### Blog Module Example

```rust
// crates/modules/rustok-blog/src/revisions.rs

use rustok_content_revisions::{RevisionableContent, RevisionService};
use crate::dto::PostResponse;

impl RevisionableContent for PostResponse {
    fn content_type(&self) -> &'static str {
        "blog_post"
    }
    
    fn to_revision_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
    
    fn from_revision_json(value: serde_json::Value) -> Result<Self, RevisionError> {
        serde_json::from_value(value).map_err(|e| RevisionError::Deserialization(e.to_string()))
    }
    
    fn tracked_fields() -> Vec<&'static str> {
        vec![
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
        ]
    }
    
    fn is_localized_field(field: &str) -> bool {
        matches!(field, "title" | "content" | "excerpt" | "seo_title" | "seo_description")
    }
}

// Hook into post update lifecycle
pub async fn on_post_updated(
    revision_service: &RevisionService,
    tenant_id: Uuid,
    post_id: Uuid,
    locale: &str,
    old_post: &PostResponse,
    new_post: &PostResponse,
    updated_by: Uuid,
) -> Result<(), RevisionError> {
    revision_service.create_revision(
        tenant_id,
        post_id,
        locale,
        old_post,
        new_post,
        updated_by,
        ChangeSource::AdminUi,
        None,
    ).await?;
    
    Ok(())
}
```

### Multilingual Support

#### Unique vs Localized Fields

```rust
// Unique fields (same for all locales)
- status
- featured_image_url
- category_id
- tags

// Localized fields (different per locale)
- title
- content
- excerpt
- seo_title
- seo_description
- slug
```

#### Revision per Locale

```
Blog Post (id: 123)
├─ Locale: en
│  ├─ Revision 1: Initial content
│  ├─ Revision 2: Updated title
│  └─ Revision 3: Fixed typo
│
├─ Locale: ru
│  ├─ Revision 1: Initial translation
│  └─ Revision 2: Improved translation
│
└─ Locale: fr
   └─ Revision 1: French translation
```

#### Restore Behavior

```rust
// Restore localized field → only for current locale
revision_service.restore_revision(
    tenant_id,
    post_id,
    "en",  // Only English content is restored
    target_revision,
    user_id,
).await?;

// Restore unique field → for all locales
// (handled automatically by RevisionService)
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

type ContentVersion {
  id: UUID!
  versionName: String!
  versionLabel: String
  snapshot: JSON!
  revisionId: UUID!
  createdBy: UUID!
  createdAt: DateTime!
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
  
  # List all versions for a content item
  contentVersions(
    contentType: String!
    contentId: UUID!
    locale: String!
  ): [ContentVersion!]!
}

extend type Mutation {
  # Restore content to a previous revision
  restoreContentRevision(
    contentType: String!
    contentId: UUID!
    locale: String!
    revisionNumber: Int!
  ): Boolean!
  
  # Create a named version
  createContentVersion(
    contentType: String!
    contentId: UUID!
    locale: String!
    versionName: String!
    versionLabel: String
  ): ContentVersion!
  
  # Delete a version
  deleteContentVersion(versionId: UUID!): Boolean!
}
```

#### REST

```
GET    /api/content/{type}/{id}/revisions?locale=en
GET    /api/content/{type}/{id}/revisions/{revision_number}?locale=en
POST   /api/content/{type}/{id}/revisions/{revision_number}/restore?locale=en

GET    /api/content/{type}/{id}/versions?locale=en
POST   /api/content/{type}/{id}/versions?locale=en
DELETE /api/content/versions/{version_id}
```

### Retention Policy

```rust
pub struct RetentionConfig {
    /// How many days to keep revisions
    pub revision_retention_days: u32,
    
    /// How many days to keep versions (0 = forever)
    pub version_retention_days: Option<u32>,
    
    /// Maximum number of revisions per content item
    pub max_revisions_per_item: Option<u32>,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            revision_retention_days: 30,  // Keep revisions for 30 days
            version_retention_days: None,  // Keep versions forever
            max_revisions_per_item: Some(100),  // Max 100 revisions per item
        }
    }
}
```

### Performance Considerations

1. **Delta-based storage** — экономит место, хранит только изменения
2. **Indexes** — быстрый lookup по content_type + content_id + locale
3. **Retention cleanup** — автоматическая очистка старых revisions
4. **Lazy loading** — revisions загружаются только когда нужны
5. **Caching** — cache reconstructed content для часто запрашиваемых revisions

### Security & Permissions

```rust
pub enum RevisionPermission {
    /// Can view revision history
    View,
    
    /// Can restore to previous revision
    Restore,
    
    /// Can create named versions
    CreateVersion,
    
    /// Can delete versions
    DeleteVersion,
}

// Check permissions before operations
fn check_permission(
    user: &User,
    content_type: &str,
    permission: RevisionPermission,
) -> Result<(), PermissionError> {
    // Map to existing permission system
    // e.g., "blog_posts:view_revisions", "blog_posts:restore_revisions"
}
```

## Comparison: Strapi vs Directus vs RusTok (Proposed)

| Feature | Strapi v5 | Directus | RusTok (Proposed) |
|---------|-----------|----------|-------------------|
| Storage | Full snapshots | Delta-based | Delta-based ✅ |
| Multilingual | Unique + Localized fields | Separate translations | Unique + Localized fields ✅ |
| Named versions | ❌ | ✅ | ✅ |
| Retention policy | 14-30 days | Configurable | Configurable ✅ |
| API tracking | ❌ | ✅ | ✅ |
| Auto-tracking | Content Manager only | All changes | All changes ✅ |
| Restore | ✅ | ✅ | ✅ |
| Diff view | ✅ | ✅ | ✅ (planned) |

## Implementation Phases

### Phase 1: Core Infrastructure
- [ ] Create `rustok-content-revisions` crate
- [ ] Implement database schema (migrations)
- [ ] Implement `RevisionableContent` trait
- [ ] Implement `RevisionService` (create, list, restore)
- [ ] Add retention cleanup job

### Phase 2: Blog Integration
- [ ] Implement `RevisionableContent` for `PostResponse`
- [ ] Hook into post update lifecycle
- [ ] Add GraphQL mutations/queries
- [ ] Add REST endpoints
- [ ] Admin UI: revision history viewer
- [ ] Admin UI: restore functionality

### Phase 3: Forum Integration
- [ ] Implement `RevisionableContent` for `TopicResponse`
- [ ] Hook into topic update lifecycle
- [ ] Wiki mode: track all edits
- [ ] Admin UI integration

### Phase 4: Advanced Features
- [ ] Named versions (`VersionService`)
- [ ] Diff view (compare two revisions)
- [ ] Bulk restore
- [ ] Revision comments/notes
- [ ] Revision tags/labels

### Phase 5: Other Modules
- [ ] Pages module integration
- [ ] Products module integration
- [ ] Custom content types support

## Open Questions

1. **Should we track deletions?** (Strapi: no, Directus: yes)
2. **Should API changes create revisions?** (Strapi: no, Directus: yes)
3. **How to handle relations?** (track relation IDs or full objects?)
4. **Should we support branching?** (like git branches)
5. **How to handle conflicts?** (when restoring while content changed)

## Recommendations

1. **Use delta-based storage** (like Directus) — more efficient
2. **Track all changes** (API + UI) — complete history
3. **Support named versions** — important milestones
4. **Configurable retention** — different for dev/prod
5. **Multilingual per-field** — granular control
6. **Start with blog** — prove the concept
7. **Make it generic** — reusable for any content type

## Next Steps

1. ✅ Research complete
2. ⏳ Get feedback on this proposal
3. ⏳ Create `rustok-content-revisions` crate
4. ⏳ Implement Phase 1 (core infrastructure)
5. ⏳ Integrate with blog module
6. ⏳ Test and iterate

---

**Вопросы для обсуждения:**
- Какой подход вам ближе: Strapi (full snapshots) или Directus (delta-based)?
- Нужно ли отслеживать API changes или только UI?
- Какие content types нужны в первую очередь?
- Какой retention period нужен для production?
