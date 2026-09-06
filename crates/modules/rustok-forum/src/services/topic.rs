use std::collections::{HashMap, HashSet};

struct TopicTranslationUpsertInput {
    title: Option<String>,
    body: Option<rustok_api::RichTextDocument>,
}

struct TopicResponseParts {
    channel_slugs: Vec<String>,
    tags: Vec<String>,
    metadata: Value,
    vote_summary: VoteSummary,
    is_subscribed: bool,
    solution_reply_id: Option<Uuid>,
}

use chrono::Utc;
use flex::{
    persist_localized_values, prepare_attached_values_create, prepare_attached_values_update,
    resolve_attached_payload,
};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, Condition, DatabaseConnection, DatabaseTransaction, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Select, TransactionTrait,
    sea_query::{Expr, Query, SelectStatement},
};
use serde_json::Value;
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource, RichTextDocument};
use rustok_content::{
    available_locales_from, normalize_locale_code, resolve_by_locale_with_fallback,
};
use rustok_core::SecurityContext;
use rustok_core::field_schema::{CustomFieldsSchema, FieldDefinition, FieldType, ValidationRule};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::{TaxonomyService, TaxonomyTermKind};

use crate::dto::{ListTopicsFilter, TopicListItem, TopicResponse, UpdateTopicInput};
use crate::entities::{
    forum_solution, forum_topic, forum_topic_channel_access, forum_topic_tag,
    forum_topic_translation,
};
use crate::error::{ForumError, ForumResult};
use crate::richtext::{normalize_discussion, project_stored_discussion, serialize_discussion};
use crate::services::category::CategoryService;
use crate::services::rbac::{enforce_owned_scope, enforce_scope};
use crate::services::subscription::SubscriptionService;
use crate::services::user_stats::UserStatsService;
use crate::services::vote::{VoteService, VoteSummary};
use crate::state_machine::TopicStatus;

mod topic_field_definitions_storage {
    rustok_core::define_field_definitions_entity!("topic_field_definitions");
}

