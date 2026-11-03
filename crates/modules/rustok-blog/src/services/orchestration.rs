//! Owner in-transaction orchestration commands for cross-module content bridges.
//!
//! Exposes typed commands and loaders that execute within the caller's transaction
//! (such as `rustok-content-orchestration`), preserving transaction atomicity
//! while keeping Blog persistence entities strictly crate-private.

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseTransaction, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder,
};
use uuid::Uuid;

use crate::entities::{blog_category, blog_post, blog_post_tag, blog_post_translation};
use crate::error::{BlogError, BlogResult};

/// Verifies that a Blog category exists for the given tenant within an active transaction.
pub async fn ensure_blog_category_exists_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> BlogResult<()> {
    if tenant_id.is_nil() {
        return Err(BlogError::validation("tenant_id must not be nil"));
    }
    if category_id.is_nil() {
        return Err(BlogError::validation("category_id must not be nil"));
    }
    let exists = blog_category::Entity::find_by_id(category_id)
        .filter(blog_category::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await
        .map_err(BlogError::from)?;
    if exists.is_none() {
        return Err(BlogError::CategoryNotFound(category_id));
    }
    Ok(())
}

/// Verifies that a Blog post slug is unique for the given tenant within an active transaction.
pub async fn ensure_blog_slug_unique_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    slug: &str,
) -> BlogResult<()> {
    if tenant_id.is_nil() {
        return Err(BlogError::validation("tenant_id must not be nil"));
    }
    let count = blog_post::Entity::find()
        .filter(blog_post::Column::TenantId.eq(tenant_id))
        .filter(blog_post::Column::Slug.eq(slug))
        .count(txn)
        .await
        .map_err(BlogError::from)?;
    if count > 0 {
        return Err(BlogError::DuplicateSlug {
            slug: slug.to_string(),
        });
    }
    Ok(())
}

/// Input payload for creating a single translation of a promoted post.
#[derive(Debug, Clone)]
pub struct PromotedPostTranslationInput {
    pub locale: String,
    pub title: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input payload for creating an orchestrated post and its translations inside a transaction.
#[derive(Debug, Clone)]
pub struct CreatePromotedPostInput {
    pub post_id: Uuid,
    pub tenant_id: Uuid,
    pub author_id: Uuid,
    pub category_id: Option<Uuid>,
    pub status: String,
    pub slug: String,
    pub metadata: serde_json::Value,
    pub comment_count: i32,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    pub translations: Vec<PromotedPostTranslationInput>,
}

/// Inserts a promoted blog post and its translations within the caller's transaction.
pub async fn create_promoted_post_in_tx(
    txn: &DatabaseTransaction,
    input: CreatePromotedPostInput,
) -> BlogResult<()> {
    if input.tenant_id.is_nil() {
        return Err(BlogError::validation("tenant_id must not be nil"));
    }
    if input.post_id.is_nil() {
        return Err(BlogError::validation("post_id must not be nil"));
    }
    if input.author_id.is_nil() {
        return Err(BlogError::validation("author_id must not be nil"));
    }
    if input.slug.trim().is_empty() {
        return Err(BlogError::validation("slug must not be empty"));
    }
    let now = Utc::now();
    blog_post::ActiveModel {
        id: Set(input.post_id),
        tenant_id: Set(input.tenant_id),
        author_id: Set(input.author_id),
        category_id: Set(input.category_id),
        status: Set(input.status),
        slug: Set(input.slug),
        metadata: Set(input.metadata),
        featured_image_url: Set(None),
        published_at: Set(input.published_at.map(Into::into)),
        created_at: Set(input.created_at.into()),
        updated_at: Set(now.into()),
        archived_at: Set(input.archived_at.map(Into::into)),
        scheduled_at: Set(None),
        is_pinned: Set(false),
        pinned_at: Set(None),
        comment_count: Set(input.comment_count),
        version: Set(1),
    }
    .insert(txn)
    .await
    .map_err(BlogError::from)?;

    for translation in input.translations {
        blog_post_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            post_id: Set(input.post_id),
            locale: Set(translation.locale),
            title: Set(translation.title),
            excerpt: Set(None),
            seo_title: Set(None),
            seo_description: Set(None),
            body: Set(translation.body),
            created_at: Set(translation.created_at.into()),
            updated_at: Set(translation.updated_at.into()),
        }
        .insert(txn)
        .await
        .map_err(BlogError::from)?;
    }
    Ok(())
}

