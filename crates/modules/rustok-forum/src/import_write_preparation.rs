use std::collections::{BTreeMap, BTreeSet};

use rustok_api::RichTextDocument;
use rustok_content::normalize_locale_code;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::import_mapping::{
    FORUM_IMPORT_SOURCE_NODEBB, ForumImportEntityKind, ForumImportExternalRef,
    MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH,
};
use crate::import_resolution::{
    ForumResolvedImportApplicationBatch, ForumResolvedImportAuthor, ForumResolvedImportCategory,
    ForumResolvedImportReply, ForumResolvedImportTopic,
};
use crate::state_machine::{ReplyStatus, TopicStatus};

pub const MAX_FORUM_IMPORT_WRITE_RECORDS_PER_BATCH: usize =
    MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForumImportWriteEventMode {
    SuppressInteractiveEvents,
    EmitDomainEvents,
}

#[derive(Clone, Debug)]
pub struct ForumImportCategoryWriteDecision {
    pub source: ForumImportExternalRef,
    pub slug: String,
    pub position: i32,
    pub moderated: bool,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug)]
pub struct ForumImportTopicWriteDecision {
    pub source: ForumImportExternalRef,
    pub body: RichTextDocument,
    pub status: TopicStatus,
    pub metadata: Value,
    pub tags: Vec<String>,
    pub channel_slugs: Option<Vec<String>>,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug)]
pub struct ForumImportReplyWriteDecision {
    pub source: ForumImportExternalRef,
    pub content: RichTextDocument,
    pub status: ReplyStatus,
    pub parent_reply_id: Option<Uuid>,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug)]
pub struct ForumImportWritePreparationRequest {
    pub resolved: ForumResolvedImportApplicationBatch,
    pub event_mode: ForumImportWriteEventMode,
    pub categories: Vec<ForumImportCategoryWriteDecision>,
    pub topics: Vec<ForumImportTopicWriteDecision>,
    pub replies: Vec<ForumImportReplyWriteDecision>,
}

#[derive(Clone, Debug)]
pub struct ForumPreparedImportCategory {
    pub source: ForumImportExternalRef,
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub locale: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub position: i32,
    pub moderated: bool,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug)]
pub struct ForumPreparedImportTopic {
    pub source: ForumImportExternalRef,
    pub id: Uuid,
    pub category_id: Uuid,
    pub author: Option<ForumResolvedImportAuthor>,
    pub locale: String,
    pub title: String,
    pub slug: Option<String>,
    pub body_source: ForumImportExternalRef,
    pub body: RichTextDocument,
    pub status: TopicStatus,
    pub metadata: Value,
    pub tags: Vec<String>,
    pub channel_slugs: Option<Vec<String>>,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug)]
pub struct ForumPreparedImportReply {
    pub source: ForumImportExternalRef,
    pub id: Uuid,
    pub topic_id: Uuid,
    pub author: Option<ForumResolvedImportAuthor>,
    pub locale: String,
    pub content: RichTextDocument,
    pub status: ReplyStatus,
    pub parent_reply_id: Option<Uuid>,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug)]
pub struct ForumPreparedImportWriteBatch {
    pub tenant_id: Uuid,
    pub locale: String,
    pub event_mode: ForumImportWriteEventMode,
    pub categories: Vec<ForumPreparedImportCategory>,
    pub topics: Vec<ForumPreparedImportTopic>,
    pub replies: Vec<ForumPreparedImportReply>,
}

