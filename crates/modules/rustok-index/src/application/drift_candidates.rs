use std::fmt;

use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{EntityKey, LinkName, LinkedEntityKey, SchemaRef};

const MAX_CANDIDATE_PAGE_SIZE: usize = 32;
const MAX_CANDIDATE_CURSOR_BYTES: usize = 4 * 1024;
const MAX_CANDIDATE_FENCE_BYTES: usize = 512;
const MAX_FAILURE_CODE_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftCandidateScope {
    tenant_id: Uuid,
    schema: SchemaRef,
}

impl IndexDriftCandidateScope {
    pub fn new(tenant_id: Uuid, schema: SchemaRef) -> Result<Self, IndexDriftCandidateError> {
        if tenant_id.is_nil() {
            return Err(IndexDriftCandidateError::NilTenantId);
        }
        if schema.version.get() == 0 {
            return Err(IndexDriftCandidateError::ZeroSchemaVersion);
        }
        Ok(Self { tenant_id, schema })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }
}

/// Opaque reader-owned continuation state.
///
/// Debug output deliberately reveals only the encoded length. A future transport must wrap this
/// value in an authenticated confidential envelope rather than expose it directly.
#[derive(Clone, PartialEq, Eq)]
pub struct IndexDriftCandidateCursor(String);

impl IndexDriftCandidateCursor {
    pub fn new(value: impl Into<String>) -> Result<Self, IndexDriftCandidateError> {
        let value = value.into();
        validate_opaque_value(
            &value,
            MAX_CANDIDATE_CURSOR_BYTES,
            IndexDriftCandidateError::EmptyCursor,
            |actual, max| IndexDriftCandidateError::CursorTooLarge { actual, max },
        )?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Debug for IndexDriftCandidateCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftCandidateCursor")
            .field("encoded_bytes", &self.0.len())
            .finish_non_exhaustive()
    }
}

/// Immutable snapshot/high-watermark identity chosen by the concrete candidate reader.
///
/// Every continuation request must carry the same fence. The database-neutral contract does not
/// prescribe whether PostgreSQL later implements this as an exported snapshot, a captured
/// high-watermark, or another read-only repeatable boundary.
#[derive(Clone, PartialEq, Eq)]
pub struct IndexDriftCandidateFence(String);

