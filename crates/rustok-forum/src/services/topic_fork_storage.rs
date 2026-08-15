async fn lock_topic_fork_tenant_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            for (scope, seed) in [
                (format!("forum-topic-fork:{tenant_id}"), 22_i32),
                (tenant_id.to_string(), 0_i32),
            ] {
                txn.execute(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT pg_advisory_xact_lock(hashtextextended($1, $2))",
                    vec![scope.into(), seed.into()],
                ))
                .await?;
            }
            Ok(())
        }
        DatabaseBackend::Sqlite => {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"
                INSERT INTO forum_topic_fork_locks (tenant_id, touched_at)
                VALUES (?, CURRENT_TIMESTAMP)
                ON CONFLICT(tenant_id) DO UPDATE SET touched_at = CURRENT_TIMESTAMP
                "#,
                vec![tenant_id.into()],
            ))
            .await?;
            Ok(())
        }
        backend => Err(ForumError::Validation(format!(
            "Forum topic fork does not support database backend {backend:?}"
        ))),
    }
}

async fn lock_fork_counter_scopes_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            let mut topic_ids = [source_topic_id, target_topic_id];
            topic_ids.sort();
            for scope in [
                format!("forum:category:{tenant_id}:{category_id}"),
                format!("forum:topic:{tenant_id}:{}", topic_ids[0]),
                format!("forum:topic:{tenant_id}:{}", topic_ids[1]),
            ] {
                txn.execute(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT forum_counter_lock($1)",
                    vec![scope.into()],
                ))
                .await?;
            }
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(ForumError::Validation(format!(
            "Forum topic fork counter locking does not support {backend:?}"
        ))),
    }
}

async fn lock_fork_author_scopes_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    replies: &[forum_reply::Model],
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            let mut author_ids = replies
                .iter()
                .filter_map(|reply| reply.author_id)
                .collect::<Vec<_>>();
            author_ids.sort();
            author_ids.dedup();
            for author_id in author_ids {
                txn.execute(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT forum_counter_lock($1)",
                    vec![format!("forum:user:{tenant_id}:{author_id}").into()],
                ))
                .await?;
            }
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(ForumError::Validation(format!(
            "Forum topic fork author locking does not support {backend:?}"
        ))),
    }
}

async fn lock_source_topic_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
) -> ForumResult<()> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT id FROM forum_topics WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE",
            vec![tenant_id.into(), source_topic_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT id FROM forum_topics WHERE tenant_id = ? AND id = ? AND deleted_at IS NULL",
            vec![tenant_id.into(), source_topic_id.into()],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic fork source locking does not support {backend:?}"
            )));
        }
    };
    if txn.query_one(statement).await?.is_none() {
        return Err(ForumError::TopicNotFound(source_topic_id));
    }
    Ok(())
}

async fn lock_reply_rows_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    reply_ids: &[Uuid],
) -> ForumResult<()> {
    let mut ids = reply_ids.to_vec();
    ids.sort();
    ids.dedup();
    for reply_id in ids {
        let statement = match txn.get_database_backend() {
            DatabaseBackend::Postgres => Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT id FROM forum_replies WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
                vec![tenant_id.into(), reply_id.into()],
            ),
            DatabaseBackend::Sqlite => Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT id FROM forum_replies WHERE tenant_id = ? AND id = ?",
                vec![tenant_id.into(), reply_id.into()],
            ),
            backend => {
                return Err(ForumError::Validation(format!(
                    "Forum topic fork reply locking does not support {backend:?}"
                )));
            }
        };
        if txn.query_one(statement).await?.is_none() {
            return Err(ForumError::ReplyNotFound(reply_id));
        }
    }
    Ok(())
}

async fn lock_topic_reply_create_scopes_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_ids: &[Uuid],
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            let mut ids = topic_ids.to_vec();
            ids.sort();
            ids.dedup();
            for topic_id in ids {
                txn.execute(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT pg_advisory_xact_lock(hashtextextended($1, 5))",
                    vec![format!("{tenant_id}:{topic_id}:reply-create").into()],
                ))
                .await?;
            }
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(ForumError::Validation(format!(
            "Forum topic fork reply-create locking does not support {backend:?}"
        ))),
    }
}