#[derive(Debug, Error)]
pub enum ForumImportWritePreparationError {
    #[error("Forum import write preparation requires a non-nil tenant id")]
    NilTenantId,
    #[error("Forum import write preparation requires at least one resolved record")]
    EmptyBatch,
    #[error("Forum import write preparation exceeds {max} resolved records: {actual}")]
    TooManyRecords { max: usize, actual: usize },
    #[error("Forum import write preparation locale is invalid: {locale}")]
    InvalidLocale { locale: String },
    #[error(
        "Forum import write preparation locale must already be normalized: {actual} -> {normalized}"
    )]
    LocaleNotNormalized { actual: String, normalized: String },
    #[error("Forum import write preparation has nil {kind} target id for {source:?}")]
    NilTargetId {
        kind: &'static str,
        source: ForumImportExternalRef,
    },
    #[error("Forum import write preparation repeats {kind} target id {id}")]
    DuplicateTargetId { kind: &'static str, id: Uuid },
    #[error("Forum import write preparation has nil user id for {source:?}")]
    NilAuthorId { source: ForumImportExternalRef },
    #[error("Forum import write preparation requires NodeBB {expected:?} source, got {source:?}")]
    InvalidSourceRef {
        expected: ForumImportEntityKind,
        source: ForumImportExternalRef,
    },
    #[error("Forum import write preparation contains duplicate {kind} decision for {source:?}")]
    DuplicateDecision {
        kind: &'static str,
        source: ForumImportExternalRef,
    },
    #[error("Forum import write preparation is missing {kind} decision for {source:?}")]
    MissingDecision {
        kind: &'static str,
        source: ForumImportExternalRef,
    },
    #[error("Forum import write preparation contains unused {kind} decision for {source:?}")]
    UnexpectedDecision {
        kind: &'static str,
        source: ForumImportExternalRef,
    },
    #[error("Forum import category slug is empty for {source:?}")]
    EmptyCategorySlug { source: ForumImportExternalRef },
    #[error("Forum import category position must be non-negative for {source:?}: {position}")]
    NegativeCategoryPosition {
        source: ForumImportExternalRef,
        position: i32,
    },
    #[error(
        "Forum import source category position is outside owner range for {source:?}: {position}"
    )]
    SourceCategoryPositionOutOfRange {
        source: ForumImportExternalRef,
        position: i64,
    },
    #[error(
        "Forum import category position decision differs from source for {source:?}: {source_position} != {decision_position}"
    )]
    CategoryPositionChanged {
        source: ForumImportExternalRef,
        source_position: i64,
        decision_position: i32,
    },
    #[error(
        "Forum import write timestamp must be non-negative for {kind} {source:?}: {timestamp_ms}"
    )]
    NegativeTimestamp {
        kind: &'static str,
        source: ForumImportExternalRef,
        timestamp_ms: i64,
    },
    #[error(
        "Forum import write timestamp differs from source for {kind} {source:?}: {source_timestamp_ms} != {decision_timestamp_ms}"
    )]
    TimestampChanged {
        kind: &'static str,
        source: ForumImportExternalRef,
        source_timestamp_ms: i64,
        decision_timestamp_ms: i64,
    },
    #[error("Forum import reply deleted fact requires deleted status for {source:?}")]
    DeletedReplyStatusRequired { source: ForumImportExternalRef },
    #[error("Forum import live reply cannot be prepared with deleted status for {source:?}")]
    LiveReplyCannotBeDeleted { source: ForumImportExternalRef },
    #[error("Forum import category parent {parent_id} is outside the bounded batch for {source:?}")]
    CategoryParentOutsideBatch {
        source: ForumImportExternalRef,
        parent_id: Uuid,
    },
    #[error("Forum import category target cycle reaches {id}")]
    CategoryCycle { id: Uuid },
    #[error(
        "Forum import topic category {category_id} is outside the bounded batch for {source:?}"
    )]
    TopicCategoryOutsideBatch {
        source: ForumImportExternalRef,
        category_id: Uuid,
    },
    #[error("Forum import reply topic {topic_id} is outside the bounded batch for {source:?}")]
    ReplyTopicOutsideBatch {
        source: ForumImportExternalRef,
        topic_id: Uuid,
    },
    #[error(
        "Forum import reply parent {parent_reply_id} is outside the bounded batch for {source:?}"
    )]
    ReplyParentOutsideBatch {
        source: ForumImportExternalRef,
        parent_reply_id: Uuid,
    },
    #[error("Forum import reply parent {parent_reply_id} belongs to another topic for {source:?}")]
    ReplyParentTopicMismatch {
        source: ForumImportExternalRef,
        parent_reply_id: Uuid,
    },
    #[error("Forum import reply cannot parent itself for {source:?}")]
    ReplySelfParent { source: ForumImportExternalRef },
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ForumImportWritePreparer;