pub struct TopicService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl TopicService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }


    #[instrument(skip(self))]
    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        locale: &str,
    ) -> ForumResult<TopicResponse> {
        self.get_with_locale_fallback(tenant_id, security, topic_id, locale, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<TopicResponse> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let topic = self.find_topic(tenant_id, topic_id).await?;
        let translations = self.load_translations(tenant_id, topic_id).await?;
        let channel_slugs = self.load_channel_slugs(tenant_id, topic_id).await?;
        let metadata = self
            .resolve_topic_metadata(
                tenant_id,
                topic.id,
                &topic.metadata,
                &locale,
                fallback_locale.as_deref(),
            )
            .await?;
        let tags = self
            .load_topic_tags(tenant_id, topic.id, &locale, fallback_locale.as_deref())
            .await?;
        let solution_reply_id = self.load_solution_reply_id(tenant_id, topic_id).await?;
        let vote_summary = VoteService::new(self.db.clone())
            .topic_vote_summary(tenant_id, topic_id, security.user_id)
            .await?;
        let is_subscribed = SubscriptionService::new(self.db.clone())
            .topic_subscription_flags(tenant_id, &[topic_id], security.user_id)
            .await?
            .get(&topic_id)
            .copied()
            .unwrap_or(false);
        to_topic_response(
            topic,
            translations,
            TopicResponseParts {
                channel_slugs,
                tags,
                metadata,
                vote_summary,
                is_subscribed,
                solution_reply_id,
            },
            &locale,
            fallback_locale.as_deref(),
        )
    }
    pub(crate) async fn find_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ForumResult<forum_topic::Model> {
        forum_topic::Entity::find_by_id(topic_id)
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ForumError::TopicNotFound(topic_id))
    }

    pub(crate) async fn find_topic_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ForumResult<forum_topic::Model> {
        forum_topic::Entity::find_by_id(topic_id)
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?
            .ok_or(ForumError::TopicNotFound(topic_id))
    }

    pub(crate) async fn adjust_reply_count_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        delta: i32,
    ) -> ForumResult<forum_topic::Model> {
        let topic = Self::find_topic_in_tx(txn, tenant_id, topic_id).await?;
        let mut active: forum_topic::ActiveModel = topic.clone().into();
        active.reply_count = Set((topic.reply_count + delta).max(0));
        active.last_reply_at = Set(Some(Utc::now().into()));
        active.updated_at = Set(Utc::now().into());
        active.update(txn).await?;
        Ok(topic)
    }

    pub(crate) async fn set_pinned_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        is_pinned: bool,
    ) -> ForumResult<()> {
        let topic = Self::find_topic_in_tx(txn, tenant_id, topic_id).await?;
        let mut active: forum_topic::ActiveModel = topic.into();
        active.is_pinned = Set(is_pinned);
        active.updated_at = Set(Utc::now().into());
        active.update(txn).await?;
        Ok(())
    }

    pub(crate) async fn set_locked_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        is_locked: bool,
    ) -> ForumResult<()> {
        let topic = Self::find_topic_in_tx(txn, tenant_id, topic_id).await?;
        let mut active: forum_topic::ActiveModel = topic.into();
        active.is_locked = Set(is_locked);
        active.updated_at = Set(Utc::now().into());
        active.update(txn).await?;
        Ok(())
    }

    pub(crate) async fn set_status_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        status: TopicStatus,
    ) -> ForumResult<()> {
        let topic = Self::find_topic_in_tx(txn, tenant_id, topic_id).await?;
        let mut active: forum_topic::ActiveModel = topic.into();
        active.status = Set(status);
        active.updated_at = Set(Utc::now().into());
        active.update(txn).await?;
        Ok(())
    }

    async fn load_translations(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ForumResult<Vec<forum_topic_translation::Model>> {
        Ok(forum_topic_translation::Entity::find()
            .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_translation::Column::TopicId.eq(topic_id))
            .all(&self.db)
            .await?)
    }

    async fn load_translations_for_topics(
        &self,
        tenant_id: Uuid,
        topic_ids: &[Uuid],
    ) -> ForumResult<Vec<forum_topic_translation::Model>> {
        if topic_ids.is_empty() {
            return Ok(Vec::new());
        }
        Ok(forum_topic_translation::Entity::find()
            .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_translation::Column::TopicId.is_in(topic_ids.to_vec()))
            .all(&self.db)
            .await?)
    }

    async fn load_translations_map_for_topics(
        &self,
        tenant_id: Uuid,
        topic_ids: &[Uuid],
    ) -> ForumResult<HashMap<Uuid, Vec<forum_topic_translation::Model>>> {
        let mut map: HashMap<Uuid, Vec<forum_topic_translation::Model>> = HashMap::new();
        for translation in self
            .load_translations_for_topics(tenant_id, topic_ids)
            .await?
        {
            map.entry(translation.topic_id)
                .or_default()
                .push(translation);
        }
        Ok(map)
    }

    async fn load_channel_slugs(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ForumResult<Vec<String>> {
        Ok(forum_topic_channel_access::Entity::find()
            .filter(forum_topic_channel_access::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_channel_access::Column::TopicId.eq(topic_id))
            .order_by_asc(forum_topic_channel_access::Column::ChannelSlug)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|item| item.channel_slug)
            .collect())
    }

    async fn load_solution_reply_id(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ForumResult<Option<Uuid>> {
        Ok(forum_solution::Entity::find()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(topic_id))
            .one(&self.db)
            .await?
            .map(|solution| solution.reply_id))
    }

    async fn load_topic_tags(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<Vec<String>> {
        let term_ids = forum_topic_tag::Entity::find()
            .filter(forum_topic_tag::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_tag::Column::TopicId.eq(topic_id))
            .order_by_asc(forum_topic_tag::Column::CreatedAt)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|row| row.term_id)
            .collect::<Vec<_>>();

        if term_ids.is_empty() {
            return Ok(Vec::new());
        }

        let resolved_names = TaxonomyService::new(self.db.clone())
            .resolve_term_names(tenant_id, &term_ids, locale, fallback_locale)
            .await?;
        let mut tags = term_ids
            .into_iter()
            .filter_map(|term_id| resolved_names.get(&term_id).cloned())
            .collect::<Vec<_>>();
        tags.sort();
        tags.dedup();
        Ok(tags)
    }

    async fn load_solution_reply_ids_map(
        &self,
        tenant_id: Uuid,
        topic_ids: &[Uuid],
    ) -> ForumResult<HashMap<Uuid, Uuid>> {
        if topic_ids.is_empty() {
            return Ok(HashMap::new());
        }

        Ok(forum_solution::Entity::find()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.is_in(topic_ids.to_vec()))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|solution| (solution.topic_id, solution.reply_id))
            .collect())
    }

    async fn load_channel_slugs_map(
        &self,
        tenant_id: Uuid,
        topic_ids: &[Uuid],
    ) -> ForumResult<HashMap<Uuid, Vec<String>>> {
        if topic_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = forum_topic_channel_access::Entity::find()
            .filter(forum_topic_channel_access::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_channel_access::Column::TopicId.is_in(topic_ids.to_vec()))
            .all(&self.db)
            .await?;
        let mut map: HashMap<Uuid, Vec<String>> = HashMap::new();
        for row in rows {
            map.entry(row.topic_id).or_default().push(row.channel_slug);
        }
        for values in map.values_mut() {
            values.sort();
        }
        Ok(map)
    }

    async fn sync_channel_access_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        channel_slugs: Option<&[String]>,
    ) -> ForumResult<()> {
        forum_topic_channel_access::Entity::delete_many()
            .filter(forum_topic_channel_access::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_channel_access::Column::TopicId.eq(topic_id))
            .exec(txn)
            .await?;

        for channel_slug in normalize_channel_slugs(channel_slugs.unwrap_or(&[])) {
            forum_topic_channel_access::ActiveModel {
                tenant_id: Set(tenant_id),
                topic_id: Set(topic_id),
                channel_slug: Set(channel_slug),
            }
            .insert(txn)
            .await?;
        }

        Ok(())
    }

    async fn sync_topic_tags_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: &str,
        tags: &[String],
    ) -> ForumResult<()> {
        forum_topic_tag::Entity::delete_many()
            .filter(forum_topic_tag::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_tag::Column::TopicId.eq(topic_id))
            .exec(txn)
            .await?;

        if tags.is_empty() {
            return Ok(());
        }

        let taxonomy_service = TaxonomyService::new(self.db.clone());
        let term_ids = taxonomy_service
            .ensure_terms_for_module_in_tx(
                txn,
                tenant_id,
                TaxonomyTermKind::Tag,
                "forum",
                locale,
                tags,
            )
            .await?;
        let now = Utc::now();

        for term_id in term_ids {
            forum_topic_tag::ActiveModel {
                id: Set(Uuid::new_v4()),
                topic_id: Set(topic_id),
                term_id: Set(term_id),
                tenant_id: Set(tenant_id),
                created_at: Set(now.into()),
            }
            .insert(txn)
            .await?;
        }

        Ok(())
    }

    async fn hydrate_topic_list_items(
        &self,
        tenant_id: Uuid,
        viewer_user_id: Option<Uuid>,
        topics: Vec<forum_topic::Model>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<Vec<TopicListItem>> {
        if topics.is_empty() {
            return Ok(Vec::new());
        }

        let topic_ids: Vec<Uuid> = topics.iter().map(|topic| topic.id).collect();
        let translations_by_topic_id = self
            .load_translations_map_for_topics(tenant_id, &topic_ids)
            .await?;
        let channels = self.load_channel_slugs_map(tenant_id, &topic_ids).await?;
        let solution_reply_ids = self
            .load_solution_reply_ids_map(tenant_id, &topic_ids)
            .await?;
        let schema = load_topic_custom_fields_schema(&self.db, tenant_id).await?;
        let vote_summaries = VoteService::new(self.db.clone())
            .topic_vote_summaries(tenant_id, &topic_ids, viewer_user_id)
            .await?;
        let subscription_flags = SubscriptionService::new(self.db.clone())
            .topic_subscription_flags(tenant_id, &topic_ids, viewer_user_id)
            .await?;

        let mut items = Vec::with_capacity(topics.len());
        for topic in topics {
            let localized = translations_by_topic_id
                .get(&topic.id)
                .cloned()
                .unwrap_or_default();
            let resolved = resolve_by_locale_with_fallback(
                &localized,
                locale,
                fallback_locale,
                |translation| translation.locale.as_str(),
            );
            let metadata = self
                .resolve_topic_metadata_with_schema(
                    tenant_id,
                    topic.id,
                    &topic.metadata,
                    locale,
                    fallback_locale,
                    &schema,
                )
                .await?;

            items.push(TopicListItem {
                id: topic.id,
                requested_locale: locale.to_string(),
                locale: locale.to_string(),
                effective_locale: resolved.effective_locale,
                available_locales: available_locales_from(&localized, |translation| {
                    translation.locale.as_str()
                }),
                category_id: topic.category_id,
                author_id: topic.author_id,
                title: resolved
                    .item
                    .map(|translation| translation.title.clone())
                    .unwrap_or_default(),
                slug: resolved
                    .item
                    .and_then(|translation| translation.slug.clone())
                    .unwrap_or_default(),
                metadata,
                status: topic.status.to_string(),
                channel_slugs: channels.get(&topic.id).cloned().unwrap_or_default(),
                vote_score: vote_summaries
                    .get(&topic.id)
                    .map(|summary| summary.score)
                    .unwrap_or_default(),
                current_user_vote: vote_summaries
                    .get(&topic.id)
                    .and_then(|summary| summary.current_user_vote),
                is_subscribed: subscription_flags.get(&topic.id).copied().unwrap_or(false),
                solution_reply_id: solution_reply_ids.get(&topic.id).copied(),
                is_pinned: topic.is_pinned,
                is_locked: topic.is_locked,
                reply_count: topic.reply_count,
                created_at: topic.created_at.to_rfc3339(),
            });
        }

        Ok(items)
    }

    async fn upsert_translation_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: &str,
        input: TopicTranslationUpsertInput,
    ) -> ForumResult<()> {
        let TopicTranslationUpsertInput { title, body } = input;
        let existing = forum_topic_translation::Entity::find()
            .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_translation::Column::TopicId.eq(topic_id))
            .filter(forum_topic_translation::Column::Locale.eq(locale))
            .one(txn)
            .await?;
        let now = Utc::now();

        match existing {
            Some(existing) => {
                let mut active: forum_topic_translation::ActiveModel = existing.into();
                if let Some(title) = title {
                    validate_topic_title(&title)?;
                    active.title = Set(title);
                }
                if let Some(body) = body {
                    active.body = Set(serialize_discussion(body)?);
                }
                active.updated_at = Set(now.into());
                active.update(txn).await?;
            }
            None => {
                let seed = forum_topic_translation::Entity::find()
                    .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
                    .filter(forum_topic_translation::Column::TopicId.eq(topic_id))
                    .order_by_asc(forum_topic_translation::Column::CreatedAt)
                    .one(txn)
                    .await?;
                let title = title
                    .or_else(|| seed.as_ref().map(|translation| translation.title.clone()))
                    .ok_or_else(|| {
                        ForumError::Validation("Title is required for a new locale".to_string())
                    })?;
                validate_topic_title(&title)?;
                let stored_body = match body {
                    Some(body) => serialize_discussion(body)?,
                    None => seed
                        .as_ref()
                        .map(|translation| translation.body.clone())
                        .ok_or_else(|| {
                            ForumError::Validation("Body is required for a new locale".to_string())
                        })?,
                };

                forum_topic_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    topic_id: Set(topic_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(locale.to_string()),
                    title: Set(title),
                    slug: Set(seed.and_then(|translation| translation.slug)),
                    body: Set(stored_body),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await?;
            }
        }

        Ok(())
    }

    async fn prepare_topic_custom_fields_for_create(
        &self,
        tenant_id: Uuid,
        locale: &str,
        payload: Value,
    ) -> ForumResult<flex::PreparedAttachedValuesWrite> {
        let schema = load_topic_custom_fields_schema(&self.db, tenant_id).await?;
        let (reserved_payload, flex_payload) = split_topic_metadata_payload(&schema, &payload);
        prepare_attached_values_create(schema, Some(Value::Object(flex_payload)), locale)
            .map(|mut prepared| {
                prepared.metadata = Some(merge_reserved_topic_metadata(
                    reserved_payload,
                    prepared.metadata,
                ));
                prepared
            })
            .map_err(|error| ForumError::Validation(error.to_string()))
    }

    async fn prepare_topic_custom_fields_for_update(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: &str,
        existing_metadata: &Value,
        payload: Value,
    ) -> ForumResult<flex::PreparedAttachedValuesWrite> {
        let schema = load_topic_custom_fields_schema(&self.db, tenant_id).await?;
        let (reserved_payload, flex_payload) = split_topic_metadata_payload(&schema, &payload);
        let (_, existing_flex_metadata) = split_topic_metadata_payload(&schema, existing_metadata);
        prepare_attached_values_update(
            &self.db,
            flex::AttachedEntityRef {
                tenant_id,
                entity_type: "topic",
                entity_id: topic_id,
            },
            schema,
            locale,
            &Value::Object(existing_flex_metadata),
            Some(Value::Object(flex_payload)),
        )
        .await
        .map(|mut prepared| {
            prepared.metadata = Some(merge_reserved_topic_metadata(
                reserved_payload,
                prepared.metadata,
            ));
            prepared
        })
        .map_err(|error| ForumError::Validation(error.to_string()))
    }

    async fn resolve_topic_metadata(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        metadata: &Value,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<Value> {
        let schema = load_topic_custom_fields_schema(&self.db, tenant_id).await?;
        self.resolve_topic_metadata_with_schema(
            tenant_id,
            topic_id,
            metadata,
            locale,
            fallback_locale,
            &schema,
        )
        .await
    }

    async fn resolve_topic_metadata_with_schema(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        metadata: &Value,
        locale: &str,
        fallback_locale: Option<&str>,
        schema: &CustomFieldsSchema,
    ) -> ForumResult<Value> {
        let schema = CustomFieldsSchema::new(
            schema
                .active_definitions()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>(),
        );
        resolve_attached_payload(
            &self.db,
            flex::AttachedEntityRef {
                tenant_id,
                entity_type: "topic",
                entity_id: topic_id,
            },
            schema,
            metadata,
            locale,
            fallback_locale.unwrap_or(PLATFORM_FALLBACK_LOCALE),
        )
        .await
        .map(|payload| payload.unwrap_or_else(|| serde_json::json!({})))
        .map_err(|error| ForumError::Validation(error.to_string()))
    }
}