async fn find_topic_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<forum_topic::Model> {
    forum_topic::Entity::find_by_id(topic_id)
        .filter(forum_topic::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or(ForumError::TopicNotFound(topic_id))
}

async fn ensure_target_topic_absent_in_tx(
    txn: &DatabaseTransaction,
    target_topic_id: Uuid,
) -> ForumResult<()> {
    if forum_topic::Entity::find_by_id(target_topic_id)
        .one(txn)
        .await?
        .is_some()
    {
        return Err(ForumError::Validation(
            "Forum topic fork target topic ID already exists".to_string(),
        ));
    }
    Ok(())
}

async fn ensure_category_active_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<()> {
    if forum_category_lifecycle::Entity::find()
        .filter(forum_category_lifecycle::Column::TenantId.eq(tenant_id))
        .filter(forum_category_lifecycle::Column::CategoryId.eq(category_id))
        .one(txn)
        .await?
        .is_some()
    {
        return Err(ForumError::Validation(
            "Forum topic fork requires an active source category".to_string(),
        ));
    }
    Ok(())
}

async fn load_valid_source_solution_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
) -> ForumResult<Option<forum_solution::Model>> {
    let Some(solution) = forum_solution::Entity::find()
        .filter(forum_solution::Column::TenantId.eq(tenant_id))
        .filter(forum_solution::Column::TopicId.eq(source_topic_id))
        .one(txn)
        .await?
    else {
        return Ok(None);
    };
    let valid = forum_reply::Entity::find_by_id(solution.reply_id)
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(source_topic_id))
        .filter(forum_reply::Column::Status.eq(ReplyStatus::Approved))
        .one(txn)
        .await?
        .is_some();
    if !valid {
        return Err(ForumError::Validation(
            "Forum topic fork requires a valid approved source solution".to_string(),
        ));
    }
    Ok(Some(solution))
}

async fn create_target_topic_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source: &forum_topic::Model,
    actor_id: Uuid,
    prepared: &PreparedForkInput,
    now: DateTime<Utc>,
) -> ForumResult<forum_topic::Model> {
    let actual_source_published = forum_reply::Entity::find()
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(source.id))
        .filter(forum_reply::Column::Status.eq(ReplyStatus::Approved))
        .count(txn)
        .await?;
    let actual_source_published = i32::try_from(actual_source_published).map_err(|_| {
        ForumError::Validation(
            "Forum topic fork source published reply count exceeds supported range".to_string(),
        )
    })?;
    if actual_source_published != source.reply_count {
        return Err(ForumError::Validation(
            "Forum topic fork source published reply counter is inconsistent".to_string(),
        ));
    }

    let topic = forum_topic::ActiveModel {
        id: Set(prepared.target_topic_id),
        tenant_id: Set(tenant_id),
        category_id: Set(source.category_id),
        author_id: Set(Some(actor_id)),
        status: Set(TopicStatus::Open),
        metadata: Set(json!({})),
        is_pinned: Set(false),
        is_locked: Set(false),
        reply_count: Set(0),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        last_reply_at: Set(None),
    }
    .insert(txn)
    .await?;
    forum_topic_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        topic_id: Set(prepared.target_topic_id),
        tenant_id: Set(tenant_id),
        locale: Set(prepared.locale.clone()),
        title: Set(prepared.title.clone()),
        slug: Set(prepared.slug.clone()),
        body: Set(prepared.stored_body.clone()),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(txn)
    .await?;
    Ok(topic)
}