impl ForumImportWritePreparer {
    pub fn prepare(
        &self,
        request: &ForumImportWritePreparationRequest,
    ) -> Result<ForumPreparedImportWriteBatch, ForumImportWritePreparationError> {
        let locale = validate_batch(&request.resolved)?;
        let category_ids = validate_target_ids(
            "category",
            request
                .resolved
                .categories
                .iter()
                .map(|record| (&record.source, record.id)),
        )?;
        let topic_ids = validate_target_ids(
            "topic",
            request
                .resolved
                .topics
                .iter()
                .map(|record| (&record.source, record.id)),
        )?;
        let reply_ids = validate_target_ids(
            "reply",
            request
                .resolved
                .replies
                .iter()
                .map(|record| (&record.source, record.id)),
        )?;
        validate_author_ids(&request.resolved)?;
        validate_relations(&request.resolved, &category_ids, &topic_ids, &reply_ids)?;

        let mut category_decisions = DecisionIndex::new("category", &request.categories)?;
        let mut topic_decisions = DecisionIndex::new("topic", &request.topics)?;
        let mut reply_decisions = DecisionIndex::new("reply", &request.replies)?;

        let categories = request
            .resolved
            .categories
            .iter()
            .map(|record| prepare_category(record, &locale, &mut category_decisions))
            .collect::<Result<Vec<_>, _>>()?;
        let topics = request
            .resolved
            .topics
            .iter()
            .map(|record| prepare_topic(record, &locale, &mut topic_decisions))
            .collect::<Result<Vec<_>, _>>()?;
        let replies = request
            .resolved
            .replies
            .iter()
            .map(|record| prepare_reply(record, &locale, &mut reply_decisions))
            .collect::<Result<Vec<_>, _>>()?;

        category_decisions.reject_unused()?;
        topic_decisions.reject_unused()?;
        reply_decisions.reject_unused()?;
        validate_prepared_reply_parents(&replies)?;

        Ok(ForumPreparedImportWriteBatch {
            tenant_id: request.resolved.tenant_id,
            locale,
            event_mode: request.event_mode,
            categories,
            topics,
            replies,
        })
    }
}

trait DecisionSource {
    fn source(&self) -> &ForumImportExternalRef;
}

impl DecisionSource for ForumImportCategoryWriteDecision {
    fn source(&self) -> &ForumImportExternalRef {
        &self.source
    }
}

impl DecisionSource for ForumImportTopicWriteDecision {
    fn source(&self) -> &ForumImportExternalRef {
        &self.source
    }
}

impl DecisionSource for ForumImportReplyWriteDecision {
    fn source(&self) -> &ForumImportExternalRef {
        &self.source
    }
}

struct DecisionIndex<'a, T> {
    kind: &'static str,
    by_source: BTreeMap<RefKey, &'a T>,
    used: BTreeSet<RefKey>,
}

impl<'a, T: DecisionSource> DecisionIndex<'a, T> {
    fn new(
        kind: &'static str,
        decisions: &'a [T],
    ) -> Result<Self, ForumImportWritePreparationError> {
        let mut by_source = BTreeMap::new();
        for decision in decisions {
            let source = decision.source();
            let key = ref_key(source);
            if by_source.insert(key, decision).is_some() {
                return Err(ForumImportWritePreparationError::DuplicateDecision {
                    kind,
                    source: source.clone(),
                });
            }
        }
        Ok(Self {
            kind,
            by_source,
            used: BTreeSet::new(),
        })
    }

    fn require(
        &mut self,
        source: &ForumImportExternalRef,
    ) -> Result<&'a T, ForumImportWritePreparationError> {
        let key = ref_key(source);
        let Some(decision) = self.by_source.get(&key).copied() else {
            return Err(ForumImportWritePreparationError::MissingDecision {
                kind: self.kind,
                source: source.clone(),
            });
        };
        self.used.insert(key);
        Ok(decision)
    }

    fn reject_unused(&self) -> Result<(), ForumImportWritePreparationError> {
        for (key, decision) in &self.by_source {
            if !self.used.contains(key) {
                return Err(ForumImportWritePreparationError::UnexpectedDecision {
                    kind: self.kind,
                    source: decision.source().clone(),
                });
            }
        }
        Ok(())
    }
}

