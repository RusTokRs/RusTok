use sea_orm::QuerySelect;

pub(crate) struct PreparedImportTopicInsert {
    record: crate::import_write_preparation::ForumPreparedImportTopic,
    document: RichTextDocument,
    stored_body: String,
    normalized_slug: Option<String>,
    normalized_tags: Vec<String>,
    prepared_custom_fields: flex::PreparedAttachedValuesWrite,
    created_at: chrono::DateTime<Utc>,
}

impl PreparedImportTopicInsert {
    pub(crate) fn id(&self) -> Uuid {
        self.record.id
    }

    pub(crate) fn author_id(&self) -> Option<Uuid> {
        self.record.author.as_ref().map(|author| author.user_id)
    }
}

impl TopicService {
    pub(crate) async fn prepare_import_topic(
        &self,
        tenant_id: Uuid,
        record: &crate::import_write_preparation::ForumPreparedImportTopic,
    ) -> ForumResult<PreparedImportTopicInsert> {
        if tenant_id.is_nil() || record.id.is_nil() || record.category_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum import topic preparation requires non-nil tenant, topic and category IDs"
                    .to_string(),
            ));
        }
        validate_topic_title(&record.title)?;
        let locale = normalize_locale(&record.locale)?;
        if locale != record.locale {
            return Err(ForumError::Validation(
                "Forum import topic locale must already be normalized".to_string(),
            ));
        }
        let normalized_tags = normalize_tags(&record.tags);
        validate_normalized_topic_tags(&normalized_tags)?;
        let document = crate::richtext::normalize_discussion(record.body.clone())?;
        let stored_body = crate::richtext::serialize_discussion(document.clone())?;
        let prepared_custom_fields = self
            .prepare_topic_custom_fields_for_create(tenant_id, &locale, record.metadata.clone())
            .await?;
        let normalized_slug = record
            .slug
            .as_ref()
            .map(|value| normalize_slug(value))
            .filter(|value| !value.is_empty());
        let created_at = chrono::DateTime::<Utc>::from_timestamp_millis(record.created_at_ms)
            .ok_or_else(|| {
                ForumError::Validation(
                    "Forum import topic creation timestamp is outside owner range".to_string(),
                )
            })?;

        Ok(PreparedImportTopicInsert {
            record: record.clone(),
            document,
            stored_body,
            normalized_slug,
            normalized_tags,
            prepared_custom_fields,
            created_at,
        })
    }

    pub(crate) async fn insert_import_topic_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        prepared: &PreparedImportTopicInsert,
        relation: &crate::import_relation_preparation::ForumPreparedImportContentRelations,
        relations: &super::mention_relation::MentionRelationService,
        relation_event_mode: crate::import_relation_preparation::ForumImportRelationEventMode,
        write_event_mode: crate::import_write_preparation::ForumImportWriteEventMode,
    ) -> ForumResult<()> {
        if relation.source != prepared.record.source
            || relation.target != crate::mentions::ForumContentTarget::topic(prepared.record.id)
            || relation.locale != prepared.record.locale
        {
            return Err(ForumError::Validation(
                "Forum import topic relation admission does not match prepared topic".to_string(),
            ));
        }

        CategoryService::ensure_exists_in_tx(txn, tenant_id, prepared.record.category_id).await?;
        let author_id = prepared.author_id();

        // Topics are materialized open/unlocked inside the private transaction so
        // owner reply-insert guards can admit historical replies. The exact
        // prepared status/lock is restored by finalize_import_topic_in_tx before
        // the transaction can commit.
        forum_topic::ActiveModel {
            id: Set(prepared.record.id),
            tenant_id: Set(tenant_id),
            category_id: Set(prepared.record.category_id),
            author_id: Set(author_id),
            status: Set(TopicStatus::Open),
            metadata: Set(prepared
                .prepared_custom_fields
                .metadata
                .clone()
                .unwrap_or_else(|| serde_json::json!({}))),
            is_pinned: Set(prepared.record.is_pinned),
            is_locked: Set(false),
            reply_count: Set(0),
            created_at: Set(prepared.created_at.into()),
            updated_at: Set(prepared.created_at.into()),
            last_reply_at: Set(None),
        }
        .insert(txn)
        .await?;

        super::topic_tag_lock::lock_active_topic_tag_write_in_tx(
            txn,
            tenant_id,
            prepared.record.id,
        )
        .await?;
        super::topic_tag_lock::lock_topic_tag_scopes_in_tx(txn, tenant_id, &[prepared.record.id])
            .await?;

        forum_topic_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            topic_id: Set(prepared.record.id),
            tenant_id: Set(tenant_id),
            locale: Set(prepared.record.locale.clone()),
            title: Set(prepared.record.title.clone()),
            slug: Set(prepared.normalized_slug.clone()),
            body: Set(prepared.stored_body.clone()),
            created_at: Set(prepared.created_at.into()),
            updated_at: Set(prepared.created_at.into()),
        }
        .insert(txn)
        .await?;

        relations
            .persist_import_admitted_in_tx(
                txn,
                tenant_id,
                relation,
                &prepared.document,
                author_id,
                relation_event_mode,
            )
            .await?;

        if let (Some(persist_locale), Some(values)) = (
            prepared.prepared_custom_fields.locale.as_deref(),
            prepared.prepared_custom_fields.localized_values.as_ref(),
        ) {
            persist_localized_values(
                txn,
                tenant_id,
                "topic",
                prepared.record.id,
                persist_locale,
                values,
            )
            .await
            .map_err(|error| ForumError::Validation(error.to_string()))?;
        }

        self.sync_channel_access_in_tx(
            txn,
            tenant_id,
            prepared.record.id,
            prepared.record.channel_slugs.as_deref(),
        )
        .await?;
        self.sync_topic_tags_in_tx(
            txn,
            tenant_id,
            prepared.record.id,
            &prepared.record.locale,
            &prepared.normalized_tags,
        )
        .await?;

        CategoryService::adjust_counters_in_tx(txn, tenant_id, prepared.record.category_id, 1, 0)
            .await?;
        UserStatsService::adjust_topic_count_in_tx(txn, tenant_id, author_id, 1).await?;

        if write_event_mode
            == crate::import_write_preparation::ForumImportWriteEventMode::EmitDomainEvents
        {
            self.event_bus
                .publish_in_tx(
                    txn,
                    tenant_id,
                    author_id,
                    DomainEvent::ForumTopicCreated {
                        topic_id: prepared.record.id,
                        category_id: prepared.record.category_id,
                        author_id,
                        locale: prepared.record.locale.clone(),
                    },
                )
                .await?;
        }

        Ok(())
    }

    pub(crate) async fn finalize_import_topic_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        prepared: &PreparedImportTopicInsert,
        approved_reply_count: i32,
        last_reply_at: Option<chrono::DateTime<Utc>>,
    ) -> ForumResult<()> {
        let topic = Self::find_topic_in_tx(txn, tenant_id, prepared.record.id).await?;
        let mut active: forum_topic::ActiveModel = topic.into();
        active.status = Set(prepared.record.status);
        active.is_locked = Set(prepared.record.is_locked);
        active.reply_count = Set(approved_reply_count.max(0));
        active.last_reply_at = Set(last_reply_at.map(Into::into));
        active.updated_at = Set(prepared.created_at.into());
        active.update(txn).await?;

        // The temporary row was inserted as `Open` and counted (+1) so reply
        // guards could admit historical replies. A topic imported straight into
        // `Archived` must not keep that contribution to the public aggregates.
        if prepared.record.status == crate::state_machine::TopicStatus::Archived {
            CategoryService::adjust_counters_in_tx(
                txn,
                tenant_id,
                prepared.record.category_id,
                -1,
                0,
            )
            .await?;
            UserStatsService::adjust_topic_count_in_tx(txn, tenant_id, prepared.author_id(), -1)
                .await?;
        }
        Ok(())
    }
}

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
    is_deleted: bool,
}

