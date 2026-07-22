include!("reply_legacy.rs");

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

        let has_content_change =
            input.content.is_some() || input.content_json.is_some() || input.content_format.is_some();
        if !has_content_change && quote_inputs.is_none() {
            return self.get(tenant_id, security, reply_id, &locale).await;
        }

        let prepared_body = if has_content_change {
            prepare_content_payload(
                input.content_format.as_deref(),
                input.content.as_deref(),
                input.content_json.as_ref(),
                &locale,
                "Reply content",
            )
            .map_err(ForumError::Validation)?
        } else {
            let body = forum_reply_body::Entity::find()
                .filter(forum_reply_body::Column::TenantId.eq(tenant_id))
                .filter(forum_reply_body::Column::ReplyId.eq(reply_id))
                .filter(forum_reply_body::Column::Locale.eq(&locale))
                .one(&self.db)
                .await?
                .ok_or_else(ForumError::relation_revision_unavailable)?;
            rustok_core::PreparedContent {
                body: body.body,
                format: body.body_format,
            }
        };
        let quotes = super::relation_quote_input::resolve_inline_update_quotes(
            &self.db,
            tenant_id,
            crate::mentions::ForumContentTarget::reply(reply_id),
            &locale,
            quote_inputs,
        )
        .await?;
        let relation_service =
            super::mention_relation::MentionRelationService::new(self.db.clone());
        let prepared_relations = relation_service
            .prepare(
                tenant_id,
                crate::mentions::ForumContentTarget::reply(reply_id),
                &locale,
                &prepared_body.body,
                &prepared_body.format,
                &security,
                quotes,
            )
            .await?;

        let txn = self.db.begin().await?;
        self.upsert_body_in_tx(
            &txn,
            tenant_id,
            reply_id,
            &locale,
            prepared_body.body,
            prepared_body.format,
        )
        .await?;
        relation_service
            .persist_in_tx(&txn, prepared_relations)
            .await?;

        let mut active: forum_reply::ActiveModel = existing.into();
        active.updated_at = Set(Utc::now().into());
        active.update(&txn).await?;
        txn.commit().await?;
        self.get(tenant_id, security, reply_id, &locale).await
    }
}
