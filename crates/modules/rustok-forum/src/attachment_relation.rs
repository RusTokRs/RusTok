use std::collections::BTreeSet;

use rustok_api::normalize_locale_tag;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::mentions::ForumContentTarget;

pub const MAX_FORUM_ATTACHMENTS_PER_SET: usize = 32;
pub const MAX_FORUM_ATTACHMENT_CAPTION_BYTES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ForumAttachmentRelationRevision(i64);

impl ForumAttachmentRelationRevision {
    pub const EMPTY: Self = Self(0);
    pub const FIRST: Self = Self(1);

    pub fn new(value: i64) -> Result<Self, ForumAttachmentRelationRevisionError> {
        if value < 0 {
            return Err(ForumAttachmentRelationRevisionError::Negative { value });
        }
        Ok(Self(value))
    }

    pub const fn value(self) -> i64 {
        self.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn next(self) -> Result<Self, ForumAttachmentRelationRevisionError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(ForumAttachmentRelationRevisionError::Exhausted)
    }

    pub fn check_expected(
        self,
        current: Option<Self>,
    ) -> Result<ForumAttachmentRelationWriteKind, ForumAttachmentRelationRevisionError> {
        let current = current.unwrap_or(Self::EMPTY);
        if self != current {
            return Err(ForumAttachmentRelationRevisionError::Conflict {
                expected: self.value(),
                current: current.value(),
            });
        }
        if current.is_empty() {
            Ok(ForumAttachmentRelationWriteKind::Create)
        } else {
            Ok(ForumAttachmentRelationWriteKind::Replace)
        }
    }
}

