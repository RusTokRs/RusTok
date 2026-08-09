use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use rustok_api::{Action, Resource};
use rustok_content::normalize_locale_code;
use rustok_core::{PermissionScope, SecurityContext};
use rustok_outbox::TransactionalEventBus;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait};
use uuid::Uuid;

use crate::entities::{forum_category, forum_reply, forum_topic};
use crate::error::{ForumError, ForumResult};
use crate::import_mapping::{
    FORUM_IMPORT_SOURCE_NODEBB, ForumImportEntityKind, ForumImportExternalRef,
};
use crate::import_relation_preparation::{
    ForumImportRelationEventMode, ForumPreparedImportRelationBatch,
};
use crate::import_resolution::ForumResolvedImportAuthor;
use crate::import_write_preparation::{
    ForumImportWriteEventMode, ForumPreparedImportCategory, ForumPreparedImportReply,
    MAX_FORUM_IMPORT_WRITE_RECORDS_PER_BATCH,
};
use crate::state_machine::ReplyStatus;

pub const MAX_FORUM_IMPORT_APPLY_RECORDS_PER_BATCH: usize =
    MAX_FORUM_IMPORT_WRITE_RECORDS_PER_BATCH;

/// Immediate result of one successful atomic import application.
///
/// This is not a durable receipt, replay token or checkpoint. The shared
/// migration runner remains responsible for durable execution semantics.
#[derive(Clone, Debug)]
pub struct ForumImportWriteResult {
    pub tenant_id: Uuid,
    pub category_ids: Vec<Uuid>,
    pub topic_ids: Vec<Uuid>,
    pub reply_ids: Vec<Uuid>,
}

#[derive(Clone)]
pub struct ForumImportWriteService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl ForumImportWriteService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    /// Applies one bounded, self-contained 34M relation batch in exactly one
    /// Forum-owned transaction.
    ///
    /// Historical source authors are data, not authorization identities. The
    /// importing operator therefore needs tenant-wide Manage scope for every
    /// owner kind present in the batch.
    pub async fn apply_prepared_batch(
        &self,
        security: &SecurityContext,
        batch: &ForumPreparedImportRelationBatch,
    ) -> ForumResult<ForumImportWriteResult> {
        validate_batch_shape(security, batch)?;
        validate_relation_alignment(batch)?;
        let category_order = ordered_category_indices(&batch.writes.categories)?;
        let reply_order = ordered_reply_indices(&batch.writes.replies)?;

        let topic_service =
            super::topic::TopicService::new(self.db.clone(), self.event_bus.clone());
        let reply_service =
            super::reply_owner::ReplyService::new(self.db.clone(), self.event_bus.clone());
        let relation_service = super::mention_relation::MentionRelationService::new(self.db.clone());

        // Re-run owner content admission before opening the transaction. These
        // preparations read current owner schemas but do not mutate Forum state.
        let mut prepared_topics = Vec::with_capacity(batch.writes.topics.len());
        for record in &batch.writes.topics {
            prepared_topics.push(
                topic_service
                    .prepare_import_topic(batch.writes.tenant_id, record)
                    .await?,
            );
        }
        let mut prepared_replies = Vec::with_capacity(batch.writes.replies.len());
        for record in &batch.writes.replies {
            prepared_replies.push(
                reply_service.prepare_import_reply(batch.writes.tenant_id, record)?,
            );
        }

        let txn = self.db.begin().await?;
        ensure_target_ids_absent_in_tx(&txn, batch).await?;

        for index in category_order {
            super::category::CategoryService::insert_import_category_in_tx(
                &txn,
                batch.writes.tenant_id,
                &batch.writes.categories[index],
            )
            .await?;
        }

        for (index, prepared) in prepared_topics.iter().enumerate() {
            topic_service
                .insert_import_topic_in_tx(
                    &txn,
                    batch.writes.tenant_id,
                    prepared,
                    &batch.topics[index],
                    &relation_service,
                    batch.relation_event_mode,
                    batch.writes.event_mode,
                )
                .await?;
        }

        for index in reply_order {
            reply_service
                .insert_import_reply_in_tx(
                    &txn,
                    batch.writes.tenant_id,
                    &prepared_replies[index],
                    &batch.replies[index],
                    batch.relation_event_mode,
                    batch.writes.event_mode,
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
                batch.writes.tenant_id,
                prepared,
                count,
                last_reply_at,
            )
            .await?;
        }

        // Projection invalidation is a consistency signal, not an interactive
        // historical event. It is always emitted even when import events are
        // suppressed, and it is attributed to the importing operator.
        super::projection_invalidation::publish_forum_projection_scope_direct_in_tx(
            &txn,
            batch.writes.tenant_id,
            security.user_id,
        )
        .await?;

        txn.commit().await?;

        Ok(ForumImportWriteResult {
            tenant_id: batch.writes.tenant_id,
            category_ids: batch.writes.categories.iter().map(|record| record.id).collect(),
            topic_ids: batch.writes.topics.iter().map(|record| record.id).collect(),
            reply_ids: batch.writes.replies.iter().map(|record| record.id).collect(),
        })
    }
}

