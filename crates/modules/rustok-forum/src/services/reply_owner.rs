pub(crate) struct InsertImportReplyWithTombstoneParams<'a> {
    pub tenant_id: Uuid,
    pub prepared: &'a PreparedImportReplyInsert,
    pub relation: &'a crate::import_relation_preparation::ForumPreparedImportContentRelations,
    pub tombstone:
        Option<&'a crate::import_tombstone_preparation::ForumPreparedDeletedReplyTombstone>,
    pub relation_event_mode: crate::import_relation_preparation::ForumImportRelationEventMode,
    pub write_event_mode: crate::import_write_preparation::ForumImportWriteEventMode,
}

impl ReplyService {
    pub(crate) fn prepare_import_reply_with_tombstone(
        &self,
        tenant_id: Uuid,
        record: &crate::import_write_preparation::ForumPreparedImportReply,
        tombstone: Option<&crate::import_tombstone_preparation::ForumPreparedDeletedReplyTombstone>,
    ) -> ForumResult<PreparedImportReplyInsert> {
        if record.status != ReplyStatus::Deleted {
            if tombstone.is_some() {
                return Err(ForumError::Validation(
                    "Forum import live reply cannot carry a deleted tombstone".to_string(),
                ));
            }
            return self.prepare_import_reply(tenant_id, record);
        }

        let tombstone = tombstone.ok_or_else(|| {
            ForumError::Validation(
                "Forum import deleted reply requires an admitted tombstone timestamp".to_string(),
            )
        })?;
        validate_import_reply_tombstone(record, tombstone)?;

        if tenant_id.is_nil() || record.id.is_nil() || record.topic_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum import reply preparation requires non-nil tenant, reply and topic IDs"
                    .to_string(),
            ));
        }
        let locale = normalize_locale(&record.locale)?;
        if locale != record.locale {
            return Err(ForumError::Validation(
                "Forum import reply locale must already be normalized".to_string(),
            ));
        }
        let document = crate::richtext::normalize_discussion(record.content.clone())?;
        let stored_body = crate::richtext::serialize_discussion(document.clone())?;
        let created_at = chrono::DateTime::<Utc>::from_timestamp_millis(record.created_at_ms)
            .ok_or_else(|| {
                ForumError::Validation(
                    "Forum import reply creation timestamp is outside owner range".to_string(),
                )
            })?;
        let deleted_at = chrono::DateTime::<Utc>::from_timestamp_millis(tombstone.deleted_at_ms)
            .ok_or_else(|| {
                ForumError::Validation(
                    "Forum import reply tombstone timestamp is outside owner range".to_string(),
                )
            })?;
        if deleted_at < created_at {
            return Err(ForumError::Validation(
                "Forum import reply tombstone cannot predate reply creation".to_string(),
            ));
        }

        Ok(PreparedImportReplyInsert {
            record: record.clone(),
            document,
            stored_body,
            created_at,
        })
    }

    pub(crate) async fn insert_import_reply_with_tombstone_in_tx(
        &self,
        txn: &DatabaseTransaction,
        params: InsertImportReplyWithTombstoneParams<'_>,
    ) -> ForumResult<()> {
        let tenant_id = params.tenant_id;
        let prepared = params.prepared;
        let relation = params.relation;
        let tombstone = params.tombstone;
        let relation_event_mode = params.relation_event_mode;
        let write_event_mode = params.write_event_mode;

        if prepared.record.status != ReplyStatus::Deleted {
            if tombstone.is_some() {
                return Err(ForumError::Validation(
                    "Forum import live reply cannot carry a deleted tombstone".to_string(),
                ));
            }
            return self
                .insert_import_reply_in_tx(
                    txn,
                    tenant_id,
                    prepared,
                    relation,
                    relation_event_mode,
                    write_event_mode,
                )
                .await;
        }

        let tombstone = tombstone.ok_or_else(|| {
            ForumError::Validation(
                "Forum import deleted reply requires an admitted tombstone timestamp".to_string(),
            )
        })?;
        validate_import_reply_tombstone(&prepared.record, tombstone)?;

        if relation.source != prepared.record.source
            || relation.target != ForumContentTarget::reply(prepared.record.id)
            || relation.locale != prepared.record.locale
        {
            return Err(ForumError::Validation(
                "Forum import reply relation admission does not match prepared reply".to_string(),
            ));
        }

        let topic =
            TopicService::find_topic_in_tx(txn, tenant_id, prepared.record.topic_id).await?;
        if topic.status != TopicStatus::Open || topic.is_locked {
            return Err(ForumError::Validation(
                "Forum import reply insertion requires the private provisional topic state"
                    .to_string(),
            ));
        }

        if let Some(parent_reply_id) = prepared.record.parent_reply_id {
            let parent =
                reply::ReplyService::find_reply_in_tx(txn, tenant_id, parent_reply_id).await?;
            if parent.topic_id != prepared.record.topic_id {
                return Err(ForumError::Validation(
                    "Forum import reply parent belongs to another topic".to_string(),
                ));
            }
            // Historical reconstruction differs from interactive create: a child
            // may legitimately survive after its parent was later soft-deleted.
            // The enclosing 34P tombstone batch proves every final Deleted row.
        }

        let position =
            allocate_reply_position_in_tx(txn, tenant_id, prepared.record.topic_id).await?;
        let author_id = prepared.record.author.as_ref().map(|author| author.user_id);

        forum_reply::ActiveModel {
            id: Set(prepared.record.id),
            tenant_id: Set(tenant_id),
            topic_id: Set(prepared.record.topic_id),
            author_id: Set(author_id),
            parent_reply_id: Set(prepared.record.parent_reply_id),
            status: Set(ReplyStatus::Deleted),
            position: Set(position),
            created_at: Set(prepared.created_at.into()),
            updated_at: Set(prepared.created_at.into()),
        }
        .insert(txn)
        .await?;

        forum_reply_body::ActiveModel {
            id: Set(Uuid::new_v4()),
            reply_id: Set(prepared.record.id),
            tenant_id: Set(tenant_id),
            locale: Set(prepared.record.locale.clone()),
            body: Set(prepared.stored_body.clone()),
            created_at: Set(prepared.created_at.into()),
            updated_at: Set(prepared.created_at.into()),
        }
        .insert(txn)
        .await?;

        // Final-deleted historical content keeps its relation projection for
        // owner consistency but never emits interactive mention-added events.
        self.relations
            .persist_import_admitted_in_tx(
                txn,
                tenant_id,
                relation,
                &prepared.document,
                author_id,
                crate::import_relation_preparation::ForumImportRelationEventMode::SuppressAddedTargetEvents,
            )
            .await?;

        persist_import_reply_tombstone_in_tx(txn, tenant_id, prepared, tombstone).await?;
        Ok(())
    }
}

