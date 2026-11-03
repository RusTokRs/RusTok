use chrono::Utc;
use rustok_blog::{
    CreatePromotedPostInput, PromotedPostTranslationInput, create_promoted_post_in_tx,
    ensure_blog_category_exists_in_tx, ensure_blog_slug_unique_in_tx,
};
use rustok_content::{
    ContentError, ContentResult, PromoteTopicToPostInput, PromoteTopicToPostOutput,
};
use rustok_forum::services::topic_routes::forum_topic_route;
use rustok_forum::{TopicStatus, forum_topic};
use rustok_taxonomy::TaxonomyService;
use sea_orm::{ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter};
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
    if tenant_id.is_nil() {
        return Err(ContentError::validation("tenant_id must not be nil"));
    }
    if input.topic_id.is_nil() {
        return Err(ContentError::validation("topic_id must not be nil"));
    }
    if let Some(category_id) = input.blog_category_id
        && category_id.is_nil()
    {
        return Err(ContentError::validation("blog_category_id must not be nil"));
    }
    if let Some(actor_id) = actor_id
        && actor_id.is_nil()
    {
        return Err(ContentError::validation("actor_id must not be nil"));
    }
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
        ensure_blog_category_exists_in_tx(txn, tenant_id, category_id)
            .await
            .map_err(crate::errors::blog_error_to_content_error)?;
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
    ensure_blog_slug_unique_in_tx(txn, tenant_id, &slug)
        .await
        .map_err(crate::errors::blog_error_to_content_error)?;

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

    let post_translations = translations
        .iter()
        .map(|translation| PromotedPostTranslationInput {
            locale: translation.locale.clone(),
            title: translation.title.clone(),
            body: translation.body.clone(),
            created_at: translation.created_at.into(),
            updated_at: translation.updated_at.into(),
        })
        .collect();

    create_promoted_post_in_tx(
        txn,
        CreatePromotedPostInput {
            post_id,
            tenant_id,
            author_id,
            category_id: input.blog_category_id,
            status: post_status.to_string(),
            slug: slug.clone(),
            metadata: serde_json::json!({
                "orchestration": {
                    "source_type": "forum_topic",
                    "source_id": topic.id,
                    "source_category_id": topic.category_id,
                }
            }),
            comment_count: active_comments,
            created_at: topic.created_at.into(),
            published_at: if post_status == "published" {
                Some(topic.created_at.into())
            } else {
                None
            },
            archived_at: if post_status == "archived" {
                Some(now)
            } else {
                None
            },
            translations: post_translations,
        },
    )
    .await
    .map_err(crate::errors::blog_error_to_content_error)?;

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

    forum_topic::Entity::delete_many()
        .filter(forum_topic::Column::TenantId.eq(tenant_id))
        .filter(forum_topic::Column::Id.eq(topic.id))
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
