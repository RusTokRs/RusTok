include!("reply_owner_legacy.rs");

use crate::dto::{CreateReplyCommandInput, UpdateReplyCommandInput};

impl ReplyService {
    #[instrument(skip(self, security, input))]
    pub async fn create_command(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        input: CreateReplyCommandInput,
    ) -> ForumResult<ReplyResponse> {
        let (input, quote_inputs) = input.into_parts();
        enforce_scope(&security, Resource::ForumReplies, Action::Create)?;
        let locale = normalize_locale(&input.locale)?;
        let prepared_body = prepare_content_payload(
            Some(&input.content_format),
            Some(&input.content),
            input.content_json.as_ref(),
            &locale,
            "Reply content",
        )
        .map_err(ForumError::Validation)?;
        let reply_id = Uuid::new_v4();
        let quotes = super::relation_quote_input::normalize_quote_inputs(quote_inputs)?;
        let prepared_relations = self
            .relations
            .prepare(
                tenant_id,
                ForumContentTarget::reply(reply_id),
                &locale,
                &prepared_body.body,
                &prepared_body.format,
                &security,
                quotes,
            )
            .await?;

        let txn = self.db.begin().await?;
        let topic = TopicService::find_topic_in_tx(&txn, tenant_id, topic_id).await?;
        match topic.status {
            TopicStatus::Closed => return Err(ForumError::TopicClosed),
            TopicStatus::Archived => return Err(ForumError::TopicArchived),
            TopicStatus::Open => {}
        }
        if topic.is_locked {
            return Err(ForumError::TopicLocked);
        }

        let category =
            CategoryService::find_category_in_tx(&txn, tenant_id, topic.category_id).await?;

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
        }

        let position = allocate_reply_position_in_tx(&txn, tenant_id, topic_id).await?;
        let status = if category.moderated {
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
            body: Set(prepared_body.body),
            body_format: Set(prepared_body.format),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        self.relations
            .persist_in_tx(&txn, prepared_relations)
            .await?;

        if status == ReplyStatus::Approved {
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
        self.inner.get(tenant_id, security, reply_id, &locale).await
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