async fn clone_topic_access_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
) -> ForumResult<()> {
    let specifications = [
        (
            "forum_topic_channel_access",
            "tenant_id, topic_id, channel_slug",
            "channel_slug",
        ),
        (
            "forum_topic_audience_policies",
            "tenant_id, topic_id, minimum_trust_level, updated_at",
            "minimum_trust_level, CURRENT_TIMESTAMP",
        ),
        (
            "forum_topic_audience_roles",
            "tenant_id, topic_id, role",
            "role",
        ),
        (
            "forum_topic_audience_channels",
            "tenant_id, topic_id, channel_slug",
            "channel_slug",
        ),
        (
            "forum_topic_audience_groups",
            "tenant_id, topic_id, group_id",
            "group_id",
        ),
        (
            "forum_topic_audience_users",
            "tenant_id, topic_id, user_id, effect",
            "user_id, effect",
        ),
        (
            "forum_topic_reply_create_audience_policies",
            "tenant_id, topic_id, minimum_trust_level, updated_at",
            "minimum_trust_level, CURRENT_TIMESTAMP",
        ),
        (
            "forum_topic_reply_create_audience_roles",
            "tenant_id, topic_id, role",
            "role",
        ),
        (
            "forum_topic_reply_create_audience_channels",
            "tenant_id, topic_id, channel_slug",
            "channel_slug",
        ),
        (
            "forum_topic_reply_create_audience_groups",
            "tenant_id, topic_id, group_id",
            "group_id",
        ),
        (
            "forum_topic_reply_create_audience_users",
            "tenant_id, topic_id, user_id, effect",
            "user_id, effect",
        ),
    ];
    let backend = txn.get_database_backend();
    let placeholders = match backend {
        DatabaseBackend::Postgres => ("$1", "$2", "$3"),
        DatabaseBackend::Sqlite => ("?", "?", "?"),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic fork access cloning does not support {backend:?}"
            )));
        }
    };
    for (table, insert_columns, selected_columns) in specifications {
        let sql = format!(
            "INSERT INTO {table} ({insert_columns}) SELECT tenant_id, {}, {selected_columns} FROM {table} WHERE tenant_id = {} AND topic_id = {}",
            placeholders.0, placeholders.1, placeholders.2
        );
        txn.execute(Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                target_topic_id.into(),
                tenant_id.into(),
                source_topic_id.into(),
            ],
        ))
        .await?;
    }
    Ok(())
}

async fn clone_topic_tags_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    now: DateTime<Utc>,
) -> ForumResult<()> {
    let tags = forum_topic_tag::Entity::find()
        .filter(forum_topic_tag::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_tag::Column::TopicId.eq(source_topic_id))
        .limit((MAX_FORUM_TOPIC_TAGS + 1) as u64)
        .all(txn)
        .await?;
    ensure_bound(tags.len(), MAX_FORUM_TOPIC_TAGS, "topic tags")?;
    for tag in tags {
        forum_topic_tag::ActiveModel {
            id: Set(Uuid::new_v4()),
            topic_id: Set(target_topic_id),
            term_id: Set(tag.term_id),
            tenant_id: Set(tenant_id),
            created_at: Set(now.into()),
        }
        .insert(txn)
        .await?;
    }
    Ok(())
}

async fn validate_cloned_topic_shape_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source: &forum_topic::Model,
    target: &forum_topic::Model,
) -> ForumResult<()> {
    let source_visibility = load_policy_for_topic(txn, tenant_id, source).await?;
    let target_visibility = load_policy_for_topic(txn, tenant_id, target).await?;
    if source_visibility.inherited_category_layers != target_visibility.inherited_category_layers
        || source_visibility.configured_constraints != target_visibility.configured_constraints
    {
        return Err(ForumError::Validation(
            "Forum topic fork visibility policy clone is inconsistent".to_string(),
        ));
    }
    let source_reply_create =
        load_topic_reply_create_audience_policy_for_topic(txn, tenant_id, source).await?;
    let target_reply_create =
        load_topic_reply_create_audience_policy_for_topic(txn, tenant_id, target).await?;
    if source_reply_create.inherited_category_layers
        != target_reply_create.inherited_category_layers
        || source_reply_create.configured_constraints != target_reply_create.configured_constraints
    {
        return Err(ForumError::Validation(
            "Forum topic fork reply-create policy clone is inconsistent".to_string(),
        ));
    }
    if load_topic_channels_in_tx(txn, tenant_id, source.id).await?
        != load_topic_channels_in_tx(txn, tenant_id, target.id).await?
    {
        return Err(ForumError::Validation(
            "Forum topic fork channel access clone is inconsistent".to_string(),
        ));
    }
    if load_topic_tag_ids_in_tx(txn, tenant_id, source.id).await?
        != load_topic_tag_ids_in_tx(txn, tenant_id, target.id).await?
    {
        return Err(ForumError::Validation(
            "Forum topic fork tag clone is inconsistent".to_string(),
        ));
    }
    Ok(())
}

async fn load_topic_channels_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<Vec<String>> {
    let mut channels = forum_topic_channel_access::Entity::find()
        .filter(forum_topic_channel_access::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_channel_access::Column::TopicId.eq(topic_id))
        .all(txn)
        .await?
        .into_iter()
        .map(|row| row.channel_slug)
        .collect::<Vec<_>>();
    channels.sort();
    Ok(channels)
}

