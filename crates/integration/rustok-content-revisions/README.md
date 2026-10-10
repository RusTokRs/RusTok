# rustok-content-revisions

Межмодульный крейт для интеграции revision history в RusTok platform.

## Обзор

Этот крейт предоставляет unified API для tracking content revisions across всех модулей RusTok (blog, forum, commerce, etc.).

## Architecture

```
Business Modules (blog, forum, commerce)
    │
    │ uses
    ▼
rustok-content-revisions (this crate)
    │
    │ depends on
    ▼
rustok-revisions (standalone library)
```

## Installation

Добавьте в `Cargo.toml` вашего модуля:

```toml
[dependencies]
rustok-content-revisions = { path = "../../integration/rustok-content-revisions" }
```

## Quick Start

### 1. Setup в вашем приложении

```rust
use rustok_content_revisions::{ContentRevisionService, ContentRevisionConfig};
use sea_orm::Database;

#[tokio::main]
async fn main() {
    // Connect to database
    let db = Database::connect("postgres://localhost/rustok")
        .await
        .expect("Failed to connect");

    // Run migrations
    use rustok_content_revisions::Migrator;
    use sea_orm_migration::MigratorTrait;
    Migrator::up(&db, None).await.expect("Failed to run migrations");

    // Create service with default configuration
    let config = ContentRevisionConfig::default();
    let revision_service = ContentRevisionService::new(db, config);

    // Inject into your services
    let blog_service = BlogService::new(Arc::new(revision_service));
}
```

### 2. Implement Revisionable для вашего content type

```rust
use rustok_revisions_derive::Revisionable;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

### 3. Track changes в вашем service

```rust
use rustok_content_revisions::ContentRevisionService;
use std::sync::Arc;

pub struct BlogService {
    db: DatabaseConnection,
    revision_service: Arc<ContentRevisionService>,
}

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

## Configuration

### Default Configuration

По умолчанию настроены следующие content types:

```rust
ContentRevisionConfig {
    enabled: true,
    content_types: {
        "blog_post" => ContentTypeConfig {
            enabled: true,
            track_on: [Update],
            condition: status == "published",
            retention: KeepLast(50),
            tracked_fields: ["title", "content", "status"],
            ignored_fields: ["updated_at", "view_count"],
        },
        "blog_category" => disabled,
        "forum_topic" => disabled,
        "forum_post" => disabled,
        "product" => disabled,
    },
}
```

### Custom Configuration

```rust
use rustok_content_revisions::{ContentRevisionConfig, ContentTypeConfig};
use rustok_revisions::{RetentionPolicy, RevisionEvent};

let mut config = ContentRevisionConfig::new();

// Add custom content type
config.set_content_type(
    "wiki_page".to_string(),
    ContentTypeConfig::new(true)
        .track_on(vec![RevisionEvent::Update])
        .retention(RetentionPolicy::KeepAll)
        .tracked_fields(vec!["title", "content"])
        .ignored_fields(vec!["updated_at"]),
);

// Enable forum topics (disabled by default)
config.set_content_type(
    "forum_topic".to_string(),
    ContentTypeConfig::new(true)
        .track_on(vec![RevisionEvent::Update])
        .retention(RetentionPolicy::KeepLast(20)),
);
```

## API Reference

### ContentRevisionService

#### track_update

Track an update to content.

```rust
pub async fn track_update<T: Revisionable>(
    &self,
    tenant_id: Uuid,
    content_id: Uuid,
    locale: &str,
    old_content: &T,
    new_content: &T,
    user_id: Uuid,
) -> Result<Option<Revision>, ContentRevisionError>;
```

**Returns:**
- `Ok(Some(revision))` — revision создана
- `Ok(None)` — tracking disabled или нет изменений
- `Err(...)` — ошибка

#### track_create

Track content creation.

```rust
pub async fn track_create<T: Revisionable>(
    &self,
    tenant_id: Uuid,
    content_id: Uuid,
    locale: &str,
    content: &T,
    user_id: Uuid,
) -> Result<Option<Revision>, ContentRevisionError>;
```

#### list_revisions

List all revisions for a content item.

```rust
pub async fn list_revisions(
    &self,
    tenant_id: Uuid,
    content_type: &str,
    content_id: Uuid,
    locale: &str,
) -> Result<Vec<Revision>, ContentRevisionError>;
```