/// Loads tag taxonomy term IDs linked to a post in chronological link order.
pub async fn load_post_tag_ids_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
) -> BlogResult<Vec<Uuid>> {
    if tenant_id.is_nil() {
        return Err(BlogError::validation("tenant_id must not be nil"));
    }
    if post_id.is_nil() {
        return Err(BlogError::validation("post_id must not be nil"));
    }
    let relations = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .order_by_asc(blog_post_tag::Column::CreatedAt)
        .all(txn)
        .await
        .map_err(BlogError::from)?;
    Ok(relations.into_iter().map(|item| item.tag_id).collect())
}

/// A translation record loaded for demoting a blog post to a forum topic.
#[derive(Debug, Clone)]
pub struct DemotedPostTranslationRecord {
    pub locale: String,
    pub title: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A post record loaded for demoting a blog post to a forum topic.
#[derive(Debug, Clone)]
pub struct DemotedPostRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub author_id: Uuid,
    pub category_id: Option<Uuid>,
    pub status: String,
    pub slug: String,
    pub created_at: DateTime<Utc>,
    pub translations: Vec<DemotedPostTranslationRecord>,
}

/// Loads a blog post and its translations for demotion within an active transaction.
pub async fn load_post_for_demotion_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
) -> BlogResult<DemotedPostRecord> {
    if tenant_id.is_nil() {
        return Err(BlogError::validation("tenant_id must not be nil"));
    }
    if post_id.is_nil() {
        return Err(BlogError::validation("post_id must not be nil"));
    }
    let post = blog_post::Entity::find_by_id(post_id)
        .filter(blog_post::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await
        .map_err(BlogError::from)?
        .ok_or(BlogError::PostNotFound(post_id))?;

    let translations = blog_post_translation::Entity::find()
        .filter(blog_post_translation::Column::PostId.eq(post_id))
        .all(txn)
        .await
        .map_err(BlogError::from)?
        .into_iter()
        .map(|t| DemotedPostTranslationRecord {
            locale: t.locale,
            title: t.title,
            body: t.body,
            created_at: t.created_at.into(),
            updated_at: t.updated_at.into(),
        })
        .collect();

    Ok(DemotedPostRecord {
        id: post.id,
        tenant_id: post.tenant_id,
        author_id: post.author_id,
        category_id: post.category_id,
        status: post.status,
        slug: post.slug,
        created_at: post.created_at.into(),
        translations,
    })
}

/// Deletes a demoted blog post, its translations, and cleans up its tag usage within the transaction.
pub async fn delete_demoted_post_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
) -> BlogResult<()> {
    if tenant_id.is_nil() {
        return Err(BlogError::validation("tenant_id must not be nil"));
    }
    if post_id.is_nil() {
        return Err(BlogError::validation("post_id must not be nil"));
    }
    // Remove tags and refresh usage projection
    crate::services::remove_post_tag_links_in_tx(txn, tenant_id, post_id).await?;

    // Delete post translations
    blog_post_translation::Entity::delete_many()
        .filter(blog_post_translation::Column::PostId.eq(post_id))
        .exec(txn)
        .await
        .map_err(BlogError::from)?;

    // Delete post
    blog_post::Entity::delete_many()
        .filter(blog_post::Column::TenantId.eq(tenant_id))
        .filter(blog_post::Column::Id.eq(post_id))
        .exec(txn)
        .await
        .map_err(BlogError::from)?;

    Ok(())
}
