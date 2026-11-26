//! Owner in-transaction orchestration commands for cross-module content bridges.
//!
//! Exposes typed commands and loaders that execute within the caller's transaction
//! (such as `rustok-content-orchestration`), preserving transaction atomicity
//! while keeping Blog persistence entities strictly crate-private.

use chrono::{DateTime, Utc};
use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseTransaction, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder,
};
use tracing::instrument;
use uuid::Uuid;

use super::{
    category::{
        CATEGORY_DESCRIPTION_MAX_CHARS, CATEGORY_NAME_MAX_CHARS, CATEGORY_SLUG_MAX_CHARS,
        record_category_copy_change_in_tx,
    },
    post::{MAX_TAG_NAME_CHARS, storage_to_status, status_to_storage, validate_tags},
    rbac::enforce_scope,
    tag::{
        TAG_SLUG_MAX_CHARS, publish_blog_reindex_in_tx, remove_post_tag_links_with_port_in_tx,
        sync_post_tags_in_tx,
    },
};
use crate::entities::{
    blog_category, blog_comment_projection_delivery, blog_post, blog_post_channel_visibility,
    blog_post_tag, blog_post_translation,
};
use crate::error::{BlogError, BlogResult};
use crate::ports::taxonomy::BlogTaxonomyPort;

#[derive(Clone, Copy)]
pub enum BlogTaxonomyTranslationKind {
    Category,
    Tag,
}

impl BlogTaxonomyTranslationKind {
    pub fn authorize(self, security: &SecurityContext, action: Action) -> BlogResult<()> {
        let resource = match self {
            Self::Category => Resource::BlogCategories,
            Self::Tag => Resource::Tags,
        };
        enforce_scope(security, resource, action)
    }

    pub fn field_limits(self) -> (u32, u32, Option<u32>) {
        match self {
            Self::Category => (
                CATEGORY_NAME_MAX_CHARS,
                CATEGORY_SLUG_MAX_CHARS,
                Some(CATEGORY_DESCRIPTION_MAX_CHARS),
            ),
            Self::Tag => (MAX_TAG_NAME_CHARS as u32, TAG_SLUG_MAX_CHARS, None),
        }
    }

    /// Records the Blog-owned effects of a Taxonomy translation in the same transaction.
    pub async fn record_applied_in_tx(
        self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        term_id: Uuid,
        security: &SecurityContext,
    ) -> BlogResult<()> {
        self.authorize(security, Action::Update)?;
        if matches!(self, Self::Category) {
            record_category_copy_change_in_tx(txn, tenant_id, term_id).await?;
        }
        publish_blog_reindex_in_tx(txn, tenant_id, security.user_id).await
    }
}

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
    if slug.trim().is_empty() {
        return Err(BlogError::validation("slug must not be empty"));
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
    /// Actor that performed the conversion; carried on the emitted Blog events.
    pub actor_id: Option<Uuid>,
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
#[instrument(skip(txn, input), fields(tenant_id = %input.tenant_id, post_id = %input.post_id))]
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
    // Reject an unknown status instead of persisting an arbitrary string that
    // would later break every read that maps the stored value back to the typed
    // state machine.
    let status = storage_to_status(&input.status)?;
    if input.slug.trim().is_empty() {
        return Err(BlogError::validation("slug must not be empty"));
    }
    if let Some(category_id) = input.category_id
        && category_id.is_nil()
    {
        return Err(BlogError::validation("category_id must not be nil"));
    }
    if input.translations.is_empty() {
        return Err(BlogError::validation(
            "post must have at least one translation",
        ));
    }
    for translation in &input.translations {
        if translation.locale.trim().is_empty() {
            return Err(BlogError::validation(
                "translation locale must not be empty",
            ));
        }
        if translation.title.trim().is_empty() {
            return Err(BlogError::validation("translation title must not be empty"));
        }
    }
    let now = Utc::now();
    blog_post::ActiveModel {
        id: Set(input.post_id),
        tenant_id: Set(input.tenant_id),
        author_id: Set(input.author_id),
        category_id: Set(input.category_id),
        status: Set(status_to_storage(status)),
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

    let actor_id = input.actor_id;
    // The creation fact carries one representative locale; the reindex request
    // covers the whole post and every translation.
    let primary_locale = input.translations[0].locale.clone();
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

    // Emit the same Blog-owned facts a normal creation would, or the bridge's
    // raw insert stays invisible to search/feed/comment consumers that listen to
    // Blog events. These run in the caller's transaction.
    publish_blog_event_in_tx(
        txn,
        input.tenant_id,
        actor_id,
        DomainEvent::BlogPostCreated {
            post_id: input.post_id,
            author_id: Some(input.author_id),
            locale: primary_locale,
        },
    )
    .await?;
    if status == crate::state_machine::BlogPostStatus::Published {
        publish_blog_event_in_tx(
            txn,
            input.tenant_id,
            actor_id,
            DomainEvent::BlogPostPublished {
                post_id: input.post_id,
                author_id: Some(input.author_id),
            },
        )
        .await?;
    }
    publish_blog_event_in_tx(
        txn,
        input.tenant_id,
        actor_id,
        DomainEvent::ReindexRequested {
            target_type: "blog".to_string(),
            target_id: Some(input.post_id),
        },
    )
    .await?;
    Ok(())
}

/// Publishes a Blog root event inside the caller's transaction via the owner
/// outbox. Centralised so the orchestration bridge emits Blog events through the
/// same validation/registration path as the rest of the module.
async fn publish_blog_event_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    event: DomainEvent,
) -> BlogResult<()> {
    TransactionalEventBus::publish_root_in_tx(txn, tenant_id, actor_id, event)
        .await
        .map_err(BlogError::from)
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
        .order_by_asc(blog_post_tag::Column::TagId)
        .all(txn)
        .await
        .map_err(BlogError::from)?;
    Ok(relations.into_iter().map(|item| item.tag_id).collect())
}

