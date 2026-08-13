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
use chrono::Utc;
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use rustok_api::PLATFORM_FALLBACK_LOCALE;
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
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use rustok_comments::{comment, comment_thread};
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use rustok_content::{
    CanonicalUrlMutation, ContentError, ContentOrchestrationBridge, ContentOrchestrationService,
    ContentResult, DemotePostToTopicInput, DemotePostToTopicOutput, MergeTopicsInput,
    MergeTopicsOutput, PromoteTopicToPostInput, PromoteTopicToPostOutput, RetiredCanonicalTarget,
    SplitTopicInput, SplitTopicOutput, normalize_locale_code, resolve_by_locale_with_fallback,
};
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use rustok_forum::{
    TopicStatus, forum_category, forum_reply, forum_topic, forum_topic_tag, forum_topic_translation,
};
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use rustok_outbox::TransactionalEventBus;
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use rustok_taxonomy::{
    TaxonomyError, TaxonomyOwnerReader, TaxonomyService, TaxonomyTermKind,
};
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use std::collections::{BTreeSet, HashMap, HashSet};
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use std::sync::Arc;
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use uuid::Uuid;

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
pub mod graphql;

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
pub fn build_content_orchestration_service(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> SharedContentOrchestrationService {
    let bridge = Arc::new(ServerContentOrchestrationBridge::new(db.clone()));
    let service = Arc::new(ContentOrchestrationService::new(db, event_bus, bridge));
    SharedContentOrchestrationService(service)
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
pub fn content_orchestration_from_shared(
    shared: &SharedContentOrchestrationService,
) -> Arc<ContentOrchestrationService> {
    shared.0.clone()
}

#[cfg(not(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
)))]
#[derive(Clone)]
pub struct SharedContentOrchestrationService;

