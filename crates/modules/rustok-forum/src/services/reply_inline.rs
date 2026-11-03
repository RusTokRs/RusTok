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
