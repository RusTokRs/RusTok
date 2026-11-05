use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::import_mapping::{
    FORUM_IMPORT_SOURCE_NODEBB, ForumImportEntityKind, ForumImportExternalRef,
    MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH,
};
use crate::import_relation_preparation::ForumPreparedImportRelationBatch;
use crate::state_machine::ReplyStatus;

pub const MAX_FORUM_IMPORT_REPLY_TOMBSTONES_PER_BATCH: usize =
    MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH;

/// Explicit exporter/audit enrichment for one NodeBB reply tombstone.
///
/// Current NodeBB post state exposes the deleted flag but does not guarantee a
/// deletion timestamp. This record is therefore a separate, opt-in source fact:
/// callers must only populate it from an exporter or audit source that actually
/// captured the historical deletion time.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodebbReplyTombstoneRecord {
    pub pid: i64,
    #[serde(rename = "deletedTimestamp", alias = "deleted_timestamp")]
    pub deleted_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForumImportReplyTombstoneFact {
    pub source: ForumImportExternalRef,
    pub deleted_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForumPreparedDeletedReplyTombstone {
    pub source: ForumImportExternalRef,
    pub reply_id: Uuid,
    pub deleted_at_ms: i64,
}

#[derive(Clone, Debug)]
pub struct ForumImportTombstonePreparationRequest {
    pub relations: ForumPreparedImportRelationBatch,
    pub deleted_replies: Vec<ForumImportReplyTombstoneFact>,
}

#[derive(Clone, Debug)]
pub struct ForumPreparedImportTombstoneBatch {
    pub relations: ForumPreparedImportRelationBatch,
    pub deleted_replies: Vec<ForumPreparedDeletedReplyTombstone>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ForumImportTombstonePreparationError {
    #[error("Forum import tombstone enrichment exceeds {max} records: {actual}")]
    TooManyTombstones { max: usize, actual: usize },
    #[error("NodeBB tombstone post id must be positive: {pid}")]
    InvalidNodebbPostId { pid: i64 },
    #[error("NodeBB tombstone timestamp must be non-negative for post {pid}: {deleted_at_ms}")]
    NegativeNodebbTombstoneTimestamp { pid: i64, deleted_at_ms: i64 },
    #[error("NodeBB tombstone timestamp is outside owner range for post {pid}: {deleted_at_ms}")]
    NodebbTombstoneTimestampOutOfRange { pid: i64, deleted_at_ms: i64 },
    #[error("NodeBB tombstone sidecar repeats post id {pid}")]
    DuplicateNodebbPostId { pid: i64 },
    #[error("Forum import tombstone requires a NodeBB Post source, got {source:?}")]
    InvalidSource { source: ForumImportExternalRef },
    #[error("Forum import tombstone source key is invalid: {source:?}")]
    InvalidSourceKey { source: ForumImportExternalRef },
    #[error("Forum import tombstone repeats reply source {source:?}")]
    DuplicateTombstoneSource { source: ForumImportExternalRef },
    #[error("Forum import relation batch repeats reply source {source:?}")]
    DuplicateReplySource { source: ForumImportExternalRef },
    #[error(
        "Forum import relation count differs from prepared reply count: {relations} != {writes}"
    )]
    RelationCountMismatch { writes: usize, relations: usize },
    #[error("Forum import relation facts do not align with reply {source:?}")]
    RelationAlignmentMismatch { source: ForumImportExternalRef },
    #[error("Forum import deleted reply is missing an admitted tombstone: {source:?}")]
    MissingDeletedReplyTombstone { source: ForumImportExternalRef },
    #[error("Forum import live reply cannot have a tombstone: {source:?}")]
    LiveReplyHasTombstone { source: ForumImportExternalRef },
    #[error("Forum import tombstone does not match a prepared reply: {source:?}")]
    UnexpectedTombstone { source: ForumImportExternalRef },
    #[error(
        "Forum import tombstone predates reply creation for {source:?}: {deleted_at_ms} < {created_at_ms}"
    )]
    TombstonePredatesCreation {
        source: ForumImportExternalRef,
        created_at_ms: i64,
        deleted_at_ms: i64,
    },
    #[error(
        "Forum import tombstone timestamp is outside owner range for {source:?}: {deleted_at_ms}"
    )]
    TombstoneTimestampOutOfRange {
        source: ForumImportExternalRef,
        deleted_at_ms: i64,
    },
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NodebbForumReplyTombstoneMapper;