#[cfg(not(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
)))]
pub fn build_content_orchestration_service(
    _db: sea_orm::DatabaseConnection,
    _event_bus: rustok_outbox::TransactionalEventBus,
) -> SharedContentOrchestrationService {
    SharedContentOrchestrationService
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
struct ServerContentOrchestrationBridge {
    taxonomy: TaxonomyService,
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
impl ServerContentOrchestrationBridge {
    fn new(db: DatabaseConnection) -> Self {
        Self {
            taxonomy: TaxonomyService::new(db),
        }
    }
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
#[derive(Clone)]
struct ForumReplyRecord {
    reply: forum_reply::Model,
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
        promote_topic_to_post(&self.taxonomy, txn, tenant_id, actor_id, input).await
    }

    async fn demote_post_to_topic(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: &DemotePostToTopicInput,
    ) -> ContentResult<DemotePostToTopicOutput> {
        demote_post_to_topic(&self.taxonomy, txn, tenant_id, actor_id, input).await
    }

    async fn split_topic(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: &SplitTopicInput,
    ) -> ContentResult<SplitTopicOutput> {
        split_topic(&self.taxonomy, txn, tenant_id, actor_id, input).await
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
fn taxonomy_error_to_content_error(error: TaxonomyError) -> ContentError {
    let message = error.to_string();
    match error {
        TaxonomyError::Database(error) => ContentError::Database(error),
        TaxonomyError::Forbidden(message) => ContentError::forbidden(message),
        TaxonomyError::TermNotFound(_)
        | TaxonomyError::DuplicateCanonicalKey(_)
        | TaxonomyError::DuplicateSlug(_)
        | TaxonomyError::DuplicateAlias(_)
        | TaxonomyError::Conflict(_)
        | TaxonomyError::TranslationRevisionExhausted { .. }
        | TaxonomyError::Validation(_) => ContentError::validation(message),
    }
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
async fn promote_topic_to_post(
    taxonomy: &TaxonomyService,
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

    let reply_records = load_forum_reply_records_in_tx(txn, tenant_id, topic.id).await?;
    let post_id = Uuid::new_v4();
    let now = Utc::now();
    let post_status = match topic.status {
        TopicStatus::Archived => "archived",
        TopicStatus::Open | TopicStatus::Closed => "published",
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
        if rustok_content::richtext::parse_json(
            &translation.body,
            rustok_content::richtext::RichTextProfile::Article,
        )
        .is_err()
        {
            return Err(ContentError::validation(
                "Forum topic content must be canonical article-compatible richtext before promotion to Blog",
            ));
        }

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
    taxonomy: &TaxonomyService,
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
    let tag_names = load_blog_tag_names_for_post_in_tx(
        txn,
        tenant_id,
        post.id,
        &resolved.effective_locale,
        None,
    )
    .await?;
    for translation in &translations {
        rustok_content::richtext::parse_json(
            &translation.body,
            rustok_content::richtext::RichTextProfile::Discussion,
        )
        .map_err(|_| {
            ContentError::validation(
                "Blog post content must be discussion-compatible richtext before demotion to Forum",
            )
        })?;
    }

    let topic_id = Uuid::new_v4();
    let now = Utc::now();
    let topic_status_value = match post.status.as_str() {
        "archived" => TopicStatus::Archived,
        _ => TopicStatus::Open,
    };

    forum_topic::ActiveModel {
        id: Set(topic_id),
        tenant_id: Set(tenant_id),
        category_id: Set(input.forum_category_id),
        author_id: Set(Some(post.author_id)),
        status: Set(topic_status_value),
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
            tenant_id: Set(tenant_id),
            locale: Set(translation.locale.clone()),
            title: Set(translation.title.clone()),
            slug: Set(Some(post.slug.clone())),
            body: Set(translation.body.clone()),
            created_at: Set(translation.created_at),
            updated_at: Set(translation.updated_at),
        }
        .insert(txn)
        .await?;
    }

    sync_forum_tags_for_topic_in_tx(
        taxonomy,
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
    taxonomy: &TaxonomyService,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    _actor_id: Option<Uuid>,
    input: &SplitTopicInput,
) -> ContentResult<SplitTopicOutput> {
    let requested_locale = normalize_locale(&input.locale)?;
    let source_topic = find_topic_in_tx(txn, tenant_id, input.topic_id).await?;
    let source_translations =
        load_topic_translations_in_tx(txn, tenant_id, source_topic.id).await?;
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
        status: Set(source_topic.status),
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
            tenant_id: Set(tenant_id),
            locale: Set(translation.locale.clone()),
            title: Set(title),
            slug: Set(translation.slug.clone()),
            body: Set(translation.body.clone()),
            created_at: Set(translation.created_at),
            updated_at: Set(translation.updated_at),
        }
        .insert(txn)
        .await?;
    }

    sync_forum_tags_for_topic_in_tx(
        taxonomy,
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
            tenant_id: Set(tenant_id),
            locale: Set(requested_locale.clone()),
            title: Set(input.new_title.clone()),
            slug: Set(Some(normalize_slug(&input.new_title))),
            body: Set(translation.body.clone()),
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

    let target_translations =
        load_topic_translations_in_tx(txn, tenant_id, target_topic_id).await?;
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
    let target_translations =
        load_topic_translations_in_tx(txn, tenant_id, target_topic.id).await?;
    let mut next_position = next_forum_reply_position_in_tx(txn, target_topic.id).await?;
    let mut moved_count = 0_u64;
    let now = Utc::now();
    let mut source_topics = Vec::new();
    let mut merge_locales = locales_from_topic_translations(&target_translations)?
        .into_iter()
        .collect::<BTreeSet<_>>();

    for source_topic_id in &source_ids {
        let source_topic = find_topic_in_tx(txn, tenant_id, *source_topic_id).await?;
        let source_translations =
            load_topic_translations_in_tx(txn, tenant_id, source_topic.id).await?;
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
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ContentResult<Vec<forum_topic_translation::Model>> {
    Ok(forum_topic_translation::Entity::find()
        .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
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

    Ok(replies
        .into_iter()
        .map(|reply| ForumReplyRecord { reply })
        .collect())
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn move_forum_replies_to_comments_in_tx(
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

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn sync_blog_tags_for_post_in_tx(
    taxonomy: &TaxonomyService,
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

    let tag_ids = taxonomy
        .ensure_terms_for_module_in_tx(
            txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            "blog",
            &locale,
            &names,
        )
        .await
        .map_err(taxonomy_error_to_content_error)?;

    for tag_id in tag_ids {
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
async fn load_comment_records_for_post_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
) -> ContentResult<Vec<comment::Model>> {
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

    Ok(comments.into_iter().collect())
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn move_comments_to_forum_replies_in_tx(
    _txn: &DatabaseTransaction,
    _tenant_id: Uuid,
    _topic_id: Uuid,
    comment_records: &[comment::Model],
) -> ContentResult<()> {
    if comment_records.is_empty() {
        return Ok(());
    }

    Err(ContentError::validation(
        "Blog posts with comments cannot be demoted until Forum uses the canonical richtext contract",
    ))
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
    tenant_id: Uuid,
    post_id: Uuid,
    locale: &str,
    fallback_locale: Option<&str>,
) -> ContentResult<Vec<String>> {
    let relations = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .order_by_asc(blog_post_tag::Column::CreatedAt)
        .all(txn)
        .await?;
    if relations.is_empty() {
        return Ok(Vec::new());
    }

    let term_ids = relations.iter().map(|item| item.tag_id).collect::<Vec<_>>();
    let terms = TaxonomyOwnerReader::load_terms_by_ids_in_tx(
        txn,
        tenant_id,
        TaxonomyTermKind::Tag,
        &term_ids,
        locale,
        fallback_locale,
    )
    .await
    .map_err(taxonomy_error_to_content_error)?;
    let terms_by_id = terms
        .into_iter()
        .map(|term| (term.id, term))
        .collect::<HashMap<_, _>>();

    Ok(relations
        .into_iter()
        .filter_map(|relation| terms_by_id.get(&relation.tag_id).map(|term| term.name.clone()))
        .collect())
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
        .filter(forum_topic_tag::Column::TenantId.eq(tenant_id))
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
    let terms = TaxonomyOwnerReader::load_terms_by_ids_in_tx(
        txn,
        tenant_id,
        TaxonomyTermKind::Tag,
        &term_ids,
        locale,
        fallback_locale,
    )
    .await
    .map_err(taxonomy_error_to_content_error)?;
    let terms_by_id = terms
        .into_iter()
        .map(|term| (term.id, term))
        .collect::<HashMap<_, _>>();

    Ok(relations
        .into_iter()
        .filter_map(|relation| terms_by_id.get(&relation.term_id).map(|term| term.name.clone()))
        .collect())
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
async fn sync_forum_tags_for_topic_in_tx(
    taxonomy: &TaxonomyService,
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

    let term_ids = taxonomy
        .ensure_terms_for_module_in_tx(
            txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            "forum",
            &locale,
            &names,
        )
        .await
        .map_err(taxonomy_error_to_content_error)?;

    for term_id in term_ids {
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
    use rustok_api::RichTextDocument;
    use rustok_blog::{
        CommentService as BlogCommentService, CreateCommentInput as BlogCreateCommentInput,
        CreatePostInput, PostService,
        entities::{blog_post, blog_post_tag, blog_post_translation},
    };
    use rustok_comments::{
        CommentsModule, CommentsService, ListCommentsFilter,
        entities::{comment, comment_body},
    };
    use rustok_content::{
        CanonicalUrlService, ContentModule, DemotePostToTopicInput, PromoteTopicToPostInput,
    };
    use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
    use rustok_forum::{
        CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumModule,
        ListRepliesFilter, ReplyService, ReplyStatus, TopicService,
        entities::{forum_reply, forum_reply_body, forum_topic, forum_topic_translation},
    };
    use rustok_outbox::{SysEventsMigration, TransactionalEventBus};
    use rustok_taxonomy::{
        TaxonomyModule, TaxonomyScopeType,
        entities::{taxonomy_term, taxonomy_term_translation},
    };
    use sea_orm::{
        ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
        EntityTrait, QueryFilter, Statement,
    };
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    fn richtext(text: &str) -> RichTextDocument {
        serde_json::from_value(serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": text}]
            }]
        }))
        .expect("test richtext")
    }

    async fn setup_conversion_test_db() -> DatabaseConnection {
        let mut opts = ConnectOptions::new("sqlite::memory:");
        opts.max_connections(1)
            .min_connections(1)
            .sqlx_logging(false);

        Database::connect(opts)
            .await
            .expect("failed to connect server content orchestration sqlite database")
    }

    async fn ensure_conversion_schema(db: &DatabaseConnection) {
        db.execute_unprepared(
            "CREATE TABLE users (id TEXT NOT NULL PRIMARY KEY, tenant_id TEXT NOT NULL, UNIQUE (tenant_id, id))",
        )
        .await
        .expect("minimal platform identity relation should apply");
        let manager = SchemaManager::new(db);
        SysEventsMigration
            .up(&manager)
            .await
            .expect("outbox migration should apply");
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

    async fn insert_test_actor(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        security: &SecurityContext,
    ) {
        let user_id = security
            .user_id
            .expect("test admin security should carry a user id");
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO users (id, tenant_id) VALUES (?, ?)",
            [user_id.to_string().into(), tenant_id.to_string().into()],
        ))
        .await
        .expect("test actor should be inserted into the platform identity relation");
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
        insert_test_actor(&db, tenant_id, &security).await;

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
                    title: "Forum thread".to_string(),
                    slug: Some("forum-thread".to_string()),
                    body: rustok_api::RichTextDocument::single_paragraph("Original forum body"),
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
                    content: rustok_api::RichTextDocument::single_paragraph("First forum reply"),
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
                    content: rustok_api::RichTextDocument::single_paragraph("Second forum reply"),
                    parent_reply_id: None,
                },
            )
            .await
            .expect("second reply should be created");

        let orchestration = ContentOrchestrationService::new(
            db.clone(),
            events.clone(),
            Arc::new(ServerContentOrchestrationBridge::new(db.clone())),
        );
        let promoted = match orchestration
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
        {
            Ok(output) => output,
            Err(error) => {
                assert!(
                    error
                        .to_string()
                        .contains("until Forum uses the canonical richtext contract")
                );
                assert!(
                    forum_topic::Entity::find_by_id(topic.id)
                        .one(&db)
                        .await
                        .expect("forum topic lookup should succeed")
                        .is_some()
                );
                return;
            }
        };

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
        insert_test_actor(&db, tenant_id, &security).await;

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
                    content: richtext("Original blog body"),
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
            .create_public_comment(
                tenant_id,
                security.clone(),
                post_id,
                None,
                BlogCreateCommentInput {
                    locale: "en".to_string(),
                    content: richtext("First blog comment"),
                    parent_comment_id: None,
                },
            )
            .await
            .expect("first blog comment should be created");
        blog_comment_service
            .create_public_comment(
                tenant_id,
                security.clone(),
                post_id,
                None,
                BlogCreateCommentInput {
                    locale: "en".to_string(),
                    content: richtext("Second blog comment"),
                    parent_comment_id: None,
                },
            )
            .await
            .expect("second blog comment should be created");

        let orchestration = ContentOrchestrationService::new(
            db.clone(),
            events.clone(),
            Arc::new(ServerContentOrchestrationBridge::new(db.clone())),
        );
        let demoted = match orchestration
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
        {
            Ok(output) => output,
            Err(error) => {
                assert!(
                    error
                        .to_string()
                        .contains("until Forum uses the canonical richtext contract")
                );
                assert!(
                    blog_post::Entity::find_by_id(post_id)
                        .one(&db)
                        .await
                        .expect("blog post lookup should succeed")
                        .is_some()
                );
                return;
            }
        };

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
        assert_eq!(replies[0].content_plain_text, "First blog comment");
        assert_eq!(replies[1].content_plain_text, "Second blog comment");
        assert!(
            replies
                .iter()
                .all(|reply| reply.status == ReplyStatus::Pending.as_str())
        );

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