fn validate_batch(
    batch: &ForumResolvedImportApplicationBatch,
) -> Result<String, ForumImportWritePreparationError> {
    if batch.tenant_id.is_nil() {
        return Err(ForumImportWritePreparationError::NilTenantId);
    }
    let actual = batch
        .categories
        .len()
        .saturating_add(batch.topics.len())
        .saturating_add(batch.replies.len());
    if actual == 0 {
        return Err(ForumImportWritePreparationError::EmptyBatch);
    }
    if actual > MAX_FORUM_IMPORT_WRITE_RECORDS_PER_BATCH {
        return Err(ForumImportWritePreparationError::TooManyRecords {
            max: MAX_FORUM_IMPORT_WRITE_RECORDS_PER_BATCH,
            actual,
        });
    }
    let normalized = normalize_locale_code(&batch.locale).ok_or_else(|| {
        ForumImportWritePreparationError::InvalidLocale {
            locale: batch.locale.clone(),
        }
    })?;
    if normalized != batch.locale {
        return Err(ForumImportWritePreparationError::LocaleNotNormalized {
            actual: batch.locale.clone(),
            normalized,
        });
    }
    Ok(batch.locale.clone())
}

fn validate_target_ids<'a>(
    kind: &'static str,
    records: impl IntoIterator<Item = (&'a ForumImportExternalRef, Uuid)>,
) -> Result<BTreeSet<Uuid>, ForumImportWritePreparationError> {
    let mut ids = BTreeSet::new();
    for (source, id) in records {
        if id.is_nil() {
            return Err(ForumImportWritePreparationError::NilTargetId {
                kind,
                source: source.clone(),
            });
        }
        if !ids.insert(id) {
            return Err(ForumImportWritePreparationError::DuplicateTargetId { kind, id });
        }
    }
    Ok(ids)
}

fn validate_author_ids(
    batch: &ForumResolvedImportApplicationBatch,
) -> Result<(), ForumImportWritePreparationError> {
    for topic in &batch.topics {
        validate_author(topic.author.as_ref())?;
    }
    for reply in &batch.replies {
        validate_author(reply.author.as_ref())?;
    }
    Ok(())
}

fn validate_author(
    author: Option<&ForumResolvedImportAuthor>,
) -> Result<(), ForumImportWritePreparationError> {
    let Some(author) = author else {
        return Ok(());
    };
    validate_source_ref(&author.source, ForumImportEntityKind::User)?;
    if author.user_id.is_nil() {
        return Err(ForumImportWritePreparationError::NilAuthorId {
            source: author.source.clone(),
        });
    }
    Ok(())
}

fn validate_relations(
    batch: &ForumResolvedImportApplicationBatch,
    category_ids: &BTreeSet<Uuid>,
    topic_ids: &BTreeSet<Uuid>,
    reply_ids: &BTreeSet<Uuid>,
) -> Result<(), ForumImportWritePreparationError> {
    let mut parent_by_category = BTreeMap::new();
    for category in &batch.categories {
        validate_source_ref(&category.source, ForumImportEntityKind::Category)?;
        if let Some(parent_id) = category.parent_id
            && !category_ids.contains(&parent_id)
        {
            return Err(
                ForumImportWritePreparationError::CategoryParentOutsideBatch {
                    source: category.source.clone(),
                    parent_id,
                },
            );
        }
        parent_by_category.insert(category.id, category.parent_id);
    }
    validate_category_cycles(&parent_by_category)?;

    for topic in &batch.topics {
        validate_source_ref(&topic.source, ForumImportEntityKind::Topic)?;
        validate_source_ref(&topic.body_source, ForumImportEntityKind::Post)?;
        if !category_ids.contains(&topic.category_id) {
            return Err(
                ForumImportWritePreparationError::TopicCategoryOutsideBatch {
                    source: topic.source.clone(),
                    category_id: topic.category_id,
                },
            );
        }
    }
    for reply in &batch.replies {
        validate_source_ref(&reply.source, ForumImportEntityKind::Post)?;
        if !topic_ids.contains(&reply.topic_id) {
            return Err(ForumImportWritePreparationError::ReplyTopicOutsideBatch {
                source: reply.source.clone(),
                topic_id: reply.topic_id,
            });
        }
    }

    if reply_ids.len() != batch.replies.len() {
        return Err(ForumImportWritePreparationError::TooManyRecords {
            max: MAX_FORUM_IMPORT_WRITE_RECORDS_PER_BATCH,
            actual: batch.replies.len(),
        });
    }
    Ok(())
}