impl NodebbForumReplyTombstoneMapper {
    pub fn map_batch(
        &self,
        records: &[NodebbReplyTombstoneRecord],
    ) -> Result<Vec<ForumImportReplyTombstoneFact>, ForumImportTombstonePreparationError> {
        if records.len() > MAX_FORUM_IMPORT_REPLY_TOMBSTONES_PER_BATCH {
            return Err(ForumImportTombstonePreparationError::TooManyTombstones {
                max: MAX_FORUM_IMPORT_REPLY_TOMBSTONES_PER_BATCH,
                actual: records.len(),
            });
        }

        let mut seen = BTreeSet::new();
        let mut facts = Vec::with_capacity(records.len());
        for record in records {
            if record.pid <= 0 {
                return Err(ForumImportTombstonePreparationError::InvalidNodebbPostId {
                    pid: record.pid,
                });
            }
            if !seen.insert(record.pid) {
                return Err(
                    ForumImportTombstonePreparationError::DuplicateNodebbPostId { pid: record.pid },
                );
            }
            if record.deleted_at_ms < 0 {
                return Err(
                    ForumImportTombstonePreparationError::NegativeNodebbTombstoneTimestamp {
                        pid: record.pid,
                        deleted_at_ms: record.deleted_at_ms,
                    },
                );
            }
            if DateTime::<Utc>::from_timestamp_millis(record.deleted_at_ms).is_none() {
                return Err(
                    ForumImportTombstonePreparationError::NodebbTombstoneTimestampOutOfRange {
                        pid: record.pid,
                        deleted_at_ms: record.deleted_at_ms,
                    },
                );
            }
            facts.push(ForumImportReplyTombstoneFact {
                source: ForumImportExternalRef {
                    source: FORUM_IMPORT_SOURCE_NODEBB.to_owned(),
                    kind: ForumImportEntityKind::Post,
                    key: format!("post:{}", record.pid),
                },
                deleted_at_ms: record.deleted_at_ms,
            });
        }
        Ok(facts)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ForumImportTombstonePreparer;

impl ForumImportTombstonePreparer {
    pub fn prepare(
        &self,
        request: ForumImportTombstonePreparationRequest,
    ) -> Result<ForumPreparedImportTombstoneBatch, ForumImportTombstonePreparationError> {
        if request.deleted_replies.len() > MAX_FORUM_IMPORT_REPLY_TOMBSTONES_PER_BATCH {
            return Err(ForumImportTombstonePreparationError::TooManyTombstones {
                max: MAX_FORUM_IMPORT_REPLY_TOMBSTONES_PER_BATCH,
                actual: request.deleted_replies.len(),
            });
        }
        if request.relations.replies.len() != request.relations.writes.replies.len() {
            return Err(
                ForumImportTombstonePreparationError::RelationCountMismatch {
                    writes: request.relations.writes.replies.len(),
                    relations: request.relations.replies.len(),
                },
            );
        }

        let mut reply_sources = BTreeSet::new();
        for (reply, relation) in request
            .relations
            .writes
            .replies
            .iter()
            .zip(&request.relations.replies)
        {
            validate_post_source(&reply.source)?;
            if relation.source != reply.source
                || relation.target != crate::mentions::ForumContentTarget::reply(reply.id)
                || relation.locale != reply.locale
            {
                return Err(
                    ForumImportTombstonePreparationError::RelationAlignmentMismatch {
                        source: reply.source.clone(),
                    },
                );
            }
            if !reply_sources.insert(ref_key(&reply.source)) {
                return Err(ForumImportTombstonePreparationError::DuplicateReplySource {
                    source: reply.source.clone(),
                });
            }
        }

        let mut facts = BTreeMap::new();
        for fact in &request.deleted_replies {
            validate_post_source(&fact.source)?;
            if fact.deleted_at_ms < 0
                || DateTime::<Utc>::from_timestamp_millis(fact.deleted_at_ms).is_none()
            {
                return Err(
                    ForumImportTombstonePreparationError::TombstoneTimestampOutOfRange {
                        source: fact.source.clone(),
                        deleted_at_ms: fact.deleted_at_ms,
                    },
                );
            }
            if facts.insert(ref_key(&fact.source), fact).is_some() {
                return Err(
                    ForumImportTombstonePreparationError::DuplicateTombstoneSource {
                        source: fact.source.clone(),
                    },
                );
            }
        }

        let mut prepared = Vec::new();
        for reply in &request.relations.writes.replies {
            let fact = facts.remove(&ref_key(&reply.source));
            match (reply.status, fact) {
                (ReplyStatus::Deleted, Some(fact)) => {
                    if fact.deleted_at_ms < reply.created_at_ms {
                        return Err(
                            ForumImportTombstonePreparationError::TombstonePredatesCreation {
                                source: reply.source.clone(),
                                created_at_ms: reply.created_at_ms,
                                deleted_at_ms: fact.deleted_at_ms,
                            },
                        );
                    }
                    prepared.push(ForumPreparedDeletedReplyTombstone {
                        source: reply.source.clone(),
                        reply_id: reply.id,
                        deleted_at_ms: fact.deleted_at_ms,
                    });
                }
                (ReplyStatus::Deleted, None) => {
                    return Err(
                        ForumImportTombstonePreparationError::MissingDeletedReplyTombstone {
                            source: reply.source.clone(),
                        },
                    );
                }
                (_, Some(_)) => {
                    return Err(
                        ForumImportTombstonePreparationError::LiveReplyHasTombstone {
                            source: reply.source.clone(),
                        },
                    );
                }
                (_, None) => {}
            }
        }

        if let Some(fact) = facts.into_values().next() {
            return Err(ForumImportTombstonePreparationError::UnexpectedTombstone {
                source: fact.source.clone(),
            });
        }

        Ok(ForumPreparedImportTombstoneBatch {
            relations: request.relations,
            deleted_replies: prepared,
        })
    }
}

fn validate_post_source(
    source: &ForumImportExternalRef,
) -> Result<(), ForumImportTombstonePreparationError> {
    if source.source != FORUM_IMPORT_SOURCE_NODEBB || source.kind != ForumImportEntityKind::Post {
        return Err(ForumImportTombstonePreparationError::InvalidSource {
            source: source.clone(),
        });
    }
    let Some(raw_id) = source.key.strip_prefix("post:") else {
        return Err(ForumImportTombstonePreparationError::InvalidSourceKey {
            source: source.clone(),
        });
    };
    let Ok(id) = raw_id.parse::<i64>() else {
        return Err(ForumImportTombstonePreparationError::InvalidSourceKey {
            source: source.clone(),
        });
    };
    if id <= 0 || source.key != format!("post:{id}") {
        return Err(ForumImportTombstonePreparationError::InvalidSourceKey {
            source: source.clone(),
        });
    }
    Ok(())
}

type RefKey = (String, &'static str, String);

fn ref_key(source: &ForumImportExternalRef) -> RefKey {
    (
        source.source.clone(),
        match source.kind {
            ForumImportEntityKind::Category => "category",
            ForumImportEntityKind::Topic => "topic",
            ForumImportEntityKind::Post => "post",
            ForumImportEntityKind::User => "user",
        },
        source.key.clone(),
    )
}