async fn load_topic_tag_ids_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<Vec<Uuid>> {
    let mut term_ids = forum_topic_tag::Entity::find()
        .filter(forum_topic_tag::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_tag::Column::TopicId.eq(topic_id))
        .limit((MAX_FORUM_TOPIC_TAGS + 1) as u64)
        .all(txn)
        .await?
        .into_iter()
        .map(|row| row.term_id)
        .collect::<Vec<_>>();
    ensure_bound(term_ids.len(), MAX_FORUM_TOPIC_TAGS, "topic tags")?;
    term_ids.sort();
    Ok(term_ids)
}

async fn reconcile_target_topic_in_tx(
    txn: &DatabaseTransaction,
    target: forum_topic::Model,
    copied_published_reply_count: i32,
    target_last_reply_at: Option<DateTimeWithTimeZone>,
    now: DateTime<Utc>,
) -> ForumResult<forum_topic::Model> {
    let mut active: forum_topic::ActiveModel = target.into();
    active.reply_count = Set(copied_published_reply_count);
    active.last_reply_at = Set(target_last_reply_at);
    active.updated_at = Set(now.into());
    let updated = active.update(txn).await?;
    if updated.reply_count != copied_published_reply_count
        || updated.last_reply_at != target_last_reply_at
    {
        return Err(ForumError::Validation(
            "Forum topic fork target reply counters are inconsistent".to_string(),
        ));
    }
    Ok(updated)
}

async fn increment_category_counters_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    copied_published_reply_count: i32,
    now: DateTime<Utc>,
) -> ForumResult<()> {
    let category = forum_category::Entity::find_by_id(category_id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or(ForumError::CategoryNotFound(category_id))?;
    if category.topic_count < 0 || category.reply_count < 0 {
        return Err(ForumError::Validation(
            "Forum topic fork category counters are inconsistent".to_string(),
        ));
    }
    let expected_topic_count = category.topic_count.checked_add(1).ok_or_else(|| {
        ForumError::Validation("Forum topic fork category topic counter overflow".to_string())
    })?;
    let expected_reply_count = category
        .reply_count
        .checked_add(copied_published_reply_count)
        .ok_or_else(|| {
            ForumError::Validation("Forum topic fork category reply counter overflow".to_string())
        })?;
    let mut active: forum_category::ActiveModel = category.into();
    active.topic_count = Set(expected_topic_count);
    active.reply_count = Set(expected_reply_count);
    active.updated_at = Set(now.into());
    let updated = active.update(txn).await?;
    if updated.topic_count != expected_topic_count || updated.reply_count != expected_reply_count {
        return Err(ForumError::Validation(
            "Forum topic fork category counter reconciliation failed".to_string(),
        ));
    }
    Ok(())
}

async fn validate_source_unchanged_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_before: &forum_topic::Model,
    solution_before: Option<&forum_solution::Model>,
) -> ForumResult<()> {
    let source_after = find_topic_in_tx(txn, tenant_id, source_before.id).await?;
    if &source_after != source_before {
        return Err(ForumError::Validation(
            "Forum topic fork changed source topic state".to_string(),
        ));
    }
    let solution_after = forum_solution::Entity::find_by_id((source_before.id, tenant_id))
        .one(txn)
        .await?;
    if solution_after.as_ref() != solution_before {
        return Err(ForumError::Validation(
            "Forum topic fork changed source accepted solution".to_string(),
        ));
    }
    Ok(())
}