fn validate_category_cycles(
    parent_by_category: &BTreeMap<Uuid, Option<Uuid>>,
) -> Result<(), ForumImportWritePreparationError> {
    for start in parent_by_category.keys().copied() {
        let mut seen = BTreeSet::new();
        let mut current = start;
        loop {
            if !seen.insert(current) {
                return Err(ForumImportWritePreparationError::CategoryCycle { id: current });
            }
            let Some(Some(parent)) = parent_by_category.get(&current).copied() else {
                break;
            };
            current = parent;
        }
    }
    Ok(())
}

fn prepare_category(
    record: &ForumResolvedImportCategory,
    locale: &str,
    decisions: &mut DecisionIndex<'_, ForumImportCategoryWriteDecision>,
) -> Result<ForumPreparedImportCategory, ForumImportWritePreparationError> {
    let decision = decisions.require(&record.source)?;
    validate_source_ref(&decision.source, ForumImportEntityKind::Category)?;
    let slug = decision.slug.trim();
    if slug.is_empty() {
        return Err(ForumImportWritePreparationError::EmptyCategorySlug {
            source: record.source.clone(),
        });
    }
    if decision.position < 0 {
        return Err(ForumImportWritePreparationError::NegativeCategoryPosition {
            source: record.source.clone(),
            position: decision.position,
        });
    }
    if let Some(source_position) = record.position {
        let expected = i32::try_from(source_position).map_err(|_| {
            ForumImportWritePreparationError::SourceCategoryPositionOutOfRange {
                source: record.source.clone(),
                position: source_position,
            }
        })?;
        if expected < 0 {
            return Err(
                ForumImportWritePreparationError::SourceCategoryPositionOutOfRange {
                    source: record.source.clone(),
                    position: source_position,
                },
            );
        }
        if expected != decision.position {
            return Err(ForumImportWritePreparationError::CategoryPositionChanged {
                source: record.source.clone(),
                source_position,
                decision_position: decision.position,
            });
        }
    }
    validate_timestamp("category", &record.source, None, decision.created_at_ms)?;

    Ok(ForumPreparedImportCategory {
        source: record.source.clone(),
        id: record.id,
        parent_id: record.parent_id,
        locale: locale.to_owned(),
        name: record.name.clone(),
        slug: slug.to_owned(),
        description: record.description.clone(),
        position: decision.position,
        moderated: decision.moderated,
        icon: decision.icon.clone(),
        color: decision.color.clone(),
        created_at_ms: decision.created_at_ms,
    })
}

fn prepare_topic(
    record: &ForumResolvedImportTopic,
    locale: &str,
    decisions: &mut DecisionIndex<'_, ForumImportTopicWriteDecision>,
) -> Result<ForumPreparedImportTopic, ForumImportWritePreparationError> {
    let decision = decisions.require(&record.source)?;
    validate_source_ref(&decision.source, ForumImportEntityKind::Topic)?;
    validate_timestamp(
        "topic",
        &record.source,
        record.created_at_ms,
        decision.created_at_ms,
    )?;

    Ok(ForumPreparedImportTopic {
        source: record.source.clone(),
        id: record.id,
        category_id: record.category_id,
        author: record.author.clone(),
        locale: locale.to_owned(),
        title: record.title.clone(),
        slug: record.slug.clone(),
        body_source: record.body_source.clone(),
        body: decision.body.clone(),
        status: decision.status,
        metadata: decision.metadata.clone(),
        tags: decision.tags.clone(),
        channel_slugs: decision.channel_slugs.clone(),
        is_pinned: record.is_pinned,
        is_locked: record.is_locked,
        created_at_ms: decision.created_at_ms,
    })
}

