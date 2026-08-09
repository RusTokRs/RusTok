impl ForumImportWriteService {
    /// Applies one already-admitted 34P tombstone batch in exactly one
    /// Forum-owned transaction.
    ///
    /// The legacy `apply_prepared_batch` entrypoint remains unchanged and
    /// therefore keeps deleted replies fail-closed. Shared migration execution
    /// should use this entrypoint only after 34P tombstone admission.
    pub async fn apply_prepared_tombstone_batch(
        &self,
        security: &SecurityContext,
        batch: &crate::import_tombstone_preparation::ForumPreparedImportTombstoneBatch,
    ) -> ForumResult<ForumImportWriteResult> {
        let batch = revalidate_prepared_tombstone_batch(batch)?;
        let relations = &batch.relations;
        validate_tombstone_apply_shape(security, relations)?;
        validate_relation_alignment(relations)?;
        let category_order = ordered_category_indices(&relations.writes.categories)?;
        let reply_order = ordered_reply_indices(&relations.writes.replies)?;
        let tombstones = batch
            .deleted_replies
            .iter()
            .map(|tombstone| (tombstone.reply_id, tombstone))
            .collect::<BTreeMap<_, _>>();

        let topic_service =
            super::topic::TopicService::new(self.db.clone(), self.event_bus.clone());
        let reply_service =
            super::reply_owner::ReplyService::new(self.db.clone(), self.event_bus.clone());
        let relation_service = super::mention_relation::MentionRelationService::new(self.db.clone());

        let mut prepared_topics = Vec::with_capacity(relations.writes.topics.len());
        for record in &relations.writes.topics {
            prepared_topics.push(
                topic_service
                    .prepare_import_topic(relations.writes.tenant_id, record)
                    .await?,
            );
        }
        let mut prepared_replies = Vec::with_capacity(relations.writes.replies.len());
        for record in &relations.writes.replies {
            prepared_replies.push(reply_service.prepare_import_reply_with_tombstone(
                relations.writes.tenant_id,
                record,
                tombstones.get(&record.id).copied(),
            )?);
        }

        let txn = self.db.begin().await?;
        ensure_target_ids_absent_in_tx(&txn, relations).await?;

        for index in category_order {
            super::category::CategoryService::insert_import_category_in_tx(
                &txn,
                relations.writes.tenant_id,
                &relations.writes.categories[index],
            )
            .await?;
        }

        for (index, prepared) in prepared_topics.iter().enumerate() {
            topic_service
                .insert_import_topic_in_tx(
                    &txn,
                    relations.writes.tenant_id,
                    prepared,
                    &relations.topics[index],
                    &relation_service,
                    relations.relation_event_mode,
                    relations.writes.event_mode,
                )
                .await?;
        }

        for index in reply_order {
            reply_service
                .insert_import_reply_with_tombstone_in_tx(
                    &txn,
                    relations.writes.tenant_id,
                    &prepared_replies[index],
                    &relations.replies[index],
                    tombstones.get(&prepared_replies[index].id()).copied(),
                    relations.relation_event_mode,
                    relations.writes.event_mode,
                )
                .await?;
        }

        let reply_aggregates = approved_reply_aggregates(&prepared_replies)?;
        for prepared in &prepared_topics {
            let (count, last_reply_at) = reply_aggregates
                .get(&prepared.id())
                .cloned()
                .unwrap_or((0, None));
            super::topic::TopicService::finalize_import_topic_in_tx(
                &txn,
                relations.writes.tenant_id,
                prepared,
                count,
                last_reply_at,
            )
            .await?;
        }

        // This consistency projection is required even when all historical
        // interactive events, including deleted-content mention events, are
        // suppressed.
        super::projection_invalidation::publish_forum_projection_scope_direct_in_tx(
            &txn,
            relations.writes.tenant_id,
            security.user_id,
        )
        .await?;

        txn.commit().await?;

        Ok(ForumImportWriteResult {
            tenant_id: relations.writes.tenant_id,
            category_ids: relations
                .writes
                .categories
                .iter()
                .map(|record| record.id)
                .collect(),
            topic_ids: relations
                .writes
                .topics
                .iter()
                .map(|record| record.id)
                .collect(),
            reply_ids: relations
                .writes
                .replies
                .iter()
                .map(|record| record.id)
                .collect(),
        })
    }
}