/// Transfer the tags through the same owner policy and projection as a post edit.
pub async fn sync_promoted_post_tags_in_tx(
    taxonomy: &dyn BlogTaxonomyPort,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
    tags: &[String],
    locale: &str,
    security: &SecurityContext,
) -> BlogResult<()> {
    validate_tags(tags)?;
    let allow_create =
        security.get_scope(Resource::Tags, Action::Create) != rustok_core::PermissionScope::None;
    sync_post_tags_in_tx(
        taxonomy,
        txn,
        tenant_id,
        post_id,
        tags,
        locale,
        allow_create,
    )
    .await
}

/// Read localized tag names for a conversion through the selected Blog provider.
pub async fn load_post_tag_names_in_tx(
    taxonomy: &dyn BlogTaxonomyPort,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
    locale: &str,
) -> BlogResult<Vec<String>> {
    let ids = load_post_tag_ids_in_tx(txn, tenant_id, post_id).await?;
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut tags = taxonomy
        .load_tags_by_ids(txn.into(), tenant_id, &ids, locale, None)
        .await?;
    ids.into_iter()
        .map(|id| {
            tags.remove(&id)
                .map(|tag| tag.name)
                .ok_or(BlogError::TaxonomyTermNotFound(id))
        })
        .collect()
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
        // Tenant isolation: post was verified for tenant_id above
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
/// Emits the Blog-owned deletion fact and a targeted reindex so search/feed
/// consumers drop the demoted post instead of serving a stale document.
#[instrument(skip(txn, taxonomy), fields(tenant_id = %tenant_id, post_id = %post_id))]
pub async fn delete_demoted_post_in_tx(
    taxonomy: &dyn BlogTaxonomyPort,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
    actor_id: Option<Uuid>,
) -> BlogResult<()> {
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

    // Remove tags and refresh usage projection
    remove_post_tag_links_with_port_in_tx(Some(taxonomy), txn, tenant_id, post_id).await?;

    // Delete comment projection delivery records
    blog_comment_projection_delivery::Entity::delete_many()
        .filter(blog_comment_projection_delivery::Column::TenantId.eq(tenant_id))
        .filter(blog_comment_projection_delivery::Column::PostId.eq(post_id))
        .exec(txn)
        .await
        .map_err(BlogError::from)?;

    // Delete preview tokens
    crate::services::preview_token::delete_post_preview_tokens_in_tx(txn, tenant_id, post_id)
        .await?;

    // Delete channel visibility
    blog_post_channel_visibility::Entity::delete_many()
        .filter(blog_post_channel_visibility::Column::TenantId.eq(tenant_id))
        .filter(blog_post_channel_visibility::Column::PostId.eq(post_id))
        .exec(txn)
        .await
        .map_err(BlogError::from)?;

    // Delete post translations
    blog_post_translation::Entity::delete_many()
        .filter(blog_post_translation::Column::PostId.eq(post_id))
        .exec(txn)
        .await
        .map_err(BlogError::from)?;

    // Delete post with optimistic concurrency verification
    let deleted = blog_post::Entity::delete_many()
        .filter(blog_post::Column::TenantId.eq(tenant_id))
        .filter(blog_post::Column::Id.eq(post_id))
        .filter(blog_post::Column::Version.eq(post.version))
        .exec(txn)
        .await
        .map_err(BlogError::from)?;
    if deleted.rows_affected != 1 {
        return Err(BlogError::conflict(
            "Blog post changed concurrently before demotion deletion",
        ));
    }

    // Publish after the row is gone so the reindex reflects the removal.
    publish_blog_event_in_tx(
        txn,
        tenant_id,
        actor_id,
        DomainEvent::BlogPostDeleted { post_id },
    )
    .await?;
    publish_blog_event_in_tx(
        txn,
        tenant_id,
        actor_id,
        DomainEvent::ReindexRequested {
            target_type: "blog".to_string(),
            target_id: Some(post_id),
        },
    )
    .await?;

    Ok(())
}
