use super::{category_audience, category_visibility, topic_audience, topic_audience_lock};

#[path = "topic_route_tombstone_visibility.rs"]
pub mod route_tombstone_visibility;

use crate::dto::{CreateTopicCommandInput, UpdateTopicCommandInput};

impl TopicService {
    #[instrument(skip(self, security, input))]
    pub async fn create_command(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateTopicCommandInput,
    ) -> ForumResult<TopicResponse> {
        let create_audience =
            ForumTopicCreateAudienceAuthorizationService::without_facts_provider(
                self.db.clone(),
            );
        self.create_with_audience_authorization(
            tenant_id,
            security,
            None,
            input,
            &create_audience,
        )
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
        let locale = normalize_locale(&input.locale)?;
        let normalized_tags = normalize_tags(&input.tags);
        validate_normalized_topic_tags(&normalized_tags)?;
        let document = crate::richtext::normalize_discussion(input.body)?;
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

        let now = Utc::now();
        forum_topic::ActiveModel {
            id: Set(topic_id),
            tenant_id: Set(tenant_id),
            category_id: Set(input.category_id),
            author_id: Set(security.user_id),
            status: Set(TopicStatus::Open),
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

    #[instrument(skip(self, security, input))]
    pub async fn update_command(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: UpdateTopicCommandInput,
    ) -> ForumResult<TopicResponse> {
        self.inner
            .update_with_inline_relations(tenant_id, topic_id, security, input)
            .await
    }
}
