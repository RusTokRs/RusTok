# Blog Module — Revision History Integration Guide

Этот документ описывает как интегрировать revision history в blog module.

## Обзор

Blog module использует `rustok-content-revisions` для tracking изменений в blog posts.

## Setup

### 1. Dependency добавлена

```toml
# crates/modules/rustok-blog/Cargo.toml
[dependencies]
rustok-content-revisions = { path = "../../integration/rustok-content-revisions" }
```

### 2. Implement Revisionable для BlogPost

Создайте domain model с `Revisionable` trait:

```rust
// crates/modules/rustok-blog/src/domain/post.rs

use rustok_revisions::Revisionable;
use rustok_revisions_derive::Revisionable;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Domain model для blog post с revision tracking
#[derive(Debug, Clone, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "blog_post")]
pub struct BlogPostRevisionable {
    pub id: Uuid,
    pub tenant_id: Uuid,
    
    #[revision(tracked)]
    pub title: String,
    
    #[revision(tracked)]
    pub content: String,
    
    #[revision(tracked)]
    pub status: String,
    
    #[revision(tracked)]
    pub slug: String,
    
    #[revision(ignored)]
    pub updated_at: DateTime<Utc>,
    
    #[revision(ignored)]
    pub comment_count: i32,
    
    #[revision(ignored)]
    pub view_count: i32,
}

impl BlogPostRevisionable {
    /// Convert from SeaORM entity
    pub fn from_entity(
        entity: &crate::entities::blog_post::Model,
        title: String,
        content: String,
    ) -> Self {
        Self {
            id: entity.id,
            tenant_id: entity.tenant_id,
            title,
            content,
            status: entity.status.clone(),
            slug: entity.slug.clone(),
            updated_at: entity.updated_at.into(),
            comment_count: entity.comment_count,
            view_count: 0, // TODO: Add view_count to entity
        }
    }
}
```

### 3. Inject ContentRevisionService в BlogService

```rust
// crates/modules/rustok-blog/src/services/post_service.rs

use rustok_content_revisions::ContentRevisionService;
use std::sync::Arc;

pub struct BlogPostService {
    db: DatabaseConnection,
    revision_service: Arc<ContentRevisionService>,
}

impl BlogPostService {
    pub fn new(
        db: DatabaseConnection,
        revision_service: Arc<ContentRevisionService>,
    ) -> Self {
        Self {
            db,
            revision_service,
        }
    }
    
    pub async fn update_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        update_data: UpdatePostData,
        user_id: Uuid,
    ) -> Result<BlogPost, Error> {
        // 1. Get old post
        let old_post = self.get_post(tenant_id, post_id).await?;
        
        // 2. Update in database
        let mut active_model: blog_post::ActiveModel = old_post.clone().into();
        active_model.status = Set(update_data.status);
        // ... other fields
        
        let updated_entity = active_model.update(&self.db).await?;
        
        // 3. Get translations for revision tracking
        let old_translations = self.get_translations(tenant_id, post_id).await?;
        let new_translations = update_data.translations;
        
        // 4. Track revision для каждой locale
        for (locale, new_translation) in &new_translations {
            let old_translation = old_translations.get(locale);
            
            if let Some(old_trans) = old_translation {
                let old_revisionable = BlogPostRevisionable {
                    id: old_post.id,
                    tenant_id: old_post.tenant_id,
                    title: old_trans.title.clone(),
                    content: old_trans.content.clone(),
                    status: old_post.status.clone(),
                    slug: old_post.slug.clone(),
                    updated_at: old_post.updated_at.into(),
                    comment_count: old_post.comment_count,
                    view_count: 0,
                };
                
                let new_revisionable = BlogPostRevisionable {
                    id: updated_entity.id,
                    tenant_id: updated_entity.tenant_id,
                    title: new_translation.title.clone(),
                    content: new_translation.content.clone(),
                    status: updated_entity.status.clone(),
                    slug: updated_entity.slug.clone(),
                    updated_at: updated_entity.updated_at.into(),
                    comment_count: updated_entity.comment_count,
                    view_count: 0,
                };
                
                // Track revision
                self.revision_service.track_update(
                    tenant_id,
                    post_id,
                    locale,
                    &old_revisionable,
                    &new_revisionable,
                    user_id,
                ).await?;
            }
        }
        
        // 5. Return updated post
        Ok(self.get_post_with_translations(tenant_id, post_id).await?)
    }
}
```