impl IndexDriftCandidateFence {
    pub fn new(value: impl Into<String>) -> Result<Self, IndexDriftCandidateError> {
        let value = value.into();
        validate_opaque_value(
            &value,
            MAX_CANDIDATE_FENCE_BYTES,
            IndexDriftCandidateError::EmptyFence,
            |actual, max| IndexDriftCandidateError::FenceTooLarge { actual, max },
        )?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Debug for IndexDriftCandidateFence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftCandidateFence")
            .field("encoded_bytes", &self.0.len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftCandidateRequest {
    scope: IndexDriftCandidateScope,
    fence: Option<IndexDriftCandidateFence>,
    cursor: Option<IndexDriftCandidateCursor>,
    limit: usize,
}

impl IndexDriftCandidateRequest {
    pub fn new(
        scope: IndexDriftCandidateScope,
        fence: Option<IndexDriftCandidateFence>,
        cursor: Option<IndexDriftCandidateCursor>,
        limit: usize,
    ) -> Result<Self, IndexDriftCandidateError> {
        if !(1..=MAX_CANDIDATE_PAGE_SIZE).contains(&limit) {
            return Err(IndexDriftCandidateError::InvalidPageLimit {
                actual: limit,
                max: MAX_CANDIDATE_PAGE_SIZE,
            });
        }
        if fence.is_some() != cursor.is_some() {
            return Err(IndexDriftCandidateError::IncompleteContinuation);
        }
        Ok(Self {
            scope,
            fence,
            cursor,
            limit,
        })
    }

    pub fn first_page(
        scope: IndexDriftCandidateScope,
        limit: usize,
    ) -> Result<Self, IndexDriftCandidateError> {
        Self::new(scope, None, None, limit)
    }

    pub fn scope(&self) -> &IndexDriftCandidateScope {
        &self.scope
    }

    pub fn fence(&self) -> Option<&IndexDriftCandidateFence> {
        self.fence.as_ref()
    }

    pub fn cursor(&self) -> Option<&IndexDriftCandidateCursor> {
        self.cursor.as_ref()
    }

    pub fn limit(&self) -> usize {
        self.limit
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftStaleEntityCandidate {
    key: EntityKey,
    indexed_source_version: u64,
}

impl IndexDriftStaleEntityCandidate {
    pub fn new(
        key: EntityKey,
        indexed_source_version: u64,
    ) -> Result<Self, IndexDriftCandidateError> {
        validate_entity_key(&key)?;
        if indexed_source_version == 0 {
            return Err(IndexDriftCandidateError::ZeroIndexedSourceVersion);
        }
        Ok(Self {
            key,
            indexed_source_version,
        })
    }

    pub fn key(&self) -> &EntityKey {
        &self.key
    }

    pub fn indexed_source_version(&self) -> u64 {
        self.indexed_source_version
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftOrphanLinkCandidate {
    source_key: EntityKey,
    indexed_source_version: u64,
    link_name: LinkName,
    ordinal: u32,
    target: LinkedEntityKey,
}

impl IndexDriftOrphanLinkCandidate {
    pub fn new(
        source_key: EntityKey,
        indexed_source_version: u64,
        link_name: LinkName,
        ordinal: u32,
        target: LinkedEntityKey,
    ) -> Result<Self, IndexDriftCandidateError> {
        validate_entity_key(&source_key)?;
        if indexed_source_version == 0 {
            return Err(IndexDriftCandidateError::ZeroIndexedSourceVersion);
        }
        if target.entity_id.is_nil() {
            return Err(IndexDriftCandidateError::NilTargetEntityId);
        }
        if target.schema.version.get() == 0 {
            return Err(IndexDriftCandidateError::ZeroTargetSchemaVersion);
        }
        Ok(Self {
            source_key,
            indexed_source_version,
            link_name,
            ordinal,
            target,
        })
    }

    pub fn source_key(&self) -> &EntityKey {
        &self.source_key
    }

    pub fn indexed_source_version(&self) -> u64 {
        self.indexed_source_version
    }

    pub fn link_name(&self) -> &LinkName {
        &self.link_name
    }

    pub fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub fn target(&self) -> &LinkedEntityKey {
        &self.target
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDriftCandidate {
    StaleEntity(IndexDriftStaleEntityCandidate),
    OrphanLink(IndexDriftOrphanLinkCandidate),
}

impl IndexDriftCandidate {
    pub fn stale_entity(
        key: EntityKey,
        indexed_source_version: u64,
    ) -> Result<Self, IndexDriftCandidateError> {
        IndexDriftStaleEntityCandidate::new(key, indexed_source_version).map(Self::StaleEntity)
    }

    pub fn orphan_link(
        source_key: EntityKey,
        indexed_source_version: u64,
        link_name: LinkName,
        ordinal: u32,
        target: LinkedEntityKey,
    ) -> Result<Self, IndexDriftCandidateError> {
        IndexDriftOrphanLinkCandidate::new(
            source_key,
            indexed_source_version,
            link_name,
            ordinal,
            target,
        )
        .map(Self::OrphanLink)
    }

    fn source_key(&self) -> &EntityKey {
        match self {
            Self::StaleEntity(candidate) => candidate.key(),
            Self::OrphanLink(candidate) => candidate.source_key(),
        }
    }

    fn order_key(&self) -> IndexDriftCandidateOrderKey {
        match self {
            Self::StaleEntity(candidate) => {
                IndexDriftCandidateOrderKey::StaleEntity(candidate.key.clone())
            }
            Self::OrphanLink(candidate) => IndexDriftCandidateOrderKey::OrphanLink {
                source_key: candidate.source_key.clone(),
                link_name: candidate.link_name.clone(),
                ordinal: candidate.ordinal,
                target: candidate.target.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum IndexDriftCandidateOrderKey {
    StaleEntity(EntityKey),
    OrphanLink {
        source_key: EntityKey,
        link_name: LinkName,
        ordinal: u32,
        target: LinkedEntityKey,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftCandidatePage {
    fence: IndexDriftCandidateFence,
    candidates: Vec<IndexDriftCandidate>,
    next_cursor: Option<IndexDriftCandidateCursor>,
}

impl IndexDriftCandidatePage {
    pub fn new(
        request: &IndexDriftCandidateRequest,
        fence: IndexDriftCandidateFence,
        candidates: Vec<IndexDriftCandidate>,
        next_cursor: Option<IndexDriftCandidateCursor>,
    ) -> Result<Self, IndexDriftCandidateError> {
        if candidates.len() > request.limit {
            return Err(IndexDriftCandidateError::PageTooLarge {
                actual: candidates.len(),
                max: request.limit,
            });
        }
        if request
            .fence
            .as_ref()
            .is_some_and(|expected| expected != &fence)
        {
            return Err(IndexDriftCandidateError::FenceChanged);
        }
        if next_cursor.is_some() && candidates.is_empty() {
            return Err(IndexDriftCandidateError::EmptyPageContinuation);
        }
        if request
            .cursor
            .as_ref()
            .is_some_and(|current| next_cursor.as_ref().is_some_and(|next| current == next))
        {
            return Err(IndexDriftCandidateError::CursorDidNotAdvance);
        }

        let mut previous = None;
        for (position, candidate) in candidates.iter().enumerate() {
            let key = candidate.source_key();
            if key.tenant_id != request.scope.tenant_id || key.schema != request.scope.schema {
                return Err(IndexDriftCandidateError::CandidateScopeMismatch { position });
            }
            let order_key = candidate.order_key();
            if previous
                .as_ref()
                .is_some_and(|previous| previous >= &order_key)
            {
                return Err(IndexDriftCandidateError::UnstableCandidateOrder { position });
            }
            previous = Some(order_key);
        }

        Ok(Self {
            fence,
            candidates,
            next_cursor,
        })
    }

    pub fn fence(&self) -> &IndexDriftCandidateFence {
        &self.fence
    }

    pub fn candidates(&self) -> &[IndexDriftCandidate] {
        &self.candidates
    }

    pub fn next_cursor(&self) -> Option<&IndexDriftCandidateCursor> {
        self.next_cursor.as_ref()
    }

    pub fn is_complete(&self) -> bool {
        self.next_cursor.is_none()
    }

    pub fn into_parts(
        self,
    ) -> (
        IndexDriftCandidateFence,
        Vec<IndexDriftCandidate>,
        Option<IndexDriftCandidateCursor>,
    ) {
        (self.fence, self.candidates, self.next_cursor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftCandidateFailureKind {
    Retryable,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("Index drift candidate reader reported a {kind:?} failure ({code})")]
pub struct IndexDriftCandidateFailure {
    kind: IndexDriftCandidateFailureKind,
    code: String,
}

impl IndexDriftCandidateFailure {
    pub fn retryable(code: impl Into<String>) -> Result<Self, IndexDriftCandidateError> {
        Self::new(IndexDriftCandidateFailureKind::Retryable, code)
    }

    pub fn permanent(code: impl Into<String>) -> Result<Self, IndexDriftCandidateError> {
        Self::new(IndexDriftCandidateFailureKind::Permanent, code)
    }

    fn new(
        kind: IndexDriftCandidateFailureKind,
        code: impl Into<String>,
    ) -> Result<Self, IndexDriftCandidateError> {
        let code = code.into();
        if !valid_machine_name(&code) {
            return Err(IndexDriftCandidateError::InvalidFailureCode(code));
        }
        Ok(Self { kind, code })
    }

    pub fn kind(&self) -> IndexDriftCandidateFailureKind {
        self.kind
    }

    pub fn code(&self) -> &str {
        &self.code
    }
}

#[async_trait]
pub trait IndexDriftCandidateReader: Send + Sync {
    async fn read_candidate_page(
        &self,
        request: IndexDriftCandidateRequest,
    ) -> Result<IndexDriftCandidatePage, IndexDriftCandidateFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexDriftCandidateError {
    #[error("Index drift candidate tenant cannot be nil")]
    NilTenantId,
    #[error("Index drift candidate schema version must be positive")]
    ZeroSchemaVersion,
    #[error("Index drift candidate entity cannot be nil")]
    NilEntityId,
    #[error("Index drift candidate target entity cannot be nil")]
    NilTargetEntityId,
    #[error("Index drift candidate target schema version must be positive")]
    ZeroTargetSchemaVersion,
    #[error("Index drift candidate indexed source version must be positive")]
    ZeroIndexedSourceVersion,
    #[error("Index drift candidate page limit is invalid: actual={actual}, max={max}")]
    InvalidPageLimit { actual: usize, max: usize },
    #[error("Index drift candidate continuation requires both fence and cursor")]
    IncompleteContinuation,
    #[error("Index drift candidate cursor cannot be empty")]
    EmptyCursor,
    #[error("Index drift candidate cursor is too large: actual={actual}, max={max}")]
    CursorTooLarge { actual: usize, max: usize },
    #[error("Index drift candidate fence cannot be empty")]
    EmptyFence,
    #[error("Index drift candidate fence is too large: actual={actual}, max={max}")]
    FenceTooLarge { actual: usize, max: usize },
    #[error("Index drift candidate page exceeds its request: actual={actual}, max={max}")]
    PageTooLarge { actual: usize, max: usize },
    #[error("Index drift candidate page changed its immutable fence")]
    FenceChanged,
    #[error("Index drift candidate page returned an empty page with continuation")]
    EmptyPageContinuation,
    #[error("Index drift candidate continuation cursor did not advance")]
    CursorDidNotAdvance,
    #[error("Index drift candidate at position {position} escapes the request scope")]
    CandidateScopeMismatch { position: usize },
    #[error("Index drift candidate order is unstable at position {position}")]
    UnstableCandidateOrder { position: usize },
    #[error("Index drift candidate failure code is invalid: {0}")]
    InvalidFailureCode(String),
}

fn validate_entity_key(key: &EntityKey) -> Result<(), IndexDriftCandidateError> {
    if key.tenant_id.is_nil() {
        return Err(IndexDriftCandidateError::NilTenantId);
    }
    if key.schema.version.get() == 0 {
        return Err(IndexDriftCandidateError::ZeroSchemaVersion);
    }
    if key.entity_id.is_nil() {
        return Err(IndexDriftCandidateError::NilEntityId);
    }
    Ok(())
}

fn validate_opaque_value<TooLarge>(
    value: &str,
    max: usize,
    empty: IndexDriftCandidateError,
    too_large: TooLarge,
) -> Result<(), IndexDriftCandidateError>
where
    TooLarge: FnOnce(usize, usize) -> IndexDriftCandidateError,
{
    if value.is_empty() {
        return Err(empty);
    }
    if value.len() > max {
        return Err(too_large(value.len(), max));
    }
    Ok(())
}

fn valid_machine_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_FAILURE_CODE_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EntityName, LocaleKey, ModuleName, SchemaVersion};

    fn schema(entity: &str) -> SchemaRef {
        SchemaRef {
            module: ModuleName::new("rustok-product").expect("valid module"),
            entity: EntityName::new(entity).expect("valid entity"),
            version: SchemaVersion::new(1),
        }
    }

    fn key(entity_id: Uuid) -> EntityKey {
        EntityKey {
            tenant_id: Uuid::from_u128(1),
            schema: schema("product"),
            entity_id,
            locale: Some(LocaleKey::new("en-US").expect("valid locale")),
        }
    }

    #[test]
    fn continuation_requires_fence_and_cursor_together() {
        let scope = IndexDriftCandidateScope::new(Uuid::from_u128(1), schema("product"))
            .expect("valid scope");
        let fence = IndexDriftCandidateFence::new("snapshot-1").expect("valid fence");
        let error = IndexDriftCandidateRequest::new(scope, Some(fence), None, 16)
            .expect_err("partial continuation must fail");
        assert_eq!(error, IndexDriftCandidateError::IncompleteContinuation);
    }

    #[test]
    fn page_rejects_scope_escape_and_unstable_order() {
        let scope = IndexDriftCandidateScope::new(Uuid::from_u128(1), schema("product"))
            .expect("valid scope");
        let request = IndexDriftCandidateRequest::first_page(scope, 16).expect("valid request");
        let fence = IndexDriftCandidateFence::new("snapshot-1").expect("valid fence");

        let later =
            IndexDriftCandidate::stale_entity(key(Uuid::from_u128(3)), 3).expect("valid candidate");
        let earlier =
            IndexDriftCandidate::stale_entity(key(Uuid::from_u128(2)), 2).expect("valid candidate");
        let error =
            IndexDriftCandidatePage::new(&request, fence.clone(), vec![later, earlier], None)
                .expect_err("descending candidates must fail");
        assert_eq!(
            error,
            IndexDriftCandidateError::UnstableCandidateOrder { position: 1 }
        );

        let mut escaped = key(Uuid::from_u128(4));
        escaped.tenant_id = Uuid::from_u128(2);
        let escaped = IndexDriftCandidate::stale_entity(escaped, 4).expect("valid candidate");
        let error = IndexDriftCandidatePage::new(&request, fence, vec![escaped], None)
            .expect_err("cross-tenant candidate must fail");
        assert_eq!(
            error,
            IndexDriftCandidateError::CandidateScopeMismatch { position: 0 }
        );
    }

    #[test]
    fn page_requires_fence_stability_and_cursor_progress() {
        let scope = IndexDriftCandidateScope::new(Uuid::from_u128(1), schema("product"))
            .expect("valid scope");
        let request = IndexDriftCandidateRequest::new(
            scope,
            Some(IndexDriftCandidateFence::new("snapshot-1").expect("valid fence")),
            Some(IndexDriftCandidateCursor::new("cursor-1").expect("valid cursor")),
            16,
        )
        .expect("valid continuation request");
        let candidate =
            IndexDriftCandidate::stale_entity(key(Uuid::from_u128(2)), 2).expect("valid candidate");

        let error = IndexDriftCandidatePage::new(
            &request,
            IndexDriftCandidateFence::new("snapshot-2").expect("valid fence"),
            vec![candidate.clone()],
            None,
        )
        .expect_err("changed fence must fail");
        assert_eq!(error, IndexDriftCandidateError::FenceChanged);

        let error = IndexDriftCandidatePage::new(
            &request,
            IndexDriftCandidateFence::new("snapshot-1").expect("valid fence"),
            vec![candidate],
            Some(IndexDriftCandidateCursor::new("cursor-1").expect("valid cursor")),
        )
        .expect_err("same cursor must fail");
        assert_eq!(error, IndexDriftCandidateError::CursorDidNotAdvance);
    }

    #[test]
    fn orphan_candidate_keeps_only_typed_identity() {
        let source_key = key(Uuid::from_u128(2));
        let target = LinkedEntityKey {
            schema: schema("product-variant"),
            entity_id: Uuid::from_u128(3),
            locale: None,
        };
        let candidate = IndexDriftCandidate::orphan_link(
            source_key,
            7,
            LinkName::new("variants").expect("valid link"),
            0,
            target,
        )
        .expect("valid orphan candidate");
        assert!(matches!(candidate, IndexDriftCandidate::OrphanLink(_)));
    }

    #[test]
    fn opaque_debug_does_not_reveal_values() {
        let cursor = IndexDriftCandidateCursor::new("private-cursor").expect("valid cursor");
        let fence = IndexDriftCandidateFence::new("private-fence").expect("valid fence");
        assert!(!format!("{cursor:?}").contains("private-cursor"));
        assert!(!format!("{fence:?}").contains("private-fence"));
    }
}