async fn validate_target_solution_absent_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    target_topic_id: Uuid,
) -> ForumResult<()> {
    if forum_solution::Entity::find_by_id((target_topic_id, tenant_id))
        .one(txn)
        .await?
        .is_some()
    {
        return Err(ForumError::Validation(
            "Forum topic fork target must remain unsolved".to_string(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_fork_operation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    operation_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    root_reply_id: Uuid,
    category_id: Uuid,
    actor_id: Uuid,
    reason: &str,
    command_fingerprint: &str,
    copied_reply_count: i32,
    copied_published_reply_count: i32,
    copied_body_count: i32,
    copied_reply_revision_count: i32,
    copied_relation_revision_count: i32,
    copied_mention_count: i32,
    copied_quote_count: i32,
    now: DateTime<Utc>,
) -> ForumResult<()> {
    let (backend, sql) = match txn.get_database_backend() {
        DatabaseBackend::Postgres => (
            DatabaseBackend::Postgres,
            r#"
            INSERT INTO forum_topic_fork_operations (
                tenant_id, operation_id, source_topic_id, target_topic_id,
                root_reply_id, category_id, actor_id, reason,
                command_fingerprint, copied_reply_count,
                copied_published_reply_count, copied_body_count,
                copied_reply_revision_count, copied_relation_revision_count,
                copied_mention_count, copied_quote_count, event_id, forked_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9,
                $10, $11, $12, $13, $14, $15, $16, $17, $18
            )
            "#,
        ),
        DatabaseBackend::Sqlite => (
            DatabaseBackend::Sqlite,
            r#"
            INSERT INTO forum_topic_fork_operations (
                tenant_id, operation_id, source_topic_id, target_topic_id,
                root_reply_id, category_id, actor_id, reason,
                command_fingerprint, copied_reply_count,
                copied_published_reply_count, copied_body_count,
                copied_reply_revision_count, copied_relation_revision_count,
                copied_mention_count, copied_quote_count, event_id, forked_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic fork receipt does not support {backend:?}"
            )));
        }
    };
    txn.execute(Statement::from_sql_and_values(
        backend,
        sql,
        vec![
            tenant_id.into(),
            operation_id.into(),
            source_topic_id.into(),
            target_topic_id.into(),
            root_reply_id.into(),
            category_id.into(),
            actor_id.into(),
            reason.to_string().into(),
            command_fingerprint.to_string().into(),
            copied_reply_count.into(),
            copied_published_reply_count.into(),
            copied_body_count.into(),
            copied_reply_revision_count.into(),
            copied_relation_revision_count.into(),
            copied_mention_count.into(),
            copied_quote_count.into(),
            operation_id.into(),
            now.into(),
        ],
    ))
    .await?;
    Ok(())
}

async fn insert_fork_reply_audit_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    operation_id: Uuid,
    audit: &[ForkReplyAudit],
    now: DateTime<Utc>,
) -> ForumResult<()> {
    let (backend, sql) = match txn.get_database_backend() {
        DatabaseBackend::Postgres => (
            DatabaseBackend::Postgres,
            "INSERT INTO forum_topic_fork_reply_items (tenant_id, operation_id, source_reply_id, target_reply_id, source_parent_reply_id, target_parent_reply_id, source_position, target_position, was_published, copied_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        ),
        DatabaseBackend::Sqlite => (
            DatabaseBackend::Sqlite,
            "INSERT INTO forum_topic_fork_reply_items (tenant_id, operation_id, source_reply_id, target_reply_id, source_parent_reply_id, target_parent_reply_id, source_position, target_position, was_published, copied_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic fork reply audit does not support {backend:?}"
            )));
        }
    };
    for item in audit {
        txn.execute(Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                tenant_id.into(),
                operation_id.into(),
                item.source_reply_id.into(),
                item.target_reply_id.into(),
                item.source_parent_reply_id.into(),
                item.target_parent_reply_id.into(),
                item.source_position.into(),
                item.target_position.into(),
                item.was_published.into(),
                now.into(),
            ],
        ))
        .await?;
    }
    Ok(())
}

async fn insert_fork_revision_audit_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    operation_id: Uuid,
    audit: &[ForkRevisionAudit],
    now: DateTime<Utc>,
) -> ForumResult<()> {
    let (backend, sql) = match txn.get_database_backend() {
        DatabaseBackend::Postgres => (
            DatabaseBackend::Postgres,
            "INSERT INTO forum_topic_fork_revision_items (tenant_id, operation_id, revision_kind, source_revision_id, target_revision_id, source_reply_id, target_reply_id, locale, copied_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        ),
        DatabaseBackend::Sqlite => (
            DatabaseBackend::Sqlite,
            "INSERT INTO forum_topic_fork_revision_items (tenant_id, operation_id, revision_kind, source_revision_id, target_revision_id, source_reply_id, target_reply_id, locale, copied_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic fork revision audit does not support {backend:?}"
            )));
        }
    };
    for item in audit {
        txn.execute(Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                tenant_id.into(),
                operation_id.into(),
                item.revision_kind.to_string().into(),
                item.source_revision_id.into(),
                item.target_revision_id.into(),
                item.source_reply_id.into(),
                item.target_reply_id.into(),
                item.locale.clone().into(),
                now.into(),
            ],
        ))
        .await?;
    }
    Ok(())
}