fn validate_batch_shape(
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
        if reply.status == ReplyStatus::Deleted {
            return Err(ForumError::Validation(
                "Forum import deleted replies remain blocked until a tombstone timestamp is admitted"
                    .to_string(),
            ));
        }
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

fn validate_relation_alignment(batch: &ForumPreparedImportRelationBatch) -> ForumResult<()> {
    if batch.topics.len() != batch.writes.topics.len()
        || batch.replies.len() != batch.writes.replies.len()
    {
        return Err(ForumError::Validation(
            "Forum import relation facts must match every prepared topic and reply exactly"
                .to_string(),
        ));
    }

    for (record, relation) in batch.writes.topics.iter().zip(&batch.topics) {
        if relation.source != record.source
            || relation.target != crate::mentions::ForumContentTarget::topic(record.id)
            || relation.locale != record.locale
        {
            return Err(ForumError::Validation(
                "Forum import topic relation facts are not aligned with prepared writes".to_string(),
            ));
        }
    }
    for (record, relation) in batch.writes.replies.iter().zip(&batch.replies) {
        if relation.source != record.source
            || relation.target != crate::mentions::ForumContentTarget::reply(record.id)
            || relation.locale != record.locale
        {
            return Err(ForumError::Validation(
                "Forum import reply relation facts are not aligned with prepared writes".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_source_ref(
    source: &ForumImportExternalRef,
    expected: ForumImportEntityKind,
) -> ForumResult<()> {
    if source.source != FORUM_IMPORT_SOURCE_NODEBB || source.kind != expected {
        return Err(ForumError::Validation(format!(
            "Forum import application requires NodeBB {expected:?} source identity"
        )));
    }
    Ok(())
}

fn validate_record_locale(
    batch_locale: &str,
    source: &ForumImportExternalRef,
    locale: &str,
) -> ForumResult<()> {
    if locale != batch_locale {
        return Err(ForumError::Validation(format!(
            "Forum import record locale differs from batch locale for {source:?}"
        )));
    }
    Ok(())
}

fn validate_author(author: Option<&ForumResolvedImportAuthor>) -> ForumResult<()> {
    let Some(author) = author else {
        return Ok(());
    };
    validate_source_ref(&author.source, ForumImportEntityKind::User)?;
    if author.user_id.is_nil() {
        return Err(ForumError::Validation(
            "Forum import author target ID cannot be nil".to_string(),
        ));
    }
    Ok(())
}

fn validate_timestamp(
    kind: &'static str,
    source: &ForumImportExternalRef,
    timestamp_ms: i64,
) -> ForumResult<()> {
    if timestamp_ms < 0 {
        return Err(ForumError::Validation(format!(
            "Forum import {kind} timestamp cannot be negative for {source:?}"
        )));
    }
    Ok(())
}

fn require_all_manage(security: &SecurityContext, resource: Resource) -> ForumResult<()> {
    if security.get_scope(resource, Action::Manage) != PermissionScope::All {
        return Err(ForumError::forbidden(
            "Forum historical import requires tenant-wide Manage permission",
        ));
    }
    Ok(())
}

fn unique_ids(
    kind: &'static str,
    ids: impl IntoIterator<Item = Uuid>,
) -> ForumResult<BTreeSet<Uuid>> {
    let mut unique = BTreeSet::new();
    for id in ids {
        if id.is_nil() {
            return Err(ForumError::Validation(format!(
                "Forum import {kind} target ID cannot be nil"
            )));
        }
        if !unique.insert(id) {
            return Err(ForumError::Validation(format!(
                "Forum import repeats {kind} target ID {id}"
            )));
        }
    }
    Ok(unique)
}

fn ordered_category_indices(records: &[ForumPreparedImportCategory]) -> ForumResult<Vec<usize>> {
    let mut parents = BTreeMap::new();
    let mut placements = BTreeSet::new();
    for record in records {
        parents.insert(record.id, record.parent_id);
        if record.position < 0 {
            return Err(ForumError::Validation(
                "Forum import category position cannot be negative".to_string(),
            ));
        }
        if !placements.insert((record.parent_id, record.position)) {
            return Err(ForumError::Validation(
                "Forum import categories cannot claim the same sibling position".to_string(),
            ));
        }
    }

    let mut keyed = Vec::with_capacity(records.len());
    for (index, record) in records.iter().enumerate() {
        let mut depth = 0usize;
        let mut current = record.id;
        let mut seen = BTreeSet::new();
        seen.insert(current);
        loop {
            let parent = parents.get(&current).copied().flatten();
            let Some(parent) = parent else { break };
            if !parents.contains_key(&parent) {
                return Err(ForumError::Validation(
                    "Forum import category parent must be inside the bounded batch".to_string(),
                ));
            }
            if !seen.insert(parent) {
                return Err(ForumError::Validation(
                    "Forum import category parent graph contains a cycle".to_string(),
                ));
            }
            depth = depth.saturating_add(1);
            current = parent;
        }
        keyed.push((depth, record.parent_id, record.position, record.id, index));
    }
    keyed.sort_by_key(|item| (item.0, item.1, item.2, item.3));
    Ok(keyed.into_iter().map(|item| item.4).collect())
}

fn ordered_reply_indices(records: &[ForumPreparedImportReply]) -> ForumResult<Vec<usize>> {
    let by_id = records
        .iter()
        .enumerate()
        .map(|(index, record)| (record.id, index))
        .collect::<BTreeMap<_, _>>();
    let mut keyed = Vec::with_capacity(records.len());

    for (index, record) in records.iter().enumerate() {
        let mut depth = 0usize;
        let mut current = record;
        let mut seen = BTreeSet::new();
        seen.insert(record.id);
        while let Some(parent_id) = current.parent_reply_id {
            let Some(parent_index) = by_id.get(&parent_id).copied() else {
                return Err(ForumError::Validation(
                    "Forum import reply parent must be inside the bounded batch".to_string(),
                ));
            };
            let parent = &records[parent_index];
            if parent.topic_id != record.topic_id {
                return Err(ForumError::Validation(
                    "Forum import reply parent belongs to another topic".to_string(),
                ));
            }
            if !seen.insert(parent.id) {
                return Err(ForumError::Validation(
                    "Forum import reply parent graph contains a cycle".to_string(),
                ));
            }
            depth = depth.saturating_add(1);
            current = parent;
        }
        keyed.push((record.topic_id, depth, index));
    }

    keyed.sort_by_key(|item| (item.0, item.1, item.2));
    Ok(keyed.into_iter().map(|item| item.2).collect())
}

async fn ensure_target_ids_absent_in_tx(
    txn: &sea_orm::DatabaseTransaction,
    batch: &ForumPreparedImportRelationBatch,
) -> ForumResult<()> {
    let category_ids = batch
        .writes
        .categories
        .iter()
        .map(|record| record.id)
        .collect::<Vec<_>>();
    if !category_ids.is_empty()
        && forum_category::Entity::find()
            .filter(forum_category::Column::Id.is_in(category_ids))
            .one(txn)
            .await?
            .is_some()
    {
        return Err(ForumError::Validation(
            "Forum import category target ID already exists".to_string(),
        ));
    }

    let topic_ids = batch
        .writes
        .topics
        .iter()
        .map(|record| record.id)
        .collect::<Vec<_>>();
    if !topic_ids.is_empty()
        && forum_topic::Entity::find()
            .filter(forum_topic::Column::Id.is_in(topic_ids))
            .one(txn)
            .await?
            .is_some()
    {
        return Err(ForumError::Validation(
            "Forum import topic target ID already exists".to_string(),
        ));
    }

    let reply_ids = batch
        .writes
        .replies
        .iter()
        .map(|record| record.id)
        .collect::<Vec<_>>();
    if !reply_ids.is_empty()
        && forum_reply::Entity::find()
            .filter(forum_reply::Column::Id.is_in(reply_ids))
            .one(txn)
            .await?
            .is_some()
    {
        return Err(ForumError::Validation(
            "Forum import reply target ID already exists".to_string(),
        ));
    }

    Ok(())
}

fn approved_reply_aggregates(
    replies: &[super::reply_owner::PreparedImportReplyInsert],
) -> ForumResult<BTreeMap<Uuid, (i32, Option<DateTime<Utc>>)>> {
    let mut aggregates = BTreeMap::new();
    for reply in replies {
        if reply.status() != ReplyStatus::Approved {
            continue;
        }
        let entry = aggregates.entry(reply.topic_id()).or_insert((0i32, None));
        entry.0 = entry.0.checked_add(1).ok_or_else(|| {
            ForumError::Validation("Forum imported reply count exceeds i32 range".to_string())
        })?;
        let created_at = reply.created_at();
        let should_replace = match entry.1.as_ref() {
            Some(current) => created_at > *current,
            None => true,
        };
        if should_replace {
            entry.1 = Some(created_at);
        }
    }
    Ok(aggregates)
}