fn validate_import_reply_tombstone(
    record: &crate::import_write_preparation::ForumPreparedImportReply,
    tombstone: &crate::import_tombstone_preparation::ForumPreparedDeletedReplyTombstone,
) -> ForumResult<()> {
    if tombstone.source != record.source || tombstone.reply_id != record.id {
        return Err(ForumError::Validation(
            "Forum import reply tombstone does not match prepared reply identity".to_string(),
        ));
    }
    if tombstone.deleted_at_ms < record.created_at_ms {
        return Err(ForumError::Validation(
            "Forum import reply tombstone cannot predate reply creation".to_string(),
        ));
    }
    if chrono::DateTime::<Utc>::from_timestamp_millis(tombstone.deleted_at_ms).is_none() {
        return Err(ForumError::Validation(
            "Forum import reply tombstone timestamp is outside owner range".to_string(),
        ));
    }
    Ok(())
}

async fn persist_import_reply_tombstone_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    prepared: &PreparedImportReplyInsert,
    tombstone: &crate::import_tombstone_preparation::ForumPreparedDeletedReplyTombstone,
) -> ForumResult<()> {
    let deleted_at = chrono::DateTime::<Utc>::from_timestamp_millis(tombstone.deleted_at_ms)
        .ok_or_else(|| {
            ForumError::Validation(
                "Forum import reply tombstone timestamp is outside owner range".to_string(),
            )
        })?;

    let before = count_import_delete_revisions_in_tx(txn, tenant_id, prepared.record.id).await?;
    if before != 0 {
        return Err(ForumError::Validation(
            "Forum import deleted reply must not have a pre-existing delete revision".to_string(),
        ));
    }

    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE forum_replies \
             SET deleted_at = $1, updated_at = $1 \
             WHERE tenant_id = $2 AND id = $3 AND status = 'deleted' AND deleted_at IS NULL",
            vec![
                deleted_at.into(),
                tenant_id.into(),
                prepared.record.id.into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE forum_replies \
             SET deleted_at = ?, updated_at = ? \
             WHERE tenant_id = ? AND id = ? AND status = 'deleted' AND deleted_at IS NULL",
            vec![
                deleted_at.into(),
                deleted_at.into(),
                tenant_id.into(),
                prepared.record.id.into(),
            ],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum import reply tombstones do not support database backend {backend:?}"
            )));
        }
    };
    if txn.execute_raw(statement).await?.rows_affected() != 1 {
        return Err(ForumError::Validation(
            "Forum import reply tombstone update did not claim exactly one reply".to_string(),
        ));
    }

    let after = count_import_delete_revisions_in_tx(txn, tenant_id, prepared.record.id).await?;
    if after != 1 {
        return Err(ForumError::Validation(
            "Forum import reply tombstone must create exactly one delete revision".to_string(),
        ));
    }

    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE forum_reply_revisions \
             SET created_at = $1 \
             WHERE tenant_id = $2 AND reply_id = $3 AND revision_reason = 'delete'",
            vec![
                deleted_at.into(),
                tenant_id.into(),
                prepared.record.id.into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE forum_reply_revisions \
             SET created_at = ? \
             WHERE tenant_id = ? AND reply_id = ? AND revision_reason = 'delete'",
            vec![
                deleted_at.into(),
                tenant_id.into(),
                prepared.record.id.into(),
            ],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum import reply tombstones do not support database backend {backend:?}"
            )));
        }
    };
    if txn.execute_raw(statement).await?.rows_affected() != 1 {
        return Err(ForumError::Validation(
            "Forum import reply delete revision retimestamp did not affect exactly one row"
                .to_string(),
        ));
    }

    Ok(())
}

async fn count_import_delete_revisions_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    reply_id: Uuid,
) -> ForumResult<i64> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT COUNT(*) AS revision_count \
             FROM forum_reply_revisions \
             WHERE tenant_id = $1 AND reply_id = $2 AND revision_reason = 'delete'",
            vec![tenant_id.into(), reply_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) AS revision_count \
             FROM forum_reply_revisions \
             WHERE tenant_id = ? AND reply_id = ? AND revision_reason = 'delete'",
            vec![tenant_id.into(), reply_id.into()],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum import reply tombstones do not support database backend {backend:?}"
            )));
        }
    };
    let row = txn.query_one_raw(statement).await?.ok_or_else(|| {
        ForumError::Validation("Forum import reply revision count returned no row".to_string())
    })?;
    Ok(row.try_get("", "revision_count")?)
}

pub(crate) struct PreparedImportReplyInsert {
    record: crate::import_write_preparation::ForumPreparedImportReply,
    document: rustok_api::RichTextDocument,
    stored_body: String,
    created_at: chrono::DateTime<Utc>,
}

