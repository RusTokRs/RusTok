use std::collections::{BTreeSet, HashMap, HashSet};

use rustok_api::{PortContext, normalize_locale_tag};
use rustok_media::{MediaReferenceAdmission, MediaReferenceAdmissionPort};
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::{ForumError, ForumResult, mentions::ForumContentTarget};

pub const MAX_FORUM_ATTACHMENTS_PER_REVISION: usize = 32;
pub const MAX_FORUM_ATTACHMENT_CAPTION_BYTES: usize = 512;
pub const FORUM_ATTACHMENT_MEDIA_CAPABILITY: &str = "forum.attachment.media_reference_admission";
pub const FORUM_ATTACHMENT_MEDIA_CAPABILITY_UNAVAILABLE_CODE: &str =
    "FORUM_ATTACHMENT_MEDIA_CAPABILITY_UNAVAILABLE";
pub const FORUM_ATTACHMENT_MEDIA_INVALID_RESPONSE_CODE: &str =
    "FORUM_ATTACHMENT_MEDIA_INVALID_RESPONSE";

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
    /// captured topic/reply revisions advance this value monotonically.
    pub source_revision: u64,
    pub locale: String,
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
pub struct ForumPreparedAttachmentRelationBatch {
    source: ForumAttachmentSourceRevision,
    attachments: Vec<ForumPreparedAttachmentRelation>,
}

impl ForumPreparedAttachmentRelationBatch {
    pub fn source(&self) -> &ForumAttachmentSourceRevision {
        &self.source
    }

    pub fn attachments(&self) -> &[ForumPreparedAttachmentRelation] {
        &self.attachments
    }

    pub fn is_empty(&self) -> bool {
        self.attachments.is_empty()
    }
}

/// A Forum attachment batch that has passed both Forum structural admission and
/// Media-owned reference admission.
///
/// Future persistence entrypoints should require this wrapper instead of the
/// structurally prepared batch so lifecycle safety cannot be bypassed by a
/// caller that only knows a Media UUID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForumMediaAdmittedAttachmentRelationBatch {
    prepared: ForumPreparedAttachmentRelationBatch,
}

impl ForumMediaAdmittedAttachmentRelationBatch {
    pub fn source(&self) -> &ForumAttachmentSourceRevision {
        self.prepared.source()
    }

    pub fn attachments(&self) -> &[ForumPreparedAttachmentRelation] {
        self.prepared.attachments()
    }

    pub fn is_empty(&self) -> bool {
        self.prepared.is_empty()
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
        let locale = normalize_locale_tag(&request.locale)
            .ok_or(ForumAttachmentRelationAdmissionError::InvalidLocale)?;
        if request.attachments.len() > MAX_FORUM_ATTACHMENTS_PER_REVISION {
            return Err(ForumAttachmentRelationAdmissionError::BatchTooLarge {
                max: MAX_FORUM_ATTACHMENTS_PER_REVISION,
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
            attachments,
        })
    }
}

/// Resolve the Media-owned persistence-safety fact for a prepared Forum batch.
///
/// Empty/text-only batches do not require Media. Non-empty batches fail closed
/// when Media is not composed, when the owner call fails, when the response is
/// malformed or when any requested asset is not currently referenceable.
pub async fn admit_attachment_relations_for_persistence(
    media_port: Option<&dyn MediaReferenceAdmissionPort>,
    context: PortContext,
    prepared: ForumPreparedAttachmentRelationBatch,
) -> ForumResult<ForumMediaAdmittedAttachmentRelationBatch> {
    let expected_tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
        ForumError::Validation(
            "Forum attachment Media context requires a UUID tenant identity".to_string(),
        )
    })?;
    if prepared.source().tenant_id() != expected_tenant_id {
        return Err(ForumError::Validation(
            "Forum attachment batch tenant does not match the Media context".to_string(),
        ));
    }
    if prepared.is_empty() {
        return Ok(ForumMediaAdmittedAttachmentRelationBatch { prepared });
    }

    let media_port = media_port.ok_or_else(|| {
        ForumError::capability_unavailable(
            FORUM_ATTACHMENT_MEDIA_CAPABILITY,
            FORUM_ATTACHMENT_MEDIA_CAPABILITY_UNAVAILABLE_CODE,
        )
    })?;
    let mut seen = HashSet::with_capacity(prepared.attachments().len());
    let media_ids = prepared
        .attachments()
        .iter()
        .filter_map(|relation| seen.insert(relation.media_id).then_some(relation.media_id))
        .collect::<Vec<_>>();
    let admissions = media_port
        .admit_references(context, media_ids.clone())
        .await
        .map_err(map_attachment_media_port_error)?;
    validate_media_reference_admissions(expected_tenant_id, &media_ids, &admissions)?;

    Ok(ForumMediaAdmittedAttachmentRelationBatch { prepared })
}

