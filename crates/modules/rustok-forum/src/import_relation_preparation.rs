use std::collections::{BTreeMap, BTreeSet};

use rustok_content::normalize_locale_code;
use rustok_profiles::ProfileService;
use thiserror::Error;
use uuid::Uuid;

use crate::import_mapping::{
    FORUM_IMPORT_SOURCE_NODEBB, ForumImportEntityKind, ForumImportExternalRef,
    MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH,
};
use crate::import_write_preparation::{
    ForumImportWriteEventMode, ForumPreparedImportReply, ForumPreparedImportTopic,
    ForumPreparedImportWriteBatch,
};
use crate::mentions::{
    FORUM_MAX_MENTION_TARGETS_PER_REVISION, FORUM_MAX_QUOTE_REFERENCES_PER_REVISION,
    ForumContentTarget, ForumMentionAudience, ForumMentionPolicy, ForumQuoteReference,
    extract_forum_mention_candidates,
};

pub const MAX_FORUM_IMPORT_RELATION_TARGETS_PER_BATCH: usize =
    MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForumImportRelationMode {
    SuppressRelations,
    MaterializeRelations,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForumImportRelationEventMode {
    SuppressAddedTargetEvents,
    EmitAddedTargetEvents,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForumImportMentionBinding {
    pub handle: String,
    pub user_id: Uuid,
}

#[derive(Clone, Debug)]
pub struct ForumImportContentRelationDecision {
    pub source: ForumImportExternalRef,
    pub mode: ForumImportRelationMode,
    pub mentions: Vec<ForumImportMentionBinding>,
    pub audiences: Vec<ForumMentionAudience>,
    pub quotes: Vec<ForumQuoteReference>,
}

#[derive(Clone, Debug)]
pub struct ForumImportRelationPreparationRequest {
    pub writes: ForumPreparedImportWriteBatch,
    pub topics: Vec<ForumImportContentRelationDecision>,
    pub replies: Vec<ForumImportContentRelationDecision>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForumPreparedImportMention {
    pub handle: String,
    pub user_id: Uuid,
}

#[derive(Clone, Debug)]
pub struct ForumPreparedImportContentRelations {
    pub source: ForumImportExternalRef,
    pub target: ForumContentTarget,
    pub locale: String,
    pub mode: ForumImportRelationMode,
    pub mentions: Vec<ForumPreparedImportMention>,
    pub audiences: Vec<ForumMentionAudience>,
    pub quotes: Vec<ForumQuoteReference>,
}

#[derive(Clone, Debug)]
pub struct ForumPreparedImportRelationBatch {
    pub writes: ForumPreparedImportWriteBatch,
    pub relation_event_mode: ForumImportRelationEventMode,
    pub topics: Vec<ForumPreparedImportContentRelations>,
    pub replies: Vec<ForumPreparedImportContentRelations>,
}

#[derive(Debug, Error)]
pub enum ForumImportRelationPreparationError {
    #[error("Forum import relation preparation requires a non-nil tenant id")]
    NilTenantId,
    #[error("Forum import relation preparation exceeds {max} content targets: {actual}")]
    TooManyTargets { max: usize, actual: usize },
    #[error("Forum import relation preparation locale is invalid: {locale}")]
    InvalidLocale { locale: String },
    #[error(
        "Forum import relation preparation locale must already be normalized: {actual} -> {normalized}"
    )]
    LocaleNotNormalized { actual: String, normalized: String },
    #[error(
        "Forum import relation preparation record locale differs from batch locale for {source:?}: {actual}"
    )]
    RecordLocaleMismatch {
        source: ForumImportExternalRef,
        actual: String,
    },
    #[error("Forum import relation preparation has nil {kind} target id for {source:?}")]
    NilTargetId {
        kind: &'static str,
        source: ForumImportExternalRef,
    },
    #[error(
        "Forum import relation preparation requires NodeBB {expected:?} source, got {source:?}"
    )]
    InvalidSourceRef {
        expected: ForumImportEntityKind,
        source: ForumImportExternalRef,
    },
    #[error("Forum import relation preparation contains duplicate {kind} decision for {source:?}")]
    DuplicateDecision {
        kind: &'static str,
        source: ForumImportExternalRef,
    },
    #[error("Forum import relation preparation is missing {kind} decision for {source:?}")]
    MissingDecision {
        kind: &'static str,
        source: ForumImportExternalRef,
    },
    #[error("Forum import relation preparation contains unused {kind} decision for {source:?}")]
    UnexpectedDecision {
        kind: &'static str,
        source: ForumImportExternalRef,
    },
    #[error("Forum import relation suppression requires empty relation facts for {source:?}")]
    SuppressedRelationsContainFacts { source: ForumImportExternalRef },
    #[error(
        "Forum import relation materialization does not support quote revisions yet for {source:?}"
    )]
    QuoteRelationsUnsupported { source: ForumImportExternalRef },
    #[error(
        "Forum import relation decision exceeds {max} mention targets for {source:?}: {actual}"
    )]
    TooManyMentionTargets {
        source: ForumImportExternalRef,
        max: usize,
        actual: usize,
    },
    #[error("Forum import relation decision exceeds {max} quote targets for {source:?}: {actual}")]
    TooManyQuoteTargets {
        source: ForumImportExternalRef,
        max: usize,
        actual: usize,
    },
    #[error(
        "Forum import relation decision contains invalid mention handle {handle:?} for {source:?}"
    )]
    InvalidMentionHandle {
        source: ForumImportExternalRef,
        handle: String,
    },
    #[error(
        "Forum import relation decision contains nil mentioned user id for {source:?}/{handle}"
    )]
    NilMentionUserId {
        source: ForumImportExternalRef,
        handle: String,
    },
    #[error(
        "Forum import relation decision repeats normalized mention handle {handle} for {source:?}"
    )]
    DuplicateMentionHandle {
        source: ForumImportExternalRef,
        handle: String,
    },
    #[error(
        "Forum import relation decision maps multiple mention handles onto user {user_id} for {source:?}"
    )]
    DuplicateMentionUser {
        source: ForumImportExternalRef,
        user_id: Uuid,
    },
    #[error("Forum import relation decision repeats audience {audience:?} for {source:?}")]
    DuplicateAudience {
        source: ForumImportExternalRef,
        audience: ForumMentionAudience,
    },
    #[error("Forum import RichText mention projection is invalid for {source:?}: {reason}")]
    InvalidRichTextProjection {
        source: ForumImportExternalRef,
        reason: String,
    },
    #[error("Forum import mention handles differ from RichText for {source:?}")]
    MentionHandleMismatch { source: ForumImportExternalRef },
    #[error("Forum import mention audiences differ from RichText for {source:?}")]
    MentionAudienceMismatch { source: ForumImportExternalRef },
    #[error(
        "Forum import relation suppression conflicts with EmitDomainEvents for mentioned content {source:?}"
    )]
    EventModeRequiresMaterialization { source: ForumImportExternalRef },
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ForumImportRelationPreparer;