impl PreparedImportReplyInsert {
    pub(crate) fn id(&self) -> Uuid {
        self.record.id
    }

    pub(crate) fn topic_id(&self) -> Uuid {
        self.record.topic_id
    }

    pub(crate) fn status(&self) -> ReplyStatus {
        self.record.status
    }

    pub(crate) fn created_at(&self) -> chrono::DateTime<Utc> {
        self.created_at
    }
}

impl ReplyService {
    pub(crate) fn prepare_import_reply(
        &self,
        tenant_id: Uuid,
        record: &crate::import_write_preparation::ForumPreparedImportReply,
    ) -> ForumResult<PreparedImportReplyInsert> {
        if tenant_id.is_nil() || record.id.is_nil() || record.topic_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum import reply preparation requires non-nil tenant, reply and topic IDs"
                    .to_string(),
            ));
        }
        if record.status == ReplyStatus::Deleted {
            return Err(ForumError::Validation(
                "Forum import deleted reply requires an admitted tombstone timestamp before persistence"
                    .to_string(),
            ));
        }
        let locale = normalize_locale(&record.locale)?;
        if locale != record.locale {
            return Err(ForumError::Validation(
                "Forum import reply locale must already be normalized".to_string(),
            ));
        }
        let document = crate::richtext::normalize_discussion(record.content.clone())?;
        let stored_body = crate::richtext::serialize_discussion(document.clone())?;
        let created_at = chrono::DateTime::<Utc>::from_timestamp_millis(record.created_at_ms)
            .ok_or_else(|| {
                ForumError::Validation(
                    "Forum import reply creation timestamp is outside owner range".to_string(),
                )
            })?;

        Ok(PreparedImportReplyInsert {
            record: record.clone(),
            document,
            stored_body,
            created_at,
        })
    }

    pub(crate) async fn insert_import_reply_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        prepared: &PreparedImportReplyInsert,
        relation: &crate::import_relation_preparation::ForumPreparedImportContentRelations,
        relation_event_mode: crate::import_relation_preparation::ForumImportRelationEventMode,
        write_event_mode: crate::import_write_preparation::ForumImportWriteEventMode,
    ) -> ForumResult<()> {
        if relation.source != prepared.record.source
            || relation.target != ForumContentTarget::reply(prepared.record.id)
            || relation.locale != prepared.record.locale
        {
            return Err(ForumError::Validation(
                "Forum import reply relation admission does not match prepared reply".to_string(),
            ));
        }

        let topic =
            TopicService::find_topic_in_tx(txn, tenant_id, prepared.record.topic_id).await?;
        if topic.status != TopicStatus::Open || topic.is_locked {
            return Err(ForumError::Validation(
                "Forum import reply insertion requires the private provisional topic state"
                    .to_string(),
            ));
        }

        if let Some(parent_reply_id) = prepared.record.parent_reply_id {
            let parent =
                reply::ReplyService::find_reply_in_tx(txn, tenant_id, parent_reply_id).await?;
            if parent.topic_id != prepared.record.topic_id {
                return Err(ForumError::Validation(
                    "Forum import reply parent belongs to another topic".to_string(),
                ));
            }
            if parent.status == ReplyStatus::Deleted {
                return Err(ForumError::Validation(
                    "Forum import reply cannot use a deleted parent without tombstone admission"
                        .to_string(),
                ));
            }
        }

        let position =
            allocate_reply_position_in_tx(txn, tenant_id, prepared.record.topic_id).await?;
        let author_id = prepared.record.author.as_ref().map(|author| author.user_id);

        forum_reply::ActiveModel {
            id: Set(prepared.record.id),
            tenant_id: Set(tenant_id),
            topic_id: Set(prepared.record.topic_id),
            author_id: Set(author_id),
            parent_reply_id: Set(prepared.record.parent_reply_id),
            status: Set(prepared.record.status),
            position: Set(position),
            created_at: Set(prepared.created_at.into()),
            updated_at: Set(prepared.created_at.into()),
        }
        .insert(txn)
        .await?;

        forum_reply_body::ActiveModel {
            id: Set(Uuid::new_v4()),
            reply_id: Set(prepared.record.id),
            tenant_id: Set(tenant_id),
            locale: Set(prepared.record.locale.clone()),
            body: Set(prepared.stored_body.clone()),
            created_at: Set(prepared.created_at.into()),
            updated_at: Set(prepared.created_at.into()),
        }
        .insert(txn)
        .await?;

        self.relations
            .persist_import_admitted_in_tx(
                txn,
                tenant_id,
                relation,
                &prepared.document,
                author_id,
                relation_event_mode,
            )
            .await?;

        if prepared.record.status == ReplyStatus::Approved {
            TopicService::adjust_reply_count_in_tx(txn, tenant_id, prepared.record.topic_id, 1)
                .await?;
            CategoryService::adjust_counters_in_tx(txn, tenant_id, topic.category_id, 0, 1).await?;
            UserStatsService::adjust_reply_count_in_tx(txn, tenant_id, author_id, 1).await?;

            if write_event_mode
                == crate::import_write_preparation::ForumImportWriteEventMode::EmitDomainEvents
            {
                self.event_bus
                    .publish_in_tx(
                        txn,
                        tenant_id,
                        author_id,
                        DomainEvent::ForumTopicReplied {
                            topic_id: prepared.record.topic_id,
                            reply_id: prepared.record.id,
                            author_id,
                        },
                    )
                    .await?;
            }
        }

        Ok(())
    }
}

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, Statement, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, PortContext, Resource};
use rustok_content::normalize_locale_code;
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{
    ListRepliesFilter, ReplyListItem, ReplyPage, ReplyResponse, bounded_forum_read_limit,
};
use crate::entities::{forum_reply, forum_reply_body, forum_solution, forum_topic_merge_operation};
use crate::error::{ForumError, ForumResult};
use crate::mentions::ForumContentTarget;
use crate::services::engagement_mode::ForumSettingsProviders;
use crate::state_machine::{ReplyStatus, TopicStatus};