fn revalidate_prepared_tombstone_batch(
    batch: &crate::import_tombstone_preparation::ForumPreparedImportTombstoneBatch,
) -> ForumResult<crate::import_tombstone_preparation::ForumPreparedImportTombstoneBatch> {
    let checked = crate::import_tombstone_preparation::ForumImportTombstonePreparer
        .prepare(
            crate::import_tombstone_preparation::ForumImportTombstonePreparationRequest {
                relations: batch.relations.clone(),
                deleted_replies: batch
                    .deleted_replies
                    .iter()
                    .map(|tombstone| {
                        crate::import_tombstone_preparation::ForumImportReplyTombstoneFact {
                            source: tombstone.source.clone(),
                            deleted_at_ms: tombstone.deleted_at_ms,
                        }
                    })
                    .collect(),
            },
        )
        .map_err(|error| {
            ForumError::Validation(format!(
                "Forum import tombstone batch revalidation failed: {error}"
            ))
        })?;

    if checked.deleted_replies != batch.deleted_replies {
        return Err(ForumError::Validation(
            "Forum import tombstone batch identity differs after revalidation".to_string(),
        ));
    }
    Ok(checked)
}

fn validate_tombstone_apply_shape(
    security: &SecurityContext,
    batch: &ForumPreparedImportRelationBatch,
) -> ForumResult<()> {
    if batch.writes.tenant_id.is_nil() {
        return Err(ForumError::Validation(
            "Forum import application requires a non-nil tenant ID".to_string(),
        ));
    }
    let total = batch
        .writes
        .categories
        .len()
        .saturating_add(batch.writes.topics.len())
        .saturating_add(batch.writes.replies.len());
    if total == 0 || total > MAX_FORUM_IMPORT_APPLY_RECORDS_PER_BATCH {
        return Err(ForumError::Validation(format!(
            "Forum import application requires 1..={MAX_FORUM_IMPORT_APPLY_RECORDS_PER_BATCH} owner records"
        )));
    }

    let locale = normalize_locale_code(&batch.writes.locale).ok_or_else(|| {
        ForumError::Validation("Forum import application requires a valid locale".to_string())
    })?;
    if locale != batch.writes.locale {
        return Err(ForumError::Validation(
            "Forum import application locale must already be normalized".to_string(),
        ));
    }

    if !batch.writes.categories.is_empty() {
        require_all_manage(security, Resource::ForumCategories)?;
    }
    if !batch.writes.topics.is_empty() {
        require_all_manage(security, Resource::ForumTopics)?;
    }
    if !batch.writes.replies.is_empty() {
        require_all_manage(security, Resource::ForumReplies)?;
    }

    let expected_relation_event_mode = match batch.writes.event_mode {
        ForumImportWriteEventMode::SuppressInteractiveEvents => {
            ForumImportRelationEventMode::SuppressAddedTargetEvents
        }
        ForumImportWriteEventMode::EmitDomainEvents => {
            ForumImportRelationEventMode::EmitAddedTargetEvents
        }
    };
    if batch.relation_event_mode != expected_relation_event_mode {
        return Err(ForumError::Validation(
            "Forum import relation event mode differs from prepared write event mode".to_string(),
        ));
    }

    let category_ids = unique_ids(
        "category",
        batch.writes.categories.iter().map(|record| record.id),
    )?;
    let topic_ids = unique_ids("topic", batch.writes.topics.iter().map(|record| record.id))?;
    let reply_ids = unique_ids("reply", batch.writes.replies.iter().map(|record| record.id))?;

    for category in &batch.writes.categories {
        validate_source_ref(&category.source, ForumImportEntityKind::Category)?;
        validate_record_locale(&batch.writes.locale, &category.source, &category.locale)?;
        validate_timestamp("category", &category.source, category.created_at_ms)?;
    }

    for topic in &batch.writes.topics {
        validate_source_ref(&topic.source, ForumImportEntityKind::Topic)?;
        validate_source_ref(&topic.body_source, ForumImportEntityKind::Post)?;
        validate_record_locale(&batch.writes.locale, &topic.source, &topic.locale)?;
        validate_author(topic.author.as_ref())?;
        validate_timestamp("topic", &topic.source, topic.created_at_ms)?;
        if !category_ids.contains(&topic.category_id) {
            return Err(ForumError::Validation(
                "Forum import topic category must be inside the bounded batch".to_string(),
            ));
        }
    }

    for reply in &batch.writes.replies {
        validate_source_ref(&reply.source, ForumImportEntityKind::Post)?;
        validate_record_locale(&batch.writes.locale, &reply.source, &reply.locale)?;
        validate_author(reply.author.as_ref())?;
        validate_timestamp("reply", &reply.source, reply.created_at_ms)?;
        if !topic_ids.contains(&reply.topic_id) {
            return Err(ForumError::Validation(
                "Forum import reply topic must be inside the bounded batch".to_string(),
            ));
        }
        if let Some(parent_reply_id) = reply.parent_reply_id {
            if !reply_ids.contains(&parent_reply_id) {
                return Err(ForumError::Validation(
                    "Forum import reply parent must be inside the bounded batch".to_string(),
                ));
            }
        }
    }

    Ok(())
}