### 4. Add revision tracking в lifecycle hooks

```rust
impl BlogPostService {
    pub async fn publish_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        user_id: Uuid,
    ) -> Result<BlogPost, Error> {
        let mut post = self.get_post(tenant_id, post_id).await?;
        let old_post = post.clone();
        
        // Create named version before publishing
        let translations = self.get_translations(tenant_id, post_id).await?;
        if let Some(en_trans) = translations.get("en") {
            let revisionable = BlogPostRevisionable::from_entity(&post, en_trans.title.clone(), en_trans.content.clone());
            
            self.revision_service.create_named_version(
                tenant_id,
                post_id,
                "en",
                &revisionable,
                "before-publication",
                user_id,
            ).await?;
        }
        
        // Update status
        let mut active_model: blog_post::ActiveModel = post.clone().into();
        active_model.status = Set("published".to_string());
        active_model.published_at = Set(Some(Utc::now().into()));
        
        let updated = active_model.update(&self.db).await?;
        
        // Track the publish action
        if let Some(en_trans) = translations.get("en") {
            let old_revisionable = BlogPostRevisionable::from_entity(&old_post, en_trans.title.clone(), en_trans.content.clone());
            let new_revisionable = BlogPostRevisionable::from_entity(&updated, en_trans.title.clone(), en_trans.content.clone());
            
            self.revision_service.track_update(
                tenant_id,
                post_id,
                "en",
                &old_revisionable,
                &new_revisionable,
                user_id,
            ).await?;
        }
        
        Ok(self.get_post_with_translations(tenant_id, post_id).await?)
    }
    
    pub async fn restore_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        target_revision: i32,
        user_id: Uuid,
    ) -> Result<BlogPost, Error> {
        let current = self.get_post(tenant_id, post_id).await?;
        let translations = self.get_translations(tenant_id, post_id).await?;
        
        if let Some(en_trans) = translations.get("en") {
            let current_revisionable = BlogPostRevisionable::from_entity(&current, en_trans.title.clone(), en_trans.content.clone());
            
            let restored = self.revision_service.restore_revision(
                tenant_id,
                post_id,
                "en",
                target_revision,
                &current_revisionable,
                user_id,
            ).await?;
            
            // Save restored content to database
            let mut active_model: blog_post::ActiveModel = current.clone().into();
            active_model.status = Set(restored.status);
            active_model.slug = Set(restored.slug);
            
            let updated = active_model.update(&self.db).await?;
            
            // Update translation
            self.update_translation(
                tenant_id,
                post_id,
                "en",
                restored.title,
                restored.content,
            ).await?;
            
            Ok(self.get_post_with_translations(tenant_id, post_id).await?)
        } else {
            Err(Error::TranslationNotFound("en".to_string()))
        }
    }
}
```

### 5. Add GraphQL API