use super::category::CategoryService;
use super::category_audience::lock_category_tree_in_tx;
use super::category_lifecycle::ensure_category_restore_target_is_active_in_tx;
use super::mention_relation::MentionRelationService;
use super::projection_invalidation::{
    publish_forum_category_projection_in_tx, publish_forum_topic_projection_in_tx,
};
use super::rbac::{DeleteSnapshotReason, enforce_author_deletion_policy, enforce_owned_scope, enforce_scope};
use super::reply;
use super::reply_create_audience_authorization::ForumReplyCreateAudienceAuthorizationService;
use super::topic_owner::TopicService;
use super::topic_reply_create_audience::lock_topic_reply_create_audience_in_tx;
use super::user_stats::UserStatsService;

/// Exact Forum-owned facts produced by the transactional reply removal path.
///
/// The state/counter/solution mutation stays inside `ReplyService`; callers may
/// use these facts only to publish the established owner event/projection in the
/// same transaction.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReplyRemovalOutcome {
    pub(crate) topic_id: Uuid,
    pub(crate) category_id: Uuid,
    pub(crate) old_status: ReplyStatus,
    pub(crate) was_public: bool,
    pub(crate) solution_removed: bool,
}

/// Public owner service for reply commands.
///
/// Root-service lifecycle decisions live here so database triggers are
/// invariant guards rather than the primary workflow engine.
pub struct ReplyService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    relations: MentionRelationService,
    settings: ForumSettingsProviders,
    inner: reply::ReplyService,
}