use chrono::Utc;
use flex::{
    AttachedPayloadResolutionInput, field_definition_from_source, impl_field_definition_source,
    persist_localized_values, prepare_donor_attached_values_create,
    prepare_donor_attached_values_update, resolve_attached_payload, resolve_attached_payloads,
};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, Condition, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    DatabaseTransaction, EntityTrait, Order, PaginatorTrait, QueryFilter, QueryOrder, Select,
    Statement, TransactionTrait,
    sea_query::{Expr, NullOrdering, Query, SelectStatement},
};
use serde_json::Value;
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource, RichTextDocument};
use rustok_content::{
    available_locales_from, normalize_locale_code, resolve_by_locale_with_fallback,
};
use rustok_core::SecurityContext;
use rustok_core::field_schema::{CustomFieldsSchema, FieldDefinition};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::{TaxonomyService, TaxonomyTermKind};

use crate::dto::{
    ListTopicsFilter, TopicListCursor, TopicListItem, TopicPage, TopicResponse, UpdateTopicInput,
};
use crate::entities::{
    forum_reply, forum_solution, forum_topic, forum_topic_channel_access, forum_topic_tag,
    forum_topic_translation,
};
use crate::error::{ForumError, ForumResult};
use crate::richtext::{
    normalize_discussion, project_discussion, project_stored_discussion, serialize_discussion,
};
use crate::services::category::CategoryService;
use crate::services::content_limits::ForumContentLimits;
use crate::services::engagement_mode::ForumSettingsProviders;
use crate::services::rbac::{enforce_owned_scope, enforce_scope};
use crate::services::subscription::SubscriptionService;
use crate::services::user_stats::UserStatsService;
use crate::services::vote::{VoteService, VoteSummary};
use crate::state_machine::{ReplyStatus, TopicStatus};

mod topic_field_definitions_storage {
    rustok_core::define_field_definitions_entity!("topic_field_definitions");
}
impl_field_definition_source!(topic_field_definitions_storage::Model);

pub struct TopicService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    settings: ForumSettingsProviders,
}

