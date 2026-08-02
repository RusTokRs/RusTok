use crate::dto::UpdateReplyCommandInput;

impl ReplyService {
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
        enforce_owned_scope(
            &security,
            Resource::ForumReplies,
            Action::Update,
            existing.author_id,
        )?;

        let has_content_change = input.content.is_some();
        if !has_content_change && quote_inputs.is_none() {
            return self.get(tenant_id, security, reply_id, &locale).await;
        }

        let (document, stored_body) = if let Some(content) = input.content {
            let document = crate::richtext::normalize_discussion(content)?;
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

        let topic_id = existing.topic_id;
        let txn = self.db.begin().await?;
        super::relation_quote_input::lock_source_and_assert_latest_in_tx(
            &txn,
            tenant_id,
            crate::mentions::ForumContentTarget::reply(reply_id),
            &locale,
            quote_expectation,
        )
        .await?;
        if let Some(stored_body) = stored_body {
            self.upsert_body_in_tx(&txn, tenant_id, reply_id, &locale, stored_body)
                .await?;
        }
        relation_service
            .persist_in_tx(&txn, prepared_relations)
            .await?;

        let mut active: forum_reply::ActiveModel = existing.into();
        active.updated_at = Set(Utc::now().into());
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