fn prepare_reply(
    record: &ForumResolvedImportReply,
    locale: &str,
    decisions: &mut DecisionIndex<'_, ForumImportReplyWriteDecision>,
) -> Result<ForumPreparedImportReply, ForumImportWritePreparationError> {
    let decision = decisions.require(&record.source)?;
    validate_source_ref(&decision.source, ForumImportEntityKind::Post)?;
    validate_timestamp(
        "reply",
        &record.source,
        record.created_at_ms,
        decision.created_at_ms,
    )?;
    match (record.deleted, decision.status) {
        (true, ReplyStatus::Deleted) => {}
        (true, _) => {
            return Err(
                ForumImportWritePreparationError::DeletedReplyStatusRequired {
                    source: record.source.clone(),
                },
            );
        }
        (false, ReplyStatus::Deleted) => {
            return Err(ForumImportWritePreparationError::LiveReplyCannotBeDeleted {
                source: record.source.clone(),
            });
        }
        (false, _) => {}
    }

    Ok(ForumPreparedImportReply {
        source: record.source.clone(),
        id: record.id,
        topic_id: record.topic_id,
        author: record.author.clone(),
        locale: locale.to_owned(),
        content: decision.content.clone(),
        status: decision.status,
        parent_reply_id: decision.parent_reply_id,
        created_at_ms: decision.created_at_ms,
    })
}

fn validate_timestamp(
    kind: &'static str,
    source: &ForumImportExternalRef,
    source_timestamp_ms: Option<i64>,
    decision_timestamp_ms: i64,
) -> Result<(), ForumImportWritePreparationError> {
    if decision_timestamp_ms < 0 {
        return Err(ForumImportWritePreparationError::NegativeTimestamp {
            kind,
            source: source.clone(),
            timestamp_ms: decision_timestamp_ms,
        });
    }
    if let Some(source_timestamp_ms) = source_timestamp_ms
        && source_timestamp_ms != decision_timestamp_ms
    {
        return Err(ForumImportWritePreparationError::TimestampChanged {
            kind,
            source: source.clone(),
            source_timestamp_ms,
            decision_timestamp_ms,
        });
    }
    Ok(())
}

fn validate_prepared_reply_parents(
    replies: &[ForumPreparedImportReply],
) -> Result<(), ForumImportWritePreparationError> {
    let topic_by_reply = replies
        .iter()
        .map(|reply| (reply.id, reply.topic_id))
        .collect::<BTreeMap<_, _>>();
    for reply in replies {
        let Some(parent_reply_id) = reply.parent_reply_id else {
            continue;
        };
        if parent_reply_id == reply.id {
            return Err(ForumImportWritePreparationError::ReplySelfParent {
                source: reply.source.clone(),
            });
        }
        let Some(parent_topic_id) = topic_by_reply.get(&parent_reply_id).copied() else {
            return Err(ForumImportWritePreparationError::ReplyParentOutsideBatch {
                source: reply.source.clone(),
                parent_reply_id,
            });
        };
        if parent_topic_id != reply.topic_id {
            return Err(ForumImportWritePreparationError::ReplyParentTopicMismatch {
                source: reply.source.clone(),
                parent_reply_id,
            });
        }
    }
    Ok(())
}

fn validate_source_ref(
    source: &ForumImportExternalRef,
    expected: ForumImportEntityKind,
) -> Result<(), ForumImportWritePreparationError> {
    if source.source != FORUM_IMPORT_SOURCE_NODEBB || source.kind != expected {
        return Err(ForumImportWritePreparationError::InvalidSourceRef {
            expected,
            source: source.clone(),
        });
    }
    Ok(())
}

type RefKey = (String, &'static str, String);

fn ref_key(source: &ForumImportExternalRef) -> RefKey {
    (
        source.source.clone(),
        source_kind_label(source.kind),
        source.key.clone(),
    )
}

const fn source_kind_label(kind: ForumImportEntityKind) -> &'static str {
    match kind {
        ForumImportEntityKind::Category => "category",
        ForumImportEntityKind::Topic => "topic",
        ForumImportEntityKind::Post => "post",
        ForumImportEntityKind::User => "user",
    }
}
