impl TopicService {
    pub(crate) async fn create_with_relations(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateTopicInput,
    ) -> ForumResult<TopicResponse> {
        enforce_scope(&security, Resource::ForumTopics, Action::Create)?;
        validate_topic_title(&input.title)?;
        let locale = normalize_locale(&input.locale)?;
        let normalized_tags = normalize_tags(&input.tags);
        let prepared_body = prepare_content_payload(
            Some(&input.body_format),
            Some(&input.body),
            input.content_json.as_ref(),
            &locale,
            "Topic body",
        )
        .map_err(ForumError::Validation)?;
        let prepared_custom_fields = self
            .prepare_topic_custom_fields_for_create(tenant_id, &locale, input.metadata.clone())
            .await?;
        let topic_id = Uuid::new_v4();
        let relation_service =
            super::mention_relation::MentionRelationService::new(self.db.clone());
        let prepared_relations = relation_service
            .prepare(
                tenant_id,
                crate::mentions::ForumContentTarget::topic(topic_id),
                &locale,
                &prepared_body.body,
                &prepared_body.format,
                &security,
                std::iter::empty(),
            )
            .await?;

        let txn = self.db.begin().await?;
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
            body: Set(prepared_body.body),
            body_format: Set(prepared_body.format),
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

        txn.commit().await?;
        self.get(tenant_id, security, topic_id, &locale).await
    }

    pub(crate) async fn update_with_relations(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: UpdateTopicInput,
    ) -> ForumResult<TopicResponse> {
        let locale = normalize_locale(&input.locale)?;
        let topic = self.find_topic(tenant_id, topic_id).await?;
        enforce_owned_scope(
            &security,
            Resource::ForumTopics,
            Action::Update,
            topic.author_id,
        )?;
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
        let prepared_relation_body = self
            .prepare_topic_relation_body_for_update(tenant_id, topic_id, &locale, &input)
            .await?;
        let relation_service =
            super::mention_relation::MentionRelationService::new(self.db.clone());
        let prepared_relations = if let Some(body) = prepared_relation_body.as_ref() {
            Some(
                relation_service
                    .prepare(
                        tenant_id,
                        crate::mentions::ForumContentTarget::topic(topic_id),
                        &locale,
                        &body.body,
                        &body.format,
                        &security,
                        std::iter::empty(),
                    )
                    .await?,
            )
        } else {
            None
        };

        let txn = self.db.begin().await?;
        let mut active: forum_topic::ActiveModel = topic.into();
        active.updated_at = Set(Utc::now().into());
        if let Some(prepared_custom_fields) = prepared_custom_fields.as_ref() {
            active.metadata = Set(prepared_custom_fields
                .metadata
                .clone()
                .unwrap_or_else(|| serde_json::json!({})));
        }
        active.update(&txn).await?;

        if let Some(prepared_custom_fields) = prepared_custom_fields.as_ref() {
            if let (Some(persist_locale), Some(values)) = (
                prepared_custom_fields.locale.as_deref(),
                prepared_custom_fields.localized_values.as_ref(),
            ) {
                persist_localized_values(
                    &txn,
                    tenant_id,
                    "topic",
                    topic_id,
                    persist_locale,
                    values,
                )
                .await
                .map_err(|error| ForumError::Validation(error.to_string()))?;
            }
        }

        self.upsert_translation_in_tx(
            &txn,
            tenant_id,
            topic_id,
            &locale,
            TopicTranslationUpsertInput {
                title: input.title,
                body: input.body,
                body_format: input.body_format,
                content_json: input.content_json,
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

        txn.commit().await?;
        self.get(tenant_id, security, topic_id, &locale).await
    }

    async fn prepare_topic_relation_body_for_update(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: &str,
        input: &UpdateTopicInput,
    ) -> ForumResult<Option<rustok_core::PreparedContent>> {
        if input.body.is_some() || input.content_json.is_some() || input.body_format.is_some() {
            return prepare_content_payload(
                input.body_format.as_deref(),
                input.body.as_deref(),
                input.content_json.as_ref(),
                locale,
                "Topic body",
            )
            .map(Some)
            .map_err(ForumError::Validation);
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
        Ok(Some(rustok_core::PreparedContent {
            body: seed.body,
            format: seed.body_format,
        }))
    }
}