fn validate_media_reference_admissions(
    expected_tenant_id: Uuid,
    requested_ids: &[Uuid],
    admissions: &[MediaReferenceAdmission],
) -> ForumResult<()> {
    let requested = requested_ids.iter().copied().collect::<HashSet<_>>();
    let mut by_id = HashMap::with_capacity(admissions.len());
    for admission in admissions.iter().copied() {
        if admission.tenant_id != expected_tenant_id
            || !requested.contains(&admission.media_id)
            || by_id.insert(admission.media_id, admission).is_some()
        {
            return Err(invalid_attachment_media_response());
        }
    }
    if by_id.len() != requested.len() {
        return Err(invalid_attachment_media_response());
    }

    for media_id in requested_ids {
        let admission = by_id
            .get(media_id)
            .ok_or_else(invalid_attachment_media_response)?;
        if !admission.referenceable {
            return Err(ForumError::Validation(format!(
                "Forum attachment Media asset is not referenceable: {media_id}"
            )));
        }
    }
    Ok(())
}

fn invalid_attachment_media_response() -> ForumError {
    ForumError::capability_failure(
        FORUM_ATTACHMENT_MEDIA_CAPABILITY,
        FORUM_ATTACHMENT_MEDIA_INVALID_RESPONSE_CODE,
        "Media reference-admission response violated the bounded owner contract",
        false,
    )
}

fn map_attachment_media_port_error(error: rustok_api::PortError) -> ForumError {
    ForumError::capability_failure(
        FORUM_ATTACHMENT_MEDIA_CAPABILITY,
        error.code,
        error.message,
        error.retryable,
    )
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

    #[test]
    fn media_reference_admission_requires_exact_referenceable_owner_results() {
        let tenant_id = Uuid::new_v4();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let requested = [first, second];
        let allowed = [
            MediaReferenceAdmission {
                media_id: first,
                tenant_id,
                referenceable: true,
            },
            MediaReferenceAdmission {
                media_id: second,
                tenant_id,
                referenceable: true,
            },
        ];
        assert!(validate_media_reference_admissions(tenant_id, &requested, &allowed).is_ok());

        let mut denied = allowed;
        denied[1].referenceable = false;
        assert!(matches!(
            validate_media_reference_admissions(tenant_id, &requested, &denied),
            Err(ForumError::Validation(_))
        ));
    }

    #[test]
    fn media_reference_admission_rejects_missing_duplicate_foreign_and_extra_results() {
        let tenant_id = Uuid::new_v4();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let requested = [first, second];
        let first_allowed = MediaReferenceAdmission {
            media_id: first,
            tenant_id,
            referenceable: true,
        };

        for response in [
            vec![first_allowed],
            vec![first_allowed, first_allowed],
            vec![
                first_allowed,
                MediaReferenceAdmission {
                    media_id: second,
                    tenant_id: Uuid::new_v4(),
                    referenceable: true,
                },
            ],
            vec![
                first_allowed,
                MediaReferenceAdmission {
                    media_id: Uuid::new_v4(),
                    tenant_id,
                    referenceable: true,
                },
            ],
        ] {
            let error = validate_media_reference_admissions(tenant_id, &requested, &response)
                .expect_err("malformed Media response must fail closed");
            match error {
                ForumError::CapabilityFailure {
                    capability,
                    source_code,
                    retryable,
                    ..
                } => {
                    assert_eq!(capability, FORUM_ATTACHMENT_MEDIA_CAPABILITY);
                    assert_eq!(source_code, FORUM_ATTACHMENT_MEDIA_INVALID_RESPONSE_CODE);
                    assert!(!retryable);
                }
                other => panic!("unexpected malformed-response error: {other:?}"),
            }
        }
    }
}