impl<'de> Deserialize<'de> for ForumAttachmentRelationRevision {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = i64::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumAttachmentRelationWriteKind {
    Create,
    Replace,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ForumAttachmentRelationRevisionError {
    #[error("Forum attachment relation revision cannot be negative: {value}")]
    Negative { value: i64 },
    #[error("Forum attachment relation revision counter is exhausted")]
    Exhausted,
    #[error(
        "Forum attachment relation revision conflict: expected {expected}, current {current}"
    )]
    Conflict { expected: i64, current: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ForumAttachmentUsage {
    Inline,
    Attachment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForumAttachmentRelationInput {
    pub media_id: Uuid,
    pub usage: ForumAttachmentUsage,
    pub position: u16,
    pub caption: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForumAttachmentRelationAdmissionRequest {
    pub tenant_id: Uuid,
    pub target: ForumContentTarget,
    /// Logical Forum owner content revision. Initial content is revision 1;
    /// captured topic/reply revisions advance this value monotonically. This is
    /// provenance only and is intentionally independent from the attachment-set
    /// concurrency token below.
    pub source_revision: u64,
    pub locale: String,
    /// Last committed attachment-set revision observed by the command caller.
    /// `0` means that no attachment set has been committed yet.
    pub expected_relation_revision: ForumAttachmentRelationRevision,
    pub attachments: Vec<ForumAttachmentRelationInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct ForumAttachmentSourceRevision {
    tenant_id: Uuid,
    target: ForumContentTarget,
    source_revision: u64,
    locale: String,
}

impl ForumAttachmentSourceRevision {
    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn target(&self) -> ForumContentTarget {
        self.target
    }

    pub fn source_revision(&self) -> u64 {
        self.source_revision
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForumPreparedAttachmentRelation {
    pub media_id: Uuid,
    pub usage: ForumAttachmentUsage,
    pub position: u16,
    pub caption: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForumAttachmentRelationRecord {
    pub reference_id: Uuid,
    pub media_id: Uuid,
    pub usage: ForumAttachmentUsage,
    pub position: u16,
    pub caption: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForumAttachmentRelationSet {
    pub tenant_id: Uuid,
    pub target: ForumContentTarget,
    pub locale: String,
    pub relation_revision: ForumAttachmentRelationRevision,
    pub source_revision: Option<u64>,
    pub attachments: Vec<ForumAttachmentRelationRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForumPreparedAttachmentRelationBatch {
    source: ForumAttachmentSourceRevision,
    expected_relation_revision: ForumAttachmentRelationRevision,
    attachments: Vec<ForumPreparedAttachmentRelation>,
}

impl ForumPreparedAttachmentRelationBatch {
    pub fn source(&self) -> &ForumAttachmentSourceRevision {
        &self.source
    }

    pub fn expected_relation_revision(&self) -> ForumAttachmentRelationRevision {
        self.expected_relation_revision
    }

    pub fn attachments(&self) -> &[ForumPreparedAttachmentRelation] {
        &self.attachments
    }

    pub fn is_empty(&self) -> bool {
        self.attachments.is_empty()
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ForumAttachmentRelationAdmissionError {
    #[error("Forum attachment relation requires a non-nil tenant ID")]
    NilTenant,
    #[error("Forum attachment relation requires a non-nil topic/reply target ID")]
    NilTarget,
    #[error("Forum attachment relation requires a positive source revision")]
    InvalidSourceRevision,
    #[error("Forum attachment relation source revision exceeds the database range")]
    SourceRevisionOutOfRange,
    #[error("Forum attachment relation requires a valid locale")]
    InvalidLocale,
    #[error("Forum attachment relation batch exceeds {max} records: {actual}")]
    BatchTooLarge { max: usize, actual: usize },
    #[error("Forum attachment relation contains a nil Media asset ID at position {position}")]
    NilMediaId { position: u16 },
    #[error("Forum attachment relation repeats position {position}")]
    DuplicatePosition { position: u16 },
    #[error("Forum attachment relation positions must be contiguous from zero")]
    NonContiguousPositions,
    #[error("Forum attachment caption exceeds {max} UTF-8 bytes at position {position}")]
    CaptionTooLong { position: u16, max: usize },
    #[error("Forum attachment caption contains a control character at position {position}")]
    CaptionContainsControl { position: u16 },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ForumAttachmentRelationPreparer;

impl ForumAttachmentRelationPreparer {
    pub fn prepare(
        &self,
        request: ForumAttachmentRelationAdmissionRequest,
    ) -> Result<ForumPreparedAttachmentRelationBatch, ForumAttachmentRelationAdmissionError> {
        if request.tenant_id.is_nil() {
            return Err(ForumAttachmentRelationAdmissionError::NilTenant);
        }
        if request.target.id().is_nil() {
            return Err(ForumAttachmentRelationAdmissionError::NilTarget);
        }
        if request.source_revision == 0 {
            return Err(ForumAttachmentRelationAdmissionError::InvalidSourceRevision);
        }
        if i64::try_from(request.source_revision).is_err() {
            return Err(ForumAttachmentRelationAdmissionError::SourceRevisionOutOfRange);
        }
        let locale = normalize_locale_tag(&request.locale)
            .ok_or(ForumAttachmentRelationAdmissionError::InvalidLocale)?;
        if request.attachments.len() > MAX_FORUM_ATTACHMENTS_PER_SET {
            return Err(ForumAttachmentRelationAdmissionError::BatchTooLarge {
                max: MAX_FORUM_ATTACHMENTS_PER_SET,
                actual: request.attachments.len(),
            });
        }

        let mut positions = BTreeSet::new();
        let mut attachments = Vec::with_capacity(request.attachments.len());
        for relation in request.attachments {
            if relation.media_id.is_nil() {
                return Err(ForumAttachmentRelationAdmissionError::NilMediaId {
                    position: relation.position,
                });
            }
            if !positions.insert(relation.position) {
                return Err(ForumAttachmentRelationAdmissionError::DuplicatePosition {
                    position: relation.position,
                });
            }
            let caption = normalize_caption(relation.caption, relation.position)?;
            attachments.push(ForumPreparedAttachmentRelation {
                media_id: relation.media_id,
                usage: relation.usage,
                position: relation.position,
                caption,
            });
        }

        if positions
            .iter()
            .copied()
            .map(usize::from)
            .ne(0..attachments.len())
        {
            return Err(ForumAttachmentRelationAdmissionError::NonContiguousPositions);
        }

        attachments.sort_by_key(|relation| relation.position);
        Ok(ForumPreparedAttachmentRelationBatch {
            source: ForumAttachmentSourceRevision {
                tenant_id: request.tenant_id,
                target: request.target,
                source_revision: request.source_revision,
                locale,
            },
            expected_relation_revision: request.expected_relation_revision,
            attachments,
        })
    }
}

fn normalize_caption(
    caption: Option<String>,
    position: u16,
) -> Result<Option<String>, ForumAttachmentRelationAdmissionError> {
    let Some(caption) = caption else {
        return Ok(None);
    };
    let caption = caption.trim().to_string();
    if caption.is_empty() {
        return Ok(None);
    }
    if caption.len() > MAX_FORUM_ATTACHMENT_CAPTION_BYTES {
        return Err(ForumAttachmentRelationAdmissionError::CaptionTooLong {
            position,
            max: MAX_FORUM_ATTACHMENT_CAPTION_BYTES,
        });
    }
    if caption.chars().any(char::is_control) {
        return Err(ForumAttachmentRelationAdmissionError::CaptionContainsControl { position });
    }
    Ok(Some(caption))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_request() -> ForumAttachmentRelationAdmissionRequest {
        ForumAttachmentRelationAdmissionRequest {
            tenant_id: Uuid::new_v4(),
            target: ForumContentTarget::topic(Uuid::new_v4()),
            source_revision: 1,
            locale: "en".to_string(),
            expected_relation_revision: ForumAttachmentRelationRevision::EMPTY,
            attachments: vec![
                ForumAttachmentRelationInput {
                    media_id: Uuid::new_v4(),
                    usage: ForumAttachmentUsage::Inline,
                    position: 0,
                    caption: Some(" First caption ".to_string()),
                },
                ForumAttachmentRelationInput {
                    media_id: Uuid::new_v4(),
                    usage: ForumAttachmentUsage::Attachment,
                    position: 1,
                    caption: None,
                },
            ],
        }
    }

    #[test]
    fn preparer_admits_valid_batch() {
        let preparer = ForumAttachmentRelationPreparer;
        let request = valid_request();
        let batch = preparer.prepare(request).expect("batch should be admitted");

        assert_eq!(batch.attachments().len(), 2);
        assert_eq!(
            batch.expected_relation_revision(),
            ForumAttachmentRelationRevision::EMPTY
        );
        assert_eq!(batch.source().source_revision(), 1);
        assert_eq!(batch.source().locale(), "en");
        assert_eq!(batch.attachments()[0].position, 0);
        assert_eq!(
            batch.attachments()[0].caption.as_deref(),
            Some("First caption")
        );
        assert_eq!(batch.attachments()[1].position, 1);
        assert_eq!(batch.attachments()[1].caption, None);
        assert!(!batch.is_empty());
    }

    #[test]
    fn preparer_rejects_nil_tenant() {
        let preparer = ForumAttachmentRelationPreparer;
        let mut request = valid_request();
        request.tenant_id = Uuid::nil();
        assert_eq!(
            preparer.prepare(request).unwrap_err(),
            ForumAttachmentRelationAdmissionError::NilTenant
        );
    }

    #[test]
    fn preparer_rejects_nil_target() {
        let preparer = ForumAttachmentRelationPreparer;
        let mut request = valid_request();
        request.target = ForumContentTarget::topic(Uuid::nil());
        assert_eq!(
            preparer.prepare(request).unwrap_err(),
            ForumAttachmentRelationAdmissionError::NilTarget
        );
    }

    #[test]
    fn preparer_rejects_zero_source_revision() {
        let preparer = ForumAttachmentRelationPreparer;
        let mut request = valid_request();
        request.source_revision = 0;
        assert_eq!(
            preparer.prepare(request).unwrap_err(),
            ForumAttachmentRelationAdmissionError::InvalidSourceRevision
        );
    }

    #[test]
    fn preparer_rejects_invalid_locale() {
        let preparer = ForumAttachmentRelationPreparer;
        let mut request = valid_request();
        request.locale = "".to_string();
        assert_eq!(
            preparer.prepare(request).unwrap_err(),
            ForumAttachmentRelationAdmissionError::InvalidLocale
        );
    }

    #[test]
    fn preparer_rejects_source_revision_outside_database_range() {
        let preparer = ForumAttachmentRelationPreparer;
        let mut request = valid_request();
        request.source_revision = u64::MAX;
        assert_eq!(
            preparer.prepare(request).unwrap_err(),
            ForumAttachmentRelationAdmissionError::SourceRevisionOutOfRange
        );
    }

    #[test]
    fn preparer_rejects_nil_media_id() {
        let preparer = ForumAttachmentRelationPreparer;
        let mut request = valid_request();
        request.attachments[0].media_id = Uuid::nil();
        assert_eq!(
            preparer.prepare(request).unwrap_err(),
            ForumAttachmentRelationAdmissionError::NilMediaId { position: 0 }
        );
    }

    #[test]
    fn preparer_rejects_duplicate_position() {
        let preparer = ForumAttachmentRelationPreparer;
        let mut request = valid_request();
        request.attachments[1].position = 0;
        assert_eq!(
            preparer.prepare(request).unwrap_err(),
            ForumAttachmentRelationAdmissionError::DuplicatePosition { position: 0 }
        );
    }

    #[test]
    fn preparer_rejects_non_contiguous_positions() {
        let preparer = ForumAttachmentRelationPreparer;
        let mut request = valid_request();
        request.attachments[1].position = 2;
        assert_eq!(
            preparer.prepare(request).unwrap_err(),
            ForumAttachmentRelationAdmissionError::NonContiguousPositions
        );
    }

    #[test]
    fn preparer_rejects_caption_with_control_characters() {
        let preparer = ForumAttachmentRelationPreparer;
        let mut request = valid_request();
        request.attachments[0].caption = Some("bad\x07caption".to_string());
        assert_eq!(
            preparer.prepare(request).unwrap_err(),
            ForumAttachmentRelationAdmissionError::CaptionContainsControl { position: 0 }
        );
    }

    #[test]
    fn preparer_rejects_batch_exceeding_maximum() {
        let preparer = ForumAttachmentRelationPreparer;
        let mut request = valid_request();
        request.attachments = (0..=MAX_FORUM_ATTACHMENTS_PER_SET)
            .map(|i| ForumAttachmentRelationInput {
                media_id: Uuid::new_v4(),
                usage: ForumAttachmentUsage::Attachment,
                position: i as u16,
                caption: None,
            })
            .collect();
        assert!(matches!(
            preparer.prepare(request).unwrap_err(),
            ForumAttachmentRelationAdmissionError::BatchTooLarge { .. }
        ));
    }
}

#[cfg(test)]
mod revision_tests {
    use super::{
        ForumAttachmentRelationRevision, ForumAttachmentRelationRevisionError,
        ForumAttachmentRelationWriteKind,
    };

    #[test]
    fn attachment_relation_revision_uses_zero_for_an_empty_stream() {
        assert!(ForumAttachmentRelationRevision::EMPTY.is_empty());
        assert_eq!(
            ForumAttachmentRelationRevision::EMPTY.next().unwrap(),
            ForumAttachmentRelationRevision::FIRST
        );
    }

    #[test]
    fn attachment_relation_revision_requires_exact_current_token() {
        assert_eq!(
            ForumAttachmentRelationRevision::EMPTY
                .check_expected(None)
                .unwrap(),
            ForumAttachmentRelationWriteKind::Create
        );
        assert_eq!(
            ForumAttachmentRelationRevision::FIRST
                .check_expected(Some(ForumAttachmentRelationRevision::FIRST))
                .unwrap(),
            ForumAttachmentRelationWriteKind::Replace
        );
        assert_eq!(
            ForumAttachmentRelationRevision::FIRST
                .check_expected(None)
                .unwrap_err(),
            ForumAttachmentRelationRevisionError::Conflict {
                expected: 1,
                current: 0
            }
        );
        assert_eq!(
            ForumAttachmentRelationRevision::EMPTY
                .check_expected(Some(ForumAttachmentRelationRevision::FIRST))
                .unwrap_err(),
            ForumAttachmentRelationRevisionError::Conflict {
                expected: 0,
                current: 1
            }
        );
    }

    #[test]
    fn attachment_relation_revision_rejects_stale_updates() {
        let current = ForumAttachmentRelationRevision::new(3).unwrap();
        assert_eq!(
            ForumAttachmentRelationRevision::new(2)
                .unwrap()
                .check_expected(Some(current))
                .unwrap_err(),
            ForumAttachmentRelationRevisionError::Conflict {
                expected: 2,
                current: 3
            }
        );
    }

    #[test]
    fn attachment_relation_revision_advances_monotonically() {
        let current = ForumAttachmentRelationRevision::new(3).unwrap();
        assert_eq!(current.next().unwrap().value(), 4);
    }

    #[test]
    fn attachment_relation_revision_rejects_negative_values_on_deserialize() {
        let error = serde_json::from_str::<ForumAttachmentRelationRevision>("-1")
            .expect_err("negative attachment revisions must fail closed");
        assert!(!error.to_string().is_empty());
    }

    #[test]
    fn attachment_relation_revision_detects_counter_exhaustion() {
        let max = ForumAttachmentRelationRevision::new(i64::MAX).unwrap();
        assert_eq!(
            max.next().unwrap_err(),
            ForumAttachmentRelationRevisionError::Exhausted
        );
    }
}
