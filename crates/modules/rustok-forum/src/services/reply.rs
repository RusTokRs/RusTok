use std::collections::HashMap;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource};
use rustok_content::{
    available_locales_from, normalize_locale_code, resolve_by_locale_with_fallback,
};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{
    ListRepliesFilter, MAX_FORUM_READ_LIMIT, ReplyCursor, ReplyListItem, ReplyPage, ReplyResponse,
};
use crate::entities::{forum_reply, forum_reply_body, forum_solution};
use crate::error::{ForumError, ForumResult};
use crate::richtext::project_stored_discussion;
use crate::services::engagement_mode::ForumSettingsProviders;
use crate::services::pending_visibility::reply_owner_read_condition;
use crate::services::rbac::{enforce_owned_scope, enforce_scope};
use crate::services::vote::{VoteService, VoteSummary};
use crate::state_machine::ReplyStatus;

pub struct ReplyService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    settings: ForumSettingsProviders,
}

impl ReplyService {
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
        enforce_scope(&security, Resource::ForumReplies, Action::Read)?;
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let reply = self.find_reply(tenant_id, reply_id).await?;
        let bodies = self.load_bodies(tenant_id, reply_id).await?;
        let solution_reply_id = self
            .load_solution_reply_id_for_topic(tenant_id, reply.topic_id)
            .await?;
        let vote_summary = VoteService::new(self.db.clone())
            .with_settings_providers(self.settings.clone())
            .reply_vote_summary(tenant_id, reply_id, security.user_id)
            .await?;
        to_reply_response(
            reply,
            bodies,
            vote_summary,
            solution_reply_id,
            &locale,
            fallback_locale.as_deref(),
        )
    }

    #[instrument(skip(self, security))]
    pub async fn list_for_topic_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
    ) -> ForumResult<ReplyPage<ReplyListItem>> {
        enforce_scope(&security, Resource::ForumReplies, Action::List)?;
        let locale = filter
            .locale
            .clone()
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let locale = normalize_locale(&locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;

        let ReplyPage {
            items: replies,
            next_cursor,
        } = self
            .fetch_reply_page(tenant_id, topic_id, &filter, None, &security)
            .await?;
        let solution_reply_id = self
            .load_solution_reply_id_for_topic(tenant_id, topic_id)
            .await?;
        let reply_ids: Vec<Uuid> = replies.iter().map(|reply| reply.id).collect();
        let bodies_map = self.load_bodies_map(tenant_id, &reply_ids).await?;
        let vote_summaries = VoteService::new(self.db.clone())
            .with_settings_providers(self.settings.clone())
            .reply_vote_summaries(tenant_id, &reply_ids, security.user_id)
            .await?;

        let items = replies
            .into_iter()
            .map(|reply| {
                let bodies = bodies_map.get(&reply.id).cloned().unwrap_or_default();
                let resolved = resolve_reply_body(&bodies, &locale, fallback_locale.as_deref());
                let body = resolved.item.ok_or_else(|| {
                    ForumError::Validation("Forum reply body is unavailable".to_string())
                })?;
                let preview: String = project_stored_discussion(&body.body)?
                    .plain_text
                    .chars()
                    .take(200)
                    .collect();
                Ok(ReplyListItem {
                    id: reply.id,
                    locale: locale.clone(),
                    effective_locale: resolved.effective_locale,
                    available_locales: available_locales_from(&bodies, |body| body.locale.as_str()),
                    topic_id: reply.topic_id,
                    author_id: reply.author_id,
                    content_preview: preview,
                    status: reply.status.to_string(),
                    is_deleted: reply.status == ReplyStatus::Deleted,
                    vote_score: vote_summaries
                        .get(&reply.id)
                        .map(|summary| summary.score)
                        .unwrap_or_default(),
                    current_user_vote: vote_summaries
                        .get(&reply.id)
                        .and_then(|summary| summary.current_user_vote),
                    is_solution: Some(reply.id) == solution_reply_id,
                    parent_reply_id: reply.parent_reply_id,
                    created_at: reply.created_at.to_rfc3339(),
                })
            })
            .collect::<ForumResult<Vec<_>>>()?;

        Ok(ReplyPage { items, next_cursor })
    }

    #[instrument(skip(self, security))]
    pub async fn list_response_for_topic_by_statuses_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
        statuses: Option<&[ReplyStatus]>,
    ) -> ForumResult<ReplyPage<ReplyResponse>> {
        enforce_scope(&security, Resource::ForumReplies, Action::List)?;
        let locale = filter
            .locale
            .clone()
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let locale = normalize_locale(&locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let ReplyPage {
            items: replies,
            next_cursor,
        } = self
            .fetch_reply_page(tenant_id, topic_id, &filter, statuses, &security)
            .await?;
        let solution_reply_id = self
            .load_solution_reply_id_for_topic(tenant_id, topic_id)
            .await?;
        let reply_ids: Vec<Uuid> = replies.iter().map(|reply| reply.id).collect();
        let bodies_map = self.load_bodies_map(tenant_id, &reply_ids).await?;
        let vote_summaries = VoteService::new(self.db.clone())
            .with_settings_providers(self.settings.clone())
            .reply_vote_summaries(tenant_id, &reply_ids, security.user_id)
            .await?;

        let items = replies
            .into_iter()
            .map(|reply| {
                let reply_id = reply.id;
                to_reply_response(
                    reply,
                    bodies_map.get(&reply_id).cloned().unwrap_or_default(),
                    vote_summaries.get(&reply_id).copied().unwrap_or_default(),
                    solution_reply_id,
                    &locale,
                    fallback_locale.as_deref(),
                )
            })
            .collect::<ForumResult<Vec<_>>>()?;

        Ok(ReplyPage { items, next_cursor })
    }

    pub(crate) async fn find_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<forum_reply::Model> {
        Self::find_reply_in_conn(&self.db, tenant_id, reply_id).await
    }

    pub(crate) async fn find_reply_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<forum_reply::Model> {
        Self::find_reply_in_conn(txn, tenant_id, reply_id).await
    }

    pub(crate) async fn find_reply_for_update_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<forum_reply::Model> {
        let query = forum_reply::Entity::find_by_id(reply_id)
            .filter(forum_reply::Column::TenantId.eq(tenant_id));
        let reply = match txn.get_database_backend() {
            DatabaseBackend::Postgres => query.lock_exclusive().one(txn).await?,
            DatabaseBackend::Sqlite => query.one(txn).await?,
            backend => {
                return Err(ForumError::Validation(format!(
                    "Forum reply row locking does not support database backend {backend:?}"
                )));
            }
        };
        reply.ok_or(ForumError::ReplyNotFound(reply_id))
    }

    async fn find_reply_in_conn(
        conn: &impl sea_orm::ConnectionTrait,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<forum_reply::Model> {
        forum_reply::Entity::find_by_id(reply_id)
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .one(conn)
            .await?
            .ok_or(ForumError::ReplyNotFound(reply_id))
    }

    pub(crate) async fn set_status_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
        status: ReplyStatus,
    ) -> ForumResult<forum_reply::Model> {
        let reply = Self::find_reply_in_tx(txn, tenant_id, reply_id).await?;
        let mut active: forum_reply::ActiveModel = reply.clone().into();
        active.status = Set(status);
        active.updated_at = Set(Utc::now().into());
        active.update(txn).await?;
        Ok(reply)
    }

    async fn load_bodies(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<Vec<forum_reply_body::Model>> {
        Ok(forum_reply_body::Entity::find()
            .filter(forum_reply_body::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_body::Column::ReplyId.eq(reply_id))
            .all(&self.db)
            .await?)
    }

    async fn load_solution_reply_id_for_topic(
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

    /// Keyset page of replies ordered `(created_at ASC, id ASC)`. Fetches one extra row
    /// to learn whether another page exists, so the list never counts rows.
    ///
    /// Pending replies of other authors are excluded for every caller that is not a moderator,
    /// including callers that request explicit statuses.
    async fn fetch_reply_page(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        filter: &ListRepliesFilter,
        statuses: Option<&[ReplyStatus]>,
        security: &SecurityContext,
    ) -> ForumResult<ReplyPage<forum_reply::Model>> {
        // The 100-item cap must hold at the owner boundary, not only at DTO
        // deserialization: Rust callers can construct the filter directly.
        let per_page = filter.per_page.clamp(1, MAX_FORUM_READ_LIMIT);
        let mut query = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::TopicId.eq(topic_id))
            .filter(reply_owner_read_condition(security));

        if let Some(statuses) = statuses
            && !statuses.is_empty()
        {
            let mut condition = Condition::any();
            for status in statuses {
                condition = condition.add(forum_reply::Column::Status.eq(*status));
            }
            query = query.filter(condition);
        }

        if let Some(raw) = filter.after.as_deref() {
            let after = ReplyCursor::decode(raw)?;
            query = query.filter(
                Condition::any()
                    .add(forum_reply::Column::CreatedAt.gt(after.created_at))
                    .add(
                        Condition::all()
                            .add(forum_reply::Column::CreatedAt.eq(after.created_at))
                            .add(forum_reply::Column::Id.gt(after.id)),
                    ),
            );
        }

        let mut replies = query
            .order_by_asc(forum_reply::Column::CreatedAt)
            .order_by_asc(forum_reply::Column::Id)
            .limit(per_page + 1)
            .all(&self.db)
            .await?;

        let has_next_page = replies.len() as u64 > per_page;
        replies.truncate(per_page as usize);
        let next_cursor = if has_next_page {
            let last = replies.last().ok_or_else(|| {
                ForumError::Validation("Reply page reported a next page without rows".to_string())
            })?;
            Some(
                ReplyCursor {
                    created_at: last.created_at,
                    id: last.id,
                }
                .encode(),
            )
        } else {
            None
        };
        Ok(ReplyPage {
            items: replies,
            next_cursor,
        })
    }

    async fn load_bodies_map(
        &self,
        tenant_id: Uuid,
        reply_ids: &[Uuid],
    ) -> ForumResult<HashMap<Uuid, Vec<forum_reply_body::Model>>> {
        if reply_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = forum_reply_body::Entity::find()
            .filter(forum_reply_body::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_body::Column::ReplyId.is_in(reply_ids.to_vec()))
            .all(&self.db)
            .await?;
        let mut map: HashMap<Uuid, Vec<forum_reply_body::Model>> = HashMap::new();
        for row in rows {
            map.entry(row.reply_id).or_default().push(row);
        }
        Ok(map)
    }

    async fn upsert_body_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
        locale: &str,
        body: String,
    ) -> ForumResult<()> {
        let existing = forum_reply_body::Entity::find()
            .filter(forum_reply_body::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_body::Column::ReplyId.eq(reply_id))
            .filter(forum_reply_body::Column::Locale.eq(locale))
            .one(txn)
            .await?;
        let now = Utc::now();

        match existing {
            Some(existing) => {
                let mut active: forum_reply_body::ActiveModel = existing.into();
                active.body = Set(body);
                active.updated_at = Set(now.into());
                active.update(txn).await?;
            }
            None => {
                forum_reply_body::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    reply_id: Set(reply_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(locale.to_string()),
                    body: Set(body),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await?;
            }
        }

        Ok(())
    }
}

fn to_reply_response(
    reply: forum_reply::Model,
    bodies: Vec<forum_reply_body::Model>,
    vote_summary: VoteSummary,
    solution_reply_id: Option<Uuid>,
    locale: &str,
    fallback_locale: Option<&str>,
) -> ForumResult<ReplyResponse> {
    let resolved = resolve_reply_body(&bodies, locale, fallback_locale);
    let body = resolved
        .item
        .ok_or_else(|| ForumError::Validation("Forum reply body is unavailable".to_string()))?;
    let content = project_stored_discussion(&body.body)?;
    let (content, content_plain_text) =
        redact_non_public_reply_content(reply.status, content.view, content.plain_text);

    Ok(ReplyResponse {
        id: reply.id,
        requested_locale: locale.to_string(),
        locale: locale.to_string(),
        effective_locale: resolved.effective_locale,
        available_locales: available_locales_from(&bodies, |body| body.locale.as_str()),
        topic_id: reply.topic_id,
        author_id: reply.author_id,
        content,
        content_plain_text,
        status: reply.status.to_string(),
        is_deleted: reply.status == ReplyStatus::Deleted,
        vote_score: vote_summary.score,
        current_user_vote: vote_summary.current_user_vote,
        is_solution: Some(reply.id) == solution_reply_id,
        parent_reply_id: reply.parent_reply_id,
        created_at: reply.created_at.to_rfc3339(),
        updated_at: reply.updated_at.to_rfc3339(),
    })
}

fn normalize_locale(locale: &str) -> ForumResult<String> {
    normalize_locale_code(locale)
        .ok_or_else(|| ForumError::Validation("Invalid locale".to_string()))
}

/// Library-level redaction for reply content returned outside its visibility
/// guard: a non-`Approved` reply yields no stored body. Query-level filters
/// and per-item checks already keep non-public replies away from readers
/// without moderation scope; this guard additionally protects responses whose
/// status changed between read and response composition and any future owner
/// read path that forgets the status gate.
fn redact_non_public_reply_content(
    status: ReplyStatus,
    view: rustok_api::RichTextView,
    plain_text: String,
) -> (rustok_api::RichTextView, String) {
    if status == ReplyStatus::Approved {
        return (view, plain_text);
    }
    (
        rustok_api::RichTextView {
            document: rustok_api::RichTextDocument::empty(),
            html: String::new(),
        },
        String::new(),
    )
}

fn resolve_reply_body<'a>(
    bodies: &'a [forum_reply_body::Model],
    locale: &str,
    fallback_locale: Option<&str>,
) -> rustok_content::ResolvedLocale<'a, forum_reply_body::Model> {
    resolve_by_locale_with_fallback(bodies, locale, fallback_locale, |body| body.locale.as_str())
}

