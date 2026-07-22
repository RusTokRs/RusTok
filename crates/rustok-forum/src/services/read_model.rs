use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use uuid::Uuid;

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource};
use rustok_content::{
    available_locales_from, normalize_locale_code, resolve_by_locale_with_fallback,
};
use rustok_core::SecurityContext;

use crate::dto::{
    CategoryCursorPage, CategoryCursorQuery, CategoryReadModel, ReplyCursorPage, ReplyCursorQuery,
    ReplyReadModel, TopicCursorPage, TopicCursorQuery, TopicReadModel, bounded_forum_read_limit,
};
use crate::entities::{
    forum_category, forum_category_translation, forum_reply, forum_reply_body, forum_solution,
    forum_topic, forum_topic_translation,
};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;
use crate::services::subscription::SubscriptionService;
use crate::services::vote::VoteService;

const CATEGORY_CURSOR_VERSION: &str = "c1";
const TOPIC_CURSOR_VERSION: &str = "t1";
const REPLY_CURSOR_VERSION: &str = "r1";

pub struct ForumReadModelService {
    db: DatabaseConnection,
}

impl ForumReadModelService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn list_categories(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: CategoryCursorQuery,
    ) -> ForumResult<CategoryCursorPage> {
        enforce_scope(&security, Resource::ForumCategories, Action::List)?;
        let locale = normalized_locale(query.locale.as_deref())?;
        let fallback_locale = normalized_optional_locale(query.fallback_locale.as_deref())?;
        let limit = bounded_forum_read_limit(query.limit);

        let mut select =
            forum_category::Entity::find().filter(forum_category::Column::TenantId.eq(tenant_id));
        if let Some(cursor) = query.cursor.as_deref() {
            let cursor = decode_category_cursor(cursor)?;
            select = select.filter(
                Condition::any()
                    .add(forum_category::Column::Position.gt(cursor.position))
                    .add(
                        Condition::all()
                            .add(forum_category::Column::Position.eq(cursor.position))
                            .add(forum_category::Column::Id.gt(cursor.id)),
                    ),
            );
        }

        let mut categories = select
            .order_by_asc(forum_category::Column::Position)
            .order_by_asc(forum_category::Column::Id)
            .limit(limit + 1)
            .all(&self.db)
            .await?;
        let has_more = categories.len() > limit as usize;
        categories.truncate(limit as usize);
        let next_cursor = has_more
            .then(|| categories.last().map(encode_category_cursor))
            .flatten();

        let ids = categories.iter().map(|item| item.id).collect::<Vec<_>>();
        let translations = category_translations_by_id(&self.db, tenant_id, &ids).await?;
        let subscriptions = SubscriptionService::new(self.db.clone())
            .category_subscription_flags(tenant_id, &ids, security.user_id)
            .await?;

        let items = categories
            .into_iter()
            .map(|category| {
                let localized = translations.get(&category.id).cloned().unwrap_or_default();
                let resolved = resolve_by_locale_with_fallback(
                    &localized,
                    &locale,
                    fallback_locale.as_deref(),
                    |translation| translation.locale.as_str(),
                );
                CategoryReadModel {
                    id: category.id,
                    parent_id: category.parent_id,
                    position: category.position,
                    requested_locale: locale.clone(),
                    effective_locale: resolved.effective_locale,
                    available_locales: available_locales_from(&localized, |translation| {
                        translation.locale.as_str()
                    }),
                    name: resolved
                        .item
                        .map(|translation| translation.name.clone())
                        .unwrap_or_default(),
                    slug: resolved
                        .item
                        .map(|translation| translation.slug.clone())
                        .unwrap_or_default(),
                    description: resolved
                        .item
                        .and_then(|translation| translation.description.clone()),
                    icon: category.icon,
                    color: category.color,
                    moderated: category.moderated,
                    topic_count: category.topic_count,
                    reply_count: category.reply_count,
                    is_subscribed: subscriptions.get(&category.id).copied().unwrap_or(false),
                }
            })
            .collect();

        Ok(CategoryCursorPage {
            items,
            next_cursor,
            has_more,
        })
    }

    pub async fn list_topics(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: TopicCursorQuery,
    ) -> ForumResult<TopicCursorPage> {
        enforce_scope(&security, Resource::ForumTopics, Action::List)?;
        let locale = normalized_locale(query.locale.as_deref())?;
        let fallback_locale = normalized_optional_locale(query.fallback_locale.as_deref())?;
        let limit = bounded_forum_read_limit(query.limit);

        let mut select =
            forum_topic::Entity::find().filter(forum_topic::Column::TenantId.eq(tenant_id));
        if let Some(category_id) = query.category_id {
            select = select.filter(forum_topic::Column::CategoryId.eq(category_id));
        }
        if let Some(status) = query.status {
            select = select.filter(forum_topic::Column::Status.eq(status));
        }
        if let Some(cursor) = query.cursor.as_deref() {
            let cursor = decode_topic_cursor(cursor)?;
            select = select.filter(
                Condition::any()
                    .add(forum_topic::Column::UpdatedAt.lt(cursor.updated_at))
                    .add(
                        Condition::all()
                            .add(forum_topic::Column::UpdatedAt.eq(cursor.updated_at))
                            .add(forum_topic::Column::Id.lt(cursor.id)),
                    ),
            );
        }

        let mut topics = select
            .order_by_desc(forum_topic::Column::UpdatedAt)
            .order_by_desc(forum_topic::Column::Id)
            .limit(limit + 1)
            .all(&self.db)
            .await?;
        let has_more = topics.len() > limit as usize;
        topics.truncate(limit as usize);
        let next_cursor = has_more
            .then(|| topics.last().map(encode_topic_cursor))
            .flatten();

        let ids = topics.iter().map(|item| item.id).collect::<Vec<_>>();
        let translations = topic_translations_by_id(&self.db, tenant_id, &ids).await?;
        let votes = VoteService::new(self.db.clone())
            .topic_vote_summaries(tenant_id, &ids, security.user_id)
            .await?;
        let subscriptions = SubscriptionService::new(self.db.clone())
            .topic_subscription_flags(tenant_id, &ids, security.user_id)
            .await?;
        let solutions = solution_ids_by_topic(&self.db, tenant_id, &ids).await?;

        let items = topics
            .into_iter()
            .map(|topic| {
                let localized = translations.get(&topic.id).cloned().unwrap_or_default();
                let resolved = resolve_by_locale_with_fallback(
                    &localized,
                    &locale,
                    fallback_locale.as_deref(),
                    |translation| translation.locale.as_str(),
                );
                let vote = votes.get(&topic.id).copied().unwrap_or_default();
                TopicReadModel {
                    id: topic.id,
                    category_id: topic.category_id,
                    author_id: topic.author_id,
                    requested_locale: locale.clone(),
                    effective_locale: resolved.effective_locale,
                    available_locales: available_locales_from(&localized, |translation| {
                        translation.locale.as_str()
                    }),
                    title: resolved
                        .item
                        .map(|translation| translation.title.clone())
                        .unwrap_or_default(),
                    slug: resolved
                        .item
                        .and_then(|translation| translation.slug.clone())
                        .unwrap_or_default(),
                    metadata: topic.metadata,
                    status: topic.status.to_string(),
                    is_pinned: topic.is_pinned,
                    is_locked: topic.is_locked,
                    reply_count: topic.reply_count,
                    vote_score: vote.score,
                    current_user_vote: vote.current_user_vote,
                    is_subscribed: subscriptions.get(&topic.id).copied().unwrap_or(false),
                    solution_reply_id: solutions.get(&topic.id).copied(),
                    created_at: topic.created_at.to_rfc3339(),
                    updated_at: topic.updated_at.to_rfc3339(),
                }
            })
            .collect();

        Ok(TopicCursorPage {
            items,
            next_cursor,
            has_more,
        })
    }

    pub async fn list_replies(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        query: ReplyCursorQuery,
    ) -> ForumResult<ReplyCursorPage> {
        enforce_scope(&security, Resource::ForumReplies, Action::List)?;
        let locale = normalized_locale(query.locale.as_deref())?;
        let fallback_locale = normalized_optional_locale(query.fallback_locale.as_deref())?;
        let limit = bounded_forum_read_limit(query.limit);

        let mut select = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::TopicId.eq(topic_id));
        if let Some(cursor) = query.cursor.as_deref() {
            let cursor = decode_reply_cursor(cursor)?;
            select = select.filter(
                Condition::any()
                    .add(forum_reply::Column::Position.gt(cursor.position))
                    .add(
                        Condition::all()
                            .add(forum_reply::Column::Position.eq(cursor.position))
                            .add(forum_reply::Column::Id.gt(cursor.id)),
                    ),
            );
        }

        let mut replies = select
            .order_by_asc(forum_reply::Column::Position)
            .order_by_asc(forum_reply::Column::Id)
            .limit(limit + 1)
            .all(&self.db)
            .await?;
        let has_more = replies.len() > limit as usize;
        replies.truncate(limit as usize);
        let next_cursor = has_more
            .then(|| replies.last().map(encode_reply_cursor))
            .flatten();

        let ids = replies.iter().map(|item| item.id).collect::<Vec<_>>();
        let bodies = reply_bodies_by_id(&self.db, tenant_id, &ids).await?;
        let votes = VoteService::new(self.db.clone())
            .reply_vote_summaries(tenant_id, &ids, security.user_id)
            .await?;
        let solution_reply_id = forum_solution::Entity::find()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(topic_id))
            .one(&self.db)
            .await?
            .map(|solution| solution.reply_id);

        let items = replies
            .into_iter()
            .map(|reply| {
                let localized = bodies.get(&reply.id).cloned().unwrap_or_default();
                let resolved = resolve_by_locale_with_fallback(
                    &localized,
                    &locale,
                    fallback_locale.as_deref(),
                    |body| body.locale.as_str(),
                );
                let content = resolved
                    .item
                    .map(|body| body.body.clone())
                    .unwrap_or_default();
                let vote = votes.get(&reply.id).copied().unwrap_or_default();
                ReplyReadModel {
                    id: reply.id,
                    topic_id: reply.topic_id,
                    author_id: reply.author_id,
                    parent_reply_id: reply.parent_reply_id,
                    position: reply.position,
                    requested_locale: locale.clone(),
                    effective_locale: resolved.effective_locale,
                    available_locales: available_locales_from(&localized, |body| {
                        body.locale.as_str()
                    }),
                    content_preview: content.chars().take(200).collect(),
                    status: reply.status.to_string(),
                    vote_score: vote.score,
                    current_user_vote: vote.current_user_vote,
                    is_solution: Some(reply.id) == solution_reply_id,
                    created_at: reply.created_at.to_rfc3339(),
                    updated_at: reply.updated_at.to_rfc3339(),
                }
            })
            .collect();

        Ok(ReplyCursorPage {
            items,
            next_cursor,
            has_more,
        })
    }
}