impl ReplyService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            inner: reply::ReplyService::new(db.clone(), event_bus.clone()),
            relations: MentionRelationService::new(db.clone()),
            settings: ForumSettingsProviders::default(),
            db,
            event_bus,
        }
    }

    pub fn with_settings_providers(mut self, settings: ForumSettingsProviders) -> Self {
        self.inner = self.inner.with_settings_providers(settings.clone());
        self.settings = settings;
        self
    }

    #[instrument(skip(self, security))]
    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        reply_id: Uuid,
        locale: &str,
    ) -> ForumResult<ReplyResponse> {
        self.get_with_locale_fallback(tenant_id, security, reply_id, locale, None)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn get_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        reply_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<ReplyResponse> {
        self.inner
            .get_with_locale_fallback(tenant_id, security, reply_id, locale, fallback_locale)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn list_for_topic_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        mut filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
    ) -> ForumResult<ReplyPage<ReplyListItem>> {
        filter.per_page = bounded_forum_read_limit(Some(filter.per_page));
        self.inner
            .list_for_topic_with_locale_fallback(
                tenant_id,
                security,
                topic_id,
                filter,
                fallback_locale,
            )
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn list_response_for_topic_by_statuses_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        mut filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
        statuses: Option<&[ReplyStatus]>,
    ) -> ForumResult<ReplyPage<ReplyResponse>> {
        filter.per_page = bounded_forum_read_limit(Some(filter.per_page));
        self.inner
            .list_response_for_topic_by_statuses_with_locale_fallback(
                tenant_id,
                security,
                topic_id,
                filter,
                fallback_locale,
                statuses,
            )
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        if tenant_id.is_nil() {
            return Err(ForumError::Validation("tenant_id must not be nil".into()));
        }
        if reply_id.is_nil() {
            return Err(ForumError::Validation("reply_id must not be nil".into()));
        }
        let existing = self.find_reply(tenant_id, reply_id).await?;
        enforce_owned_scope(
            &security,
            Resource::ForumReplies,
            Action::Delete,
            existing.author_id,
        )?;
        let settings = self.settings.module_settings(tenant_id).await?;
        let delete_reason = enforce_author_deletion_policy(
            &security,
            Resource::ForumReplies,
            settings.allow_user_content_deletion,
        )?;

        let txn = self.db.begin().await?;
        let outcome = Self::remove_in_tx(&txn, tenant_id, reply_id, delete_reason).await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ForumReplyStatusChanged {
                    reply_id,
                    topic_id: outcome.topic_id,
                    old_status: outcome.old_status.to_string(),
                    new_status: ReplyStatus::Deleted.to_string(),
                    moderator_id: security.user_id,
                },
            )
            .await?;
        if outcome.was_public {
            publish_forum_category_projection_in_tx(
                &self.event_bus,
                &txn,
                tenant_id,
                security.user_id,
                outcome.category_id,
            )
            .await?;
        }
        if outcome.was_public || outcome.solution_removed {
            publish_forum_topic_projection_in_tx(
                &self.event_bus,
                &txn,
                tenant_id,
                security.user_id,
                outcome.topic_id,
            )
            .await?;
        }

        txn.commit().await?;
        Ok(())
    }

    /// Restore a previously soft-deleted reply from its immutable delete snapshot.
    ///
    /// Deleted remains terminal in the ordinary state machine; restore is an
    /// explicit lifecycle inverse and does not loosen normal moderation transitions.
    #[instrument(skip(self, security))]
    pub async fn restore(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        if tenant_id.is_nil() {
            return Err(ForumError::Validation("tenant_id must not be nil".into()));
        }
        if reply_id.is_nil() {
            return Err(ForumError::Validation("reply_id must not be nil".into()));
        }
        enforce_scope(&security, Resource::ForumReplies, Action::Manage)?;

        let initial_reply = self.inner.find_reply(tenant_id, reply_id).await?;
        let initial_topic_id = initial_reply.topic_id;

        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        let topic =
            TopicService::find_topic_for_update_in_tx(&txn, tenant_id, initial_topic_id).await?;
        let reply = Self::find_reply_for_update_in_tx(&txn, tenant_id, reply_id).await?;
        if reply.topic_id != initial_topic_id {
            return Err(ForumError::TopicUpdateConflict(initial_topic_id));
        }
        if reply.status != ReplyStatus::Deleted {
            return Err(ForumError::ReplyRestoreUnavailable(reply_id));
        }
        if forum_topic_merge_operation::Entity::find()
            .filter(forum_topic_merge_operation::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_merge_operation::Column::SourceTopicId.eq(topic.id))
            .one(&txn)
            .await?
            .is_some()
        {
            return Err(ForumError::ReplyRestoreUnavailable(reply_id));
        }
        ensure_category_restore_target_is_active_in_tx(&txn, tenant_id, topic.category_id).await?;
        let snapshot = load_reply_delete_snapshot_in_tx(&txn, tenant_id, reply_id)
            .await?
            .ok_or(ForumError::ReplyRestoreUnavailable(reply_id))?;

        if snapshot.solution_marked_at.is_some()
            && forum_solution::Entity::find()
                .filter(forum_solution::Column::TenantId.eq(tenant_id))
                .filter(forum_solution::Column::TopicId.eq(topic.id))
                .one(&txn)
                .await?
                .is_some()
        {
            return Err(ForumError::ReplyRestoreUnavailable(reply_id));
        }

        restore_reply_from_delete_snapshot_in_tx(&txn, tenant_id, &reply, &snapshot).await?;

        if snapshot.previous_status == ReplyStatus::Approved {
            TopicService::adjust_reply_count_in_tx(&txn, tenant_id, reply.topic_id, 1).await?;
            CategoryService::adjust_counters_in_tx(&txn, tenant_id, topic.category_id, 0, 1)
                .await?;
            UserStatsService::adjust_reply_count_in_tx(&txn, tenant_id, snapshot.author_id, 1)
                .await?;
        }

        if let Some(marked_at) = snapshot.solution_marked_at.as_deref() {
            let marked_at = marked_at
                .parse::<chrono::DateTime<chrono::FixedOffset>>()
                .map_err(|_| ForumError::ReplyRestoreUnavailable(reply_id))?;
            restore_reply_solution_in_tx(
                &txn,
                tenant_id,
                topic.id,
                reply.id,
                snapshot.solution_marked_by_user_id,
                marked_at,
            )
            .await?;
            UserStatsService::adjust_solution_count_in_tx(&txn, tenant_id, snapshot.author_id, 1)
                .await?;
        }

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ForumReplyStatusChanged {
                    reply_id,
                    topic_id: topic.id,
                    old_status: ReplyStatus::Deleted.to_string(),
                    new_status: snapshot.previous_status.to_string(),
                    moderator_id: security.user_id,
                },
            )
            .await?;

        if snapshot.previous_status == ReplyStatus::Approved {
            publish_forum_category_projection_in_tx(
                &self.event_bus,
                &txn,
                tenant_id,
                security.user_id,
                topic.category_id,
            )
            .await?;
        }
        if snapshot.previous_status == ReplyStatus::Approved
            || snapshot.solution_marked_at.is_some()
        {
            publish_forum_topic_projection_in_tx(
                &self.event_bus,
                &txn,
                tenant_id,
                security.user_id,
                topic.id,
            )
            .await?;
        }

        clear_reply_delete_snapshot_in_tx(&txn, tenant_id, reply_id).await?;

        txn.commit().await?;
        Ok(())
    }

    /// Applies the complete Forum-owned reply removal mutation inside an
    /// existing owner transaction.
    ///
    /// This is the single state path for soft-delete/tombstone capture,
    /// accepted-solution cleanup and public/author/solution accounting. It does
    /// not perform authorization or publish events; the caller must publish the
    /// established `ForumReplyStatusChanged`, category projection, and topic
    /// projection in this same transaction using the returned facts.
    pub(crate) async fn remove_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
        reason: DeleteSnapshotReason,
    ) -> ForumResult<ReplyRemovalOutcome> {
        // Keep reply deletion behind the same category-tree lifecycle boundary as
        // topic delete/restore and category archive/restore. This prevents a reply
        // counter mutation from racing a concurrent category lifecycle decision.
        lock_category_tree_in_tx(txn, tenant_id).await?;
        let initial_reply = Self::find_reply_in_tx(txn, tenant_id, reply_id).await?;
        let initial_topic_id = initial_reply.topic_id;

        // All reply-moving owners lock the topic before reply rows. Keep deletion in
        // the same topic -> reply order so concurrent lifecycle mutations cannot
        // deadlock on inverted row-lock acquisition.
        let topic =
            TopicService::find_topic_for_update_in_tx(txn, tenant_id, initial_topic_id).await?;
        let reply =
            reply::ReplyService::find_reply_for_update_in_tx(txn, tenant_id, reply_id).await?;
        if reply.topic_id != initial_topic_id {
            return Err(ForumError::TopicUpdateConflict(initial_topic_id));
        }
        if reply.status == ReplyStatus::Deleted {
            return Err(ForumError::ReplyDeleted);
        }
        reply.status.validate_transition(&ReplyStatus::Deleted)?;
        let solution = forum_solution::Entity::find()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(reply.topic_id))
            .one(txn)
            .await?;
        let solution_for_snapshot = solution
            .as_ref()
            .filter(|solution| solution.reply_id == reply_id);
        let solution_removed = solution_for_snapshot.is_some();

        record_reply_delete_snapshot_in_tx(txn, tenant_id, &reply, solution_for_snapshot, reason)
            .await?;

        forum_solution::Entity::delete_many()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::ReplyId.eq(reply_id))
            .exec(txn)
            .await?;
        mark_reply_deleted_in_tx(txn, tenant_id, reply_id).await?;

        let was_public = reply.status == ReplyStatus::Approved;
        if was_public {
            TopicService::adjust_reply_count_in_tx(txn, tenant_id, reply.topic_id, -1).await?;
            CategoryService::adjust_counters_in_tx(txn, tenant_id, topic.category_id, 0, -1)
                .await?;
            UserStatsService::adjust_reply_count_in_tx(txn, tenant_id, reply.author_id, -1).await?;
        }
        if solution_removed {
            UserStatsService::adjust_solution_count_in_tx(txn, tenant_id, reply.author_id, -1)
                .await?;
        }

        Ok(ReplyRemovalOutcome {
            topic_id: reply.topic_id,
            category_id: topic.category_id,
            old_status: reply.status,
            was_public,
            solution_removed,
        })
    }

    pub(crate) async fn find_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<forum_reply::Model> {
        self.inner.find_reply(tenant_id, reply_id).await
    }

    pub(crate) async fn find_reply_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<forum_reply::Model> {
        reply::ReplyService::find_reply_in_tx(txn, tenant_id, reply_id).await
    }

    pub(crate) async fn find_reply_for_update_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<forum_reply::Model> {
        reply::ReplyService::find_reply_for_update_in_tx(txn, tenant_id, reply_id).await
    }

    pub(crate) async fn set_status_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
        status: ReplyStatus,
    ) -> ForumResult<forum_reply::Model> {
        reply::ReplyService::set_status_in_tx(txn, tenant_id, reply_id, status).await
    }
}

