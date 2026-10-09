use chrono::Utc;
use rustok_api::PLATFORM_FALLBACK_LOCALE;
use rustok_blog::{blog_category, blog_post, blog_post_translation};
use rustok_content::{
    ContentError, ContentResult, PromoteTopicToPostInput, PromoteTopicToPostOutput,
};
use rustok_forum::services::topic_routes::forum_topic_route;
use rustok_forum::{TopicStatus, forum_topic};
use rustok_taxonomy::TaxonomyService;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseTransaction, EntityTrait,
    PaginatorTrait, QueryFilter,
};
use uuid::Uuid;

use crate::bridge::OwnerRoutes;
use crate::bridge::helpers::{
    ForumReplyRecord, adjust_forum_category_counters_in_tx, find_topic_in_tx,
    load_forum_reply_records_in_tx, load_topic_translations_in_tx, locales_from_topic_translations,
    normalize_locale, normalize_slug, resolve_topic_translation,
};
use crate::bridge::tags::{load_forum_tag_names_for_topic_in_tx, sync_blog_tags_for_post_in_tx};

pub(crate) async fn promote_topic_to_post(
    taxonomy: &TaxonomyService,
    routes: &OwnerRoutes,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    input: &PromoteTopicToPostInput,
) -> ContentResult<PromoteTopicToPostOutput> {
    let requested_locale = normalize_locale(&input.locale)?;
    let topic = find_topic_in_tx(txn, tenant_id, input.topic_id).await?;
    let translations = load_topic_translations_in_tx(txn, tenant_id, topic.id).await?;
    let resolved = resolve_topic_translation(&translations, &requested_locale)?;
    let source_translation = resolved
        .item
        .ok_or_else(|| ContentError::translation_not_found(topic.id, &requested_locale))?;
    let author_id = topic
        .author_id
        .or(actor_id)
        .ok_or_else(|| ContentError::validation("Topic author is required for conversion"))?;

    if let Some(category_id) = input.blog_category_id {
        ensure_blog_category_exists_in_tx(txn, tenant_id, category_id).await?;
    }

    let slug = source_translation
        .slug
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| normalize_slug(&source_translation.title));
    if slug.is_empty() {
        return Err(ContentError::validation(
            "Topic translation must resolve to a non-empty slug",
        ));
    }
    ensure_blog_slug_unique_in_tx(txn, tenant_id, &slug).await?;

    for translation in &translations {
        rustok_content::richtext::parse_json(
            &translation.body,
            rustok_content::richtext::RichTextProfile::Article,
        )
        .map_err(|_| {
            ContentError::validation(
                "Forum topic content must be canonical article-compatible richtext before promotion to Blog",
            )
        })?;
    }

    let reply_records = load_forum_reply_records_in_tx(txn, tenant_id, topic.id).await?;
    let post_id = Uuid::new_v4();
    let active_comments =
        move_forum_replies_to_comments_in_tx(txn, tenant_id, post_id, actor_id, &reply_records)
            .await?;

    let now = Utc::now();
    let post_status = match topic.status {
        TopicStatus::Archived => "archived",
        TopicStatus::Open | TopicStatus::Closed => "published",
        TopicStatus::Pending => {
            return Err(ContentError::validation(
                "Forum topic awaiting moderation cannot be promoted to a Blog post",
            ));
        }
    };

    blog_post::ActiveModel {
        id: Set(post_id),
        tenant_id: Set(tenant_id),
        author_id: Set(author_id),
        category_id: Set(input.blog_category_id),
        status: Set(post_status.to_string()),
        slug: Set(slug.clone()),
        metadata: Set(serde_json::json!({
            "orchestration": {
                "source_type": "forum_topic",
                "source_id": topic.id,
                "source_category_id": topic.category_id,
            }
        })),
        featured_image_url: Set(None),
        published_at: Set(if post_status == "published" {
            Some(topic.created_at)
        } else {
            None
        }),
        created_at: Set(topic.created_at),
        updated_at: Set(now.into()),
        archived_at: Set(if post_status == "archived" {
            Some(now.into())
        } else {
            None
        }),
        comment_count: Set(active_comments),
        version: Set(1),
    }
    .insert(txn)
    .await?;

    for translation in &translations {
        blog_post_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            post_id: Set(post_id),
            locale: Set(translation.locale.clone()),
            title: Set(translation.title.clone()),
            excerpt: Set(None),
            seo_title: Set(None),
            seo_description: Set(None),
            body: Set(translation.body.clone()),
            created_at: Set(translation.created_at),
            updated_at: Set(translation.updated_at),
        }
        .insert(txn)
        .await?;
    }

    let tags = load_forum_tag_names_for_topic_in_tx(
        txn,
        tenant_id,
        topic.id,
        &resolved.effective_locale,
        None,
    )
    .await?;
    sync_blog_tags_for_post_in_tx(
        taxonomy,
        txn,
        tenant_id,
        post_id,
        &tags,
        &resolved.effective_locale,
    )
    .await?;

    forum_topic::Entity::delete_by_id(topic.id)
        .exec(txn)
        .await?;
    adjust_forum_category_counters_in_tx(txn, tenant_id, topic.category_id, -1, -topic.reply_count)
        .await?;

    // Routes move from Forum to Blog. Redirects that pointed at the topic leave
    // with it, and its derived canonical route is purged in every topic locale.
    // The forum route of every translation then redirects to the post. Blog
    // routes are global, so the post's canonical route is locale-neutral, and
    // the slug claim frees any redirect that still holds that route.
    let topic_locales = locales_from_topic_translations(&translations)?;
    routes
        .forum
        .remove_redirects_to_target_in_tx(txn, tenant_id, actor_id, "forum_topic", topic.id)
        .await
        .map_err(crate::errors::forum_error_to_content_error)?;
    routes
        .forum
        .purge_topic_canonical_in_tx(txn, tenant_id, actor_id, topic.id, &topic_locales)
        .await
        .map_err(crate::errors::forum_error_to_content_error)?;
    routes
        .blog
        .release_slug_route_in_tx(txn, tenant_id, actor_id, &slug)
        .await
        .map_err(crate::errors::blog_error_to_content_error)?;
    let blog_canonical = rustok_blog::canonical_post_route(&slug);
    for locale in &topic_locales {
        routes
            .forum
            .record_redirect_in_tx(
                txn,
                tenant_id,
                actor_id,
                locale,
                &forum_topic_route(topic.id),
                "blog_post",
                post_id,
                &blog_canonical,
            )
            .await
            .map_err(crate::errors::forum_error_to_content_error)?;
    }

    Ok(PromoteTopicToPostOutput {
        topic_id: topic.id,
        post_id,
        moved_comments: reply_records.len() as u64,
        effective_locale: resolved.effective_locale,
    })
}