#### get_content_at_revision

Get content at a specific revision.

```rust
pub async fn get_content_at_revision<T: Revisionable>(
    &self,
    tenant_id: Uuid,
    content_id: Uuid,
    locale: &str,
    target_revision: i32,
    current_content: &T,
) -> Result<T, ContentRevisionError>;
```

#### restore_revision

Restore content to a previous revision.

```rust
pub async fn restore_revision<T: Revisionable>(
    &self,
    tenant_id: Uuid,
    content_id: Uuid,
    locale: &str,
    target_revision: i32,
    current_content: &T,
    restored_by: Uuid,
) -> Result<T, ContentRevisionError>;
```

#### create_named_version

Create a named version (snapshot).

```rust
pub async fn create_named_version<T: Revisionable>(
    &self,
    tenant_id: Uuid,
    content_id: Uuid,
    locale: &str,
    current_content: &T,
    version_name: &str,
    created_by: Uuid,
) -> Result<Revision, ContentRevisionError>;
```

#### list_named_versions

List all named versions for a content item.

```rust
pub async fn list_named_versions(
    &self,
    tenant_id: Uuid,
    content_type: &str,
    content_id: Uuid,
    locale: &str,
) -> Result<Vec<Revision>, ContentRevisionError>;
```

#### diff_revisions

Get diff between two revisions.

```rust
pub async fn diff_revisions(
    &self,
    tenant_id: Uuid,
    content_type: &str,
    content_id: Uuid,
    locale: &str,
    from_revision: i32,
    to_revision: i32,
) -> Result<RevisionDiff, ContentRevisionError>;
```

#### is_enabled

Check if tracking is enabled for a content type.

```rust
pub fn is_enabled(&self, content_type: &str) -> bool;
```

## Examples

### Blog Post Tracking

```rust
impl BlogService {
    pub async fn publish_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        user_id: Uuid,
    ) -> Result<BlogPost, Error> {
        let mut post = self.get_post(tenant_id, post_id).await?;
        let old_post = post.clone();
        
        post.status = PostStatus::Published;
        post.updated_at = Utc::now();
        
        let updated = self.db_update_post(&post).await?;
        
        // Create named version before publishing
        self.revision_service.create_named_version(
            tenant_id,
            post_id,
            "en",
            &old_post,
            "before-publication",
            user_id,
        ).await?;
        
        // Track the publish action
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

### Restore to Previous Version

```rust
impl BlogService {
    pub async fn restore_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        target_revision: i32,
        user_id: Uuid,
    ) -> Result<BlogPost, Error> {
        let current = self.get_post(tenant_id, post_id).await?;
        
        let restored = self.revision_service.restore_revision(
            tenant_id,
            post_id,
            "en",
            target_revision,
            &current,
            user_id,
        ).await?;
        
        // Save restored content to database
        self.db_update_post(&restored).await?;
        
        Ok(restored)
    }
}
```

### Compare Versions

```rust
impl BlogService {
    pub async fn compare_versions(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        from_revision: i32,
        to_revision: i32,
    ) -> Result<RevisionDiff, Error> {
        let diff = self.revision_service.diff_revisions(
            tenant_id,
            "blog_post",
            post_id,
            "en",
            from_revision,
            to_revision,
        ).await?;
        
        println!("{}", diff.human_readable());
        
        Ok(diff)
    }
}
```

## Database Schema

Миграция создает таблицу `content_revisions`:

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

CREATE INDEX idx_content_revisions_lookup 
    ON content_revisions(tenant_id, content_type, content_id, locale);

CREATE INDEX idx_content_revisions_created_at 
    ON content_revisions(created_at);

CREATE INDEX idx_content_revisions_version_name 
    ON content_revisions(version_name);
```

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rustok_revisions::InMemoryBackend;

    #[tokio::test]
    async fn test_blog_revision_tracking() {
        let db = setup_test_db().await;
        let config = ContentRevisionConfig::default();
        let service = ContentRevisionService::new(db, config);
        
        // Test tracking
        let result = service.track_update(...).await;
        assert!(result.is_ok());
    }
}
```

## License

MIT OR Apache-2.0