fn normalize_locale(locale: &str) -> ForumResult<String> {
    normalize_locale_code(locale)
        .ok_or_else(|| ForumError::Validation("Invalid locale".to_string()))
}

async fn allocate_reply_position_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<i64> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            // FORUM-07/FORUM-08B PostgreSQL triggers replace this provisional
            // value with the monotonic per-topic allocation before INSERT.
            Ok(1)
        }
        DatabaseBackend::Sqlite => {
            let row = txn
                .query_one_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "UPDATE forum_topics \
                     SET next_reply_position = next_reply_position + 1 \
                     WHERE tenant_id = ? AND id = ? \
                     RETURNING next_reply_position - 1 AS position",
                    vec![tenant_id.into(), topic_id.into()],
                ))
                .await?
                .ok_or(ForumError::TopicNotFound(topic_id))?;
            Ok(row.try_get("", "position")?)
        }
        backend => Err(ForumError::Validation(format!(
            "Unsupported forum database backend: {backend:?}"
        ))),
    }
}

#[derive(Clone, Debug)]
struct ReplyDeleteSnapshot {
    topic_id: Uuid,
    author_id: Option<Uuid>,
    previous_status: ReplyStatus,
    solution_marked_by_user_id: Option<Uuid>,
    solution_marked_at: Option<String>,
}

async fn record_reply_delete_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    reply: &forum_reply::Model,
    solution: Option<&forum_solution::Model>,
    reason: DeleteSnapshotReason,
) -> ForumResult<()> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            INSERT INTO forum_reply_delete_snapshots (
                tenant_id, reply_id, topic_id, author_id,
                previous_status, solution_marked_by_user_id, solution_marked_at, reason, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, CURRENT_TIMESTAMP)
            "#,
            vec![
                tenant_id.into(),
                reply.id.into(),
                reply.topic_id.into(),
                reply.author_id.into(),
                reply.status.to_string().into(),
                solution.and_then(|value| value.marked_by_user_id).into(),
                solution.map(|value| value.marked_at.to_rfc3339()).into(),
                reason.as_str().into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            INSERT INTO forum_reply_delete_snapshots (
                tenant_id, reply_id, topic_id, author_id,
                previous_status, solution_marked_by_user_id, solution_marked_at, reason, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            "#,
            vec![
                tenant_id.into(),
                reply.id.into(),
                reply.topic_id.into(),
                reply.author_id.into(),
                reply.status.to_string().into(),
                solution.and_then(|value| value.marked_by_user_id).into(),
                solution.map(|value| value.marked_at.to_rfc3339()).into(),
                reason.as_str().into(),
            ],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum reply delete snapshots do not support database backend {backend:?}"
            )));
        }
    };
    txn.execute_raw(statement).await?;
    Ok(())
}

async fn load_reply_delete_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    reply_id: Uuid,
) -> ForumResult<Option<ReplyDeleteSnapshot>> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT topic_id, author_id, previous_status,
                   solution_marked_by_user_id, solution_marked_at
            FROM forum_reply_delete_snapshots
            WHERE tenant_id = $1 AND reply_id = $2
            "#,
            vec![tenant_id.into(), reply_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            SELECT topic_id, author_id, previous_status,
                   solution_marked_by_user_id, solution_marked_at
            FROM forum_reply_delete_snapshots
            WHERE tenant_id = ? AND reply_id = ?
            "#,
            vec![tenant_id.into(), reply_id.into()],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum reply restore does not support database backend {backend:?}"
            )));
        }
    };
    let Some(row) = txn.query_one_raw(statement).await? else {
        return Ok(None);
    };
    let status_value: String = row.try_get("", "previous_status")?;
    let previous_status = ReplyStatus::from_str_value(&status_value)
        .filter(|status| *status != ReplyStatus::Deleted)
        .ok_or(ForumError::ReplyRestoreUnavailable(reply_id))?;
    Ok(Some(ReplyDeleteSnapshot {
        topic_id: row.try_get("", "topic_id")?,
        author_id: row.try_get("", "author_id")?,
        previous_status,
        solution_marked_by_user_id: row.try_get("", "solution_marked_by_user_id")?,
        solution_marked_at: row.try_get("", "solution_marked_at")?,
    }))
}