#[derive(Clone, Copy)]
struct CategoryCursor {
    position: i32,
    id: Uuid,
}

#[derive(Clone)]
struct TopicCursor {
    updated_at: sea_orm::prelude::DateTimeWithTimeZone,
    id: Uuid,
}

#[derive(Clone, Copy)]
struct ReplyCursor {
    position: i64,
    id: Uuid,
}

fn encode_category_cursor(category: &forum_category::Model) -> String {
    format!(
        "{CATEGORY_CURSOR_VERSION}:{}:{}",
        category.position, category.id
    )
}

fn decode_category_cursor(value: &str) -> ForumResult<CategoryCursor> {
    let mut parts = value.splitn(3, ':');
    if parts.next() != Some(CATEGORY_CURSOR_VERSION) {
        return Err(invalid_cursor("category"));
    }
    let position = parts
        .next()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| invalid_cursor("category"))?;
    let id = parts
        .next()
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| invalid_cursor("category"))?;
    Ok(CategoryCursor { position, id })
}

fn encode_topic_cursor(topic: &forum_topic::Model) -> String {
    format!(
        "{TOPIC_CURSOR_VERSION}:{}:{}",
        topic.updated_at.timestamp_millis(),
        topic.id
    )
}

fn decode_topic_cursor(value: &str) -> ForumResult<TopicCursor> {
    let mut parts = value.splitn(3, ':');
    if parts.next() != Some(TOPIC_CURSOR_VERSION) {
        return Err(invalid_cursor("topic"));
    }
    let millis = parts
        .next()
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or_else(|| invalid_cursor("topic"))?;
    let updated_at: DateTime<Utc> =
        DateTime::<Utc>::from_timestamp_millis(millis).ok_or_else(|| invalid_cursor("topic"))?;
    let id = parts
        .next()
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| invalid_cursor("topic"))?;
    Ok(TopicCursor {
        updated_at: updated_at.fixed_offset(),
        id,
    })
}