impl TopicService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            db,
            event_bus,
            settings: ForumSettingsProviders::default(),
        }
    }

    pub fn with_settings_providers(mut self, settings: ForumSettingsProviders) -> Self {
        self.settings = settings;
        self
    }

    #[instrument(skip(self, security))]
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

    #[instrument(skip(self, security))]
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
        let is_deleted = self.is_topic_deleted(tenant_id, topic_id).await?;
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
            .with_settings_providers(self.settings.clone())
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
                is_deleted,
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

    pub(crate) async fn find_topic_for_update_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ForumResult<forum_topic::Model> {
        let query = forum_topic::Entity::find_by_id(topic_id)
            .filter(forum_topic::Column::TenantId.eq(tenant_id));
        let statement = match txn.get_database_backend() {
            DatabaseBackend::Postgres => Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "UPDATE forum_topics SET updated_at = updated_at WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
                vec![tenant_id.into(), topic_id.into()],
            ),
            DatabaseBackend::Sqlite => Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE forum_topics SET updated_at = updated_at WHERE tenant_id = ?1 AND id = ?2 AND deleted_at IS NULL",
                vec![tenant_id.into(), topic_id.into()],
            ),
            backend => {
                return Err(ForumError::Validation(format!(
                    "Forum topic row locking does not support database backend {backend:?}"
                )));
            }
        };

        if txn.execute_raw(statement).await?.rows_affected() != 1 {
            let deleted_statement = match txn.get_database_backend() {
                DatabaseBackend::Postgres => Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT deleted_at FROM forum_topics WHERE tenant_id = $1 AND id = $2",
                    vec![tenant_id.into(), topic_id.into()],
                ),
                DatabaseBackend::Sqlite => Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "SELECT deleted_at FROM forum_topics WHERE tenant_id = ?1 AND id = ?2",
                    vec![tenant_id.into(), topic_id.into()],
                ),
                backend => {
                    return Err(ForumError::Validation(format!(
                        "Forum topic deleted-state lookup does not support database backend {backend:?}"
                    )));
                }
            };
            if let Some(row) = txn.query_one_raw(deleted_statement).await? {
                let deleted_at: Option<String> = row.try_get("", "deleted_at")?;
                if deleted_at.is_some() {
                    return Err(ForumError::TopicDeleted);
                }
            }
            return Err(ForumError::TopicNotFound(topic_id));
        }

        query
            .one(txn)
            .await?
            .ok_or(ForumError::TopicNotFound(topic_id))
    }

    pub(crate) async fn claim_topic_update_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        expected_updated_at: chrono::DateTime<chrono::FixedOffset>,
    ) -> ForumResult<chrono::DateTime<chrono::Utc>> {
        let claimed_at = Utc::now();
        let statement = match txn.get_database_backend() {
            DatabaseBackend::Postgres => Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "UPDATE forum_topics SET updated_at = $3 WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL AND updated_at = $4",
                vec![
                    tenant_id.into(),
                    topic_id.into(),
                    claimed_at.fixed_offset().into(),
                    expected_updated_at.into(),
                ],
            ),
            DatabaseBackend::Sqlite => Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE forum_topics SET updated_at = ?3 WHERE tenant_id = ?1 AND id = ?2 AND deleted_at IS NULL AND updated_at = ?4",
                vec![
                    tenant_id.into(),
                    topic_id.into(),
                    claimed_at.fixed_offset().into(),
                    expected_updated_at.into(),
                ],
            ),
            backend => {
                return Err(ForumError::Validation(format!(
                    "Forum topic update locking does not support database backend {backend:?}"
                )));
            }
        };

        let result = txn.execute_raw(statement).await?;
        if result.rows_affected() != 1 {
            return Err(ForumError::TopicUpdateConflict(topic_id));
        }
        Ok(claimed_at)
    }

    pub(crate) async fn adjust_reply_count_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        delta: i32,
    ) -> ForumResult<forum_topic::Model> {
        if delta == 0 {
            return Self::find_topic_in_tx(txn, tenant_id, topic_id).await;
        }

        let now = Utc::now();
        use sea_orm::sea_query::ExprTrait;
        let mut query = forum_topic::Entity::update_many()
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .filter(forum_topic::Column::Id.eq(topic_id))
            .filter(forum_topic::Column::ReplyCount.gte(0));

        if delta > 0 {
            query = query.col_expr(
                forum_topic::Column::ReplyCount,
                Expr::col(forum_topic::Column::ReplyCount).add(delta),
            );
        } else {
            let decrement = delta.checked_abs().ok_or_else(|| {
                ForumError::Validation("Forum reply counter delta overflow".to_string())
            })?;
            query = query
                .filter(forum_topic::Column::ReplyCount.gte(decrement))
                .col_expr(
                    forum_topic::Column::ReplyCount,
                    Expr::col(forum_topic::Column::ReplyCount).sub(decrement),
                );
        }

        let updated = query
            .col_expr(forum_topic::Column::UpdatedAt, Expr::val(now))
            .exec(txn)
            .await?;
        if updated.rows_affected != 1 {
            if Self::find_topic_in_tx(txn, tenant_id, topic_id)
                .await?
                .reply_count
                < 0
            {
                return Err(ForumError::Validation(
                    "Forum topic reply counter is inconsistent".to_string(),
                ));
            }
            return Err(ForumError::Validation(
                "Forum topic reply counter does not permit the requested delta".to_string(),
            ));
        }

        // `last_reply_at` is a derived projection of the newest approved reply, so
        // both directions recompute it from the reply rows instead of trusting the
        // wall clock. Stamp it from the counter bump and a reply that carries its
        // own timestamp (starter import, restore after moderation) moves the topic
        // to a recency the data does not support, which skews the fixed
        // recent-activity order and the keyset cursor built on it.
        let last_reply_at = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::TopicId.eq(topic_id))
            .filter(forum_reply::Column::Status.eq(ReplyStatus::Approved))
            .order_by_desc(forum_reply::Column::CreatedAt)
            .one(txn)
            .await?
            .map(|reply| reply.created_at);

        let topic = Self::find_topic_in_tx(txn, tenant_id, topic_id).await?;
        let mut active: forum_topic::ActiveModel = topic.into();
        active.last_reply_at = Set(last_reply_at);
        active.updated_at = Set(now.into());
        Ok(active.update(txn).await?)
    }

    pub(crate) async fn set_pinned_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        is_pinned: bool,
    ) -> ForumResult<()> {
        let topic = Self::find_topic_for_update_in_tx(txn, tenant_id, topic_id).await?;
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
        let topic = Self::find_topic_for_update_in_tx(txn, tenant_id, topic_id).await?;
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
        let topic = Self::find_topic_for_update_in_tx(txn, tenant_id, topic_id).await?;
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

    async fn is_topic_deleted(&self, tenant_id: Uuid, topic_id: Uuid) -> ForumResult<bool> {
        let statement = match self.db.get_database_backend() {
            DatabaseBackend::Postgres => Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT CASE WHEN deleted_at IS NULL THEN 0 ELSE 1 END AS is_deleted FROM forum_topics WHERE tenant_id = $1 AND id = $2",
                vec![tenant_id.into(), topic_id.into()],
            ),
            DatabaseBackend::Sqlite => Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT CASE WHEN deleted_at IS NULL THEN 0 ELSE 1 END AS is_deleted FROM forum_topics WHERE tenant_id = ?1 AND id = ?2",
                vec![tenant_id.into(), topic_id.into()],
            ),
            backend => {
                return Err(ForumError::Validation(format!(
                    "Forum topic deleted-state lookup does not support database backend {backend:?}"
                )));
            }
        };
        let row = self
            .db
            .query_one_raw(statement)
            .await?
            .ok_or(ForumError::TopicNotFound(topic_id))?;
        let is_deleted = row
            .try_get::<i32>("", "is_deleted")
            .map(|v| v == 1)
            .or_else(|_| row.try_get::<i64>("", "is_deleted").map(|v| v == 1))
            .or_else(|_| row.try_get::<bool>("", "is_deleted"))?;
        Ok(is_deleted)
    }

    async fn load_deleted_topic_ids(
        &self,
        tenant_id: Uuid,
        topic_ids: &[Uuid],
    ) -> ForumResult<HashSet<Uuid>> {
        if topic_ids.is_empty() {
            return Ok(HashSet::new());
        }

        let backend = self.db.get_database_backend();
        let placeholders = (0..topic_ids.len())
            .map(|index| match backend {
                DatabaseBackend::Postgres => format!("{}{}", "$", index + 2),
                DatabaseBackend::Sqlite => format!("?{}", index + 2),
                _ => "?".to_string(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let tenant_placeholder = match backend {
            DatabaseBackend::Postgres => "$1",
            DatabaseBackend::Sqlite => "?1",
            backend => {
                return Err(ForumError::Validation(format!(
                    "Forum topic deleted-state lookup does not support database backend {backend:?}"
                )));
            }
        };
        let sql = format!(
            "SELECT id FROM forum_topics WHERE tenant_id = {} AND deleted_at IS NOT NULL AND id IN ({})",
            tenant_placeholder, placeholders
        );
        let mut values = Vec::with_capacity(topic_ids.len() + 1);
        values.push(tenant_id.into());
        values.extend(topic_ids.iter().copied().map(Into::into));

        let statement = Statement::from_sql_and_values(backend, sql, values);
        let rows = self.db.query_all_raw(statement).await?;
        let mut ids = HashSet::with_capacity(rows.len());
        for row in rows {
            ids.insert(row.try_get("", "id")?);
        }
        Ok(ids)
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
        let deleted_topic_ids = self.load_deleted_topic_ids(tenant_id, &topic_ids).await?;
        let schema = load_topic_custom_fields_schema(&self.db, tenant_id).await?;
        let metadata_inputs = topics
            .iter()
            .map(|topic| AttachedPayloadResolutionInput {
                entity_id: topic.id,
                shared_metadata: &topic.metadata,
            })
            .collect::<Vec<_>>();
        let metadata_by_topic_id = resolve_attached_payloads(
            &self.db,
            tenant_id,
            "topic",
            schema,
            &metadata_inputs,
            locale,
            fallback_locale.unwrap_or(PLATFORM_FALLBACK_LOCALE),
        )
        .await
        .map_err(|error| ForumError::Validation(error.to_string()))?;
        let vote_summaries = VoteService::new(self.db.clone())
            .with_settings_providers(self.settings.clone())
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
            let metadata = metadata_by_topic_id
                .get(&topic.id)
                .cloned()
                .flatten()
                .unwrap_or_else(|| serde_json::json!({}));

            items.push(TopicListItem {
                id: topic.id,
                sort_key: Some(TopicListCursor {
                    is_pinned: topic.is_pinned,
                    last_reply_at: topic.last_reply_at,
                    updated_at: topic.updated_at,
                    id: topic.id,
                    reply_count: topic.reply_count,
                    order: Default::default(),
                    selection_scope: None,
                }),
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
                is_deleted: deleted_topic_ids.contains(&topic.id),
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
        // Only caller-supplied values are checked. A seed copied from another locale is skipped.
        let content_limits =
            ForumContentLimits::resolve_in_tx(&self.settings, txn, tenant_id).await?;
        if let Some(title) = title.as_deref() {
            content_limits.validate_title(title)?;
        }
        if let Some(body) = body.as_ref() {
            content_limits.validate_body(&project_discussion(body.clone())?.plain_text)?;
        }
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
        prepare_donor_attached_values_create(schema, &payload, locale)
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
        prepare_donor_attached_values_update(
            &self.db,
            flex::AttachedEntityRef {
                tenant_id,
                entity_type: "topic",
                entity_id: topic_id,
            },
            schema,
            locale,
            existing_metadata,
            Some(&payload),
        )
        .await
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
        is_deleted: parts.is_deleted,
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
    field_definition_from_source(&row)
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

impl TopicService {
    pub const MAX_FORUM_TOPIC_LOCALE_ENUMERATION_IDS: usize = 512;

    pub(crate) async fn available_locales_for_topics(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_ids: &[Uuid],
    ) -> ForumResult<Vec<(Uuid, Vec<String>)>> {
        enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;

        if tenant_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum topic locale enumeration requires a non-nil tenant id".to_string(),
            ));
        }
        if topic_ids.len() > Self::MAX_FORUM_TOPIC_LOCALE_ENUMERATION_IDS {
            return Err(ForumError::Validation(format!(
                "Forum topic locale enumeration is limited to {} topic IDs",
                Self::MAX_FORUM_TOPIC_LOCALE_ENUMERATION_IDS
            )));
        }
        if topic_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut seen = std::collections::BTreeSet::new();
        for topic_id in topic_ids {
            if topic_id.is_nil() {
                return Err(ForumError::Validation(
                    "Forum topic locale enumeration requires non-nil topic IDs".to_string(),
                ));
            }
            if !seen.insert(*topic_id) {
                return Err(ForumError::Validation(format!(
                    "Forum topic locale enumeration repeats topic {topic_id}"
                )));
            }
        }

        let existing = forum_topic::Entity::find()
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .filter(forum_topic::Column::Id.is_in(topic_ids.to_vec()))
            .all(&self.db)
            .await?;
        let existing_ids = existing
            .into_iter()
            .map(|topic| topic.id)
            .collect::<std::collections::BTreeSet<_>>();
        for topic_id in topic_ids {
            if !existing_ids.contains(topic_id) {
                return Err(ForumError::TopicNotFound(*topic_id));
            }
        }

        let mut translations_by_topic = self
            .load_translations_map_for_topics(tenant_id, topic_ids)
            .await?;
        let mut result = Vec::with_capacity(topic_ids.len());
        for topic_id in topic_ids {
            let translations = translations_by_topic.remove(topic_id).unwrap_or_default();
            let locales =
                available_locales_from(&translations, |translation| translation.locale.as_str());
            if locales.is_empty() {
                return Err(ForumError::Validation(format!(
                    "Forum topic {topic_id} has no stored locale translation"
                )));
            }
            result.push((*topic_id, locales));
        }

        Ok(result)
    }
}

use super::category_audience::lock_category_tree_in_tx;
use super::edit_window::enforce_author_edit_window;
use super::topic_create_audience_authorization::ForumTopicCreateAudienceAuthorizationService;
use super::topic_tag_lock::{lock_active_topic_tag_write_in_tx, lock_topic_tag_scopes_in_tx};
use crate::dto::{CreateTopicCommandInput, UpdateTopicCommandInput};
use rustok_api::PortContext;

pub const MAX_FORUM_TOPIC_TAGS: usize = 100;

impl TopicService {
    pub(crate) async fn create_with_inline_relations(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateTopicCommandInput,
    ) -> ForumResult<TopicResponse> {
        let create_audience =
            ForumTopicCreateAudienceAuthorizationService::without_facts_provider(self.db.clone());
        self.create_with_audience_authorization(tenant_id, security, None, input, &create_audience)
            .await
    }

    pub(crate) async fn create_with_audience_authorization(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: Option<PortContext>,
        input: CreateTopicCommandInput,
        create_audience: &ForumTopicCreateAudienceAuthorizationService,
    ) -> ForumResult<TopicResponse> {
        let (input, quote_inputs) = input.into_parts();
        enforce_scope(&security, Resource::ForumTopics, Action::Create)?;
        validate_topic_title(&input.title)?;
        let content_limits = ForumContentLimits::resolve(&self.settings, tenant_id).await?;
        content_limits.validate_title(&input.title)?;
        let locale = normalize_locale(&input.locale)?;
        let normalized_tags = normalize_tags(&input.tags);
        validate_normalized_topic_tags(&normalized_tags)?;
        let document = crate::richtext::normalize_discussion(input.body)?;
        let plain_text = crate::richtext::project_discussion(document.clone())?.plain_text;
        content_limits.validate_body(&plain_text)?;
        let stored_body = crate::richtext::serialize_discussion(document.clone())?;
        let prepared_custom_fields = self
            .prepare_topic_custom_fields_for_create(tenant_id, &locale, input.metadata.clone())
            .await?;
        let topic_id = Uuid::new_v4();
        let quotes = super::relation_quote_input::normalize_quote_inputs(quote_inputs)?;
        let relation_service =
            super::mention_relation::MentionRelationService::new(self.db.clone());
        let prepared_relations = relation_service
            .prepare(
                tenant_id,
                crate::mentions::ForumContentTarget::topic(topic_id),
                &locale,
                &document,
                &security,
                quotes,
            )
            .await?;

        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        create_audience
            .require_in_tx(&txn, tenant_id, input.category_id, &security, context)
            .await
            .and_then(|authorization| {
                if authorization.allowed {
                    Ok(())
                } else {
                    Err(ForumError::forbidden(
                        "Forum topic creation is unavailable for the current audience",
                    ))
                }
            })?;
        CategoryService::ensure_exists_in_tx(&txn, tenant_id, input.category_id).await?;
        super::category_lifecycle::ensure_category_tree_target_is_active_in_tx(
            &txn,
            tenant_id,
            input.category_id,
        )
        .await?;
        super::posting_rate::enforce_new_topic_rate_in_tx(
            &txn,
            tenant_id,
            &security,
            content_limits.new_topic_rate_limit_seconds(),
        )
        .await?;
        // Tenant pre-moderation holds a new topic as `Pending`. The per-category `moderated` flag
        // holds replies only, so it does not hold topics. Approval moves the topic to `Open`
        // through the moderation owner. See `DECISIONS/2026-10-09-forum-topic-pre-moderation.md`.
        let pre_moderation_enabled = self
            .settings
            .module_settings_in_tx(&txn, tenant_id)
            .await?
            .pre_moderation_enabled;
        let status = if pre_moderation_enabled {
            TopicStatus::Pending
        } else {
            TopicStatus::Open
        };

        let now = Utc::now();
        forum_topic::ActiveModel {
            id: Set(topic_id),
            tenant_id: Set(tenant_id),
            category_id: Set(input.category_id),
            author_id: Set(security.user_id),
            status: Set(status),
            metadata: Set(prepared_custom_fields
                .metadata
                .clone()
                .unwrap_or_else(|| serde_json::json!({}))),
            is_pinned: Set(false),
            is_locked: Set(false),
            reply_count: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            last_reply_at: Set(None),
        }
        .insert(&txn)
        .await?;
        super::subscription::auto_subscribe::auto_subscribe_topic_author_in_tx(
            &txn,
            tenant_id,
            topic_id,
            security.user_id,
        )
        .await?;
        lock_active_topic_tag_write_in_tx(&txn, tenant_id, topic_id).await?;
        lock_topic_tag_scopes_in_tx(&txn, tenant_id, &[topic_id]).await?;

        forum_topic_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            topic_id: Set(topic_id),
            tenant_id: Set(tenant_id),
            locale: Set(locale.clone()),
            title: Set(input.title),
            slug: Set(input
                .slug
                .map(|value| normalize_slug(&value))
                .filter(|value| !value.is_empty())),
            body: Set(stored_body),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        relation_service
            .persist_in_tx(&txn, prepared_relations)
            .await?;

        if let (Some(persist_locale), Some(values)) = (
            prepared_custom_fields.locale.as_deref(),
            prepared_custom_fields.localized_values.as_ref(),
        ) {
            persist_localized_values(&txn, tenant_id, "topic", topic_id, persist_locale, values)
                .await
                .map_err(|error| ForumError::Validation(error.to_string()))?;
        }

        self.sync_channel_access_in_tx(&txn, tenant_id, topic_id, input.channel_slugs.as_deref())
            .await?;
        self.sync_topic_tags_in_tx(&txn, tenant_id, topic_id, &locale, &normalized_tags)
            .await?;
        CategoryService::adjust_counters_in_tx(&txn, tenant_id, input.category_id, 1, 0).await?;
        UserStatsService::adjust_topic_count_in_tx(&txn, tenant_id, security.user_id, 1).await?;

        // A held topic is not a publication. The moderation owner emits this
        // event exactly when Pending -> Open first makes it public.
        if status == TopicStatus::Open {
            self.event_bus
                .publish_in_tx(
                    &txn,
                    tenant_id,
                    security.user_id,
                    DomainEvent::ForumTopicCreated {
                        topic_id,
                        category_id: input.category_id,
                        author_id: security.user_id,
                        locale: locale.clone(),
                    },
                )
                .await?;
        }
        super::projection_invalidation::publish_forum_category_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            security.user_id,
            input.category_id,
        )
        .await?;

        txn.commit().await?;
        self.get(tenant_id, security, topic_id, &locale).await
    }

    pub(crate) async fn update_with_inline_relations(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: UpdateTopicCommandInput,
    ) -> ForumResult<TopicResponse> {
        let (input, quote_inputs) = input.into_parts();
        let locale = normalize_locale(&input.locale)?;
        let topic = self.find_topic(tenant_id, topic_id).await?;
        enforce_owned_scope(
            &security,
            Resource::ForumTopics,
            Action::Update,
            topic.author_id,
        )?;
        if topic.status == TopicStatus::Archived {
            // Archived (including soft-deleted) topics are immutable;
            // restore the topic before editing it.
            return Err(ForumError::TopicArchived);
        }
        let max_edit_window_minutes = self
            .settings
            .module_settings(tenant_id)
            .await?
            .max_edit_window_minutes;
        enforce_author_edit_window(
            &security,
            Resource::ForumTopics,
            topic.author_id,
            topic.created_at,
            max_edit_window_minutes,
            chrono::Utc::now(),
        )?;
        let expected_updated_at = topic.updated_at;
        let prepared_custom_fields = if let Some(metadata) = input.metadata.clone() {
            Some(
                self.prepare_topic_custom_fields_for_update(
                    tenant_id,
                    topic_id,
                    &locale,
                    &topic.metadata,
                    metadata,
                )
                .await?,
            )
        } else {
            None
        };
        let normalized_tags = input.tags.as_ref().map(|tags| normalize_tags(tags));
        if let Some(tags) = normalized_tags.as_ref() {
            validate_normalized_topic_tags(tags)?;
        }
        let mut prepared_relation_body = self
            .prepare_topic_relation_body_for_update(tenant_id, topic_id, &locale, &input)
            .await?;
        if prepared_relation_body.is_none() && quote_inputs.is_some() {
            let existing = forum_topic_translation::Entity::find()
                .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
                .filter(forum_topic_translation::Column::TopicId.eq(topic_id))
                .filter(forum_topic_translation::Column::Locale.eq(&locale))
                .one(&self.db)
                .await?
                .ok_or_else(ForumError::relation_revision_unavailable)?;
            prepared_relation_body = Some(
                crate::richtext::project_stored_discussion(&existing.body)?
                    .view
                    .document,
            );
        }
        let relation_service =
            super::mention_relation::MentionRelationService::new(self.db.clone());
        let (prepared_relations, quote_expectation) =
            if let Some(body) = prepared_relation_body.as_ref() {
                let resolved = super::relation_quote_input::resolve_inline_update_quotes(
                    &self.db,
                    tenant_id,
                    crate::mentions::ForumContentTarget::topic(topic_id),
                    &locale,
                    quote_inputs,
                )
                .await?;
                let (quotes, expectation) = resolved.into_parts();
                (
                    Some(
                        relation_service
                            .prepare(
                                tenant_id,
                                crate::mentions::ForumContentTarget::topic(topic_id),
                                &locale,
                                body,
                                &security,
                                quotes,
                            )
                            .await?,
                    ),
                    Some(expectation),
                )
            } else {
                (None, None)
            };

        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        let claimed_updated_at =
            Self::claim_topic_update_in_tx(&txn, tenant_id, topic_id, expected_updated_at).await?;
        // A content edit in a pre-moderated tenant re-enters moderation only when
        // normalized title or body actually changes: otherwise no-op saves (or metadata/tag-only edits)
        // would falsely de-publish an already approved topic.
        let existing_translation = forum_topic_translation::Entity::find()
            .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_translation::Column::TopicId.eq(topic_id))
            .filter(forum_topic_translation::Column::Locale.eq(&locale))
            .one(&txn)
            .await?;
        let title_changed = match (&input.title, &existing_translation) {
            (Some(new_title), Some(existing)) => new_title.trim() != existing.title.trim(),
            (Some(_), None) => true,
            (None, _) => false,
        };
        let body_changed = match (&prepared_relation_body, &existing_translation) {
            (Some(new_body), Some(existing)) => {
                let existing_doc = crate::richtext::project_stored_discussion(&existing.body)
                    .map(|p| p.view.document)
                    .ok();
                existing_doc.as_ref() != Some(new_body)
            }
            (Some(_), None) => true,
            (None, _) => false,
        };
        let content_changed = title_changed || body_changed;

        let repend_for_moderation = topic.status == TopicStatus::Open
            && content_changed
            && self
                .settings
                .module_settings_in_tx(&txn, tenant_id)
                .await?
                .pre_moderation_enabled;
        if let Some(expectation) = quote_expectation {
            super::relation_quote_input::lock_source_and_assert_latest_in_tx(
                &txn,
                tenant_id,
                crate::mentions::ForumContentTarget::topic(topic_id),
                &locale,
                expectation,
            )
            .await?;
        }
        if normalized_tags.is_some() {
            lock_active_topic_tag_write_in_tx(&txn, tenant_id, topic_id).await?;
            lock_topic_tag_scopes_in_tx(&txn, tenant_id, &[topic_id]).await?;
        }
        let mut active: forum_topic::ActiveModel = topic.into();
        active.updated_at = Set(claimed_updated_at.into());
        if repend_for_moderation {
            active.status = Set(TopicStatus::Pending);
        }
        if let Some(prepared_custom_fields) = prepared_custom_fields.as_ref() {
            active.metadata = Set(prepared_custom_fields
                .metadata
                .clone()
                .unwrap_or_else(|| serde_json::json!({})));
        }
        active.update(&txn).await?;

        if let Some(prepared_custom_fields) = prepared_custom_fields.as_ref()
            && let (Some(persist_locale), Some(values)) = (
                prepared_custom_fields.locale.as_deref(),
                prepared_custom_fields.localized_values.as_ref(),
            )
        {
            persist_localized_values(&txn, tenant_id, "topic", topic_id, persist_locale, values)
                .await
                .map_err(|error| ForumError::Validation(error.to_string()))?;
        }

        self.upsert_translation_in_tx(
            &txn,
            tenant_id,
            topic_id,
            &locale,
            TopicTranslationUpsertInput {
                title: input.title,
                body: input.body,
            },
        )
        .await?;

        if let Some(prepared_relations) = prepared_relations {
            relation_service
                .persist_in_tx(&txn, prepared_relations)
                .await?;
        }

        if input.channel_slugs.is_some() {
            self.sync_channel_access_in_tx(
                &txn,
                tenant_id,
                topic_id,
                input.channel_slugs.as_deref(),
            )
            .await?;
        }
        if let Some(tags) = normalized_tags.as_ref() {
            self.sync_topic_tags_in_tx(&txn, tenant_id, topic_id, &locale, tags)
                .await?;
        }

        super::projection_invalidation::publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            security.user_id,
            topic_id,
        )
        .await?;
        txn.commit().await?;
        self.get(tenant_id, security, topic_id, &locale).await
    }
}

