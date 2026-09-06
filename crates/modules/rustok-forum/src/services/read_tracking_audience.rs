const VISIBILITY_BULK_READ_CURSOR_VERSION: &str = "brv1";

#[derive(Clone, Debug)]
struct VisibilityBulkReadCursor {
    snapshot_at: sea_orm::prelude::DateTimeWithTimeZone,
    created_at: sea_orm::prelude::DateTimeWithTimeZone,
    topic_id: Uuid,
}

/// Exact visibility-scoped storefront bulk read owner.
///
/// Raw topic candidates remain bounded and cursor ordered, while only topics
/// admitted by the current route-channel and richer audience owners receive a
/// read-state update. A raw page may therefore advance with `processed == 0`.
pub struct ForumVisibilityScopedReadStateService {
    db: DatabaseConnection,
    audience_facts: Option<crate::audience::SharedForumAudienceFactsPort>,
}

impl ForumVisibilityScopedReadStateService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self::with_optional_audience_facts(db, None)
    }

    pub fn with_audience_facts(
        db: DatabaseConnection,
        audience_facts: crate::audience::SharedForumAudienceFactsPort,
    ) -> Self {
        Self::with_optional_audience_facts(db, Some(audience_facts))
    }

    fn with_optional_audience_facts(
        db: DatabaseConnection,
        audience_facts: Option<crate::audience::SharedForumAudienceFactsPort>,
    ) -> Self {
        Self { db, audience_facts }
    }

    /// Marks the exact currently visible topics in one category subtree read.
    pub async fn mark_category_read_with_audience_context(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        context: rustok_api::PortContext,
        input: MarkForumTopicsReadBatchInput,
    ) -> ForumResult<MarkForumTopicsReadBatchResult> {
        self.mark_scope_read_with_audience_context(
            tenant_id,
            BulkReadScope::Category(category_id),
            security,
            context,
            input,
        )
        .await
    }

    /// Marks the exact currently visible topics in the tenant storefront read.
    pub async fn mark_all_read_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: rustok_api::PortContext,
        input: MarkForumTopicsReadBatchInput,
    ) -> ForumResult<MarkForumTopicsReadBatchResult> {
        self.mark_scope_read_with_audience_context(
            tenant_id,
            BulkReadScope::Tenant,
            security,
            context,
            input,
        )
        .await
    }

    async fn mark_scope_read_with_audience_context(
        &self,
        tenant_id: Uuid,
        scope: BulkReadScope,
        security: SecurityContext,
        context: rustok_api::PortContext,
        input: MarkForumTopicsReadBatchInput,
    ) -> ForumResult<MarkForumTopicsReadBatchResult> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        let user_id = authenticated_user_id(
            &security,
            "Authenticated user context is required to mark visible forum topics read",
        )?;
        validate_visibility_bulk_context(tenant_id, &context)?;
        let limit = validated_bulk_read_limit(input.limit)?;

        let route_scope =
            super::topic_visibility::ForumTopicVisibilityScope::storefront_for_viewer(
                context.channel.as_deref(),
                true,
            )?;
        let channel_slug = route_scope.channel_slug().map(str::to_string);
        let channel_scope_token = visibility_channel_scope_token(channel_slug.as_deref());
        let cursor = input
            .cursor
            .as_deref()
            .map(|value| {
                decode_visibility_bulk_read_cursor(
                    value,
                    tenant_id,
                    user_id,
                    scope,
                    &channel_scope_token,
                )
            })
            .transpose()?;
        let snapshot_at = cursor
            .as_ref()
            .map(|cursor| cursor.snapshot_at)
            .unwrap_or_else(|| Utc::now().into());

        let topic_viewer =
            super::topic_audience_visibility::ForumTopicAudienceViewer::authenticated(
                security.clone(),
                context.clone(),
            )?;

        if let BulkReadScope::Category(category_id) = scope {
            let category_viewer =
                super::category_audience_visibility::ForumCategoryAudienceViewer::authenticated(
                    security.clone(),
                    context.clone(),
                )?;
            if !super::category_audience_visibility::ForumCategoryAudienceVisibilityService::new(
                self.db.clone(),
                self.audience_facts.clone(),
            )
            .is_category_visible(tenant_id, category_id, &category_viewer)
            .await?
            {
                return Err(ForumError::CategoryNotFound(category_id));
            }
        }

        let candidate_txn = self.db.begin().await?;
        let category_ids = match scope {
            BulkReadScope::Tenant => None,
            BulkReadScope::Category(category_id) => {
                Some(category_subtree_ids_in_tx(&candidate_txn, tenant_id, category_id).await?)
            }
        };

        let mut select = forum_topic::Entity::find()
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .filter(forum_topic::Column::Status.ne(TopicStatus::Archived))
            .filter(forum_topic::Column::CreatedAt.lte(snapshot_at));
        if let Some(category_ids) = category_ids {
            select = select.filter(forum_topic::Column::CategoryId.is_in(category_ids));
        }
        if let Some(cursor) = cursor.as_ref() {
            select = select.filter(
                Condition::any()
                    .add(forum_topic::Column::CreatedAt.gt(cursor.created_at))
                    .add(
                        Condition::all()
                            .add(forum_topic::Column::CreatedAt.eq(cursor.created_at))
                            .add(forum_topic::Column::Id.gt(cursor.topic_id)),
                    ),
            );
        }

        let mut candidates = select
            .order_by_asc(forum_topic::Column::CreatedAt)
            .order_by_asc(forum_topic::Column::Id)
            .limit(limit + 1)
            .all(&candidate_txn)
            .await?;
        candidate_txn.commit().await?;

        let has_more = candidates.len() > limit as usize;
        candidates.truncate(limit as usize);

        let visibility = super::topic_audience_visibility::ForumTopicAudienceVisibilityService::new(
            self.db.clone(),
            self.audience_facts.clone(),
        );
        let mut visible_topic_ids = Vec::with_capacity(candidates.len());
        for topic in &candidates {
            if visibility
                .is_topic_visible(tenant_id, topic.id, channel_slug.as_deref(), &topic_viewer)
                .await?
            {
                visible_topic_ids.push(topic.id);
            }
        }

        if !visible_topic_ids.is_empty() {
            let write_txn = self.db.begin().await?;
            lock_active_topic_read_state_writes_in_tx(&write_txn, tenant_id, &visible_topic_ids)
                .await?;
            lock_topic_read_state_scopes_in_tx(&write_txn, tenant_id, &visible_topic_ids).await?;
            let public_positions =
                latest_public_positions_in_tx(&write_txn, tenant_id, &visible_topic_ids).await?;
            let topic_revisions =
                latest_topic_revisions_in_tx(&write_txn, tenant_id, &visible_topic_ids).await?;
            let observed_at: sea_orm::prelude::DateTimeWithTimeZone = Utc::now().into();
            for topic_id in &visible_topic_ids {
                upsert_topic_read_high_water_in_tx(
                    &write_txn,
                    tenant_id,
                    user_id,
                    TopicReadHighWater {
                        topic_id: *topic_id,
                        last_read_position: public_positions.get(topic_id).copied().unwrap_or(0),
                        last_read_revision: topic_revisions.get(topic_id).copied().unwrap_or(0),
                    },
                    &observed_at,
                )
                .await?;
            }
            write_txn.commit().await?;
        }

        let next_cursor = if has_more {
            candidates.last().map(|topic| {
                encode_visibility_bulk_read_cursor(
                    tenant_id,
                    user_id,
                    scope,
                    &channel_scope_token,
                    &snapshot_at,
                    topic,
                )
            })
        } else {
            None
        };

        Ok(MarkForumTopicsReadBatchResult {
            processed: visible_topic_ids.len() as u64,
            next_cursor,
            has_more,
            snapshot_at: snapshot_at.to_rfc3339_opts(SecondsFormat::Nanos, true),
        })
    }
}

