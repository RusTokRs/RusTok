use std::collections::BTreeSet;

use chrono::Utc;
use rustok_blog::blog_post_translation;
use rustok_content::{
    ContentError, ContentResult, normalize_locale_code, resolve_by_locale_with_fallback,
};
use rustok_forum::{forum_category, forum_reply, forum_topic, forum_topic_translation};
use rustok_taxonomy::TaxonomyError;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseTransaction, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder,
};
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct ForumReplyRecord {
    pub(crate) reply: forum_reply::Model,
}

pub(crate) fn normalize_locale(locale: &str) -> ContentResult<String> {
    normalize_locale_code(locale).ok_or_else(|| ContentError::validation("Locale cannot be empty"))
}

pub(crate) fn taxonomy_error_to_content_error(error: TaxonomyError) -> ContentError {
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
        | TaxonomyError::Internal(_)
        | TaxonomyError::Validation(_) => ContentError::validation(message),
    }
}

pub(crate) fn normalize_slug(value: &str) -> String {
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

pub(crate) fn blog_post_route(slug: &str) -> String {
    format!("/modules/blog?slug={slug}")
}

pub(crate) fn forum_topic_route(topic_id: Uuid) -> String {
    format!("/modules/forum?topic={topic_id}")
}

pub(crate) fn locales_from_topic_translations(
    translations: &[forum_topic_translation::Model],
) -> ContentResult<Vec<String>> {
    locales_from_strs(
        translations
            .iter()
            .map(|translation| translation.locale.as_str()),
    )
}

pub(crate) fn locales_from_post_translations(
    translations: &[blog_post_translation::Model],
) -> ContentResult<Vec<String>> {
    locales_from_strs(
        translations
            .iter()
            .map(|translation| translation.locale.as_str()),
    )
}

pub(crate) fn locales_from_strs<'a>(
    locales: impl IntoIterator<Item = &'a str>,
) -> ContentResult<Vec<String>> {
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

pub(crate) async fn find_topic_in_tx(
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

pub(crate) async fn load_topic_translations_in_tx(
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

pub(crate) fn resolve_topic_translation<'a>(
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

pub(crate) async fn ensure_forum_category_exists_in_tx(
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

pub(crate) async fn load_forum_reply_records_in_tx(
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

pub(crate) async fn resequence_forum_topic_replies_in_tx(
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

pub(crate) async fn refresh_forum_topic_stats_in_tx(
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

pub(crate) async fn adjust_forum_category_counters_in_tx(
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

pub(crate) async fn next_forum_reply_position_in_tx(
    txn: &DatabaseTransaction,
    topic_id: Uuid,
) -> ContentResult<i64> {
    let count = forum_reply::Entity::find()
        .filter(forum_reply::Column::TopicId.eq(topic_id))
        .count(txn)
        .await?;
    Ok(count as i64 + 1)
}