async fn restore_reply_from_delete_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    reply: &forum_reply::Model,
    snapshot: &ReplyDeleteSnapshot,
) -> ForumResult<()> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            UPDATE forum_replies
            SET status = $3, deleted_at = NULL, updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = $1 AND topic_id = $2 AND id = $4 AND deleted_at IS NOT NULL
            "#,
            vec![
                tenant_id.into(),
                snapshot.topic_id.into(),
                snapshot.previous_status.to_string().into(),
                reply.id.into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            UPDATE forum_replies
            SET status = ?, deleted_at = NULL, updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = ? AND topic_id = ? AND id = ? AND deleted_at IS NOT NULL
            "#,
            vec![
                snapshot.previous_status.to_string().into(),
                tenant_id.into(),
                snapshot.topic_id.into(),
                reply.id.into(),
            ],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum reply restore does not support database backend {backend:?}"
            )));
        }
    };
    let result = txn.execute_raw(statement).await?;
    if result.rows_affected() != 1 {
        return Err(ForumError::ReplyRestoreUnavailable(reply.id));
    }
    Ok(())
}

async fn restore_reply_solution_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    reply_id: Uuid,
    marked_by_user_id: Option<Uuid>,
    marked_at: chrono::DateTime<chrono::FixedOffset>,
) -> ForumResult<()> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            INSERT INTO forum_solutions (
                tenant_id, topic_id, reply_id, marked_by_user_id, marked_at
            )
            VALUES ($1, $2, $3, $4, $5)
            "#,
            vec![
                tenant_id.into(),
                topic_id.into(),
                reply_id.into(),
                marked_by_user_id.into(),
                marked_at.into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            INSERT INTO forum_solutions (
                tenant_id, topic_id, reply_id, marked_by_user_id, marked_at
            )
            VALUES (?, ?, ?, ?, ?)
            "#,
            vec![
                tenant_id.into(),
                topic_id.into(),
                reply_id.into(),
                marked_by_user_id.into(),
                marked_at.into(),
            ],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum reply restore does not support database backend {backend:?}"
            )));
        }
    };
    txn.execute_raw(statement).await?;
    Ok(())
}

async fn clear_reply_delete_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    reply_id: Uuid,
) -> ForumResult<()> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "DELETE FROM forum_reply_delete_snapshots WHERE tenant_id = $1 AND reply_id = $2",
            vec![tenant_id.into(), reply_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "DELETE FROM forum_reply_delete_snapshots WHERE tenant_id = ? AND reply_id = ?",
            vec![tenant_id.into(), reply_id.into()],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum reply restore does not support database backend {backend:?}"
            )));
        }
    };
    txn.execute_raw(statement).await?;
    Ok(())
}

async fn mark_reply_deleted_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    reply_id: Uuid,
) -> ForumResult<()> {
    let statement = tenant_scoped_reply_statement(
        txn.get_database_backend(),
        "UPDATE forum_replies \
         SET status = 'deleted', deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
         WHERE tenant_id = ? AND id = ? AND deleted_at IS NULL",
        "UPDATE forum_replies \
         SET status = 'deleted', deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        tenant_id,
        reply_id,
    )?;
    let result = txn.execute_raw(statement).await?;
    if result.rows_affected() != 1 {
        return Err(ForumError::ReplyDeleted);
    }
    Ok(())
}

fn tenant_scoped_reply_statement(
    backend: DatabaseBackend,
    sqlite_sql: &str,
    postgres_sql: &str,
    tenant_id: Uuid,
    reply_id: Uuid,
) -> ForumResult<Statement> {
    match backend {
        DatabaseBackend::Postgres => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            postgres_sql,
            vec![tenant_id.into(), reply_id.into()],
        )),
        DatabaseBackend::Sqlite => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            sqlite_sql,
            vec![tenant_id.into(), reply_id.into()],
        )),
        backend => Err(ForumError::Validation(format!(
            "Forum reply mutations do not support database backend {backend:?}"
        ))),
    }
}

use crate::dto::{CreateReplyCommandInput, UpdateReplyCommandInput};
use crate::services::content_limits::ForumContentLimits;

impl ReplyService {
    pub(crate) const MAX_FORUM_REPLY_LOCALE_ENUMERATION_IDS: usize = 512;

    pub(crate) async fn available_locales_for_replies(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        reply_ids: &[Uuid],
    ) -> ForumResult<Vec<(Uuid, Vec<String>)>> {
        enforce_scope(&security, Resource::ForumReplies, Action::Manage)?;
        self.inner
            .available_locales_for_replies(tenant_id, security, reply_ids)
            .await
    }

    #[instrument(skip(self, security, input))]
    pub async fn create_command(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        input: CreateReplyCommandInput,
    ) -> ForumResult<ReplyResponse> {
        let create_audience =
            ForumReplyCreateAudienceAuthorizationService::without_facts_provider(self.db.clone());
        self.create_command_with_audience_authorization(
            tenant_id,
            security,
            topic_id,
            None,
            input,
            &create_audience,
        )
        .await
    }