async fn load_fork_operation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> ForumResult<Option<StoredForkOperation>> {
    let (backend, sql) = match txn.get_database_backend() {
        DatabaseBackend::Postgres => (
            DatabaseBackend::Postgres,
            "SELECT * FROM forum_topic_fork_operations WHERE tenant_id = $1 AND operation_id = $2",
        ),
        DatabaseBackend::Sqlite => (
            DatabaseBackend::Sqlite,
            "SELECT * FROM forum_topic_fork_operations WHERE tenant_id = ? AND operation_id = ?",
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic fork receipt lookup does not support {backend:?}"
            )));
        }
    };
    txn.query_one(Statement::from_sql_and_values(
        backend,
        sql,
        vec![tenant_id.into(), operation_id.into()],
    ))
    .await?
    .map(stored_operation_from_row)
    .transpose()
    .map_err(ForumError::from)
}

fn stored_operation_from_row(row: QueryResult) -> Result<StoredForkOperation, sea_orm::DbErr> {
    Ok(StoredForkOperation {
        tenant_id: row.try_get("", "tenant_id")?,
        operation_id: row.try_get("", "operation_id")?,
        source_topic_id: row.try_get("", "source_topic_id")?,
        target_topic_id: row.try_get("", "target_topic_id")?,
        root_reply_id: row.try_get("", "root_reply_id")?,
        category_id: row.try_get("", "category_id")?,
        actor_id: row.try_get("", "actor_id")?,
        reason: row.try_get("", "reason")?,
        command_fingerprint: row.try_get("", "command_fingerprint")?,
        copied_reply_count: row.try_get("", "copied_reply_count")?,
        copied_published_reply_count: row.try_get("", "copied_published_reply_count")?,
        copied_body_count: row.try_get("", "copied_body_count")?,
        copied_reply_revision_count: row.try_get("", "copied_reply_revision_count")?,
        copied_relation_revision_count: row.try_get("", "copied_relation_revision_count")?,
        copied_mention_count: row.try_get("", "copied_mention_count")?,
        copied_quote_count: row.try_get("", "copied_quote_count")?,
        event_id: row.try_get("", "event_id")?,
        forked_at: row.try_get("", "forked_at")?,
    })
}

async fn validate_replay_in_tx(
    txn: &DatabaseTransaction,
    existing: &StoredForkOperation,
    source_topic_id: Uuid,
    actor_id: Uuid,
    prepared: &PreparedForkInput,
) -> ForumResult<()> {
    if existing.source_topic_id != source_topic_id
        || existing.target_topic_id != prepared.target_topic_id
        || existing.root_reply_id != prepared.root_reply_id
        || existing.actor_id != actor_id
        || existing.reason != prepared.reason
        || existing.command_fingerprint != prepared.command_fingerprint
    {
        return Err(ForumError::TopicForkOperationConflict(
            prepared.operation_id,
        ));
    }
    let reply_count = fork_audit_count_in_tx(
        txn,
        "forum_topic_fork_reply_items",
        existing.tenant_id,
        existing.operation_id,
    )
    .await?;
    if reply_count != i64::from(existing.copied_reply_count) {
        return Err(ForumError::Validation(
            "Forum topic fork immutable reply audit is inconsistent".to_string(),
        ));
    }
    let revision_count = fork_audit_count_in_tx(
        txn,
        "forum_topic_fork_revision_items",
        existing.tenant_id,
        existing.operation_id,
    )
    .await?;
    let expected_revision_count = i64::from(existing.copied_reply_revision_count)
        + i64::from(existing.copied_relation_revision_count);
    if revision_count != expected_revision_count {
        return Err(ForumError::Validation(
            "Forum topic fork immutable revision audit is inconsistent".to_string(),
        ));
    }
    validate_existing_semantic_event_in_tx(txn, existing).await
}

async fn fork_audit_count_in_tx(
    txn: &DatabaseTransaction,
    table: &'static str,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> ForumResult<i64> {
    let backend = txn.get_database_backend();
    let sql = match backend {
        DatabaseBackend::Postgres => format!(
            "SELECT COUNT(*) AS count FROM {table} WHERE tenant_id = $1 AND operation_id = $2"
        ),
        DatabaseBackend::Sqlite => format!(
            "SELECT COUNT(*) AS count FROM {table} WHERE tenant_id = ? AND operation_id = ?"
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic fork audit lookup does not support {backend:?}"
            )));
        }
    };
    let row = txn
        .query_one(Statement::from_sql_and_values(
            backend,
            sql,
            vec![tenant_id.into(), operation_id.into()],
        ))
        .await?
        .ok_or_else(|| {
            ForumError::Validation("Forum topic fork audit count is unavailable".to_string())
        })?;
    Ok(row.try_get("", "count")?)
}