use std::collections::BTreeSet;

use super::edit_window::enforce_author_edit_window;
use crate::dto::UpdateReplyCommandInput;
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

        if tenant_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum reply locale enumeration requires a non-nil tenant id".to_string(),
            ));
        }
        if reply_ids.len() > Self::MAX_FORUM_REPLY_LOCALE_ENUMERATION_IDS {
            return Err(ForumError::Validation(format!(
                "Forum reply locale enumeration exceeds {} reply ids: {}",
                Self::MAX_FORUM_REPLY_LOCALE_ENUMERATION_IDS,
                reply_ids.len()
            )));
        }
        if reply_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut seen = BTreeSet::new();
        for reply_id in reply_ids {
            if reply_id.is_nil() {
                return Err(ForumError::Validation(
                    "Forum reply locale enumeration requires non-nil reply ids".to_string(),
                ));
            }
            if !seen.insert(*reply_id) {
                return Err(ForumError::Validation(format!(
                    "Forum reply locale enumeration contains duplicate reply id {reply_id}"
                )));
            }
        }

        let existing = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::Id.is_in(reply_ids.to_vec()))
            .all(&self.db)
            .await?;
        let existing_ids = existing
            .into_iter()
            .map(|reply| reply.id)
            .collect::<BTreeSet<_>>();
        for reply_id in reply_ids {
            if !existing_ids.contains(reply_id) {
                return Err(ForumError::ReplyNotFound(*reply_id));
            }
        }

        let mut bodies_by_reply = self.load_bodies_map(tenant_id, reply_ids).await?;
        let mut result = Vec::with_capacity(reply_ids.len());
        for reply_id in reply_ids {
            let bodies = bodies_by_reply.remove(reply_id).unwrap_or_default();
            let locales = available_locales_from(&bodies, |body| body.locale.as_str());
            if locales.is_empty() {
                return Err(ForumError::Validation(format!(
                    "Forum reply {reply_id} has no stored locale body"
                )));
            }
            result.push((*reply_id, locales));
        }

        Ok(result)
    }

    pub(crate) async fn update_with_inline_relations(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
        input: UpdateReplyCommandInput,
    ) -> ForumResult<ReplyResponse> {
        let (input, quote_inputs) = input.into_parts();
        let locale = normalize_locale(&input.locale)?;
        let existing = self.find_reply(tenant_id, reply_id).await?;
        // Optimistic-concurrency fence: the in-transaction re-read below
        // must still carry this `updated_at`, otherwise a concurrent edit
        // (or status transition) committed in between would be silently
        // overwritten by this update.
        let expected_updated_at = existing.updated_at;
        enforce_owned_scope(
            &security,
            Resource::ForumReplies,
            Action::Update,
            existing.author_id,
        )?;
        if existing.status == crate::state_machine::ReplyStatus::Deleted {
            // A deleted reply is immutable for its author: edits would
            // silently mutate content covered by the delete snapshot and
            // revive on restore with changes made after the tombstone.
            return Err(ForumError::ReplyNotFound(reply_id));
        }

        let max_edit_window_minutes = self
            .settings
            .module_settings(tenant_id)
            .await?
            .max_edit_window_minutes;
        enforce_author_edit_window(
            &security,
            Resource::ForumReplies,
            existing.author_id,
            existing.created_at,
            max_edit_window_minutes,
            chrono::Utc::now(),
        )?;
        let has_content_change = input.content.is_some();
        if !has_content_change && quote_inputs.is_none() {
            return self.get(tenant_id, security, reply_id, &locale).await;
        }

        let (document, stored_body) = if let Some(content) = input.content {
            let document = crate::richtext::normalize_discussion(content)?;
            ForumContentLimits::resolve(&self.settings, tenant_id)
                .await?
                .validate_body(
                    &crate::richtext::project_discussion(document.clone())?.plain_text,
                )?;
            let stored_body = crate::richtext::serialize_discussion(document.clone())?;
            (document, Some(stored_body))
        } else {
            let body = forum_reply_body::Entity::find()
                .filter(forum_reply_body::Column::TenantId.eq(tenant_id))
                .filter(forum_reply_body::Column::ReplyId.eq(reply_id))
                .filter(forum_reply_body::Column::Locale.eq(&locale))
                .one(&self.db)
                .await?
                .ok_or_else(ForumError::relation_revision_unavailable)?;
            (
                crate::richtext::project_stored_discussion(&body.body)?
                    .view
                    .document,
                None,
            )
        };
        let resolved = super::relation_quote_input::resolve_inline_update_quotes(
            &self.db,
            tenant_id,
            crate::mentions::ForumContentTarget::reply(reply_id),
            &locale,
            quote_inputs,
        )
        .await?;
        let (quotes, quote_expectation) = resolved.into_parts();
        let relation_service =
            super::mention_relation::MentionRelationService::new(self.db.clone());
        let prepared_relations = relation_service
            .prepare(
                tenant_id,
                crate::mentions::ForumContentTarget::reply(reply_id),
                &locale,
                &document,
                &security,
                quotes,
            )
            .await?;

        let initial_topic_id = existing.topic_id;
        let txn = self.db.begin().await?;
        super::category_audience::lock_category_tree_in_tx(&txn, tenant_id).await?;
        let locked_topic = super::topic_facade::TopicService::find_topic_for_update_in_tx(
            &txn,
            tenant_id,
            initial_topic_id,
        )
        .await?;
        if locked_topic.status == crate::state_machine::TopicStatus::Archived {
            return Err(ForumError::TopicArchived);
        }
        if locked_topic.is_locked {
            return Err(ForumError::TopicLocked);
        }
        super::relation_quote_input::lock_source_and_assert_latest_in_tx(
            &txn,
            tenant_id,
            crate::mentions::ForumContentTarget::reply(reply_id),
            &locale,
            quote_expectation,
        )
        .await?;
        let existing = Self::find_reply_in_tx(&txn, tenant_id, reply_id).await?;
        if existing.topic_id != initial_topic_id {
            return Err(ForumError::TopicUpdateConflict(initial_topic_id));
        }
        if existing.updated_at != expected_updated_at {
            return Err(ForumError::TopicUpdateConflict(existing.topic_id));
        }
        enforce_owned_scope(
            &security,
            Resource::ForumReplies,
            Action::Update,
            existing.author_id,
        )?;
        let topic_id = existing.topic_id;

        let existing_body_row = forum_reply_body::Entity::find()
            .filter(forum_reply_body::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_body::Column::ReplyId.eq(reply_id))
            .filter(forum_reply_body::Column::Locale.eq(&locale))
            .one(&txn)
            .await?;
        let body_changed = if stored_body.is_some() {
            match &existing_body_row {
                Some(existing_row) => {
                    let existing_doc = crate::richtext::project_stored_discussion(&existing_row.body)
                        .map(|p| p.view.document)
                        .ok();
                    existing_doc.as_ref() != Some(&document)
                }
                None => true,
            }
        } else {
            false
        };

        let mut repended_for_moderation = false;
        if body_changed && existing.status == ReplyStatus::Approved {
            // A content edit of a public reply in a moderated category (or
            // a pre-moderated tenant) re-enters moderation, mirroring the
            // reply status owner's counter and solution effects: otherwise
            // an author could publish a benign reply and rewrite it into
            // unreviewed content that stays `Approved`.
            let category = super::category::CategoryService::find_category_for_update_in_tx(
                &txn,
                tenant_id,
                locked_topic.category_id,
            )
            .await?;
            let pre_moderation_enabled = self
                .settings
                .module_settings_in_tx(&txn, tenant_id)
                .await?
                .pre_moderation_enabled;
            if category.moderated || pre_moderation_enabled {
                // The status write itself happens through the ActiveModel
                // below (built from `existing`), which would otherwise
                // rewrite the stale `Approved` status over this change.
                repended_for_moderation = true;
                super::topic_facade::TopicService::adjust_reply_count_in_tx(
                    &txn, tenant_id, topic_id, -1,
                )
                .await?;
                super::category::CategoryService::adjust_counters_in_tx(
                    &txn,
                    tenant_id,
                    category.id,
                    0,
                    -1,
                )
                .await?;
                crate::services::UserStatsService::adjust_reply_count_in_tx(
                    &txn,
                    tenant_id,
                    existing.author_id,
                    -1,
                )
                .await?;
                let solution_removed = forum_solution::Entity::delete_many()
                    .filter(forum_solution::Column::TenantId.eq(tenant_id))
                    .filter(forum_solution::Column::TopicId.eq(topic_id))
                    .filter(forum_solution::Column::ReplyId.eq(reply_id))
                    .exec(&txn)
                    .await?;
                if solution_removed.rows_affected > 0 {
                    crate::services::UserStatsService::adjust_solution_count_in_tx(
                        &txn,
                        tenant_id,
                        existing.author_id,
                        -1,
                    )
                    .await?;
                }
            }
        }
        if body_changed {
            if let Some(stored_body) = stored_body {
                self.upsert_body_in_tx(&txn, tenant_id, reply_id, &locale, stored_body)
                    .await?;
            }
        }
        relation_service
            .persist_in_tx(&txn, prepared_relations)
            .await?;

        let mut active: forum_reply::ActiveModel = existing.into();
        active.updated_at = Set(Utc::now().into());
        if repended_for_moderation {
            active.status = Set(ReplyStatus::Pending);
        }
        active.update(&txn).await?;
        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ReindexRequested {
                    target_type: "forum_topic".to_string(),
                    target_id: Some(topic_id),
                },
            )
            .await?;
        txn.commit().await?;
        self.get(tenant_id, security, reply_id, &locale).await
    }
}

#[cfg(test)]
#[path = "reply_tests.rs"]
mod reply_tests;