fn validate_normalized_topic_tags(tags: &[String]) -> ForumResult<()> {
    if tags.len() > MAX_FORUM_TOPIC_TAGS {
        return Err(ForumError::Validation(format!(
            "Forum topic tags must not exceed {MAX_FORUM_TOPIC_TAGS} entries"
        )));
    }
    Ok(())
}

use super::pending_visibility::topic_owner_read_condition;

impl TopicService {
    #[instrument(skip(self, security, filter, hidden_category_ids))]
    pub(crate) async fn list_with_locale_fallback_and_hidden_categories(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        mut filter: ListTopicsFilter,
        fallback_locale: Option<&str>,
        hidden_category_ids: &[Uuid],
    ) -> ForumResult<TopicPage<TopicListItem>> {
        enforce_scope(&security, Resource::ForumTopics, Action::List)?;
        filter.normalize_selection()?;
        let locale = filter
            .locale
            .clone()
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let locale = normalize_locale(&locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;

        let mut select =
            forum_topic::Entity::find().filter(forum_topic::Column::TenantId.eq(tenant_id));
        if let Some(category_id) = filter.category_id {
            select = select.filter(forum_topic::Column::CategoryId.eq(category_id));
        }
        if let Some(status) = filter.status {
            select = select.filter(forum_topic::Column::Status.eq(status));
        }
        if !hidden_category_ids.is_empty() {
            select = select
                .filter(forum_topic::Column::CategoryId.is_not_in(hidden_category_ids.to_vec()));
        }
        select = select.filter(topic_owner_read_condition(&security));

        let scope = filter.cursor_scope(
            tenant_id,
            security.user_id,
            &locale,
            fallback_locale.as_deref(),
            None,
            false,
        )?;
        self.fetch_localized_topic_page(
            select,
            &filter,
            tenant_id,
            security.user_id,
            (&locale, fallback_locale.as_deref()),
            scope,
        )
        .await
    }

    #[instrument(skip(self, security, filter, hidden_category_ids))]
    pub(crate) async fn list_storefront_visible_with_locale_fallback_and_hidden_categories(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        mut filter: ListTopicsFilter,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
        hidden_category_ids: &[Uuid],
    ) -> ForumResult<TopicPage<TopicListItem>> {
        enforce_scope(&security, Resource::ForumTopics, Action::List)?;
        filter.normalize_selection()?;
        let locale = filter
            .locale
            .clone()
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let locale = normalize_locale(&locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;

        let mut select = forum_topic::Entity::find()
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .filter(forum_topic::Column::Status.eq(TopicStatus::Open));
        if let Some(category_id) = filter.category_id {
            select = select.filter(forum_topic::Column::CategoryId.eq(category_id));
        }
        if !hidden_category_ids.is_empty() {
            select = select
                .filter(forum_topic::Column::CategoryId.is_not_in(hidden_category_ids.to_vec()));
        }
        select = apply_tenant_scoped_storefront_channel_filter(select, tenant_id, channel_slug);
        let show_locked_topics_in_lists = self
            .settings
            .module_settings(tenant_id)
            .await?
            .show_locked_topics_in_lists;
        if !show_locked_topics_in_lists {
            select = select.filter(forum_topic::Column::IsLocked.eq(false));
        }

        let cursor_channel = normalize_public_channel_slug(channel_slug);
        let scope = filter.cursor_scope(
            tenant_id,
            security.user_id,
            &locale,
            fallback_locale.as_deref(),
            cursor_channel.as_deref(),
            true,
        )?;
        self.fetch_localized_topic_page(
            select,
            &filter,
            tenant_id,
            security.user_id,
            (&locale, fallback_locale.as_deref()),
            scope,
        )
        .await
    }

    /// Select over hydrated owner-localized items, not a browser page or an arbitrary translation.
    async fn fetch_localized_topic_page(
        &self,
        select: Select<forum_topic::Entity>,
        filter: &ListTopicsFilter,
        tenant_id: Uuid,
        viewer_id: Option<Uuid>,
        locale: (&str, Option<&str>),
        scope: [u8; 32],
    ) -> ForumResult<TopicPage<TopicListItem>> {
        if filter.search.is_none() && !filter.solved_only {
            let page = self.fetch_topic_keyset_page(select, filter, scope).await?;
            let mut items = self
                .hydrate_topic_list_items(tenant_id, viewer_id, page.items, locale.0, locale.1)
                .await?;
            for topic in &mut items {
                if let Some(key) = topic.sort_key.as_mut() {
                    key.order = filter.order;
                    key.selection_scope = Some(scope);
                }
            }
            return Ok(TopicPage {
                items,
                next_cursor: page.next_cursor,
            });
        }
        let mut candidate = filter.clone();
        candidate.per_page = crate::dto::MAX_FORUM_READ_LIMIT;
        let mut items: Vec<TopicListItem> = Vec::new();
        loop {
            let page = self
                .fetch_topic_keyset_page(select.clone(), &candidate, scope)
                .await?;
            let hydrated = self
                .hydrate_topic_list_items(tenant_id, viewer_id, page.items, locale.0, locale.1)
                .await?;
            for mut topic in hydrated {
                if !filter.matches_selection(&topic) {
                    continue;
                }
                if items.len() as u64 == filter.per_page.max(1) {
                    let next_cursor = items
                        .last()
                        .and_then(|last| last.sort_key)
                        .map(|key| key.encode());
                    return Ok(TopicPage { items, next_cursor });
                }
                if let Some(key) = topic.sort_key.as_mut() {
                    key.order = filter.order;
                    key.selection_scope = Some(scope);
                }
                items.push(topic);
            }
            match page.next_cursor {
                Some(after) => candidate.after = Some(after),
                None => {
                    return Ok(TopicPage {
                        items,
                        next_cursor: None,
                    });
                }
            }
        }
    }

    /// Keyset page of topics ordered `is_pinned DESC, last_reply_at DESC NULLS LAST,
    /// updated_at DESC, id DESC`. Fetches one extra row to learn whether another page
    /// exists, so the list never counts rows.
    async fn fetch_topic_keyset_page(
        &self,
        mut select: Select<forum_topic::Entity>,
        filter: &ListTopicsFilter,
        scope: [u8; 32],
    ) -> ForumResult<TopicPage<forum_topic::Model>> {
        let per_page = filter.per_page.max(1);
        if let Some(raw) = filter.after.as_deref() {
            let after = TopicListCursor::decode(raw)?;
            if after.order != filter.order || after.selection_scope != Some(scope) {
                return Err(ForumError::Validation(
                    "Forum topic cursor does not match this selection".into(),
                ));
            }
            let condition = if filter.order == crate::ForumTopicListOrder::Top {
                Condition::any()
                    .add(forum_topic::Column::ReplyCount.lt(after.reply_count))
                    .add(
                        Condition::all()
                            .add(forum_topic::Column::ReplyCount.eq(after.reply_count))
                            .add(topic_list_after_condition(&after)),
                    )
            } else {
                topic_list_after_condition(&after)
            };
            select = select.filter(condition);
        }

        if filter.order == crate::ForumTopicListOrder::Top {
            select = select.order_by_desc(forum_topic::Column::ReplyCount);
        }
        let mut topics = order_topic_list(select)
            .limit(per_page + 1)
            .all(&self.db)
            .await?;

        let has_next_page = topics.len() as u64 > per_page;
        topics.truncate(per_page as usize);
        let next_cursor = if has_next_page {
            let last = topics.last().ok_or_else(|| {
                ForumError::Validation("Topic page reported a next page without rows".to_string())
            })?;
            let mut key = topic_list_cursor(last);
            key.order = filter.order;
            key.selection_scope = Some(scope);
            Some(key.encode())
        } else {
            None
        };
        Ok(TopicPage {
            items: topics,
            next_cursor,
        })
    }
}

/// The one ordering of every topic list: `is_pinned DESC, last_reply_at DESC NULLS LAST,
/// updated_at DESC, id DESC`. It is expressed as a column order with explicit NULL
/// placement, not as an `IS NULL` expression, so `idx_forum_topics_list_keyset` can
/// serve it. Read-model and unread projections order through this function too.
pub(crate) fn order_topic_list(select: Select<forum_topic::Entity>) -> Select<forum_topic::Entity> {
    select
        .order_by_desc(forum_topic::Column::IsPinned)
        .order_by_with_nulls(
            forum_topic::Column::LastReplyAt,
            Order::Desc,
            NullOrdering::Last,
        )
        .order_by_desc(forum_topic::Column::UpdatedAt)
        .order_by_desc(forum_topic::Column::Id)
}

/// Cursor that resumes the list directly after `topic` in `order_topic_list`.
pub(crate) fn topic_list_cursor(topic: &forum_topic::Model) -> TopicListCursor {
    TopicListCursor {
        is_pinned: topic.is_pinned,
        last_reply_at: topic.last_reply_at,
        updated_at: topic.updated_at,
        id: topic.id,
        reply_count: topic.reply_count,
        order: Default::default(),
        selection_scope: None,
    }
}

/// Rows strictly after `after` in `order_topic_list`, expanded lexicographically over
/// the four sort keys.
pub(crate) fn topic_list_after_condition(after: &TopicListCursor) -> Condition {
    let pinned_eq = forum_topic::Column::IsPinned.eq(after.is_pinned);
    let last_reply_eq = match after.last_reply_at {
        Some(value) => forum_topic::Column::LastReplyAt.eq(value),
        None => forum_topic::Column::LastReplyAt.is_null(),
    };

    let mut condition = Condition::any().add(forum_topic::Column::IsPinned.lt(after.is_pinned));
    if let Some(last_reply_at) = after.last_reply_at {
        // NULLs sort last, so after a non-NULL value come smaller values and every NULL.
        condition = condition.add(
            Condition::all().add(pinned_eq.clone()).add(
                Condition::any()
                    .add(forum_topic::Column::LastReplyAt.lt(last_reply_at))
                    .add(forum_topic::Column::LastReplyAt.is_null()),
            ),
        );
    }
    // After a NULL `last_reply_at` nothing follows within the same pinned group except
    // rows that are also NULL, which the two branches below already cover.
    condition = condition.add(
        Condition::all()
            .add(pinned_eq.clone())
            .add(last_reply_eq.clone())
            .add(forum_topic::Column::UpdatedAt.lt(after.updated_at)),
    );
    condition.add(
        Condition::all()
            .add(pinned_eq)
            .add(last_reply_eq)
            .add(forum_topic::Column::UpdatedAt.eq(after.updated_at))
            .add(forum_topic::Column::Id.lt(after.id)),
    )
}

fn apply_tenant_scoped_storefront_channel_filter(
    select: Select<forum_topic::Entity>,
    tenant_id: Uuid,
    channel_slug: Option<&str>,
) -> Select<forum_topic::Entity> {
    let unrestricted =
        forum_topic::Column::Id.not_in_subquery(tenant_topic_channel_access_subquery(tenant_id));
    let condition = match normalize_public_channel_slug(channel_slug) {
        Some(channel_slug) => {
            Condition::any()
                .add(unrestricted)
                .add(forum_topic::Column::Id.in_subquery(
                    matching_tenant_topic_channel_access_subquery(tenant_id, &channel_slug),
                ))
        }
        None => Condition::all().add(unrestricted),
    };

    select.filter(condition)
}

fn tenant_topic_channel_access_subquery(tenant_id: Uuid) -> SelectStatement {
    Query::select()
        .column(forum_topic_channel_access::Column::TopicId)
        .from(forum_topic_channel_access::Entity)
        .and_where(forum_topic_channel_access::Column::TenantId.eq(tenant_id))
        .to_owned()
}

fn matching_tenant_topic_channel_access_subquery(
    tenant_id: Uuid,
    channel_slug: &str,
) -> SelectStatement {
    Query::select()
        .column(forum_topic_channel_access::Column::TopicId)
        .from(forum_topic_channel_access::Entity)
        .and_where(forum_topic_channel_access::Column::TenantId.eq(tenant_id))
        .and_where(forum_topic_channel_access::Column::ChannelSlug.eq(channel_slug))
        .to_owned()
}

fn normalize_public_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(|slug| slug.to_ascii_lowercase())
}

/// Upper bound on candidate topics that one widget list preview may visit.
///
/// The exact `total` and the requested page both depend on the owner audience of every
/// candidate in the ordered widget query, so the scan is linear in the candidate count. A
/// request over this bound fails with a validation error instead of returning a partial total.
/// Callers narrow the preview with `category_id`.
pub(crate) const FORUM_WIDGET_TOPIC_LIST_MAX_CANDIDATES: u64 = 1_000;
const FORUM_WIDGET_TOPIC_LIST_SCAN_BATCH: u64 = 100;

/// Typed widget list query after props validation.
pub(crate) struct WidgetTopicListQuery<'a> {
    pub category_id: Option<Uuid>,
    pub page: u64,
    pub per_page: u64,
    pub include_pinned: bool,
    pub sort: &'a str,
}