async fn validate_existing_semantic_event_in_tx(
    txn: &DatabaseTransaction,
    operation: &StoredForkOperation,
) -> ForumResult<()> {
    let event = forum_domain_event::Entity::find()
        .filter(forum_domain_event::Column::EventId.eq(operation.event_id))
        .filter(forum_domain_event::Column::TenantId.eq(operation.tenant_id))
        .one(txn)
        .await?
        .ok_or_else(|| {
            ForumError::Validation(
                "Forum topic fork immutable semantic event is missing".to_string(),
            )
        })?;
    let expected_payload = topic_fork_payload(
        operation.operation_id,
        operation.source_topic_id,
        operation.target_topic_id,
        operation.root_reply_id,
        operation.category_id,
        operation.actor_id,
        &operation.reason,
        &operation.command_fingerprint,
        operation.copied_reply_count,
        operation.copied_published_reply_count,
        operation.copied_body_count,
        operation.copied_reply_revision_count,
        operation.copied_relation_revision_count,
        operation.copied_mention_count,
        operation.copied_quote_count,
    );
    if event.aggregate_type != FORUM_TOPIC_FORK_AGGREGATE_TYPE
        || event.aggregate_id != operation.target_topic_id
        || event.event_type != FORUM_TOPIC_FORK_EVENT_TYPE
        || event.schema_version != FORUM_TOPIC_FORK_SCHEMA_VERSION
        || event.actor_id != Some(operation.actor_id)
        || event.payload != expected_payload
    {
        return Err(ForumError::Validation(
            "Forum topic fork immutable semantic event is inconsistent".to_string(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn topic_fork_payload(
    operation_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    root_reply_id: Uuid,
    category_id: Uuid,
    actor_id: Uuid,
    reason: &str,
    command_fingerprint: &str,
    copied_reply_count: i32,
    copied_published_reply_count: i32,
    copied_body_count: i32,
    copied_reply_revision_count: i32,
    copied_relation_revision_count: i32,
    copied_mention_count: i32,
    copied_quote_count: i32,
) -> JsonValue {
    json!({
        "operation_id": operation_id,
        "source_topic_id": source_topic_id,
        "target_topic_id": target_topic_id,
        "root_reply_id": root_reply_id,
        "category_id": category_id,
        "actor_id": actor_id,
        "reason": reason,
        "command_fingerprint": command_fingerprint,
        "copied_reply_count": copied_reply_count,
        "copied_published_reply_count": copied_published_reply_count,
        "copied_body_count": copied_body_count,
        "copied_reply_revision_count": copied_reply_revision_count,
        "copied_relation_revision_count": copied_relation_revision_count,
        "copied_mention_count": copied_mention_count,
        "copied_quote_count": copied_quote_count,
        "reply_identity_policy": "new_deterministic_ids",
        "root_parent_policy": "detach",
        "quote_identity_policy": "preserve_original_targets",
        "solution_policy": "source_only_not_copied",
        "votes_subscriptions_read_state_policy": "not_copied",
    })
}

fn operation_to_result(operation: StoredForkOperation) -> ForumTopicForkResult {
    ForumTopicForkResult {
        operation_id: operation.operation_id,
        event_id: operation.event_id,
        source_topic_id: operation.source_topic_id,
        target_topic_id: operation.target_topic_id,
        root_reply_id: operation.root_reply_id,
        category_id: operation.category_id,
        actor_id: operation.actor_id,
        reason: operation.reason,
        copied_reply_count: operation.copied_reply_count,
        copied_published_reply_count: operation.copied_published_reply_count,
        copied_body_count: operation.copied_body_count,
        copied_reply_revision_count: operation.copied_reply_revision_count,
        copied_relation_revision_count: operation.copied_relation_revision_count,
        copied_mention_count: operation.copied_mention_count,
        copied_quote_count: operation.copied_quote_count,
        forked_at: operation.forked_at.with_timezone(&Utc),
    }
}