fn encode_reply_cursor(reply: &forum_reply::Model) -> String {
    format!("{REPLY_CURSOR_VERSION}:{}:{}", reply.position, reply.id)
}

fn decode_reply_cursor(value: &str) -> ForumResult<ReplyCursor> {
    let mut parts = value.splitn(3, ':');
    if parts.next() != Some(REPLY_CURSOR_VERSION) {
        return Err(invalid_cursor("reply"));
    }
    let position = parts
        .next()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| invalid_cursor("reply"))?;
    let id = parts
        .next()
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| invalid_cursor("reply"))?;
    Ok(ReplyCursor { position, id })
}

fn invalid_cursor(kind: &str) -> ForumError {
    ForumError::Validation(format!("Invalid {kind} cursor"))
}

fn normalized_locale(locale: Option<&str>) -> ForumResult<String> {
    normalize_locale_code(locale.unwrap_or(PLATFORM_FALLBACK_LOCALE))
        .ok_or_else(|| ForumError::Validation("Invalid locale".to_string()))
}

fn normalized_optional_locale(locale: Option<&str>) -> ForumResult<Option<String>> {
    locale
        .map(|value| {
            normalize_locale_code(value)
                .ok_or_else(|| ForumError::Validation("Invalid fallback locale".to_string()))
        })
        .transpose()
}

