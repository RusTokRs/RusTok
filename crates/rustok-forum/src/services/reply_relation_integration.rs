impl ReplyService {
    pub(crate) async fn update_with_relations(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
        input: UpdateReplyInput,
    ) -> ForumResult<ReplyResponse> {
        let locale = normalize_locale(&input.locale)?;
        let existing = self.find_reply(tenant_id, reply_id).await?;
        enforce_owned_scope(
            &security,
            Resource::ForumReplies,
            Action::Update,
            existing.author_id,
        )?;

        if input.content.is_none() && input.content_json.is_none() && input.content_format.is_none()
        {
            return self.get(tenant_id, security, reply_id, &locale).await;
        }

        let prepared_body = prepare_content_payload(
            input.content_format.as_deref(),
            input.content.as_deref(),
            input.content_json.as_ref(),
            &locale,
            "Reply content",
        )
        .map_err(ForumError::Validation)?;
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
                std::iter::empty(),
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