    pub(crate) async fn create_command_with_audience_authorization(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        context: Option<PortContext>,
        input: CreateReplyCommandInput,
        create_audience: &ForumReplyCreateAudienceAuthorizationService,
    ) -> ForumResult<ReplyResponse> {
        let (input, quote_inputs) = input.into_parts();
        enforce_scope(&security, Resource::ForumReplies, Action::Create)?;
        let locale = normalize_locale(&input.locale)?;
        let document = crate::richtext::normalize_discussion(input.content)?;
        let content_limits = ForumContentLimits::resolve(&self.settings, tenant_id).await?;
        content_limits
            .validate_body(&crate::richtext::project_discussion(document.clone())?.plain_text)?;
        let stored_body = crate::richtext::serialize_discussion(document.clone())?;
        // UUIDv7: the reply list keyset orders by (created_at, id), so ids must be time-ordered.
        let reply_id = Uuid::now_v7();
        let quotes = super::relation_quote_input::normalize_quote_inputs(quote_inputs)?;
        let prepared_relations = self
            .relations
            .prepare(
                tenant_id,
                ForumContentTarget::reply(reply_id),
                &locale,
                &document,
                &security,
                quotes,
            )
            .await?;

        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        lock_topic_reply_create_audience_in_tx(&txn, tenant_id, topic_id).await?;
        let topic = TopicService::find_topic_for_update_in_tx(&txn, tenant_id, topic_id).await?;
        let category =
            CategoryService::find_category_for_update_in_tx(&txn, tenant_id, topic.category_id)
                .await?;

        let authorization = create_audience
            .require_in_tx(&txn, tenant_id, &topic, &security, context)
            .await?;
        if !authorization.allowed {
            return Err(ForumError::forbidden(
                "Forum reply creation is unavailable for the current audience",
            ));
        }

        match topic.status {
            TopicStatus::Closed => return Err(ForumError::TopicClosed),
            TopicStatus::Archived => return Err(ForumError::TopicArchived),
            TopicStatus::Pending => return Err(ForumError::TopicAwaitingModeration),
            TopicStatus::Open => {}
        }
        if topic.is_locked {
            return Err(ForumError::TopicLocked);
        }

        if let Some(parent_reply_id) = input.parent_reply_id {
            let parent =
                reply::ReplyService::find_reply_in_tx(&txn, tenant_id, parent_reply_id).await?;
            if parent.topic_id != topic_id {
                return Err(ForumError::Validation(
                    "Parent reply belongs to another topic".to_string(),
                ));
            }
            if parent.status == ReplyStatus::Deleted {
                return Err(ForumError::Validation(
                    "Deleted reply cannot be used as a parent".to_string(),
                ));
            }
            if parent.status != ReplyStatus::Approved {
                // A public reply under a held, rejected or hidden parent
                // would surface in the thread while its parent is redacted,
                // producing orphan fragments. Only approved parents can
                // acquire children.
                return Err(ForumError::Validation(
                    "Only approved replies can be used as a parent".to_string(),
                ));
            }
            // Bound nesting depth. The loop is depth-capped, so even a
            // corrupt cyclic parent chain terminates with this error
            // instead of walking forever inside the transaction.
            let mut depth = 1_u32;
            let mut ancestor_id = parent.parent_reply_id;
            while let Some(ancestor) = ancestor_id {
                depth += 1;
                if depth > crate::constants::MAX_FORUM_REPLY_NESTING_DEPTH {
                    return Err(ForumError::Validation(format!(
                        "Forum reply nesting exceeds {} levels",
                        crate::constants::MAX_FORUM_REPLY_NESTING_DEPTH
                    )));
                }
                let ancestor_reply =
                    reply::ReplyService::find_reply_in_tx(&txn, tenant_id, ancestor).await?;
                if ancestor_reply.topic_id != topic_id {
                    return Err(ForumError::Validation(
                        "Parent reply belongs to another topic".to_string(),
                    ));
                }
                ancestor_id = ancestor_reply.parent_reply_id;
            }
        }

        super::posting_rate::enforce_new_reply_rate_in_tx(
            &txn,
            tenant_id,
            &security,
            content_limits.new_reply_rate_limit_seconds(),
        )
        .await?;
        let position = allocate_reply_position_in_tx(&txn, tenant_id, topic_id).await?;
        // Pre-moderation holds a new reply when the category or the tenant requires review. Tenant
        // pre-moderation also holds new topics, see `TopicService::create_with_inline_relations`.
        let pre_moderation_enabled = self
            .settings
            .module_settings_in_tx(&txn, tenant_id)
            .await?
            .pre_moderation_enabled;
        let status = if category.moderated || pre_moderation_enabled {
            ReplyStatus::Pending
        } else {
            ReplyStatus::Approved
        };
        let now = Utc::now();

        forum_reply::ActiveModel {
            id: Set(reply_id),
            tenant_id: Set(tenant_id),
            topic_id: Set(topic_id),
            author_id: Set(security.user_id),
            parent_reply_id: Set(input.parent_reply_id),
            status: Set(status),
            position: Set(position),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        forum_reply_body::ActiveModel {
            id: Set(Uuid::new_v4()),
            reply_id: Set(reply_id),
            tenant_id: Set(tenant_id),
            locale: Set(locale.clone()),
            body: Set(stored_body),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        self.relations
            .persist_in_tx(&txn, prepared_relations)
            .await?;

        if status == ReplyStatus::Approved {
            super::subscription::auto_subscribe::auto_subscribe_reply_participant_in_tx(
                &txn,
                tenant_id,
                topic_id,
                security.user_id,
            )
            .await?;
            TopicService::adjust_reply_count_in_tx(&txn, tenant_id, topic_id, 1).await?;
            CategoryService::adjust_counters_in_tx(&txn, tenant_id, topic.category_id, 0, 1)
                .await?;
            UserStatsService::adjust_reply_count_in_tx(&txn, tenant_id, security.user_id, 1)
                .await?;

            self.event_bus
                .publish_in_tx(
                    &txn,
                    tenant_id,
                    security.user_id,
                    DomainEvent::ForumTopicReplied {
                        topic_id,
                        reply_id,
                        author_id: security.user_id,
                    },
                )
                .await?;
        }

        txn.commit().await?;
        self.get(tenant_id, security, reply_id, &locale).await
    }

    #[instrument(skip(self, security, input))]
    pub async fn update_command(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
        input: UpdateReplyCommandInput,
    ) -> ForumResult<ReplyResponse> {
        self.inner
            .update_with_inline_relations(tenant_id, reply_id, security, input)
            .await
    }
}

