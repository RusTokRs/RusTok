impl ReplyService {
    pub(crate) fn prepare_import_reply_with_tombstone(
        &self,
        tenant_id: Uuid,
        record: &crate::import_write_preparation::ForumPreparedImportReply,
        tombstone: Option<&crate::import_tombstone_preparation::ForumPreparedDeletedReplyTombstone>,
    ) -> ForumResult<PreparedImportReplyInsert> {
        if record.status != ReplyStatus::Deleted {
            if tombstone.is_some() {
                return Err(ForumError::Validation(
                    "Forum import live reply cannot carry a deleted tombstone".to_string(),
                ));
            }
            return self.prepare_import_reply(tenant_id, record);
        }

        let tombstone = tombstone.ok_or_else(|| {
            ForumError::Validation(
                "Forum import deleted reply requires an admitted tombstone timestamp".to_string(),
            )
        })?;
        validate_import_reply_tombstone(record, tombstone)?;

        if tenant_id.is_nil() || record.id.is_nil() || record.topic_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum import reply preparation requires non-nil tenant, reply and topic IDs"
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
        let deleted_at = chrono::DateTime::<Utc>::from_timestamp_millis(tombstone.deleted_at_ms)
            .ok_or_else(|| {
                ForumError::Validation(
                    "Forum import reply tombstone timestamp is outside owner range".to_string(),
                )
            })?;
        if deleted_at < created_at {
            return Err(ForumError::Validation(
                "Forum import reply tombstone cannot predate reply creation".to_string(),
            ));
        }

        Ok(PreparedImportReplyInsert {
            record: record.clone(),
            document,
            stored_body,
            created_at,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn insert_import_reply_with_tombstone_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        prepared: &PreparedImportReplyInsert,
        relation: &crate::import_relation_preparation::ForumPreparedImportContentRelations,
        tombstone: Option<&crate::import_tombstone_preparation::ForumPreparedDeletedReplyTombstone>,
        relation_event_mode: crate::import_relation_preparation::ForumImportRelationEventMode,
        write_event_mode: crate::import_write_preparation::ForumImportWriteEventMode,
    ) -> ForumResult<()> {
        if prepared.record.status != ReplyStatus::Deleted {
            if tombstone.is_some() {
                return Err(ForumError::Validation(
                    "Forum import live reply cannot carry a deleted tombstone".to_string(),
                ));
            }
            return self
                .insert_import_reply_in_tx(
                    txn,
                    tenant_id,
                    prepared,
                    relation,
                    relation_event_mode,
                    write_event_mode,
                )
                .await;
        }

        let tombstone = tombstone.ok_or_else(|| {
            ForumError::Validation(
                "Forum import deleted reply requires an admitted tombstone timestamp".to_string(),
            )
        })?;
        validate_import_reply_tombstone(&prepared.record, tombstone)?;

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
            // Historical reconstruction differs from interactive create: a child
            // may legitimately survive after its parent was later soft-deleted.
            // The enclosing 34P tombstone batch proves every final Deleted row.
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
            status: Set(ReplyStatus::Deleted),
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

        // Final-deleted historical content keeps its relation projection for
        // owner consistency but never emits interactive mention-added events.
        self.relations
            .persist_import_admitted_in_tx(
                txn,
                tenant_id,
                relation,
                &prepared.document,
                author_id,
                crate::import_relation_preparation::ForumImportRelationEventMode::SuppressAddedTargetEvents,
            )
            .await?;

        persist_import_reply_tombstone_in_tx(txn, tenant_id, prepared, tombstone).await?;
        Ok(())
    }
}

fn validate_import_reply_tombstone(
    record: &crate::import_write_preparation::ForumPreparedImportReply,
    tombstone: &crate::import_tombstone_preparation::ForumPreparedDeletedReplyTombstone,
) -> ForumResult<()> {
    if tombstone.source != record.source || tombstone.reply_id != record.id {
        return Err(ForumError::Validation(
            "Forum import reply tombstone does not match prepared reply identity".to_string(),
        ));
    }
    if tombstone.deleted_at_ms < record.created_at_ms {
        return Err(ForumError::Validation(
            "Forum import reply tombstone cannot predate reply creation".to_string(),
        ));
    }
    if chrono::DateTime::<Utc>::from_timestamp_millis(tombstone.deleted_at_ms).is_none() {
        return Err(ForumError::Validation(
            "Forum import reply tombstone timestamp is outside owner range".to_string(),
        ));
    }
    Ok(())
}

async fn persist_import_reply_tombstone_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    prepared: &PreparedImportReplyInsert,
    tombstone: &crate::import_tombstone_preparation::ForumPreparedDeletedReplyTombstone,
) -> ForumResult<()> {
    let deleted_at = chrono::DateTime::<Utc>::from_timestamp_millis(tombstone.deleted_at_ms)
        .ok_or_else(|| {
            ForumError::Validation(
                "Forum import reply tombstone timestamp is outside owner range".to_string(),
            )
        })?;

    let before = count_import_delete_revisions_in_tx(txn, tenant_id, prepared.record.id).await?;
    if before != 0 {
        return Err(ForumError::Validation(
            "Forum import deleted reply must not have a pre-existing delete revision".to_string(),
        ));
    }

    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE forum_replies \
             SET deleted_at = $1, updated_at = $1 \
             WHERE tenant_id = $2 AND id = $3 AND status = 'deleted' AND deleted_at IS NULL",
            vec![
                deleted_at.into(),
                tenant_id.into(),
                prepared.record.id.into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE forum_replies \
             SET deleted_at = ?, updated_at = ? \
             WHERE tenant_id = ? AND id = ? AND status = 'deleted' AND deleted_at IS NULL",
            vec![
                deleted_at.into(),
                deleted_at.into(),
                tenant_id.into(),
                prepared.record.id.into(),
            ],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum import reply tombstones do not support database backend {backend:?}"
            )));
        }
    };
    if txn.execute_raw(statement).await?.rows_affected() != 1 {
        return Err(ForumError::Validation(
            "Forum import reply tombstone update did not claim exactly one reply".to_string(),
        ));
    }

    let after = count_import_delete_revisions_in_tx(txn, tenant_id, prepared.record.id).await?;
    if after != 1 {
        return Err(ForumError::Validation(
            "Forum import reply tombstone must create exactly one delete revision".to_string(),
        ));
    }

    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE forum_reply_revisions \
             SET created_at = $1 \
             WHERE tenant_id = $2 AND reply_id = $3 AND revision_reason = 'delete'",
            vec![
                deleted_at.into(),
                tenant_id.into(),
                prepared.record.id.into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE forum_reply_revisions \
             SET created_at = ? \
             WHERE tenant_id = ? AND reply_id = ? AND revision_reason = 'delete'",
            vec![
                deleted_at.into(),
                tenant_id.into(),
                prepared.record.id.into(),
            ],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum import reply tombstones do not support database backend {backend:?}"
            )));
        }
    };
    if txn.execute_raw(statement).await?.rows_affected() != 1 {
        return Err(ForumError::Validation(
            "Forum import reply delete revision retimestamp did not affect exactly one row"
                .to_string(),
        ));
    }

    Ok(())
}

async fn count_import_delete_revisions_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    reply_id: Uuid,
) -> ForumResult<i64> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT COUNT(*) AS revision_count \
             FROM forum_reply_revisions \
             WHERE tenant_id = $1 AND reply_id = $2 AND revision_reason = 'delete'",
            vec![tenant_id.into(), reply_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) AS revision_count \
             FROM forum_reply_revisions \
             WHERE tenant_id = ? AND reply_id = ? AND revision_reason = 'delete'",
            vec![tenant_id.into(), reply_id.into()],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum import reply tombstones do not support database backend {backend:?}"
            )));
        }
    };
    let row = txn.query_one_raw(statement).await?.ok_or_else(|| {
        ForumError::Validation("Forum import reply revision count returned no row".to_string())
    })?;
    Ok(row.try_get("", "revision_count")?)
}