pub(crate) async fn ensure_blog_category_exists_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ContentResult<()> {
    let exists = blog_category::Entity::find_by_id(category_id)
        .filter(blog_category::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?;
    if exists.is_none() {
        return Err(ContentError::category_not_found(category_id));
    }
    Ok(())
}

pub(crate) async fn ensure_blog_slug_unique_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    slug: &str,
) -> ContentResult<()> {
    let count = blog_post::Entity::find()
        .filter(blog_post::Column::TenantId.eq(tenant_id))
        .filter(blog_post::Column::Slug.eq(slug))
        .count(txn)
        .await?;
    if count > 0 {
        return Err(ContentError::duplicate_slug(
            slug.to_string(),
            PLATFORM_FALLBACK_LOCALE.to_string(),
        ));
    }
    Ok(())
}

pub(crate) async fn move_forum_replies_to_comments_in_tx(
    _txn: &DatabaseTransaction,
    _tenant_id: Uuid,
    _post_id: Uuid,
    _actor_id: Option<Uuid>,
    reply_records: &[ForumReplyRecord],
) -> ContentResult<i32> {
    if reply_records.is_empty() {
        return Ok(0);
    }

    Err(ContentError::validation(
        "Forum topics with replies cannot be promoted until Forum uses the canonical richtext contract",
    ))
}