```rust
// crates/modules/rustok-blog/src/graphql/post.rs

use async_graphql::*;
use rustok_content_revisions::{Revision, RevisionDiff};

#[Object]
impl BlogPost {
    async fn id(&self) -> Uuid {
        self.id
    }
    
    async fn title(&self) -> &str {
        &self.title
    }
    
    // ... other fields
    
    /// Get revision history
    async fn revisions(
        &self,
        ctx: &Context<'_>,
        locale: Option<String>,
    ) -> Result<Vec<Revision>, Error> {
        let revision_service = ctx.data::<Arc<ContentRevisionService>>()?;
        let tenant_id = ctx.data::<Uuid>()?;
        let locale = locale.unwrap_or_else(|| "en".to_string());
        
        let revisions = revision_service.list_revisions(
            *tenant_id,
            "blog_post",
            self.id,
            &locale,
        ).await?;
        
        Ok(revisions)
    }
    
    /// Restore to a specific revision
    async fn restore_revision(
        &self,
        ctx: &Context<'_>,
        revision_number: i32,
    ) -> Result<BlogPost, Error> {
        let post_service = ctx.data::<Arc<BlogPostService>>()?;
        let tenant_id = ctx.data::<Uuid>()?;
        let user_id = ctx.data::<Uuid>()?;
        
        let restored = post_service.restore_post(
            *tenant_id,
            self.id,
            revision_number,
            *user_id,
        ).await?;
        
        Ok(restored)
    }
    
    /// Compare two revisions
    async fn diff_revisions(
        &self,
        ctx: &Context<'_>,
        from_revision: i32,
        to_revision: i32,
        locale: Option<String>,
    ) -> Result<RevisionDiff, Error> {
        let revision_service = ctx.data::<Arc<ContentRevisionService>>()?;
        let tenant_id = ctx.data::<Uuid>()?;
        let locale = locale.unwrap_or_else(|| "en".to_string());
        
        let diff = revision_service.diff_revisions(
            *tenant_id,
            "blog_post",
            self.id,
            &locale,
            from_revision,
            to_revision,
        ).await?;
        
        Ok(diff)
    }
}
```

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rustok_content_revisions::{ContentRevisionService, ContentRevisionConfig};
    use rustok_revisions::InMemoryBackend;

    #[tokio::test]
    async fn test_blog_post_revision_tracking() {
        // Setup
        let db = setup_test_db().await;
        let backend = InMemoryBackend::new();
        let revision_service = Arc::new(ContentRevisionService::new(
            db.clone(),
            ContentRevisionConfig::default(),
        ));
        
        let post_service = BlogPostService::new(db, revision_service.clone());
        
        // Create post
        let post = post_service.create_post(
            tenant_id,
            CreatePostData {
                title: "Test Post".to_string(),
                content: "Test content".to_string(),
                status: "draft".to_string(),
            },
            user_id,
        ).await?;
        
        // Update post
        let updated = post_service.update_post(
            tenant_id,
            post.id,
            UpdatePostData {
                title: "Updated Title".to_string(),
                content: "Updated content".to_string(),
                status: "published".to_string(),
            },
            user_id,
        ).await?;
        
        // Check revisions
        let revisions = revision_service.list_revisions(
            tenant_id,
            "blog_post",
            post.id,
            "en",
        ).await?;
        
        assert_eq!(revisions.len(), 1);
        assert_eq!(revisions[0].delta["title"]["new"], "Updated Title");
    }
}
```

## Configuration

По умолчанию blog posts настроены так:

```rust
ContentTypeConfig {
    enabled: true,
    track_on: [Update],
    condition: status == "published",
    retention: KeepLast(50),
    tracked_fields: ["title", "content", "status", "slug"],
    ignored_fields: ["updated_at", "comment_count", "view_count"],
}
```

Это значит:
- ✅ Tracking включен
- ✅ Отслеживаются только updates (не creates)
- ✅ Отслеживаются только published posts
- ✅ Хранятся последние 50 revisions
- ✅ Отслеживаются: title, content, status, slug
- ✅ Игнорируются: updated_at, comment_count, view_count

## Примеры использования

### View revision history

```graphql
query {
  blogPost(id: "123") {
    title
    revisions(locale: "en") {
      revisionNumber
      createdAt
      createdBy {
        name
      }
      delta
      changeSummary
    }
  }
}
```

### Restore to previous version

```graphql
mutation {
  restoreBlogPost(id: "123", revisionNumber: 5) {
    title
    content
    status
  }
}
```

### Compare versions

```graphql
query {
  blogPost(id: "123") {
    diffRevisions(fromRevision: 1, toRevision: 3, locale: "en") {
      fromRevision
      toRevision
      changes {
        field
        oldValue
        newValue
      }
      summary
    }
  }
}
```

## Benefits

✅ **Audit trail** — полная история изменений  
✅ **Restore** — возможность откатиться к предыдущей версии  
✅ **Diff view** — сравнение версий  
✅ **Named versions** — snapshots для важных milestones  
✅ **Multilingual** — per-locale revision history  
✅ **Automatic cleanup** — retention policies  

## Next Steps

1. ✅ Добавить dependency
2. ⏳ Implement `Revisionable` для `BlogPost`
3. ⏳ Inject `ContentRevisionService` в `BlogPostService`
4. ⏳ Добавить tracking в lifecycle hooks
5. ⏳ Добавить GraphQL API
6. ⏳ Добавить tests

## Conclusion

Интеграция с blog module готова к использованию. Следуйте шагам выше чтобы добавить revision tracking в ваш blog module.