async fn category_translations_by_id(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    ids: &[Uuid],
) -> ForumResult<HashMap<Uuid, Vec<forum_category_translation::Model>>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = forum_category_translation::Entity::find()
        .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
        .filter(forum_category_translation::Column::CategoryId.is_in(ids.to_vec()))
        .all(db)
        .await?;
    Ok(group_by(rows, |row| row.category_id))
}

async fn topic_translations_by_id(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    ids: &[Uuid],
) -> ForumResult<HashMap<Uuid, Vec<forum_topic_translation::Model>>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = forum_topic_translation::Entity::find()
        .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_translation::Column::TopicId.is_in(ids.to_vec()))
        .all(db)
        .await?;
    Ok(group_by(rows, |row| row.topic_id))
}

async fn reply_bodies_by_id(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    ids: &[Uuid],
) -> ForumResult<HashMap<Uuid, Vec<forum_reply_body::Model>>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = forum_reply_body::Entity::find()
        .filter(forum_reply_body::Column::TenantId.eq(tenant_id))
        .filter(forum_reply_body::Column::ReplyId.is_in(ids.to_vec()))
        .all(db)
        .await?;
    Ok(group_by(rows, |row| row.reply_id))
}

async fn solution_ids_by_topic(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    ids: &[Uuid],
) -> ForumResult<HashMap<Uuid, Uuid>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(forum_solution::Entity::find()
        .filter(forum_solution::Column::TenantId.eq(tenant_id))
        .filter(forum_solution::Column::TopicId.is_in(ids.to_vec()))
        .all(db)
        .await?
        .into_iter()
        .map(|solution| (solution.topic_id, solution.reply_id))
        .collect())
}

fn group_by<T, K>(rows: Vec<T>, key: impl Fn(&T) -> K) -> HashMap<K, Vec<T>>
where
    K: std::hash::Hash + Eq,
{
    let mut grouped = HashMap::new();
    for row in rows {
        grouped.entry(key(&row)).or_insert_with(Vec::new).push(row);
    }
    grouped
}