impl ForumImportRelationPreparer {
    pub fn prepare(
        &self,
        request: ForumImportRelationPreparationRequest,
    ) -> Result<ForumPreparedImportRelationBatch, ForumImportRelationPreparationError> {
        validate_batch(&request.writes)?;
        let relation_event_mode = relation_event_mode(request.writes.event_mode);
        let mut topic_decisions = DecisionIndex::new("topic", &request.topics)?;
        let mut reply_decisions = DecisionIndex::new("reply", &request.replies)?;

        let topics = request
            .writes
            .topics
            .iter()
            .map(|record| {
                prepare_topic_relations(record, request.writes.event_mode, &mut topic_decisions)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let replies = request
            .writes
            .replies
            .iter()
            .map(|record| {
                prepare_reply_relations(record, request.writes.event_mode, &mut reply_decisions)
            })
            .collect::<Result<Vec<_>, _>>()?;

        topic_decisions.reject_unused()?;
        reply_decisions.reject_unused()?;

        Ok(ForumPreparedImportRelationBatch {
            writes: request.writes,
            relation_event_mode,
            topics,
            replies,
        })
    }
}

trait DecisionSource {
    fn source(&self) -> &ForumImportExternalRef;
}

impl DecisionSource for ForumImportContentRelationDecision {
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
    ) -> Result<Self, ForumImportRelationPreparationError> {
        let mut by_source = BTreeMap::new();
        for decision in decisions {
            let key = ref_key(decision.source());
            if by_source.insert(key, decision).is_some() {
                return Err(ForumImportRelationPreparationError::DuplicateDecision {
                    kind,
                    source: decision.source().clone(),
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
    ) -> Result<&'a T, ForumImportRelationPreparationError> {
        let key = ref_key(source);
        let Some(decision) = self.by_source.get(&key).copied() else {
            return Err(ForumImportRelationPreparationError::MissingDecision {
                kind: self.kind,
                source: source.clone(),
            });
        };
        self.used.insert(key);
        Ok(decision)
    }

    fn reject_unused(&self) -> Result<(), ForumImportRelationPreparationError> {
        for (key, decision) in &self.by_source {
            if !self.used.contains(key) {
                return Err(ForumImportRelationPreparationError::UnexpectedDecision {
                    kind: self.kind,
                    source: decision.source().clone(),
                });
            }
        }
        Ok(())
    }
}

fn validate_batch(
    batch: &ForumPreparedImportWriteBatch,
) -> Result<(), ForumImportRelationPreparationError> {
    if batch.tenant_id.is_nil() {
        return Err(ForumImportRelationPreparationError::NilTenantId);
    }
    let actual = batch.topics.len().saturating_add(batch.replies.len());
    if actual > MAX_FORUM_IMPORT_RELATION_TARGETS_PER_BATCH {
        return Err(ForumImportRelationPreparationError::TooManyTargets {
            max: MAX_FORUM_IMPORT_RELATION_TARGETS_PER_BATCH,
            actual,
        });
    }
    let normalized = normalize_locale_code(&batch.locale).ok_or_else(|| {
        ForumImportRelationPreparationError::InvalidLocale {
            locale: batch.locale.clone(),
        }
    })?;
    if normalized != batch.locale {
        return Err(ForumImportRelationPreparationError::LocaleNotNormalized {
            actual: batch.locale.clone(),
            normalized,
        });
    }
    for topic in &batch.topics {
        validate_source_ref(&topic.source, ForumImportEntityKind::Topic)?;
        validate_source_ref(&topic.body_source, ForumImportEntityKind::Post)?;
        validate_record_locale(&batch.locale, &topic.source, &topic.locale)?;
        if topic.id.is_nil() {
            return Err(ForumImportRelationPreparationError::NilTargetId {
                kind: "topic",
                source: topic.source.clone(),
            });
        }
    }
    for reply in &batch.replies {
        validate_source_ref(&reply.source, ForumImportEntityKind::Post)?;
        validate_record_locale(&batch.locale, &reply.source, &reply.locale)?;
        if reply.id.is_nil() {
            return Err(ForumImportRelationPreparationError::NilTargetId {
                kind: "reply",
                source: reply.source.clone(),
            });
        }
    }
    Ok(())
}

fn validate_record_locale(
    batch_locale: &str,
    source: &ForumImportExternalRef,
    record_locale: &str,
) -> Result<(), ForumImportRelationPreparationError> {
    if record_locale != batch_locale {
        return Err(ForumImportRelationPreparationError::RecordLocaleMismatch {
            source: source.clone(),
            actual: record_locale.to_string(),
        });
    }
    Ok(())
}

fn prepare_topic_relations(
    record: &ForumPreparedImportTopic,
    event_mode: ForumImportWriteEventMode,
    decisions: &mut DecisionIndex<'_, ForumImportContentRelationDecision>,
) -> Result<ForumPreparedImportContentRelations, ForumImportRelationPreparationError> {
    let decision = decisions.require(&record.source)?;
    validate_source_ref(&decision.source, ForumImportEntityKind::Topic)?;
    prepare_content_relations(
        &record.source,
        ForumContentTarget::topic(record.id),
        &record.locale,
        &record.body,
        event_mode,
        decision,
    )
}

fn prepare_reply_relations(
    record: &ForumPreparedImportReply,
    event_mode: ForumImportWriteEventMode,
    decisions: &mut DecisionIndex<'_, ForumImportContentRelationDecision>,
) -> Result<ForumPreparedImportContentRelations, ForumImportRelationPreparationError> {
    let decision = decisions.require(&record.source)?;
    validate_source_ref(&decision.source, ForumImportEntityKind::Post)?;
    prepare_content_relations(
        &record.source,
        ForumContentTarget::reply(record.id),
        &record.locale,
        &record.content,
        event_mode,
        decision,
    )
}

fn prepare_content_relations(
    source: &ForumImportExternalRef,
    target: ForumContentTarget,
    locale: &str,
    document: &rustok_api::RichTextDocument,
    event_mode: ForumImportWriteEventMode,
    decision: &ForumImportContentRelationDecision,
) -> Result<ForumPreparedImportContentRelations, ForumImportRelationPreparationError> {
    if decision.quotes.len() > FORUM_MAX_QUOTE_REFERENCES_PER_REVISION {
        return Err(ForumImportRelationPreparationError::TooManyQuoteTargets {
            source: source.clone(),
            max: FORUM_MAX_QUOTE_REFERENCES_PER_REVISION,
            actual: decision.quotes.len(),
        });
    }
    if !decision.quotes.is_empty() {
        return Err(
            ForumImportRelationPreparationError::QuoteRelationsUnsupported {
                source: source.clone(),
            },
        );
    }

    let policy = ForumMentionPolicy {
        max_targets: FORUM_MAX_MENTION_TARGETS_PER_REVISION,
        allow_moderator_audience: true,
    };
    let extracted = extract_forum_mention_candidates(document, policy).map_err(|error| {
        ForumImportRelationPreparationError::InvalidRichTextProjection {
            source: source.clone(),
            reason: error.to_string(),
        }
    })?;

    match decision.mode {
        ForumImportRelationMode::SuppressRelations => {
            if !decision.mentions.is_empty()
                || !decision.audiences.is_empty()
                || !decision.quotes.is_empty()
            {
                return Err(
                    ForumImportRelationPreparationError::SuppressedRelationsContainFacts {
                        source: source.clone(),
                    },
                );
            }
            if event_mode == ForumImportWriteEventMode::EmitDomainEvents
                && extracted.target_count() > 0
            {
                return Err(
                    ForumImportRelationPreparationError::EventModeRequiresMaterialization {
                        source: source.clone(),
                    },
                );
            }
            Ok(ForumPreparedImportContentRelations {
                source: source.clone(),
                target,
                locale: locale.to_string(),
                mode: decision.mode,
                mentions: Vec::new(),
                audiences: Vec::new(),
                quotes: Vec::new(),
            })
        }
        ForumImportRelationMode::MaterializeRelations => {
            let mentions = normalize_mention_bindings(source, &decision.mentions)?;
            let audiences = normalize_audiences(source, &decision.audiences)?;
            let actual = mentions.len().saturating_add(audiences.len());
            if actual > FORUM_MAX_MENTION_TARGETS_PER_REVISION {
                return Err(ForumImportRelationPreparationError::TooManyMentionTargets {
                    source: source.clone(),
                    max: FORUM_MAX_MENTION_TARGETS_PER_REVISION,
                    actual,
                });
            }
            let mention_handles = mentions
                .iter()
                .map(|mention| mention.handle.as_str())
                .collect::<Vec<_>>();
            if mention_handles.as_slice() != extracted.handles() {
                return Err(ForumImportRelationPreparationError::MentionHandleMismatch {
                    source: source.clone(),
                });
            }
            if audiences.as_slice() != extracted.audiences() {
                return Err(
                    ForumImportRelationPreparationError::MentionAudienceMismatch {
                        source: source.clone(),
                    },
                );
            }
            Ok(ForumPreparedImportContentRelations {
                source: source.clone(),
                target,
                locale: locale.to_string(),
                mode: decision.mode,
                mentions,
                audiences,
                quotes: Vec::new(),
            })
        }
    }
}

fn normalize_mention_bindings(
    source: &ForumImportExternalRef,
    bindings: &[ForumImportMentionBinding],
) -> Result<Vec<ForumPreparedImportMention>, ForumImportRelationPreparationError> {
    let mut by_handle = BTreeMap::new();
    let mut user_ids = BTreeSet::new();
    for binding in bindings {
        let handle = ProfileService::normalize_handle(&binding.handle).map_err(|_| {
            ForumImportRelationPreparationError::InvalidMentionHandle {
                source: source.clone(),
                handle: binding.handle.clone(),
            }
        })?;
        if binding.user_id.is_nil() {
            return Err(ForumImportRelationPreparationError::NilMentionUserId {
                source: source.clone(),
                handle,
            });
        }
        if by_handle.contains_key(&handle) {
            return Err(
                ForumImportRelationPreparationError::DuplicateMentionHandle {
                    source: source.clone(),
                    handle,
                },
            );
        }
        if !user_ids.insert(binding.user_id) {
            return Err(ForumImportRelationPreparationError::DuplicateMentionUser {
                source: source.clone(),
                user_id: binding.user_id,
            });
        }
        by_handle.insert(handle, binding.user_id);
    }
    Ok(by_handle
        .into_iter()
        .map(|(handle, user_id)| ForumPreparedImportMention { handle, user_id })
        .collect())
}

fn normalize_audiences(
    source: &ForumImportExternalRef,
    audiences: &[ForumMentionAudience],
) -> Result<Vec<ForumMentionAudience>, ForumImportRelationPreparationError> {
    let mut normalized = BTreeSet::new();
    for audience in audiences {
        if !normalized.insert(*audience) {
            return Err(ForumImportRelationPreparationError::DuplicateAudience {
                source: source.clone(),
                audience: *audience,
            });
        }
    }
    Ok(normalized.into_iter().collect())
}

fn relation_event_mode(event_mode: ForumImportWriteEventMode) -> ForumImportRelationEventMode {
    match event_mode {
        ForumImportWriteEventMode::SuppressInteractiveEvents => {
            ForumImportRelationEventMode::SuppressAddedTargetEvents
        }
        ForumImportWriteEventMode::EmitDomainEvents => {
            ForumImportRelationEventMode::EmitAddedTargetEvents
        }
    }
}

fn validate_source_ref(
    source: &ForumImportExternalRef,
    expected: ForumImportEntityKind,
) -> Result<(), ForumImportRelationPreparationError> {
    if source.source != FORUM_IMPORT_SOURCE_NODEBB || source.kind != expected {
        return Err(ForumImportRelationPreparationError::InvalidSourceRef {
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