impl TopicService {
    /// Ordered widget candidate query. Ordering is applied before pagination; the id tiebreaker
    /// keeps offsets stable across scan batches.
    fn widget_topic_candidates(
        tenant_id: Uuid,
        category_id: Option<Uuid>,
        include_pinned: bool,
        sort: &str,
    ) -> ForumResult<Select<forum_topic::Entity>> {
        let mut select =
            forum_topic::Entity::find().filter(forum_topic::Column::TenantId.eq(tenant_id));
        if let Some(category_id) = category_id {
            select = select.filter(forum_topic::Column::CategoryId.eq(category_id));
        }
        if !include_pinned {
            select = select.filter(forum_topic::Column::IsPinned.eq(false));
        } else {
            select = select.order_by_desc(forum_topic::Column::IsPinned);
        }

        let select = match sort {
            "activity" => select
                .order_by_desc(forum_topic::Column::LastReplyAt)
                .order_by_desc(forum_topic::Column::UpdatedAt),
            "newest" => select.order_by_desc(forum_topic::Column::CreatedAt),
            "top" => select
                .order_by_desc(Expr::cust(
                    "COALESCE((SELECT SUM(forum_topic_votes.value) FROM forum_topic_votes \
                     WHERE forum_topic_votes.tenant_id = forum_topics.tenant_id \
                     AND forum_topic_votes.topic_id = forum_topics.id), 0)",
                ))
                .order_by_desc(forum_topic::Column::LastReplyAt)
                .order_by_desc(forum_topic::Column::UpdatedAt),
            other => {
                return Err(ForumError::Validation(format!(
                    "Unsupported Forum widget topic sort: {other}"
                )));
            }
        };
        Ok(select.order_by_desc(forum_topic::Column::Id))
    }

