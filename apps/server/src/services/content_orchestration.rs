use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseTransaction, EntityTrait, JoinType,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
};
use uuid::Uuid;

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use async_trait::async_trait;
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use rustok_blog::{blog_category, blog_post, blog_post_tag, blog_post_translation};
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use rustok_comments::dto::{CommentStatus, CommentThreadStatus};
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use rustok_comments::{comment, comment_body, comment_thread};
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use rustok_content::{
    normalize_locale_code, resolve_by_locale_with_fallback, CanonicalUrlMutation, ContentError,
    ContentOrchestrationBridge, ContentOrchestrationService, ContentResult, DemotePostToTopicInput,
    DemotePostToTopicOutput, MergeTopicsInput, MergeTopicsOutput, PromoteTopicToPostInput,
    PromoteTopicToPostOutput, RetiredCanonicalTarget, SplitTopicInput, SplitTopicOutput,
    PLATFORM_FALLBACK_LOCALE,
};
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use rustok_forum::constants::{reply_status, topic_status};
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use rustok_forum::{
    forum_category, forum_reply, forum_reply_body, forum_topic, forum_topic_tag,
    forum_topic_translation,
};
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use rustok_taxonomy::{
    entities::{taxonomy_term, taxonomy_term_alias, taxonomy_term_translation},
    TaxonomyScopeType, TaxonomyTermKind, TaxonomyTermStatus,
};

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
#[derive(Clone)]
pub struct SharedContentOrchestrationService(pub Arc<ContentOrchestrationService>);

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
pub fn init_content_orchestration(ctx: &AppContext) {
    if ctx
        .shared_store
        .get::<SharedContentOrchestrationService>()
        .is_some()
    {
        return;
    }

    let service = Arc::new(ContentOrchestrationService::new(
        ctx.db.clone(),
        crate::services::event_bus::transactional_event_bus_from_context(ctx),
        Arc::new(ServerContentOrchestrationBridge),
    ));
    ctx.shared_store
        .insert(SharedContentOrchestrationService(service));
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
pub fn content_orchestration_from_context(ctx: &AppContext) -> Arc<ContentOrchestrationService> {
    ctx.shared_store
        .get::<SharedContentOrchestrationService>()
        .expect("ContentOrchestrationService not initialized")
        .0
        .clone()
}

#[cfg(not(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
)))]
pub fn init_content_orchestration(_ctx: &AppContext) {}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
struct ServerContentOrchestrationBridge;

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
#[derive(Clone)]
struct ForumReplyRecord {
    reply: forum_reply::Model,
    bodies: Vec<forum_reply_body::Model>,
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
#[derive(Clone)]
struct CommentRecordBundle {
    comment: comment::Model,
    bodies: Vec<comment_body::Model>,
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
#[async_trait]
impl ContentOrchestrationBridge for ServerContentOrchestrationBridge {
    async fn promote_topic_to_post(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: &PromoteTopicToPostInput,
    ) -> ContentResult<PromoteTopicToPostOutput> {
        promote_topic_to_post(txn, tenant_id, actor_id, input).await
    }

    async fn demote_post_to_topic(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: &DemotePostToTopicInput,
    ) -> ContentResult<DemotePostToTopicOutput> {
        demote_post_to_topic(txn, tenant_id, actor_id, input).await
    }

    async fn split_topic(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: &SplitTopicInput,
    ) -> ContentResult<SplitTopicOutput> {
        split_topic(txn, tenant_id, actor_id, input).await
    }