fn to_topic_response(
    topic: forum_topic::Model,
    translations: Vec<forum_topic_translation::Model>,
    parts: TopicResponseParts,
    locale: &str,
    fallback_locale: Option<&str>,
) -> ForumResult<TopicResponse> {
    let resolved =
        resolve_by_locale_with_fallback(&translations, locale, fallback_locale, |translation| {
            translation.locale.as_str()
        });
    let body = resolved
        .item
        .ok_or_else(|| ForumError::Validation("Forum topic body is unavailable".to_string()))?;
    let body = project_stored_discussion(&body.body)?;

    Ok(TopicResponse {
        id: topic.id,
        requested_locale: locale.to_string(),
        locale: locale.to_string(),
        effective_locale: resolved.effective_locale,
        available_locales: available_locales_from(&translations, |translation| {
            translation.locale.as_str()
        }),
        category_id: topic.category_id,
        author_id: topic.author_id,
        title: resolved
            .item
            .map(|translation| translation.title.clone())
            .unwrap_or_default(),
        slug: resolved
            .item
            .and_then(|translation| translation.slug.clone())
            .unwrap_or_default(),
        body: body.view,
        body_plain_text: body.plain_text,
        metadata: parts.metadata,
        status: topic.status.to_string(),
        tags: parts.tags,
        channel_slugs: parts.channel_slugs,
        vote_score: parts.vote_summary.score,
        current_user_vote: parts.vote_summary.current_user_vote,
        is_subscribed: parts.is_subscribed,
        solution_reply_id: parts.solution_reply_id,
        is_pinned: topic.is_pinned,
        is_locked: topic.is_locked,
        reply_count: topic.reply_count,
        created_at: topic.created_at.to_rfc3339(),
        updated_at: topic.updated_at.to_rfc3339(),
    })
}

