# Интеграция Revision History в Blog Module

Этот документ показывает как интегрировать `rustok-revisions` в blog module для отслеживания изменений постов.

## Шаг 1: Создание Revisionable модели

Создайте файл `crates/modules/rustok-blog/src/models/post_revisionable.rs`:

```rust
use chrono::{DateTime, Utc};
use rustok_revisions::Revisionable;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Blog post модель для revision tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlogPostRevisionable {
    pub id: Uuid,
    pub tenant_id: Uuid,
    
    // Tracked fields - изменения создают ревизии
    pub title: String,
    pub content: String,
    pub excerpt: String,
    pub status: PostStatus,
    pub published_at: Option<DateTime<Utc>>,
    
    // Ignored fields - изменения НЕ создают ревизии
    pub updated_at: DateTime<Utc>,
    pub view_count: i64,
    pub likes_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PostStatus {
    Draft,
    Published,
    Archived,
}

impl Revisionable for BlogPostRevisionable {
    fn content_type() -> &'static str {
        "blog_post"
    }

    fn tracked_fields() -> Vec<&'static str> {
        vec![
            "title",
            "content",
            "excerpt",
            "status",
            "published_at",
        ]
    }

    fn ignored_fields() -> Vec<&'static str> {
        vec![
            "updated_at",
            "view_count",
            "likes_count",
        ]
    }
}

impl BlogPostRevisionable {
    /// Создать из database entity
    pub fn from_entity(entity: &crate::entities::post::Model) -> Self {
        Self {
            id: entity.id,
            tenant_id: entity.tenant_id,
            title: entity.title.clone(),
            content: entity.content.clone(),
            excerpt: entity.excerpt.clone(),
            status: match entity.status.as_str() {
                "published" => PostStatus::Published,
                "archived" => PostStatus::Archived,
                _ => PostStatus::Draft,
            },
            published_at: entity.published_at,
            updated_at: entity.updated_at,
            view_count: entity.view_count,
            likes_count: entity.likes_count,
        }
    }
}
```

## Шаг 2: Создание Revision Tracker

Создайте файл `crates/modules/rustok-blog/src/services/revision_tracker.rs`:

```rust
use rustok_content_revisions::{
    ContentRevisionConfig, ContentRevisionService, ContentTypeConfig,
};
use rustok_revisions::{RetentionPolicy, RevisionEvent};
use sea_orm::DatabaseConnection;

/// Создать конфигурацию для blog posts
pub fn create_blog_revision_config() -> ContentRevisionConfig {
    let mut config = ContentRevisionConfig::new();
    
    // Blog posts - отслеживаем только published посты
    config.add_content_type(
        "blog_post",
        ContentTypeConfig {
            enabled: true,
            track_on: vec![RevisionEvent::Update],
            condition_field: Some("status".to_string()),
            condition_value: Some("published".to_string()),
            retention: RetentionPolicy::KeepLast(50),
        },
    );
    
    // Comments - не отслеживаем
    config.add_content_type(
        "blog_comment",
        ContentTypeConfig {
            enabled: false,
            track_on: vec![],
            condition_field: None,
            condition_value: None,
            retention: RetentionPolicy::KeepAll,
        },
    );
    
    config
}

/// Создать revision service для blog
pub fn create_blog_revision_service(db: DatabaseConnection) -> ContentRevisionService {
    let config = create_blog_revision_config();
    ContentRevisionService::new(db, config)
}
```

## Шаг 3: Интеграция в Post Service

Обновите `crates/modules/rustok-blog/src/services/post_service.rs`:

```rust
use crate::models::post_revisionable::BlogPostRevisionable;
use rustok_content_revisions::ContentRevisionService;
use std::sync::Arc;
use uuid::Uuid;

pub struct PostService {
    db: DatabaseConnection,
    revision_service: Arc<ContentRevisionService>,
}

impl PostService {
    pub fn new(db: DatabaseConnection, revision_service: Arc<ContentRevisionService>) -> Self {
        Self { db, revision_service }
    }

    /// Обновить пост с отслеживанием изменений
    pub async fn update_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        update: UpdatePostInput,
        user_id: Uuid,
    ) -> Result<Post, Error> {
        // 1. Получить текущий пост
        let old_post = self.get_post(tenant_id, post_id).await?;
        let old_revisionable = BlogPostRevisionable::from_entity(&old_post);
        
        // 2. Обновить в базе
        let mut active: post::ActiveModel = old_post.clone().into();
        active.title = Set(update.title);
        active.content = Set(update.content);
        active.excerpt = Set(update.excerpt);
        active.updated_at = Set(Utc::now());
        
        let updated = active.update(&self.db).await?;
        let new_revisionable = BlogPostRevisionable::from_entity(&updated);
        
        // 3. Создать ревизию (если tracking enabled)
        self.revision_service
            .track_update(
                tenant_id,
                "blog_post",
                post_id,
                "en",
                &old_revisionable,
                &new_revisionable,
                user_id,
            )
            .await
            .map_err(|e| Error::RevisionError(e.to_string()))?;
        
        Ok(updated)
    }

    /// Опубликовать пост с созданием named version
    pub async fn publish_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        user_id: Uuid,
    ) -> Result<Post, Error> {
        let post = self.get_post(tenant_id, post_id).await?;
        let revisionable = BlogPostRevisionable::from_entity(&post);
        
        // Создать named version перед публикацией
        self.revision_service
            .create_named_version(
                tenant_id,
                "blog_post",
                post_id,
                "en",
                &revisionable,
                "before-publication",
                user_id,
            )
            .await
            .map_err(|e| Error::RevisionError(e.to_string()))?;
        
        // Обновить статус
        let mut active: post::ActiveModel = post.into();
        active.status = Set("published".to_string());
        active.published_at = Set(Some(Utc::now()));
        active.updated_at = Set(Utc::now());
        
        let published = active.update(&self.db).await?;
        let new_revisionable = BlogPostRevisionable::from_entity(&published);
        
        // Создать ревизию для публикации
        self.revision_service
            .track_update(
                tenant_id,
                "blog_post",
                post_id,
                "en",
                &revisionable,
                &new_revisionable,
                user_id,
            )
            .await
            .map_err(|e| Error::RevisionError(e.to_string()))?;
        
        Ok(published)
    }

    /// Восстановить пост к предыдущей версии
    pub async fn restore_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        revision_number: i32,
        user_id: Uuid,
    ) -> Result<Post, Error> {
        let current = self.get_post(tenant_id, post_id).await?;
        let current_revisionable = BlogPostRevisionable::from_entity(&current);
        
        // Восстановить к предыдущей версии
        let restored = self.revision_service
            .restore_revision(
                tenant_id,
                "blog_post",
                post_id,
                "en",
                revision_number,
                &current_revisionable,
                user_id,
            )
            .await
            .map_err(|e| Error::RevisionError(e.to_string()))?;
        
        // Обновить в базе
        let mut active: post::ActiveModel = current.into();
        active.title = Set(restored.title);
        active.content = Set(restored.content);
        active.excerpt = Set(restored.excerpt);
        active.status = Set(match restored.status {
            PostStatus::Draft => "draft".to_string(),
            PostStatus::Published => "published".to_string(),
            PostStatus::Archived => "archived".to_string(),
        });
        active.updated_at = Set(Utc::now());
        
        let updated = active.update(&self.db).await?;
        
        Ok(updated)
    }

    /// Получить историю ревизий поста
    pub async fn get_post_history(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
    ) -> Result<Vec<Revision>, Error> {
        self.revision_service
            .list_revisions(tenant_id, "blog_post", post_id, "en")
            .await
            .map_err(|e| Error::RevisionError(e.to_string()))
    }

    /// Сравнить две версии поста
    pub async fn compare_post_versions(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        from_revision: i32,
        to_revision: i32,
    ) -> Result<RevisionDiff, Error> {
        self.revision_service
            .diff_revisions(
                tenant_id,
                "blog_post",
                post_id,
                "en",
                from_revision,
                to_revision,
            )
            .await
            .map_err(|e| Error::RevisionError(e.to_string()))
    }
}
```

## Шаг 4: GraphQL API

Добавьте в `crates/modules/rustok-blog/src/graphql/post.rs`:

```rust
use async_graphql::*;
use rustok_revisions::{Revision, RevisionDiff};

#[Object]
impl Post {
    // ... существующие поля

    /// Получить историю ревизий поста
    async fn revisions(&self, ctx: &Context<'_>) -> Result<Vec<Revision>> {
        let post_service = ctx.data::<Arc<PostService>>()?;
        let tenant_id = ctx.data::<Uuid>()?;
        
        let revisions = post_service
            .get_post_history(*tenant_id, self.id)
            .await?;
        
        Ok(revisions)
    }

    /// Сравнить две версии поста
    async fn diff(
        &self,
        ctx: &Context<'_>,
        from_revision: i32,
        to_revision: i32,
    ) -> Result<RevisionDiff> {
        let post_service = ctx.data::<Arc<PostService>>()?;
        let tenant_id = ctx.data::<Uuid>()?;
        
        let diff = post_service
            .compare_post_versions(*tenant_id, self.id, from_revision, to_revision)
            .await?;
        
        Ok(diff)
    }
}

#[Object]
impl MutationRoot {
    /// Восстановить пост к предыдущей версии
    async fn restore_post(
        &self,
        ctx: &Context<'_>,
        post_id: Uuid,
        revision_number: i32,
    ) -> Result<Post> {
        let post_service = ctx.data::<Arc<PostService>>()?;
        let tenant_id = ctx.data::<Uuid>()?;
        let user_id = ctx.data::<Uuid>()?;
        
        let restored = post_service
            .restore_post(*tenant_id, post_id, revision_number, *user_id)
            .await?;
        
        Ok(restored)
    }
}
```

## Шаг 5: Инициализация в Module

Обновите `crates/modules/rustok-blog/src/module.rs`:

```rust
use crate::services::revision_tracker::create_blog_revision_service;
use rustok_content_revisions::ContentRevisionService;
use std::sync::Arc;

pub struct BlogModule {
    db: DatabaseConnection,
    revision_service: Arc<ContentRevisionService>,
}

impl BlogModule {
    pub async fn new(db: DatabaseConnection) -> Self {
        // Создать revision service
        let revision_service = Arc::new(create_blog_revision_service(db.clone()));
        
        // Запустить миграции для content_revisions таблицы
        use sea_orm_migration::MigratorTrait;
        use rustok_revisions::migrations::Migrator;
        
        Migrator::up(&db, None)
            .await
            .expect("Failed to run migrations");
        
        Self {
            db,
            revision_service,
        }
    }

    pub fn post_service(&self) -> PostService {
        PostService::new(self.db.clone(), self.revision_service.clone())
    }
}
```

## Примеры использования

### GraphQL Queries

```graphql
# Получить историю ревизий поста
query {
  post(id: "123") {
    title
    revisions {
      revisionNumber
      createdAt
      createdBy { name }
      delta
    }
  }
}

# Сравнить две версии
query {
  post(id: "123") {
    diff(fromRevision: 1, toRevision: 3) {
      changes {
        field
        oldValue
        newValue
      }
    }
  }
}
```

### GraphQL Mutations

```graphql
# Восстановить к предыдущей версии
mutation {
  restorePost(postId: "123", revisionNumber: 5) {
    title
    content
    status
  }
}
```

## Преимущества интеграции

✅ **Автоматическое отслеживание** — все изменения published постов сохраняются  
✅ **Named versions** — можно создавать snapshots для важных моментов  
✅ **Restore** — можно откатиться к любой предыдущей версии  
✅ **Diff view** — можно сравнивать версии  
✅ **Retention policy** — автоматическая очистка старых ревизий  
✅ **Multilingual** — поддержка разных языков  

## Configuration

По умолчанию для blog posts:
- ✅ Tracking enabled
- ✅ Только published посты отслеживаются
- ✅ Только Update events (не Create/Delete)
- ✅ KeepLast(50) — хранить последние 50 ревизий
- ✅ Ignored fields: updated_at, view_count, likes_count

## Testing

```rust
#[tokio::test]
async fn test_post_revision_tracking() {
    let db = setup_test_db().await;
    let revision_service = Arc::new(create_blog_revision_service(db.clone()));
    let post_service = PostService::new(db, revision_service);
    
    // Создать и опубликовать пост
    let post = post_service.create_post(tenant_id, create_input, user_id).await?;
    let published = post_service.publish_post(tenant_id, post.id, user_id).await?;
    
    // Обновить пост
    let updated = post_service.update_post(tenant_id, post.id, update_input, user_id).await?;
    
    // Проверить что создана ревизия
    let history = post_service.get_post_history(tenant_id, post.id).await?;
    assert_eq!(history.len(), 2); // publication + update
}
```

## Заключение

Интеграция `rustok-revisions` в blog module дает:
- Полную историю изменений постов
- Возможность отката к предыдущим версиям
- Сравнение версий
- Автоматическую очистку старых ревизий

Это production-ready решение для tracking истории изменений контента!
