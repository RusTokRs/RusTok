pub(crate) struct PreparedImportTopicInsert {
    record: crate::import_write_preparation::ForumPreparedImportTopic,
    document: RichTextDocument,
    stored_body: String,
    normalized_slug: Option<String>,
    normalized_tags: Vec<String>,
    prepared_custom_fields: flex::PreparedAttachedValuesWrite,
    created_at: chrono::DateTime<Utc>,
}

impl PreparedImportTopicInsert {
    pub(crate) fn id(&self) -> Uuid {
        self.record.id
    }

    pub(crate) fn category_id(&self) -> Uuid {
        self.record.category_id
    }

    pub(crate) fn source(&self) -> &crate::import_mapping::ForumImportExternalRef {
        &self.record.source
    }

    pub(crate) fn locale(&self) -> &str {
        &self.record.locale
    }

    pub(crate) fn author_id(&self) -> Option<Uuid> {
        self.record.author.as_ref().map(|author| author.user_id)
    }

    pub(crate) fn final_status(&self) -> TopicStatus {
        self.record.status
    }

    pub(crate) fn final_locked(&self) -> bool {
        self.record.is_locked
    }

    pub(crate) fn created_at(&self) -> chrono::DateTime<Utc> {
        self.created_at.clone()
    }
}

impl TopicService {
    pub(crate) async fn prepare_import_topic(
        &self,
        tenant_id: Uuid,
        record: &crate::import_write_preparation::ForumPreparedImportTopic,
    ) -> ForumResult<PreparedImportTopicInsert> {
        if tenant_id.is_nil() || record.id.is_nil() || record.category_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum import topic preparation requires non-nil tenant, topic and category IDs"
                    .to_string(),
            ));
        }
        validate_topic_title(&record.title)?;
        let locale = normalize_locale(&record.locale)?;
        if locale != record.locale {
            return Err(ForumError::Validation(
                "Forum import topic locale must already be normalized".to_string(),
            ));
        }
        let normalized_tags = normalize_tags(&record.tags);
        validate_normalized_topic_tags(&normalized_tags)?;
        let document = crate::richtext::normalize_discussion(record.body.clone())?;
        let stored_body = crate::richtext::serialize_discussion(document.clone())?;
        let prepared_custom_fields = self
            .prepare_topic_custom_fields_for_create(tenant_id, &locale, record.metadata.clone())
            .await?;
        let normalized_slug = record
            .slug
            .as_ref()
            .map(|value| normalize_slug(value))
            .filter(|value| !value.is_empty());
        let created_at = chrono::DateTime::<Utc>::from_timestamp_millis(record.created_at_ms)
            .ok_or_else(|| {
                ForumError::Validation(
                    "Forum import topic creation timestamp is outside owner range".to_string(),
                )
            })?;

        Ok(PreparedImportTopicInsert {
            record: record.clone(),
            document,
            stored_body,
            normalized_slug,
            normalized_tags,
            prepared_custom_fields,
            created_at,
        })
    }

    pub(crate) async fn insert_import_topic_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        prepared: &PreparedImportTopicInsert,
        relation: &crate::import_relation_preparation::ForumPreparedImportContentRelations,
        relations: &super::mention_relation::MentionRelationService,
        relation_event_mode: crate::import_relation_preparation::ForumImportRelationEventMode,
        write_event_mode: crate::import_write_preparation::ForumImportWriteEventMode,
    ) -> ForumResult<()> {
        if relation.source != prepared.record.source
            || relation.target != crate::mentions::ForumContentTarget::topic(prepared.record.id)
            || relation.locale != prepared.record.locale
        {
            return Err(ForumError::Validation(
                "Forum import topic relation admission does not match prepared topic".to_string(),
            ));
        }

        CategoryService::ensure_exists_in_tx(txn, tenant_id, prepared.record.category_id).await?;
        let author_id = prepared.author_id();

        // Topics are materialized open/unlocked inside the private transaction so
        // owner reply-insert guards can admit historical replies. The exact
        // prepared status/lock is restored by finalize_import_topic_in_tx before
        // the transaction can commit.
        forum_topic::ActiveModel {
            id: Set(prepared.record.id),
            tenant_id: Set(tenant_id),
            category_id: Set(prepared.record.category_id),
            author_id: Set(author_id),
            status: Set(TopicStatus::Open),
            metadata: Set(prepared
                .prepared_custom_fields
                .metadata
                .clone()
                .unwrap_or_else(|| serde_json::json!({}))),
            is_pinned: Set(prepared.record.is_pinned),
            is_locked: Set(false),
            reply_count: Set(0),
            created_at: Set(prepared.created_at.clone().into()),
            updated_at: Set(prepared.created_at.clone().into()),
            last_reply_at: Set(None),
        }
        .insert(txn)
        .await?;

        super::topic_tag_lock::lock_active_topic_tag_write_in_tx(
            txn,
            tenant_id,
            prepared.record.id,
        )
        .await?;
        super::topic_tag_lock::lock_topic_tag_scopes_in_tx(
            txn,
            tenant_id,
            &[prepared.record.id],
        )
        .await?;

        forum_topic_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            topic_id: Set(prepared.record.id),
            tenant_id: Set(tenant_id),
            locale: Set(prepared.record.locale.clone()),
            title: Set(prepared.record.title.clone()),
            slug: Set(prepared.normalized_slug.clone()),
            body: Set(prepared.stored_body.clone()),
            created_at: Set(prepared.created_at.clone().into()),
            updated_at: Set(prepared.created_at.clone().into()),
        }
        .insert(txn)
        .await?;

        relations
            .persist_import_admitted_in_tx(
                txn,
                tenant_id,
                relation,
                &prepared.document,
                author_id,
                relation_event_mode,
            )
            .await?;

        if let (Some(persist_locale), Some(values)) = (
            prepared.prepared_custom_fields.locale.as_deref(),
            prepared.prepared_custom_fields.localized_values.as_ref(),
        ) {
            persist_localized_values(
                txn,
                tenant_id,
                "topic",
                prepared.record.id,
                persist_locale,
                values,
            )
            .await
            .map_err(|error| ForumError::Validation(error.to_string()))?;
        }

        self.sync_channel_access_in_tx(
            txn,
            tenant_id,
            prepared.record.id,
            prepared.record.channel_slugs.as_deref(),
        )
        .await?;
        self.sync_topic_tags_in_tx(
            txn,
            tenant_id,
            prepared.record.id,
            &prepared.record.locale,
            &prepared.normalized_tags,
        )
        .await?;

        CategoryService::adjust_counters_in_tx(
            txn,
            tenant_id,
            prepared.record.category_id,
            1,
            0,
        )
        .await?;
        UserStatsService::adjust_topic_count_in_tx(txn, tenant_id, author_id, 1).await?;

        if write_event_mode
            == crate::import_write_preparation::ForumImportWriteEventMode::EmitDomainEvents
        {
            self.event_bus
                .publish_in_tx(
                    txn,
                    tenant_id,
                    author_id,
                    DomainEvent::ForumTopicCreated {
                        topic_id: prepared.record.id,
                        category_id: prepared.record.category_id,
                        author_id,
                        locale: prepared.record.locale.clone(),
                    },
                )
                .await?;
        }

        Ok(())
    }

    pub(crate) async fn finalize_import_topic_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        prepared: &PreparedImportTopicInsert,
        approved_reply_count: i32,
        last_reply_at: Option<chrono::DateTime<Utc>>,
    ) -> ForumResult<()> {
        let topic = Self::find_topic_in_tx(txn, tenant_id, prepared.record.id).await?;
        let mut active: forum_topic::ActiveModel = topic.into();
        active.status = Set(prepared.record.status);
        active.is_locked = Set(prepared.record.is_locked);
        active.reply_count = Set(approved_reply_count.max(0));
        active.last_reply_at = Set(last_reply_at.map(Into::into));
        active.updated_at = Set(prepared.created_at.clone().into());
        active.update(txn).await?;
        Ok(())
    }
}