fn validate_topic_title(title: &str) -> ForumResult<()> {
    if title.trim().is_empty() {
        return Err(ForumError::Validation(
            "Topic title cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn normalize_locale(locale: &str) -> ForumResult<String> {
    normalize_locale_code(locale)
        .ok_or_else(|| ForumError::Validation("Invalid locale".to_string()))
}

fn normalize_slug(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_dash = false;
    for ch in value.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            normalized.push('-');
            previous_dash = true;
        }
    }
    normalized.trim_matches('-').to_string()
}

fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut normalized = tags
        .iter()
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn normalize_channel_slugs(channel_slugs: &[String]) -> Vec<String> {
    let mut normalized = channel_slugs
        .iter()
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

async fn load_topic_custom_fields_schema(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> ForumResult<CustomFieldsSchema> {
    let rows = topic_field_definitions_storage::Entity::find()
        .filter(topic_field_definitions_storage::Column::TenantId.eq(tenant_id))
        .filter(topic_field_definitions_storage::Column::IsActive.eq(true))
        .order_by_asc(topic_field_definitions_storage::Column::Position)
        .all(db)
        .await?;

    let definitions = rows
        .into_iter()
        .filter_map(topic_field_definition_from_row)
        .collect();

    Ok(CustomFieldsSchema::new(definitions))
}

fn topic_field_definition_from_row(
    row: topic_field_definitions_storage::Model,
) -> Option<FieldDefinition> {
    let field_type: FieldType =
        serde_json::from_value(serde_json::Value::String(row.field_type.clone())).ok()?;
    let label = serde_json::from_value(row.label).unwrap_or_default();
    let description = row
        .description
        .and_then(|value| serde_json::from_value(value).ok());
    let validation: Option<ValidationRule> = row
        .validation
        .and_then(|value| serde_json::from_value(value).ok());

    Some(FieldDefinition {
        field_key: row.field_key,
        field_type,
        label,
        description,
        is_localized: row.is_localized,
        is_required: row.is_required,
        default_value: row.default_value,
        validation,
        position: row.position,
        is_active: row.is_active,
    })
}

fn split_topic_metadata_payload(
    schema: &CustomFieldsSchema,
    metadata: &Value,
) -> (
    serde_json::Map<String, Value>,
    serde_json::Map<String, Value>,
) {
    let known_keys = schema
        .active_definitions()
        .into_iter()
        .map(|definition| definition.field_key.as_str())
        .collect::<HashSet<_>>();
    let mut reserved = serde_json::Map::new();
    let mut custom_fields = serde_json::Map::new();

    for (key, value) in metadata.as_object().cloned().unwrap_or_default() {
        if known_keys.contains(key.as_str()) {
            custom_fields.insert(key, value);
        } else {
            reserved.insert(key, value);
        }
    }

    (reserved, custom_fields)
}

fn merge_reserved_topic_metadata(
    mut reserved: serde_json::Map<String, Value>,
    custom_fields: Option<Value>,
) -> Value {
    if let Some(custom_fields) = custom_fields.and_then(|value| value.as_object().cloned()) {
        for (key, value) in custom_fields {
            reserved.insert(key, value);
        }
    }

    Value::Object(reserved)
}

impl TopicService {

    async fn prepare_topic_relation_body_for_update(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: &str,
        input: &UpdateTopicInput,
    ) -> ForumResult<Option<RichTextDocument>> {
        if let Some(body) = input.body.clone() {
            return normalize_discussion(body).map(Some);
        }

        let existing = forum_topic_translation::Entity::find()
            .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_translation::Column::TopicId.eq(topic_id))
            .filter(forum_topic_translation::Column::Locale.eq(locale))
            .one(&self.db)
            .await?;
        if existing.is_some() {
            return Ok(None);
        }

        let seed = forum_topic_translation::Entity::find()
            .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_translation::Column::TopicId.eq(topic_id))
            .order_by_asc(forum_topic_translation::Column::CreatedAt)
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                ForumError::Validation("Body is required for a new locale".to_string())
            })?;
        Ok(Some(project_stored_discussion(&seed.body)?.view.document))
    }
}