fn validate_visibility_bulk_context(
    tenant_id: Uuid,
    context: &rustok_api::PortContext,
) -> ForumResult<()> {
    let context_tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
        ForumError::Validation(
            "Forum visibility-scoped bulk read context tenant is invalid".to_string(),
        )
    })?;
    if context_tenant_id != tenant_id {
        return Err(ForumError::Validation(
            "Forum visibility-scoped bulk read context tenant does not match the request"
                .to_string(),
        ));
    }
    Ok(())
}

fn visibility_channel_scope_token(channel_slug: Option<&str>) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(channel_slug.unwrap_or("<no-route-channel>").as_bytes());
    hex::encode(hasher.finalize())
}

fn encode_visibility_bulk_read_cursor(
    tenant_id: Uuid,
    user_id: Uuid,
    scope: BulkReadScope,
    channel_scope_token: &str,
    snapshot_at: &sea_orm::prelude::DateTimeWithTimeZone,
    topic: &forum_topic::Model,
) -> String {
    format!(
        "{VISIBILITY_BULK_READ_CURSOR_VERSION}|{tenant_id}|{user_id}|{}|{channel_scope_token}|{}|{}|{}",
        scope.cursor_token(),
        snapshot_at.to_rfc3339_opts(SecondsFormat::Nanos, true),
        topic.created_at.to_rfc3339_opts(SecondsFormat::Nanos, true),
        topic.id,
    )
}

fn decode_visibility_bulk_read_cursor(
    value: &str,
    expected_tenant_id: Uuid,
    expected_user_id: Uuid,
    expected_scope: BulkReadScope,
    expected_channel_scope_token: &str,
) -> ForumResult<VisibilityBulkReadCursor> {
    let expected_tenant = expected_tenant_id.to_string();
    let expected_user = expected_user_id.to_string();
    let expected_scope_token = expected_scope.cursor_token();
    let mut parts = value.splitn(8, '|');
    if parts.next() != Some(VISIBILITY_BULK_READ_CURSOR_VERSION)
        || parts.next() != Some(expected_tenant.as_str())
        || parts.next() != Some(expected_user.as_str())
        || parts.next() != Some(expected_scope_token.as_str())
        || parts.next() != Some(expected_channel_scope_token)
    {
        return Err(invalid_visibility_bulk_read_cursor());
    }
    let snapshot_at = parts
        .next()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .ok_or_else(invalid_visibility_bulk_read_cursor)?;
    let created_at = parts
        .next()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .ok_or_else(invalid_visibility_bulk_read_cursor)?;
    let topic_id = parts
        .next()
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(invalid_visibility_bulk_read_cursor)?;
    if created_at > snapshot_at {
        return Err(invalid_visibility_bulk_read_cursor());
    }
    Ok(VisibilityBulkReadCursor {
        snapshot_at,
        created_at,
        topic_id,
    })
}

fn invalid_visibility_bulk_read_cursor() -> ForumError {
    ForumError::Validation("Invalid forum visibility-scoped bulk read cursor".to_string())
}