    /// Exact owner-audience widget topic list.
    ///
    /// Every candidate is checked with
    /// `ForumTopicAudienceVisibilityService::is_topic_owner_visible`, which covers the inherited
    /// category floor and every richer category/topic layer. Pagination and `total` count only
    /// topics the viewer can read, so a page never has gaps and the total never counts hidden
    /// topics.
    #[instrument(skip(self, security, context, visibility, query, fallback_locale))]
    pub(crate) async fn list_widget_preview_owner_visible(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: rustok_api::PortContext,
        visibility: &crate::services::ForumTopicAudienceVisibilityService,
        query: WidgetTopicListQuery<'_>,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<TopicListItem>, u64)> {
        enforce_scope(&security, Resource::ForumTopics, Action::List)?;
        let locale = normalize_locale(&context.locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let per_page = query.per_page.clamp(1, 100);
        let viewer =
            crate::services::ForumTopicAudienceViewer::authenticated(security.clone(), context)?;

        let candidates = Self::widget_topic_candidates(
            tenant_id,
            query.category_id,
            query.include_pinned,
            query.sort,
        )?;
        let paginator = candidates.paginate(&self.db, FORUM_WIDGET_TOPIC_LIST_SCAN_BATCH);
        let candidate_count = paginator.num_items().await?;
        if candidate_count > FORUM_WIDGET_TOPIC_LIST_MAX_CANDIDATES {
            return Err(ForumError::Validation(format!(
                "Forum widget topic list has {candidate_count} candidate topics; narrow \
                 category_id so the preview scans at most {FORUM_WIDGET_TOPIC_LIST_MAX_CANDIDATES}"
            )));
        }

        let skip = query.page.saturating_sub(1).saturating_mul(per_page);
        let mut visible_total: u64 = 0;
        let mut selected = Vec::new();
        for page_index in 0..paginator.num_pages().await? {
            for topic in paginator.fetch_page(page_index).await? {
                if !visibility
                    .is_topic_owner_visible(tenant_id, topic.id, &viewer)
                    .await?
                {
                    continue;
                }
                if visible_total >= skip && (selected.len() as u64) < per_page {
                    selected.push(topic);
                }
                visible_total = visible_total.saturating_add(1);
            }
        }

        let items = self
            .hydrate_topic_list_items(
                tenant_id,
                security.user_id,
                selected,
                &locale,
                fallback_locale.as_deref(),
            )
            .await?;
        Ok((items, visible_total))
    }
}