    async fn merge_topics(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: &MergeTopicsInput,
    ) -> ContentResult<MergeTopicsOutput> {
        merge_topics(txn, tenant_id, actor_id, input).await
    }
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
fn normalize_locale(locale: &str) -> ContentResult<String> {
    normalize_locale_code(locale).ok_or_else(|| ContentError::validation("Locale cannot be empty"))
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
fn normalize_slug(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    let mut previous_dash = false;
    for ch in value.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
fn blog_post_route(slug: &str) -> String {
    format!("/modules/blog?slug={slug}")
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
fn forum_topic_route(topic_id: Uuid) -> String {
    format!("/modules/forum?topic={topic_id}")
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
fn locales_from_topic_translations(
    translations: &[forum_topic_translation::Model],
) -> ContentResult<Vec<String>> {
    locales_from_strs(
        translations
            .iter()
            .map(|translation| translation.locale.as_str()),
    )
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
fn locales_from_post_translations(
    translations: &[blog_post_translation::Model],
) -> ContentResult<Vec<String>> {
    locales_from_strs(
        translations
            .iter()
            .map(|translation| translation.locale.as_str()),
    )
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
fn locales_from_strs<'a>(locales: impl IntoIterator<Item = &'a str>) -> ContentResult<Vec<String>> {
    let mut normalized = BTreeSet::new();
    for locale in locales {
        if let Some(locale) = normalize_locale_code(locale) {
            normalized.insert(locale);
        }
    }
    if normalized.is_empty() {
        return Err(ContentError::validation(
            "Conversion requires at least one normalized locale",
        ));
    }
    Ok(normalized.into_iter().collect())
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
fn map_forum_reply_status_to_comment_status(
    status: &str,
    updated_at: chrono::DateTime<chrono::FixedOffset>,
) -> (CommentStatus, Option<chrono::DateTime<chrono::FixedOffset>>) {
    match status {
        reply_status::APPROVED => (CommentStatus::Approved, None),
        reply_status::PENDING => (CommentStatus::Pending, None),
        reply_status::FLAGGED => (CommentStatus::Spam, None),
        reply_status::DELETED => (CommentStatus::Trash, Some(updated_at)),
        reply_status::REJECTED | reply_status::HIDDEN => (CommentStatus::Trash, None),
        _ => (CommentStatus::Pending, None),
    }
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
fn map_comment_status_to_forum_reply_status(
    status: CommentStatus,
    deleted_at: Option<chrono::DateTime<chrono::FixedOffset>>,
) -> &'static str {
    match status {
        CommentStatus::Approved => reply_status::APPROVED,
        CommentStatus::Pending => reply_status::PENDING,
        CommentStatus::Spam => reply_status::FLAGGED,
        CommentStatus::Trash if deleted_at.is_some() => reply_status::DELETED,
        CommentStatus::Trash => reply_status::HIDDEN,
    }
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn promote_topic_to_post(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    input: &PromoteTopicToPostInput,
) -> ContentResult<PromoteTopicToPostOutput> {
    let requested_locale = normalize_locale(&input.locale)?;
    let topic = find_topic_in_tx(txn, tenant_id, input.topic_id).await?;
    let translations = load_topic_translations_in_tx(txn, topic.id).await?;
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

    let reply_records = load_forum_reply_records_in_tx(txn, tenant_id, topic.id).await?;
    let post_id = Uuid::new_v4();
    let now = Utc::now();
    let post_status = match topic.status.as_str() {
        topic_status::ARCHIVED => "archived",
        _ => "published",
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
        comment_count: Set(0),
        view_count: Set(0),
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
            body_format: Set(translation.body_format.clone()),
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
    sync_blog_tags_for_post_in_tx(txn, tenant_id, post_id, &tags, &resolved.effective_locale)
        .await?;

    let active_comments =
        move_forum_replies_to_comments_in_tx(txn, tenant_id, post_id, actor_id, &reply_records)
            .await?;

    let created_post = find_post_in_tx(txn, tenant_id, post_id).await?;
    let mut post_active: blog_post::ActiveModel = created_post.into();
    post_active.comment_count = Set(active_comments);
    post_active.update(txn).await?;

    forum_topic::Entity::delete_by_id(topic.id)
        .exec(txn)
        .await?;
    adjust_forum_category_counters_in_tx(txn, tenant_id, topic.category_id, -1, -topic.reply_count)
        .await?;

    let url_updates = locales_from_topic_translations(&translations)?
        .into_iter()
        .map(|locale| CanonicalUrlMutation {
            target_kind: "blog_post".to_string(),
            target_id: post_id,
            locale: locale.clone(),
            canonical_url: blog_post_route(&slug),
            alias_urls: vec![forum_topic_route(topic.id)],
            retired_targets: vec![RetiredCanonicalTarget {
                target_kind: "forum_topic".to_string(),
                target_id: topic.id,
                locale,
            }],
        })
        .collect();

    Ok(PromoteTopicToPostOutput {
        topic_id: topic.id,
        post_id,
        moved_comments: reply_records.len() as u64,
        effective_locale: resolved.effective_locale,
        url_updates,
    })
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn demote_post_to_topic(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    _actor_id: Option<Uuid>,
    input: &DemotePostToTopicInput,
) -> ContentResult<DemotePostToTopicOutput> {
    let requested_locale = normalize_locale(&input.locale)?;
    ensure_forum_category_exists_in_tx(txn, tenant_id, input.forum_category_id).await?;
    let post = find_post_in_tx(txn, tenant_id, input.post_id).await?;
    let translations = load_post_translations_in_tx(txn, post.id).await?;
    let resolved = resolve_post_translation(&translations, &requested_locale)?;
    let tag_names =
        load_blog_tag_names_for_post_in_tx(txn, post.id, &resolved.effective_locale, None).await?;

    let topic_id = Uuid::new_v4();
    let now = Utc::now();
    let topic_status_value = match post.status.as_str() {
        "archived" => topic_status::ARCHIVED,
        _ => topic_status::OPEN,
    };

    forum_topic::ActiveModel {
        id: Set(topic_id),
        tenant_id: Set(tenant_id),
        category_id: Set(input.forum_category_id),
        author_id: Set(Some(post.author_id)),
        status: Set(topic_status_value.to_string()),
        metadata: Set(serde_json::json!({})),
        is_pinned: Set(false),
        is_locked: Set(false),
        reply_count: Set(0),
        created_at: Set(post.created_at),
        updated_at: Set(now.into()),
        last_reply_at: Set(None),
    }
    .insert(txn)
    .await?;

    for translation in &translations {
        forum_topic_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            topic_id: Set(topic_id),
            locale: Set(translation.locale.clone()),
            title: Set(translation.title.clone()),
            slug: Set(Some(post.slug.clone())),
            body: Set(translation.body.clone()),
            body_format: Set(translation.body_format.clone()),
            created_at: Set(translation.created_at),
            updated_at: Set(translation.updated_at),
        }
        .insert(txn)
        .await?;
    }

    sync_forum_tags_for_topic_in_tx(
        txn,
        tenant_id,
        topic_id,
        &tag_names,
        &resolved.effective_locale,
    )
    .await?;

    let comment_records = load_comment_records_for_post_in_tx(txn, tenant_id, post.id).await?;
    move_comments_to_forum_replies_in_tx(txn, tenant_id, topic_id, &comment_records).await?;
    resequence_forum_topic_replies_in_tx(txn, tenant_id, topic_id).await?;
    refresh_forum_topic_stats_in_tx(txn, tenant_id, topic_id).await?;
    adjust_forum_category_counters_in_tx(
        txn,
        tenant_id,
        input.forum_category_id,
        1,
        comment_records.len() as i32,
    )
    .await?;

    delete_comments_for_post_in_tx(txn, tenant_id, post.id).await?;
    blog_post::Entity::delete_by_id(post.id).exec(txn).await?;

    let url_updates = locales_from_post_translations(&translations)?
        .into_iter()
        .map(|locale| CanonicalUrlMutation {
            target_kind: "forum_topic".to_string(),
            target_id: topic_id,
            locale: locale.clone(),
            canonical_url: forum_topic_route(topic_id),
            alias_urls: vec![blog_post_route(post.slug.as_str())],
            retired_targets: vec![RetiredCanonicalTarget {
                target_kind: "blog_post".to_string(),
                target_id: post.id,
                locale,
            }],
        })
        .collect();

    Ok(DemotePostToTopicOutput {
        post_id: post.id,
        topic_id,
        moved_comments: comment_records.len() as u64,
        effective_locale: resolved.effective_locale,
        url_updates,
    })
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn split_topic(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    _actor_id: Option<Uuid>,
    input: &SplitTopicInput,
) -> ContentResult<SplitTopicOutput> {
    let requested_locale = normalize_locale(&input.locale)?;
    let source_topic = find_topic_in_tx(txn, tenant_id, input.topic_id).await?;
    let source_translations = load_topic_translations_in_tx(txn, source_topic.id).await?;
    let resolved = resolve_topic_translation(&source_translations, &requested_locale)?;
    let moved_set: HashSet<Uuid> = input.reply_ids.iter().copied().collect();
    if moved_set.len() != input.reply_ids.len() {
        return Err(ContentError::validation(
            "split_topic reply_ids must be unique",
        ));
    }

    let reply_records = load_forum_reply_records_in_tx(txn, tenant_id, source_topic.id).await?;
    let moved_records = reply_records
        .iter()
        .filter(|record| moved_set.contains(&record.reply.id))
        .cloned()
        .collect::<Vec<_>>();
    if moved_records.len() != moved_set.len() {
        return Err(ContentError::validation(
            "split_topic reply_ids must belong to the source topic",
        ));
    }

    let target_topic_id = Uuid::new_v4();
    let now = Utc::now();
    let source_tags = load_forum_tag_names_for_topic_in_tx(
        txn,
        tenant_id,
        source_topic.id,
        &resolved.effective_locale,
        None,
    )
    .await?;
    forum_topic::ActiveModel {
        id: Set(target_topic_id),
        tenant_id: Set(tenant_id),
        category_id: Set(source_topic.category_id),
        author_id: Set(source_topic.author_id),
        status: Set(source_topic.status.clone()),
        metadata: Set(source_topic.metadata.clone()),
        is_pinned: Set(false),
        is_locked: Set(source_topic.is_locked),
        reply_count: Set(0),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        last_reply_at: Set(None),
    }
    .insert(txn)
    .await?;

    let mut requested_translation_written = false;
    for translation in &source_translations {
        let title = if translation.locale == requested_locale {
            requested_translation_written = true;
            input.new_title.clone()
        } else {
            translation.title.clone()
        };
        forum_topic_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            topic_id: Set(target_topic_id),
            locale: Set(translation.locale.clone()),
            title: Set(title),
            slug: Set(translation.slug.clone()),
            body: Set(translation.body.clone()),
            body_format: Set(translation.body_format.clone()),
            created_at: Set(translation.created_at),
            updated_at: Set(translation.updated_at),
        }
        .insert(txn)
        .await?;
    }

    sync_forum_tags_for_topic_in_tx(
        txn,
        tenant_id,
        target_topic_id,
        &source_tags,
        &resolved.effective_locale,
    )
    .await?;

    if !requested_translation_written {
        let translation = resolved.item.ok_or_else(|| {
            ContentError::translation_not_found(source_topic.id, requested_locale.clone())
        })?;
        forum_topic_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            topic_id: Set(target_topic_id),
            locale: Set(requested_locale.clone()),
            title: Set(input.new_title.clone()),
            slug: Set(Some(normalize_slug(&input.new_title))),
            body: Set(translation.body.clone()),
            body_format: Set(translation.body_format.clone()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(txn)
        .await?;
    }

    for (index, record) in moved_records.iter().enumerate() {
        let mut active: forum_reply::ActiveModel = record.reply.clone().into();
        active.topic_id = Set(target_topic_id);
        active.parent_reply_id = Set(record
            .reply
            .parent_reply_id
            .filter(|parent_id| moved_set.contains(parent_id)));
        active.position = Set(index as i64 + 1);
        active.updated_at = Set(now.into());
        active.update(txn).await?;
    }

    resequence_forum_topic_replies_in_tx(txn, tenant_id, source_topic.id).await?;
    refresh_forum_topic_stats_in_tx(txn, tenant_id, source_topic.id).await?;
    refresh_forum_topic_stats_in_tx(txn, tenant_id, target_topic_id).await?;

    let target_translations = load_topic_translations_in_tx(txn, target_topic_id).await?;
    let url_updates = locales_from_topic_translations(&target_translations)?
        .into_iter()
        .map(|locale| CanonicalUrlMutation {
            target_kind: "forum_topic".to_string(),
            target_id: target_topic_id,
            locale,
            canonical_url: forum_topic_route(target_topic_id),
            alias_urls: Vec::new(),
            retired_targets: Vec::new(),
        })
        .collect();

    Ok(SplitTopicOutput {
        source_topic_id: source_topic.id,
        target_topic_id,
        moved_reply_ids: input.reply_ids.clone(),
        moved_comments: moved_records.len() as u64,
        url_updates,
    })
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn merge_topics(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    _actor_id: Option<Uuid>,
    input: &MergeTopicsInput,
) -> ContentResult<MergeTopicsOutput> {
    let target_topic = find_topic_in_tx(txn, tenant_id, input.target_topic_id).await?;
    let source_ids = unique_source_ids(input.target_topic_id, &input.source_topic_ids)?;
    let target_translations = load_topic_translations_in_tx(txn, target_topic.id).await?;
    let mut next_position = next_forum_reply_position_in_tx(txn, target_topic.id).await?;
    let mut moved_count = 0_u64;
    let now = Utc::now();
    let mut source_topics = Vec::new();
    let mut merge_locales = locales_from_topic_translations(&target_translations)?
        .into_iter()
        .collect::<BTreeSet<_>>();

    for source_topic_id in &source_ids {
        let source_topic = find_topic_in_tx(txn, tenant_id, *source_topic_id).await?;
        let source_translations = load_topic_translations_in_tx(txn, source_topic.id).await?;
        merge_locales.extend(locales_from_topic_translations(&source_translations)?);
        let replies = load_forum_reply_records_in_tx(txn, tenant_id, source_topic.id).await?;
        for record in &replies {
            let mut active: forum_reply::ActiveModel = record.reply.clone().into();
            active.topic_id = Set(target_topic.id);
            active.position = Set(next_position);
            active.updated_at = Set(now.into());
            active.update(txn).await?;
            next_position += 1;
        }
        moved_count += replies.len() as u64;
        source_topics.push(source_topic);
    }

    resequence_forum_topic_replies_in_tx(txn, tenant_id, target_topic.id).await?;
    refresh_forum_topic_stats_in_tx(txn, tenant_id, target_topic.id).await?;

    let mut category_topic_delta: HashMap<Uuid, i32> = HashMap::new();
    let mut category_reply_delta: HashMap<Uuid, i32> = HashMap::new();
    *category_reply_delta
        .entry(target_topic.category_id)
        .or_default() += moved_count as i32;
    for source_topic in &source_topics {
        *category_topic_delta
            .entry(source_topic.category_id)
            .or_default() -= 1;
        *category_reply_delta
            .entry(source_topic.category_id)
            .or_default() -= source_topic.reply_count;
        forum_topic::Entity::delete_by_id(source_topic.id)
            .exec(txn)
            .await?;
    }
    for (category_id, topic_delta) in category_topic_delta {
        let reply_delta = category_reply_delta
            .remove(&category_id)
            .unwrap_or_default();
        adjust_forum_category_counters_in_tx(txn, tenant_id, category_id, topic_delta, reply_delta)
            .await?;
    }
    for (category_id, reply_delta) in category_reply_delta {
        adjust_forum_category_counters_in_tx(txn, tenant_id, category_id, 0, reply_delta).await?;
    }

    let alias_urls = source_topics
        .iter()
        .map(|source_topic| forum_topic_route(source_topic.id))
        .collect::<Vec<_>>();
    let retired_target_ids = source_topics
        .iter()
        .map(|source_topic| source_topic.id)
        .collect::<Vec<_>>();
    let url_updates = merge_locales
        .into_iter()
        .map(|locale| CanonicalUrlMutation {
            target_kind: "forum_topic".to_string(),
            target_id: target_topic.id,
            locale: locale.clone(),
            canonical_url: forum_topic_route(target_topic.id),
            alias_urls: alias_urls.clone(),
            retired_targets: retired_target_ids
                .iter()
                .copied()
                .map(|target_id| RetiredCanonicalTarget {
                    target_kind: "forum_topic".to_string(),
                    target_id,
                    locale: locale.clone(),
                })
                .collect(),
        })
        .collect();

    Ok(MergeTopicsOutput {
        target_topic_id: target_topic.id,
        source_topic_ids: source_ids,
        moved_comments: moved_count,
        url_updates,
    })
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn find_topic_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ContentResult<forum_topic::Model> {
    forum_topic::Entity::find_by_id(topic_id)
        .filter(forum_topic::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or_else(|| ContentError::node_not_found(topic_id))
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn find_post_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
) -> ContentResult<blog_post::Model> {
    blog_post::Entity::find_by_id(post_id)
        .filter(blog_post::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or_else(|| ContentError::node_not_found(post_id))
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn load_topic_translations_in_tx(
    txn: &DatabaseTransaction,
    topic_id: Uuid,
) -> ContentResult<Vec<forum_topic_translation::Model>> {
    Ok(forum_topic_translation::Entity::find()
        .filter(forum_topic_translation::Column::TopicId.eq(topic_id))
        .all(txn)
        .await?)
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn load_post_translations_in_tx(
    txn: &DatabaseTransaction,
    post_id: Uuid,
) -> ContentResult<Vec<blog_post_translation::Model>> {
    Ok(blog_post_translation::Entity::find()
        .filter(blog_post_translation::Column::PostId.eq(post_id))
        .all(txn)
        .await?)
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
fn resolve_topic_translation<'a>(
    translations: &'a [forum_topic_translation::Model],
    locale: &str,
) -> ContentResult<rustok_content::ResolvedLocale<'a, forum_topic_translation::Model>> {
    if translations.is_empty() {
        return Err(ContentError::translation_not_found(Uuid::nil(), locale));
    }
    Ok(resolve_by_locale_with_fallback(
        translations,
        locale,
        None,
        |translation| translation.locale.as_str(),
    ))
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
fn resolve_post_translation<'a>(
    translations: &'a [blog_post_translation::Model],
    locale: &str,
) -> ContentResult<rustok_content::ResolvedLocale<'a, blog_post_translation::Model>> {
    if translations.is_empty() {
        return Err(ContentError::translation_not_found(Uuid::nil(), locale));
    }
    Ok(resolve_by_locale_with_fallback(
        translations,
        locale,
        None,
        |translation| translation.locale.as_str(),
    ))
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn ensure_blog_category_exists_in_tx(
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

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn ensure_forum_category_exists_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ContentResult<()> {
    let exists = forum_category::Entity::find_by_id(category_id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?;
    if exists.is_none() {
        return Err(ContentError::category_not_found(category_id));
    }
    Ok(())
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn ensure_blog_slug_unique_in_tx(
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

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn load_forum_reply_records_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ContentResult<Vec<ForumReplyRecord>> {
    let replies = forum_reply::Entity::find()
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(topic_id))
        .order_by_asc(forum_reply::Column::Position)
        .all(txn)
        .await?;
    if replies.is_empty() {
        return Ok(Vec::new());
    }

    let reply_ids = replies.iter().map(|reply| reply.id).collect::<Vec<_>>();
    let bodies = forum_reply_body::Entity::find()
        .filter(forum_reply_body::Column::ReplyId.is_in(reply_ids))
        .all(txn)
        .await?;
    let mut bodies_map: HashMap<Uuid, Vec<forum_reply_body::Model>> = HashMap::new();
    for body in bodies {
        bodies_map.entry(body.reply_id).or_default().push(body);
    }

    Ok(replies
        .into_iter()
        .map(|reply| ForumReplyRecord {
            bodies: bodies_map.remove(&reply.id).unwrap_or_default(),
            reply,
        })
        .collect())
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn move_forum_replies_to_comments_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
    actor_id: Option<Uuid>,
    reply_records: &[ForumReplyRecord],
) -> ContentResult<i32> {
    if reply_records.is_empty() {
        return Ok(0);
    }

    let now = Utc::now();
    let active_comments = reply_records
        .iter()
        .filter(|record| record.reply.status != reply_status::DELETED)
        .count() as i32;
    let thread_id = Uuid::new_v4();
    comment_thread::ActiveModel {
        id: Set(thread_id),
        tenant_id: Set(tenant_id),
        target_type: Set("blog_post".to_string()),
        target_id: Set(post_id),
        status: Set(CommentThreadStatus::Open),
        comment_count: Set(active_comments),
        last_commented_at: Set(reply_records.last().map(|record| record.reply.created_at)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(txn)
    .await?;

    for record in reply_records {
        let author_id =
            record.reply.author_id.or(actor_id).ok_or_else(|| {
                ContentError::validation("Reply author is required for conversion")
            })?;
        let (status, deleted_at) =
            map_forum_reply_status_to_comment_status(&record.reply.status, record.reply.updated_at);
        comment::ActiveModel {
            id: Set(record.reply.id),
            tenant_id: Set(tenant_id),
            thread_id: Set(thread_id),
            author_id: Set(author_id),
            parent_comment_id: Set(record.reply.parent_reply_id),
            status: Set(status),
            position: Set(record.reply.position),
            created_at: Set(record.reply.created_at),
            updated_at: Set(record.reply.updated_at),
            deleted_at: Set(deleted_at),
        }
        .insert(txn)
        .await?;

        for body in &record.bodies {
            comment_body::ActiveModel {
                id: Set(Uuid::new_v4()),
                comment_id: Set(record.reply.id),
                locale: Set(body.locale.clone()),
                body: Set(body.body.clone()),
                body_format: Set(body.body_format.clone()),
                created_at: Set(body.created_at),
                updated_at: Set(body.updated_at),
            }
            .insert(txn)
            .await?;
        }
    }

    Ok(active_comments)
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn sync_blog_tags_for_post_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
    tag_names: &[String],
    locale: &str,
) -> ContentResult<()> {
    let locale = normalize_locale(locale)?;
    let mut names = tag_names
        .iter()
        .map(|name| name.trim().to_ascii_lowercase())
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();

    blog_post_tag::Entity::delete_many()
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .exec(txn)
        .await?;

    for name in names {
        let slug = normalize_slug(&name);
        if slug.is_empty() {
            continue;
        }
        let tag_id =
            find_or_create_blog_taxonomy_term_in_tx(txn, tenant_id, &locale, &name, &slug).await?;

        blog_post_tag::ActiveModel {
            post_id: Set(post_id),
            tag_id: Set(tag_id),
            created_at: Set(Utc::now().into()),
        }
        .insert(txn)
        .await?;
    }

    Ok(())
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn find_or_create_blog_taxonomy_term_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    locale: &str,
    name: &str,
    slug: &str,
) -> ContentResult<Uuid> {
    if let Some(term_id) = find_blog_taxonomy_term_id_in_tx(txn, tenant_id, locale, slug).await? {
        return Ok(term_id);
    }

    let now = Utc::now();
    let term_id = Uuid::new_v4();
    taxonomy_term::ActiveModel {
        id: Set(term_id),
        tenant_id: Set(tenant_id),
        kind: Set(TaxonomyTermKind::Tag),
        scope_type: Set(TaxonomyScopeType::Module),
        scope_value: Set("blog".to_string()),
        canonical_key: Set(slug.to_string()),
        status: Set(TaxonomyTermStatus::Active),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(txn)
    .await?;

    taxonomy_term_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        term_id: Set(term_id),
        tenant_id: Set(tenant_id),
        locale: Set(locale.to_string()),
        name: Set(name.to_string()),
        slug: Set(slug.to_string()),
        description: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(txn)
    .await?;

    Ok(term_id)
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn find_blog_taxonomy_term_id_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    locale: &str,
    slug: &str,
) -> ContentResult<Option<Uuid>> {
    for (scope_type, scope_value) in [
        (TaxonomyScopeType::Module, "blog"),
        (TaxonomyScopeType::Global, ""),
    ] {
        if let Some(translation) = taxonomy_term_translation::Entity::find()
            .join(
                JoinType::InnerJoin,
                taxonomy_term_translation::Relation::Term.def(),
            )
            .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term_translation::Column::Locale.eq(locale))
            .filter(taxonomy_term_translation::Column::Slug.eq(slug))
            .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Tag))
            .filter(taxonomy_term::Column::ScopeType.eq(scope_type))
            .filter(taxonomy_term::Column::ScopeValue.eq(scope_value))
            .one(txn)
            .await?
        {
            return Ok(Some(translation.term_id));
        }

        if let Some(alias) = taxonomy_term_alias::Entity::find()
            .join(
                JoinType::InnerJoin,
                taxonomy_term_alias::Relation::Term.def(),
            )
            .filter(taxonomy_term_alias::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term_alias::Column::Locale.eq(locale))
            .filter(taxonomy_term_alias::Column::Slug.eq(slug))
            .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Tag))
            .filter(taxonomy_term::Column::ScopeType.eq(scope_type))
            .filter(taxonomy_term::Column::ScopeValue.eq(scope_value))
            .one(txn)
            .await?
        {
            return Ok(Some(alias.term_id));
        }

        if let Some(term) = taxonomy_term::Entity::find()
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Tag))
            .filter(taxonomy_term::Column::ScopeType.eq(scope_type))
            .filter(taxonomy_term::Column::ScopeValue.eq(scope_value))
            .filter(taxonomy_term::Column::CanonicalKey.eq(slug))
            .one(txn)
            .await?
        {
            return Ok(Some(term.id));
        }
    }

    Ok(None)
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn load_comment_records_for_post_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
) -> ContentResult<Vec<CommentRecordBundle>> {
    let thread = comment_thread::Entity::find()
        .filter(comment_thread::Column::TenantId.eq(tenant_id))
        .filter(comment_thread::Column::TargetType.eq("blog_post"))
        .filter(comment_thread::Column::TargetId.eq(post_id))
        .one(txn)
        .await?;
    let Some(thread) = thread else {
        return Ok(Vec::new());
    };

    let comments = comment::Entity::find()
        .filter(comment::Column::TenantId.eq(tenant_id))
        .filter(comment::Column::ThreadId.eq(thread.id))
        .order_by_asc(comment::Column::Position)
        .all(txn)
        .await?;
    if comments.is_empty() {
        return Ok(Vec::new());
    }

    let comment_ids = comments.iter().map(|item| item.id).collect::<Vec<_>>();
    let bodies = comment_body::Entity::find()
        .filter(comment_body::Column::CommentId.is_in(comment_ids))
        .all(txn)
        .await?;
    let mut bodies_map: HashMap<Uuid, Vec<comment_body::Model>> = HashMap::new();
    for body in bodies {
        bodies_map.entry(body.comment_id).or_default().push(body);
    }

    Ok(comments
        .into_iter()
        .map(|comment| CommentRecordBundle {
            bodies: bodies_map.remove(&comment.id).unwrap_or_default(),
            comment,
        })
        .collect())
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn move_comments_to_forum_replies_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    comment_records: &[CommentRecordBundle],
) -> ContentResult<()> {
    for record in comment_records {
        forum_reply::ActiveModel {
            id: Set(record.comment.id),
            tenant_id: Set(tenant_id),
            topic_id: Set(topic_id),
            author_id: Set(Some(record.comment.author_id)),
            parent_reply_id: Set(record.comment.parent_comment_id),
            status: Set(map_comment_status_to_forum_reply_status(
                record.comment.status,
                record.comment.deleted_at,
            )
            .to_string()),
            position: Set(record.comment.position),
            created_at: Set(record.comment.created_at),
            updated_at: Set(record.comment.updated_at),
        }
        .insert(txn)
        .await?;

        for body in &record.bodies {
            forum_reply_body::ActiveModel {
                id: Set(Uuid::new_v4()),
                reply_id: Set(record.comment.id),
                locale: Set(body.locale.clone()),
                body: Set(body.body.clone()),
                body_format: Set(body.body_format.clone()),
                created_at: Set(body.created_at),
                updated_at: Set(body.updated_at),
            }
            .insert(txn)
            .await?;
        }
    }
    Ok(())
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn delete_comments_for_post_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
) -> ContentResult<()> {
    let thread = comment_thread::Entity::find()
        .filter(comment_thread::Column::TenantId.eq(tenant_id))
        .filter(comment_thread::Column::TargetType.eq("blog_post"))
        .filter(comment_thread::Column::TargetId.eq(post_id))
        .one(txn)
        .await?;
    if let Some(thread) = thread {
        comment_thread::Entity::delete_by_id(thread.id)
            .exec(txn)
            .await?;
    }
    Ok(())
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn resequence_forum_topic_replies_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ContentResult<()> {
    let replies = forum_reply::Entity::find()
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(topic_id))
        .order_by_asc(forum_reply::Column::Position)
        .all(txn)
        .await?;
    for (index, reply) in replies.into_iter().enumerate() {
        let mut active: forum_reply::ActiveModel = reply.into();
        active.position = Set(index as i64 + 1);
        active.update(txn).await?;
    }
    Ok(())
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn refresh_forum_topic_stats_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ContentResult<()> {
    let topic = find_topic_in_tx(txn, tenant_id, topic_id).await?;
    let replies = forum_reply::Entity::find()
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(topic_id))
        .order_by_desc(forum_reply::Column::CreatedAt)
        .all(txn)
        .await?;
    let mut active: forum_topic::ActiveModel = topic.into();
    active.reply_count = Set(replies.len() as i32);
    active.last_reply_at = Set(replies.first().map(|reply| reply.created_at));
    active.updated_at = Set(Utc::now().into());
    active.update(txn).await?;
    Ok(())
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn adjust_forum_category_counters_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    topic_delta: i32,
    reply_delta: i32,
) -> ContentResult<()> {
    let category = forum_category::Entity::find_by_id(category_id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or_else(|| ContentError::category_not_found(category_id))?;
    let mut active: forum_category::ActiveModel = category.clone().into();
    active.topic_count = Set((category.topic_count + topic_delta).max(0));
    active.reply_count = Set((category.reply_count + reply_delta).max(0));
    active.updated_at = Set(Utc::now().into());
    active.update(txn).await?;
    Ok(())
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn next_forum_reply_position_in_tx(
    txn: &DatabaseTransaction,
    topic_id: Uuid,
) -> ContentResult<i64> {
    let count = forum_reply::Entity::find()
        .filter(forum_reply::Column::TopicId.eq(topic_id))
        .count(txn)
        .await?;
    Ok(count as i64 + 1)
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn load_blog_tag_names_for_post_in_tx(
    txn: &DatabaseTransaction,
    post_id: Uuid,
    locale: &str,
    fallback_locale: Option<&str>,
) -> ContentResult<Vec<String>> {
    let relations = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .all(txn)
        .await?;
    if relations.is_empty() {
        return Ok(Vec::new());
    }

    let tag_ids = relations.iter().map(|item| item.tag_id).collect::<Vec<_>>();
    let terms = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::Id.is_in(tag_ids.clone()))
        .all(txn)
        .await?;
    let term_by_id = terms
        .into_iter()
        .map(|term| (term.id, term))
        .collect::<HashMap<_, _>>();
    let translations = taxonomy_term_translation::Entity::find()
        .filter(taxonomy_term_translation::Column::TermId.is_in(tag_ids))
        .all(txn)
        .await?;
    let mut translations_by_tag: HashMap<Uuid, Vec<taxonomy_term_translation::Model>> =
        HashMap::new();
    for translation in translations {
        translations_by_tag
            .entry(translation.term_id)
            .or_default()
            .push(translation);
    }

    let mut names = Vec::new();
    for relation in relations {
        let localized = translations_by_tag
            .get(&relation.tag_id)
            .cloned()
            .unwrap_or_default();
        let resolved =
            resolve_by_locale_with_fallback(&localized, locale, fallback_locale, |translation| {
                translation.locale.as_str()
            });
        if let Some(translation) = resolved.item {
            names.push(translation.name.clone());
        } else if let Some(term) = term_by_id.get(&relation.tag_id) {
            names.push(term.canonical_key.clone());
        }
    }

    Ok(names)
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn load_forum_tag_names_for_topic_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    locale: &str,
    fallback_locale: Option<&str>,
) -> ContentResult<Vec<String>> {
    let relations = forum_topic_tag::Entity::find()
        .filter(forum_topic_tag::Column::TopicId.eq(topic_id))
        .order_by_asc(forum_topic_tag::Column::CreatedAt)
        .all(txn)
        .await?;
    if relations.is_empty() {
        return Ok(Vec::new());
    }

    let term_ids = relations
        .iter()
        .map(|item| item.term_id)
        .collect::<Vec<_>>();
    let terms = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::Id.is_in(term_ids.clone()))
        .all(txn)
        .await?;
    let term_by_id = terms
        .into_iter()
        .map(|term| (term.id, term))
        .collect::<HashMap<_, _>>();
    let translations = taxonomy_term_translation::Entity::find()
        .filter(taxonomy_term_translation::Column::TermId.is_in(term_ids))
        .all(txn)
        .await?;
    let mut translations_by_tag: HashMap<Uuid, Vec<taxonomy_term_translation::Model>> =
        HashMap::new();
    for translation in translations {
        translations_by_tag
            .entry(translation.term_id)
            .or_default()
            .push(translation);
    }

    let mut names = Vec::new();
    for relation in relations {
        let localized = translations_by_tag
            .get(&relation.term_id)
            .cloned()
            .unwrap_or_default();
        let resolved =
            resolve_by_locale_with_fallback(&localized, locale, fallback_locale, |translation| {
                translation.locale.as_str()
            });
        if let Some(translation) = resolved.item {
            names.push(translation.name.clone());
        } else if let Some(term) = term_by_id.get(&relation.term_id) {
            names.push(term.canonical_key.clone());
        }
    }

    Ok(names)
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn sync_forum_tags_for_topic_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    tag_names: &[String],
    locale: &str,
) -> ContentResult<()> {
    let locale = normalize_locale(locale)?;
    let mut names = tag_names
        .iter()
        .map(|name| name.trim().to_ascii_lowercase())
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();

    forum_topic_tag::Entity::delete_many()
        .filter(forum_topic_tag::Column::TopicId.eq(topic_id))
        .exec(txn)
        .await?;

    for name in names {
        let slug = normalize_slug(&name);
        if slug.is_empty() {
            continue;
        }
        let term_id =
            find_or_create_forum_taxonomy_term_in_tx(txn, tenant_id, &locale, &name, &slug).await?;

        forum_topic_tag::ActiveModel {
            id: Set(Uuid::new_v4()),
            topic_id: Set(topic_id),
            term_id: Set(term_id),
            tenant_id: Set(tenant_id),
            created_at: Set(Utc::now().into()),
        }
        .insert(txn)
        .await?;
    }

    Ok(())
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn find_or_create_forum_taxonomy_term_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    locale: &str,
    name: &str,
    slug: &str,
) -> ContentResult<Uuid> {
    if let Some(term_id) = find_forum_taxonomy_term_id_in_tx(txn, tenant_id, locale, slug).await? {
        return Ok(term_id);
    }

    let now = Utc::now();
    let term_id = Uuid::new_v4();
    taxonomy_term::ActiveModel {
        id: Set(term_id),
        tenant_id: Set(tenant_id),
        kind: Set(TaxonomyTermKind::Tag),
        scope_type: Set(TaxonomyScopeType::Module),
        scope_value: Set("forum".to_string()),
        canonical_key: Set(slug.to_string()),
        status: Set(TaxonomyTermStatus::Active),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(txn)
    .await?;

    taxonomy_term_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        term_id: Set(term_id),
        tenant_id: Set(tenant_id),
        locale: Set(locale.to_string()),
        name: Set(name.to_string()),
        slug: Set(slug.to_string()),
        description: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(txn)
    .await?;

    Ok(term_id)
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn find_forum_taxonomy_term_id_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    locale: &str,
    slug: &str,
) -> ContentResult<Option<Uuid>> {
    for (scope_type, scope_value) in [
        (TaxonomyScopeType::Module, "forum"),
        (TaxonomyScopeType::Global, ""),
    ] {
        if let Some(translation) = taxonomy_term_translation::Entity::find()
            .join(
                JoinType::InnerJoin,
                taxonomy_term_translation::Relation::Term.def(),
            )
            .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term_translation::Column::Locale.eq(locale))
            .filter(taxonomy_term_translation::Column::Slug.eq(slug))
            .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Tag))
            .filter(taxonomy_term::Column::ScopeType.eq(scope_type))
            .filter(taxonomy_term::Column::ScopeValue.eq(scope_value))
            .one(txn)
            .await?
        {
            return Ok(Some(translation.term_id));
        }

        if let Some(alias) = taxonomy_term_alias::Entity::find()
            .join(
                JoinType::InnerJoin,
                taxonomy_term_alias::Relation::Term.def(),
            )
            .filter(taxonomy_term_alias::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term_alias::Column::Locale.eq(locale))
            .filter(taxonomy_term_alias::Column::Slug.eq(slug))
            .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Tag))
            .filter(taxonomy_term::Column::ScopeType.eq(scope_type))
            .filter(taxonomy_term::Column::ScopeValue.eq(scope_value))
            .one(txn)
            .await?
        {
            return Ok(Some(alias.term_id));
        }

        if let Some(term) = taxonomy_term::Entity::find()
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Tag))
            .filter(taxonomy_term::Column::ScopeType.eq(scope_type))
            .filter(taxonomy_term::Column::ScopeValue.eq(scope_value))
            .filter(taxonomy_term::Column::CanonicalKey.eq(slug))
            .one(txn)
            .await?
        {
            return Ok(Some(term.id));
        }
    }

    Ok(None)
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
fn unique_source_ids(target_topic_id: Uuid, source_ids: &[Uuid]) -> ContentResult<Vec<Uuid>> {
    let mut unique = Vec::new();
    let mut seen = HashSet::new();
    for source_id in source_ids {
        if *source_id == target_topic_id {
            return Err(ContentError::validation(
                "merge_topics source topics cannot include the target topic",
            ));
        }
        if seen.insert(*source_id) {
            unique.push(*source_id);
        }
    }
    if unique.is_empty() {
        return Err(ContentError::validation(
            "merge_topics requires at least one source topic",
        ));
    }
    Ok(unique)
}

#[cfg(all(
    test,
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
mod tests {
    use super::*;
    use rustok_blog::{
        entities::{blog_post, blog_post_tag, blog_post_translation},
        CommentService as BlogCommentService, CreateCommentInput as BlogCreateCommentInput,
        CreatePostInput, PostService,
    };
    use rustok_comments::{
        entities::{comment, comment_body},
        CommentsModule, CommentsService, ListCommentsFilter,
    };
    use rustok_content::{
        CanonicalUrlService, ContentModule, DemotePostToTopicInput, PromoteTopicToPostInput,
    };
    use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
    use rustok_forum::{
        entities::{forum_reply, forum_reply_body, forum_topic, forum_topic_translation},
        CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumModule,
        ListRepliesFilter, ReplyService, TopicService,
    };
    use rustok_outbox::TransactionalEventBus;
    use rustok_taxonomy::{
        entities::{taxonomy_term, taxonomy_term_translation},
        TaxonomyModule, TaxonomyScopeType,
    };
    use sea_orm::{
        ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, QueryFilter,
    };
    use sea_orm_migration::SchemaManager;

    #[test]
    fn forum_reply_status_mapping_is_explicit() {
        let now = Utc::now().fixed_offset();
        assert_eq!(
            map_forum_reply_status_to_comment_status(reply_status::APPROVED, now).0,
            CommentStatus::Approved
        );
        assert_eq!(
            map_forum_reply_status_to_comment_status(reply_status::PENDING, now).0,
            CommentStatus::Pending
        );
        assert_eq!(
            map_forum_reply_status_to_comment_status(reply_status::FLAGGED, now).0,
            CommentStatus::Spam
        );
        assert_eq!(
            map_forum_reply_status_to_comment_status(reply_status::DELETED, now).0,
            CommentStatus::Trash
        );
    }

    #[test]
    fn comment_status_mapping_distinguishes_hidden_and_deleted() {
        let now = Utc::now().fixed_offset();
        assert_eq!(
            map_comment_status_to_forum_reply_status(CommentStatus::Approved, None),
            reply_status::APPROVED
        );
        assert_eq!(
            map_comment_status_to_forum_reply_status(CommentStatus::Spam, None),
            reply_status::FLAGGED
        );
        assert_eq!(
            map_comment_status_to_forum_reply_status(CommentStatus::Trash, None),
            reply_status::HIDDEN
        );
        assert_eq!(
            map_comment_status_to_forum_reply_status(CommentStatus::Trash, Some(now)),
            reply_status::DELETED
        );
    }

    async fn setup_conversion_test_db() -> DatabaseConnection {
        let db_url = format!(
            "sqlite:file:server_content_orchestration_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        );
        let mut opts = ConnectOptions::new(db_url);
        opts.max_connections(5)
            .min_connections(1)
            .sqlx_logging(false);

        Database::connect(opts)
            .await
            .expect("failed to connect server content orchestration sqlite database")
    }

    async fn ensure_conversion_schema(db: &DatabaseConnection) {
        let manager = SchemaManager::new(db);
        for migration in ContentModule.migrations() {
            migration
                .up(&manager)
                .await
                .expect("content migration should apply");
        }
        for migration in CommentsModule.migrations() {
            migration
                .up(&manager)
                .await
                .expect("comments migration should apply");
        }
        for migration in TaxonomyModule.migrations() {
            migration
                .up(&manager)
                .await
                .expect("taxonomy migration should apply");
        }
        for migration in rustok_blog::BlogModule.migrations() {
            migration
                .up(&manager)
                .await
                .expect("blog migration should apply");
        }
        for migration in ForumModule.migrations() {
            migration
                .up(&manager)
                .await
                .expect("forum migration should apply");
        }
    }

    fn admin_security() -> SecurityContext {
        SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
    }

    #[tokio::test]
    async fn promote_topic_to_post_moves_replies_and_registers_redirects() {
        let db = setup_conversion_test_db().await;
        ensure_conversion_schema(&db).await;

        let transport = MemoryTransport::new();
        let _receiver = transport.subscribe();
        let events = TransactionalEventBus::new(Arc::new(transport));
        let security = admin_security();
        let tenant_id = Uuid::new_v4();

        let category = CategoryService::new(db.clone())
            .create(
                tenant_id,
                security.clone(),
                CreateCategoryInput {
                    locale: "en".to_string(),
                    name: "General".to_string(),
                    slug: "general".to_string(),
                    description: None,
                    icon: None,
                    color: None,
                    parent_id: None,
                    position: Some(0),
                    moderated: false,
                },
            )
            .await
            .expect("forum category should be created");

        let topic = TopicService::new(db.clone(), events.clone())
            .create(
                tenant_id,
                security.clone(),
                CreateTopicInput {
                    locale: "en".to_string(),
                    category_id: category.id,
                    title: "Legacy thread".to_string(),
                    slug: Some("legacy-thread".to_string()),
                    body: "Original forum body".to_string(),
                    body_format: "markdown".to_string(),
                    content_json: None,
                    metadata: serde_json::json!({}),
                    tags: vec!["release".to_string(), "notes".to_string()],
                    channel_slugs: None,
                },
            )
            .await
            .expect("forum topic should be created");

        let reply_service = ReplyService::new(db.clone(), events.clone());
        reply_service
            .create(
                tenant_id,
                security.clone(),
                topic.id,
                CreateReplyInput {
                    locale: "en".to_string(),
                    content: "First forum reply".to_string(),
                    content_format: "markdown".to_string(),
                    content_json: None,
                    parent_reply_id: None,
                },
            )
            .await
            .expect("first reply should be created");
        reply_service
            .create(
                tenant_id,
                security.clone(),
                topic.id,
                CreateReplyInput {
                    locale: "en".to_string(),
                    content: "Second forum reply".to_string(),
                    content_format: "markdown".to_string(),
                    content_json: None,
                    parent_reply_id: None,
                },
            )
            .await
            .expect("second reply should be created");

        let orchestration = ContentOrchestrationService::new(
            db.clone(),
            events.clone(),
            Arc::new(ServerContentOrchestrationBridge),
        );
        let promoted = orchestration
            .promote_topic_to_post(
                tenant_id,
                security.clone(),
                PromoteTopicToPostInput {
                    topic_id: topic.id,
                    locale: "EN_us".to_string(),
                    blog_category_id: None,
                    reason: Some("promote topic".to_string()),
                    idempotency_key: "topic-to-post-e2e".to_string(),
                },
            )
            .await
            .expect("topic should be promoted to post");

        assert!(
            forum_topic::Entity::find_by_id(topic.id)
                .one(&db)
                .await
                .expect("forum topic lookup should succeed")
                .is_none(),
            "source forum topic should be deleted after promotion"
        );

        let post = blog_post::Entity::find_by_id(promoted.target_id)
            .one(&db)
            .await
            .expect("blog post lookup should succeed")
            .expect("promoted blog post should exist");
        assert_eq!(post.slug, "legacy-thread");
        assert_eq!(post.comment_count, 2);

        let translation = blog_post_translation::Entity::find()
            .filter(blog_post_translation::Column::PostId.eq(promoted.target_id))
            .filter(blog_post_translation::Column::Locale.eq("en"))
            .one(&db)
            .await
            .expect("blog post translation lookup should succeed")
            .expect("promoted blog post translation should exist");
        assert_eq!(translation.title, "Legacy thread");
        assert_eq!(translation.body, "Original forum body");

        let tags = taxonomy_term::Entity::find()
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
            .filter(taxonomy_term::Column::ScopeValue.eq("blog"))
            .all(&db)
            .await
            .expect("blog tags query should succeed");
        let tag_translations = taxonomy_term_translation::Entity::find()
            .filter(
                taxonomy_term_translation::Column::TermId
                    .is_in(tags.iter().map(|tag| tag.id).collect::<Vec<_>>()),
            )
            .filter(taxonomy_term_translation::Column::Locale.eq("en"))
            .all(&db)
            .await
            .expect("blog tag translations query should succeed");
        let post_tags = blog_post_tag::Entity::find()
            .filter(blog_post_tag::Column::PostId.eq(promoted.target_id))
            .all(&db)
            .await
            .expect("blog post tags query should succeed");
        let tag_slugs: HashSet<String> = tag_translations.into_iter().map(|tag| tag.slug).collect();
        assert_eq!(
            tag_slugs,
            HashSet::from(["release".to_string(), "notes".to_string()])
        );
        assert_eq!(post_tags.len(), 2);

        let comments_service = CommentsService::new(db.clone());
        let (comments, total) = comments_service
            .list_comments_for_target(
                tenant_id,
                SecurityContext::system(),
                "blog_post",
                promoted.target_id,
                ListCommentsFilter {
                    locale: "en".to_string(),
                    page: 1,
                    per_page: 20,
                },
                None,
            )
            .await
            .expect("comments for promoted post should list");
        assert_eq!(total, 2);
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].body_preview, "First forum reply");
        assert_eq!(comments[1].body_preview, "Second forum reply");

        let stored_comments = comment::Entity::find()
            .all(&db)
            .await
            .expect("stored comments query should succeed");
        let stored_comment_bodies = comment_body::Entity::find()
            .all(&db)
            .await
            .expect("stored comment bodies query should succeed");
        assert_eq!(stored_comments.len(), 2);
        assert_eq!(stored_comment_bodies.len(), 2);

        let canonical = CanonicalUrlService::new(db.clone());
        let alias_resolution = canonical
            .resolve_route(
                tenant_id,
                "EN_us",
                format!("/modules/forum?topic={}", topic.id).as_str(),
            )
            .await
            .expect("legacy forum route should resolve")
            .expect("legacy forum route should exist as alias");
        assert!(alias_resolution.redirect_required);
        assert_eq!(alias_resolution.target_kind, "blog_post");
        assert_eq!(alias_resolution.target_id, promoted.target_id);
        assert_eq!(
            alias_resolution.canonical_url,
            "/modules/blog?slug=legacy-thread"
        );

        let canonical_resolution = canonical
            .resolve_route(tenant_id, "en", "/modules/blog?slug=legacy-thread")
            .await
            .expect("canonical blog route should resolve")
            .expect("canonical blog route should exist");
        assert!(!canonical_resolution.redirect_required);
        assert_eq!(canonical_resolution.target_kind, "blog_post");
        assert_eq!(canonical_resolution.target_id, promoted.target_id);

        let promoted_retry = orchestration
            .promote_topic_to_post(
                tenant_id,
                security,
                PromoteTopicToPostInput {
                    topic_id: topic.id,
                    locale: "en_us".to_string(),
                    blog_category_id: None,
                    reason: Some("promote topic".to_string()),
                    idempotency_key: "topic-to-post-e2e".to_string(),
                },
            )
            .await
            .expect("idempotent promotion retry should succeed");
        assert_eq!(promoted_retry.target_id, promoted.target_id);
        assert_eq!(promoted_retry.moved_comments, promoted.moved_comments);

        let posts_after_retry = blog_post::Entity::find()
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .all(&db)
            .await
            .expect("blog posts after retry query should succeed");
        let comments_after_retry = comment::Entity::find()
            .all(&db)
            .await
            .expect("stored comments after retry query should succeed");
        let post_tags_after_retry = blog_post_tag::Entity::find()
            .filter(blog_post_tag::Column::PostId.eq(promoted.target_id))
            .all(&db)
            .await
            .expect("blog post tags after retry query should succeed");
        assert_eq!(posts_after_retry.len(), 1);
        assert_eq!(comments_after_retry.len(), 2);
        assert_eq!(post_tags_after_retry.len(), 2);
    }

    #[tokio::test]
    async fn demote_post_to_topic_moves_comments_and_registers_redirects() {
        let db = setup_conversion_test_db().await;
        ensure_conversion_schema(&db).await;

        let transport = MemoryTransport::new();
        let _receiver = transport.subscribe();
        let events = TransactionalEventBus::new(Arc::new(transport));
        let security = admin_security();
        let tenant_id = Uuid::new_v4();

        let forum_category = CategoryService::new(db.clone())
            .create(
                tenant_id,
                security.clone(),
                CreateCategoryInput {
                    locale: "en".to_string(),
                    name: "Imported".to_string(),
                    slug: "imported".to_string(),
                    description: None,
                    icon: None,
                    color: None,
                    parent_id: None,
                    position: Some(0),
                    moderated: false,
                },
            )
            .await
            .expect("forum destination category should be created");

        let post_id = PostService::new(db.clone(), events.clone())
            .create_post(
                tenant_id,
                security.clone(),
                CreatePostInput {
                    locale: "en".to_string(),
                    title: "Legacy post".to_string(),
                    body: "Original blog body".to_string(),
                    body_format: "markdown".to_string(),
                    content_json: None,
                    excerpt: None,
                    slug: Some("legacy-post".to_string()),
                    publish: true,
                    tags: vec!["alpha".to_string(), "beta".to_string()],
                    category_id: None,
                    featured_image_url: None,
                    seo_title: None,
                    seo_description: None,
                    channel_slugs: None,
                    metadata: None,
                },
            )
            .await
            .expect("blog post should be created");

        let blog_comment_service = BlogCommentService::new(db.clone(), events.clone());
        blog_comment_service
            .create_comment(
                tenant_id,
                security.clone(),
                post_id,
                BlogCreateCommentInput {
                    locale: "en".to_string(),
                    content: "First blog comment".to_string(),
                    content_format: "markdown".to_string(),
                    content_json: None,
                    parent_comment_id: None,
                },
            )
            .await
            .expect("first blog comment should be created");
        blog_comment_service
            .create_comment(
                tenant_id,
                security.clone(),
                post_id,
                BlogCreateCommentInput {
                    locale: "en".to_string(),
                    content: "Second blog comment".to_string(),
                    content_format: "markdown".to_string(),
                    content_json: None,
                    parent_comment_id: None,
                },
            )
            .await
            .expect("second blog comment should be created");

        let orchestration = ContentOrchestrationService::new(
            db.clone(),
            events.clone(),
            Arc::new(ServerContentOrchestrationBridge),
        );
        let demoted = orchestration
            .demote_post_to_topic(
                tenant_id,
                security.clone(),
                DemotePostToTopicInput {
                    post_id,
                    locale: "en".to_string(),
                    forum_category_id: forum_category.id,
                    reason: Some("demote post".to_string()),
                    idempotency_key: "post-to-topic-e2e".to_string(),
                },
            )
            .await
            .expect("post should be demoted to topic");

        assert!(
            blog_post::Entity::find_by_id(post_id)
                .one(&db)
                .await
                .expect("blog post lookup should succeed")
                .is_none(),
            "source blog post should be deleted after demotion"
        );

        let topic = forum_topic::Entity::find_by_id(demoted.target_id)
            .one(&db)
            .await
            .expect("forum topic lookup should succeed")
            .expect("demoted forum topic should exist");
        assert_eq!(topic.category_id, forum_category.id);
        assert_eq!(topic.reply_count, 2);

        let topic_translation = forum_topic_translation::Entity::find()
            .filter(forum_topic_translation::Column::TopicId.eq(demoted.target_id))
            .filter(forum_topic_translation::Column::Locale.eq("en"))
            .one(&db)
            .await
            .expect("forum topic translation lookup should succeed")
            .expect("demoted forum topic translation should exist");
        assert_eq!(topic_translation.title, "Legacy post");
        assert_eq!(topic_translation.body, "Original blog body");

        let reply_service = ReplyService::new(db.clone(), events.clone());
        let (replies, total) = reply_service
            .list_response_for_topic_with_locale_fallback(
                tenant_id,
                SecurityContext::system(),
                demoted.target_id,
                ListRepliesFilter {
                    locale: Some("en".to_string()),
                    page: 1,
                    per_page: 20,
                },
                None,
            )
            .await
            .expect("replies for demoted topic should list");
        assert_eq!(total, 2);
        assert_eq!(replies.len(), 2);
        assert_eq!(replies[0].content, "First blog comment");
        assert_eq!(replies[1].content, "Second blog comment");
        assert!(replies
            .iter()
            .all(|reply| reply.status == reply_status::PENDING));

        let stored_replies = forum_reply::Entity::find()
            .filter(forum_reply::Column::TopicId.eq(demoted.target_id))
            .all(&db)
            .await
            .expect("stored replies query should succeed");
        let stored_reply_bodies = forum_reply_body::Entity::find()
            .all(&db)
            .await
            .expect("stored reply bodies query should succeed");
        assert_eq!(stored_replies.len(), 2);
        assert_eq!(stored_reply_bodies.len(), 2);

        let comments_service = CommentsService::new(db.clone());
        let (_, remaining_comments) = comments_service
            .list_comments_for_target(
                tenant_id,
                SecurityContext::system(),
                "blog_post",
                post_id,
                ListCommentsFilter {
                    locale: "en".to_string(),
                    page: 1,
                    per_page: 20,
                },
                None,
            )
            .await
            .expect("legacy blog comments should be queryable");
        assert_eq!(remaining_comments, 0);

        let canonical = CanonicalUrlService::new(db.clone());
        let alias_resolution = canonical
            .resolve_route(tenant_id, "en", "/modules/blog?slug=legacy-post")
            .await
            .expect("legacy blog route should resolve")
            .expect("legacy blog route should exist as alias");
        assert!(alias_resolution.redirect_required);
        assert_eq!(alias_resolution.target_kind, "forum_topic");
        assert_eq!(alias_resolution.target_id, demoted.target_id);
        assert_eq!(
            alias_resolution.canonical_url,
            format!("/modules/forum?topic={}", demoted.target_id)
        );

        let canonical_resolution = canonical
            .resolve_route(
                tenant_id,
                "en",
                format!("/modules/forum?topic={}", demoted.target_id).as_str(),
            )
            .await
            .expect("canonical forum route should resolve")
            .expect("canonical forum route should exist");
        assert!(!canonical_resolution.redirect_required);
        assert_eq!(canonical_resolution.target_kind, "forum_topic");
        assert_eq!(canonical_resolution.target_id, demoted.target_id);

        let demoted_retry = orchestration
            .demote_post_to_topic(
                tenant_id,
                security,
                DemotePostToTopicInput {
                    post_id,
                    locale: "EN".to_string(),
                    forum_category_id: forum_category.id,
                    reason: Some("demote post".to_string()),
                    idempotency_key: "post-to-topic-e2e".to_string(),
                },
            )
            .await
            .expect("idempotent demotion retry should succeed");
        assert_eq!(demoted_retry.target_id, demoted.target_id);
        assert_eq!(demoted_retry.moved_comments, demoted.moved_comments);

        let topics_after_retry = forum_topic::Entity::find()
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .all(&db)
            .await
            .expect("forum topics after retry query should succeed");
        let replies_after_retry = forum_reply::Entity::find()
            .filter(forum_reply::Column::TopicId.eq(demoted.target_id))
            .all(&db)
            .await
            .expect("forum replies after retry query should succeed");
        let residual_comments = comment::Entity::find()
            .all(&db)
            .await
            .expect("residual comments query should succeed");
        assert_eq!(topics_after_retry.len(), 1);
        assert_eq!(replies_after_retry.len(), 2);
        assert_eq!(residual_comments.len(), 0);
    }
}
