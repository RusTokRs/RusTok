pub(crate) struct PreparedImportReplyInsert {
    record: crate::import_write_preparation::ForumPreparedImportReply,
    document: rustok_api::RichTextDocument,
    stored_body: String,
    created_at: chrono::DateTime<Utc>,
}

impl PreparedImportReplyInsert {
    pub(crate) fn id(&self) -> Uuid {
        self.record.id
    }

    pub(crate) fn topic_id(&self) -> Uuid {
        self.record.topic_id
    }

    pub(crate) fn status(&self) -> ReplyStatus {
        self.record.status
    }

    pub(crate) fn created_at(&self) -> chrono::DateTime<Utc> {
        self.created_at
    }
}

impl ReplyService {
    pub(crate) fn prepare_import_reply(
        &self,
        tenant_id: Uuid,
        record: &crate::import_write_preparation::ForumPreparedImportReply,
    ) -> ForumResult<PreparedImportReplyInsert> {
        if tenant_id.is_nil() || record.id.is_nil() || record.topic_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum import reply preparation requires non-nil tenant, reply and topic IDs"
                    .to_string(),
            ));
        }
        if record.status == ReplyStatus::Deleted {
            return Err(ForumError::Validation(
                "Forum import deleted reply requires an admitted tombstone timestamp before persistence"
                    .to_string(),
            ));
        }
        let locale = normalize_locale(&record.locale)?;
        if locale != record.locale {
            return Err(ForumError::Validation(
                "Forum import reply locale must already be normalized".to_string(),
            ));
        }
        let document = crate::richtext::normalize_discussion(record.content.clone())?;
        let stored_body = crate::richtext::serialize_discussion(document.clone())?;
        let created_at = chrono::DateTime::<Utc>::from_timestamp_millis(record.created_at_ms)
            .ok_or_else(|| {
                ForumError::Validation(
                    "Forum import reply creation timestamp is outside owner range".to_string(),
                )
            })?;

        Ok(PreparedImportReplyInsert {
            record: record.clone(),
            document,
            stored_body,
            created_at,
        })
    }

    pub(crate) async fn insert_import_reply_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        prepared: &PreparedImportReplyInsert,
        relation: &crate::import_relation_preparation::ForumPreparedImportContentRelations,
        relation_event_mode: crate::import_relation_preparation::ForumImportRelationEventMode,
        write_event_mode: crate::import_write_preparation::ForumImportWriteEventMode,
    ) -> ForumResult<()> {
        if relation.source != prepared.record.source
            || relation.target != ForumContentTarget::reply(prepared.record.id)
            || relation.locale != prepared.record.locale
        {
            return Err(ForumError::Validation(
                "Forum import reply relation admission does not match prepared reply".to_string(),
            ));
        }

        let topic =
            TopicService::find_topic_in_tx(txn, tenant_id, prepared.record.topic_id).await?;
        if topic.status != TopicStatus::Open || topic.is_locked {
            return Err(ForumError::Validation(
                "Forum import reply insertion requires the private provisional topic state"
                    .to_string(),
            ));
        }

        if let Some(parent_reply_id) = prepared.record.parent_reply_id {
            let parent =
                reply::ReplyService::find_reply_in_tx(txn, tenant_id, parent_reply_id).await?;
            if parent.topic_id != prepared.record.topic_id {
                return Err(ForumError::Validation(
                    "Forum import reply parent belongs to another topic".to_string(),
                ));
            }
            if parent.status == ReplyStatus::Deleted {
                return Err(ForumError::Validation(
                    "Forum import reply cannot use a deleted parent without tombstone admission"
                        .to_string(),
                ));
            }
        }

        let position =
            allocate_reply_position_in_tx(txn, tenant_id, prepared.record.topic_id).await?;
        let author_id = prepared.record.author.as_ref().map(|author| author.user_id);

        forum_reply::ActiveModel {
            id: Set(prepared.record.id),
            tenant_id: Set(tenant_id),
            topic_id: Set(prepared.record.topic_id),
            author_id: Set(author_id),
            parent_reply_id: Set(prepared.record.parent_reply_id),
            status: Set(prepared.record.status),
            position: Set(position),
            created_at: Set(prepared.created_at.into()),
            updated_at: Set(prepared.created_at.into()),
        }
        .insert(txn)
        .await?;

        forum_reply_body::ActiveModel {
            id: Set(Uuid::new_v4()),
            reply_id: Set(prepared.record.id),
            tenant_id: Set(tenant_id),
            locale: Set(prepared.record.locale.clone()),
            body: Set(prepared.stored_body.clone()),
            created_at: Set(prepared.created_at.into()),
            updated_at: Set(prepared.created_at.into()),
        }
        .insert(txn)
        .await?;

        self.relations
            .persist_import_admitted_in_tx(
                txn,
                tenant_id,
                relation,
                &prepared.document,
                author_id,
                relation_event_mode,
            )
            .await?;

        if prepared.record.status == ReplyStatus::Approved {
            TopicService::adjust_reply_count_in_tx(txn, tenant_id, prepared.record.topic_id, 1)
                .await?;
            CategoryService::adjust_counters_in_tx(txn, tenant_id, topic.category_id, 0, 1).await?;
            UserStatsService::adjust_reply_count_in_tx(txn, tenant_id, author_id, 1).await?;

            if write_event_mode
                == crate::import_write_preparation::ForumImportWriteEventMode::EmitDomainEvents
            {
                self.event_bus
                    .publish_in_tx(
                        txn,
                        tenant_id,
                        author_id,
                        DomainEvent::ForumTopicReplied {
                            topic_id: prepared.record.topic_id,
                            reply_id: prepared.record.id,
                            author_id,
                        },
                    )
                    .await?;
            }
        }

        Ok(())
    }
}
