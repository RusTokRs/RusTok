use std::{fmt, sync::Arc};

use async_trait::async_trait;
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{EntityKey, IndexMutation, IndexRecord, RecordValidationError, SchemaRegistry};

const MAX_BOUNDARY_BYTES: usize = 191;
const MAX_FAILURE_CODE_BYTES: usize = 128;
const DIGEST_BYTES: usize = 64;
const ENTITY_STATE_DIGEST_CONTRACT: &[u8] = b"index_drift_entity_state_digest_v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftSnapshotBoundary(String);

impl IndexDriftSnapshotBoundary {
    pub fn new(value: impl Into<String>) -> Result<Self, IndexDriftDigestError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_BOUNDARY_BYTES
            || value.trim() != value
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/')
            })
        {
            return Err(IndexDriftDigestError::InvalidSnapshotBoundary);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IndexDriftSnapshotBoundary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum IndexDriftEntityState {
    Missing { key: EntityKey },
    Upsert { record: IndexRecord },
    Delete { key: EntityKey, source_version: u64 },
}

impl IndexDriftEntityState {
    pub fn missing(key: EntityKey) -> Self {
        Self::Missing { key }
    }

    pub fn upsert(record: IndexRecord) -> Self {
        Self::Upsert { record }
    }

    pub fn delete(key: EntityKey, source_version: u64) -> Self {
        Self::Delete {
            key,
            source_version,
        }
    }

    pub fn from_mutation(mutation: IndexMutation) -> Self {
        match mutation {
            IndexMutation::Upsert { record, .. } => Self::Upsert { record },
            IndexMutation::Delete {
                key,
                source_version,
                ..
            } => Self::Delete {
                key,
                source_version,
            },
        }
    }

    pub fn key(&self) -> &EntityKey {
        match self {
            Self::Missing { key } | Self::Delete { key, .. } => key,
            Self::Upsert { record } => &record.key,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftSnapshotView {
    boundary: IndexDriftSnapshotBoundary,
    state: IndexDriftEntityState,
}

impl IndexDriftSnapshotView {
    pub fn new(boundary: IndexDriftSnapshotBoundary, state: IndexDriftEntityState) -> Self {
        Self { boundary, state }
    }

    pub fn boundary(&self) -> &IndexDriftSnapshotBoundary {
        &self.boundary
    }

    pub fn state(&self) -> &IndexDriftEntityState {
        &self.state
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftSnapshotPair {
    boundary: IndexDriftSnapshotBoundary,
    source: IndexDriftEntityState,
    materialized: IndexDriftEntityState,
}

impl IndexDriftSnapshotPair {
    pub fn new(
        source: IndexDriftSnapshotView,
        materialized: IndexDriftSnapshotView,
    ) -> Result<Self, IndexDriftDigestError> {
        if source.boundary != materialized.boundary {
            return Err(IndexDriftDigestError::SnapshotBoundaryMismatch);
        }
        if source.state.key() != materialized.state.key() {
            return Err(IndexDriftDigestError::SnapshotKeyMismatch);
        }
        Ok(Self {
            boundary: source.boundary,
            source: source.state,
            materialized: materialized.state,
        })
    }

    pub fn boundary(&self) -> &IndexDriftSnapshotBoundary {
        &self.boundary
    }

    pub fn source(&self) -> &IndexDriftEntityState {
        &self.source
    }

    pub fn materialized(&self) -> &IndexDriftEntityState {
        &self.materialized
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftDigestRequest {
    key: EntityKey,
}

impl IndexDriftDigestRequest {
    pub fn new(key: EntityKey) -> Result<Self, IndexDriftDigestError> {
        if key.tenant_id.is_nil() {
            return Err(IndexDriftDigestError::NilTenantId);
        }
        if key.entity_id.is_nil() {
            return Err(IndexDriftDigestError::NilEntityId);
        }
        if key.schema.version.get() == 0 {
            return Err(IndexDriftDigestError::ZeroSchemaVersion);
        }
        Ok(Self { key })
    }

    pub fn key(&self) -> &EntityKey {
        &self.key
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftDependencyFailureKind {
    Retryable,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("Index drift dependency reported a {kind:?} failure ({code})")]
pub struct IndexDriftDependencyFailure {
    kind: IndexDriftDependencyFailureKind,
    code: String,
}

impl IndexDriftDependencyFailure {
    pub fn retryable(code: impl Into<String>) -> Result<Self, IndexDriftDigestError> {
        Self::new(IndexDriftDependencyFailureKind::Retryable, code)
    }

    pub fn permanent(code: impl Into<String>) -> Result<Self, IndexDriftDigestError> {
        Self::new(IndexDriftDependencyFailureKind::Permanent, code)
    }

    fn new(
        kind: IndexDriftDependencyFailureKind,
        code: impl Into<String>,
    ) -> Result<Self, IndexDriftDigestError> {
        let code = code.into();
        if !valid_machine_name(&code) {
            return Err(IndexDriftDigestError::InvalidFailureCode(code));
        }
        Ok(Self { kind, code })
    }

    pub fn kind(&self) -> IndexDriftDependencyFailureKind {
        self.kind
    }

    pub fn code(&self) -> &str {
        &self.code
    }
}

#[async_trait]
pub trait IndexDriftSnapshotReader: Send + Sync {
    async fn capture_entity_snapshot(
        &self,
        request: &IndexDriftDigestRequest,
    ) -> Result<IndexDriftSnapshotPair, IndexDriftDependencyFailure>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftMismatchRecordStatus {
    Created,
    Refreshed,
    Reopened,
    Suppressed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftMismatchReceipt {
    status: IndexDriftMismatchRecordStatus,
    finding_id: Uuid,
    finding_key: String,
}

impl IndexDriftMismatchReceipt {
    pub(crate) fn new(
        status: IndexDriftMismatchRecordStatus,
        finding_id: Uuid,
        finding_key: impl Into<String>,
    ) -> Result<Self, IndexDriftDigestError> {
        let finding_key = finding_key.into();
        if finding_id.is_nil() || !valid_digest(&finding_key) {
            return Err(IndexDriftDigestError::InvalidRecorderReceipt);
        }
        Ok(Self {
            status,
            finding_id,
            finding_key,
        })
    }

    pub fn status(&self) -> IndexDriftMismatchRecordStatus {
        self.status
    }

    pub fn finding_id(&self) -> Uuid {
        self.finding_id
    }

    pub fn finding_key(&self) -> &str {
        &self.finding_key
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftDigestMismatch {
    boundary: IndexDriftSnapshotBoundary,
    key: EntityKey,
    source_digest: String,
    materialized_digest: String,
}

impl IndexDriftDigestMismatch {
    pub fn boundary(&self) -> &IndexDriftSnapshotBoundary {
        &self.boundary
    }

    pub fn key(&self) -> &EntityKey {
        &self.key
    }

    pub fn source_digest(&self) -> &str {
        &self.source_digest
    }

    pub fn materialized_digest(&self) -> &str {
        &self.materialized_digest
    }
}

#[async_trait]
pub trait IndexDriftMismatchRecorder: Send + Sync {
    async fn record_digest_mismatch(
        &self,
        mismatch: &IndexDriftDigestMismatch,
    ) -> Result<IndexDriftMismatchReceipt, IndexDriftDependencyFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDriftDigestOutcome {
    Consistent {
        digest: String,
    },
    MismatchRecorded {
        source_digest: String,
        materialized_digest: String,
        receipt: IndexDriftMismatchReceipt,
    },
}

/// Missing-only diagnosis outcome over one validated exact snapshot pair.
///
/// Non-candidate state combinations remain intentionally opaque. Only authoritative source
/// `Upsert` plus materialized `Missing` can produce a finding receipt through this boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDriftMissingEntityCandidateOutcome {
    NotCandidate,
    MissingRecorded {
        source_digest: String,
        materialized_digest: String,
        receipt: IndexDriftMismatchReceipt,
    },
}

pub struct IndexDriftDigestProducer<R, W> {
    registry: Arc<SchemaRegistry>,
    snapshots: R,
    recorder: W,
}

impl<R, W> fmt::Debug for IndexDriftDigestProducer<R, W> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftDigestProducer")
            .finish_non_exhaustive()
    }
}

impl<R, W> IndexDriftDigestProducer<R, W>
where
    R: IndexDriftSnapshotReader,
    W: IndexDriftMismatchRecorder,
{
    pub fn new(registry: Arc<SchemaRegistry>, snapshots: R, recorder: W) -> Self {
        Self {
            registry,
            snapshots,
            recorder,
        }
    }

    pub async fn produce(
        &self,
        request: IndexDriftDigestRequest,
    ) -> Result<IndexDriftDigestOutcome, IndexDriftDigestError> {
        let pair = self.capture_pair(&request).await?;
        self.validate_pair(&request, &pair)?;

        let source_digest = digest_state(pair.source())?;
        let materialized_digest = digest_state(pair.materialized())?;
        if source_digest == materialized_digest {
            return Ok(IndexDriftDigestOutcome::Consistent {
                digest: source_digest,
            });
        }

        let receipt = self
            .record_mismatch(
                request.key,
                &pair,
                source_digest.clone(),
                materialized_digest.clone(),
            )
            .await?;
        Ok(IndexDriftDigestOutcome::MismatchRecorded {
            source_digest,
            materialized_digest,
            receipt,
        })
    }

    /// Captures exactly one snapshot pair and applies the missing-only candidate contract.
    pub async fn produce_missing_entity_candidate(
        &self,
        request: IndexDriftDigestRequest,
    ) -> Result<IndexDriftMissingEntityCandidateOutcome, IndexDriftDigestError> {
        let pair = self.capture_pair(&request).await?;
        self.produce_missing_entity_candidate_from_pair(request, pair)
            .await
    }

    /// Applies the missing-only candidate contract to one already-captured exact snapshot pair.
    ///
    /// Every state is validated against the frozen schema registry before classification. All
    /// combinations except source `Upsert` plus materialized `Missing` return `NotCandidate`
    /// without digest persistence or recorder access.
    pub async fn produce_missing_entity_candidate_from_pair(
        &self,
        request: IndexDriftDigestRequest,
        pair: IndexDriftSnapshotPair,
    ) -> Result<IndexDriftMissingEntityCandidateOutcome, IndexDriftDigestError> {
        self.validate_pair(&request, &pair)?;
        if !matches!(pair.source(), IndexDriftEntityState::Upsert { .. })
            || !matches!(pair.materialized(), IndexDriftEntityState::Missing { .. })
        {
            return Ok(IndexDriftMissingEntityCandidateOutcome::NotCandidate);
        }

        let source_digest = digest_state(pair.source())?;
        let materialized_digest = digest_state(pair.materialized())?;
        let receipt = self
            .record_mismatch(
                request.key,
                &pair,
                source_digest.clone(),
                materialized_digest.clone(),
            )
            .await?;
        Ok(IndexDriftMissingEntityCandidateOutcome::MissingRecorded {
            source_digest,
            materialized_digest,
            receipt,
        })
    }

    async fn capture_pair(
        &self,
        request: &IndexDriftDigestRequest,
    ) -> Result<IndexDriftSnapshotPair, IndexDriftDigestError> {
        self.snapshots
            .capture_entity_snapshot(request)
            .await
            .map_err(IndexDriftDigestError::SnapshotCaptureFailed)
    }

    fn validate_pair(
        &self,
        request: &IndexDriftDigestRequest,
        pair: &IndexDriftSnapshotPair,
    ) -> Result<(), IndexDriftDigestError> {
        if pair.source().key() != request.key() || pair.materialized().key() != request.key() {
            return Err(IndexDriftDigestError::SnapshotScopeMismatch);
        }
        validate_state(self.registry.as_ref(), pair.source())
            .map_err(IndexDriftDigestError::InvalidSourceState)?;
        validate_state(self.registry.as_ref(), pair.materialized())
            .map_err(IndexDriftDigestError::InvalidMaterializedState)?;
        Ok(())
    }

    async fn record_mismatch(
        &self,
        key: EntityKey,
        pair: &IndexDriftSnapshotPair,
        source_digest: String,
        materialized_digest: String,
    ) -> Result<IndexDriftMismatchReceipt, IndexDriftDigestError> {
        let mismatch = IndexDriftDigestMismatch {
            boundary: pair.boundary().clone(),
            key,
            source_digest,
            materialized_digest,
        };
        self.recorder
            .record_digest_mismatch(&mismatch)
            .await
            .map_err(IndexDriftDigestError::MismatchRecordFailed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexDriftDigestError {
    #[error("Index drift digest tenant id must not be nil")]
    NilTenantId,
    #[error("Index drift digest entity id must not be nil")]
    NilEntityId,
    #[error("Index drift digest schema version must be positive")]
    ZeroSchemaVersion,
    #[error("Index drift snapshot boundary is invalid")]
    InvalidSnapshotBoundary,
    #[error("Index drift snapshot views do not share one consistency boundary")]
    SnapshotBoundaryMismatch,
    #[error("Index drift snapshot views do not describe the same entity key")]
    SnapshotKeyMismatch,
    #[error("Index drift snapshot does not match the requested entity scope")]
    SnapshotScopeMismatch,
    #[error("Index drift dependency failure code is invalid: {0}")]
    InvalidFailureCode(String),
    #[error("Index drift snapshot capture failed")]
    SnapshotCaptureFailed(#[source] IndexDriftDependencyFailure),
    #[error("Index drift source state is invalid")]
    InvalidSourceState(#[source] RecordValidationError),
    #[error("Index drift materialized state is invalid")]
    InvalidMaterializedState(#[source] RecordValidationError),
    #[error("Index drift state digest serialization failed")]
    DigestSerializationFailed,
    #[error("Index drift mismatch recorder returned an invalid receipt")]
    InvalidRecorderReceipt,
    #[error("Index drift mismatch persistence failed")]
    MismatchRecordFailed(#[source] IndexDriftDependencyFailure),
}

fn validate_state(
    registry: &SchemaRegistry,
    state: &IndexDriftEntityState,
) -> Result<(), RecordValidationError> {
    let mutation = match state {
        IndexDriftEntityState::Missing { key } => IndexMutation::Delete {
            event_id: Uuid::nil(),
            key: key.clone(),
            source_version: 1,
        },
        IndexDriftEntityState::Upsert { record } => IndexMutation::Upsert {
            event_id: Uuid::nil(),
            record: record.clone(),
        },
        IndexDriftEntityState::Delete {
            key,
            source_version,
        } => IndexMutation::Delete {
            event_id: Uuid::nil(),
            key: key.clone(),
            source_version: *source_version,
        },
    };
    registry.validate_mutation(&mutation)
}

fn digest_state(state: &IndexDriftEntityState) -> Result<String, IndexDriftDigestError> {
    let encoded = postcard::to_allocvec(state)
        .map_err(|_| IndexDriftDigestError::DigestSerializationFailed)?;
    let mut hasher = Sha256::new();
    write_bytes(&mut hasher, ENTITY_STATE_DIGEST_CONTRACT);
    write_bytes(&mut hasher, &encoded);
    Ok(hex::encode(hasher.finalize()))
}

fn write_bytes(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn valid_machine_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_FAILURE_CODE_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
}

fn valid_digest(value: &str) -> bool {
    value.len() == DIGEST_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;

    use super::*;
    use crate::{
        EntityName, FieldCardinality, FieldName, IndexField, IndexSchema, IndexValue,
        IndexValueType, LocaleMode, ModuleName, SchemaRef, SchemaVersion,
    };

    #[derive(Clone)]
    struct SnapshotFixture {
        pair: IndexDriftSnapshotPair,
    }

    #[async_trait]
    impl IndexDriftSnapshotReader for SnapshotFixture {
        async fn capture_entity_snapshot(
            &self,
            _request: &IndexDriftDigestRequest,
        ) -> Result<IndexDriftSnapshotPair, IndexDriftDependencyFailure> {
            Ok(self.pair.clone())
        }
    }

    #[derive(Clone, Default)]
    struct RecorderFixture {
        mismatches: Arc<Mutex<Vec<IndexDriftDigestMismatch>>>,
    }

    #[async_trait]
    impl IndexDriftMismatchRecorder for RecorderFixture {
        async fn record_digest_mismatch(
            &self,
            mismatch: &IndexDriftDigestMismatch,
        ) -> Result<IndexDriftMismatchReceipt, IndexDriftDependencyFailure> {
            self.mismatches.lock().unwrap().push(mismatch.clone());
            Ok(IndexDriftMismatchReceipt::new(
                IndexDriftMismatchRecordStatus::Created,
                Uuid::from_u128(9),
                "a".repeat(DIGEST_BYTES),
            )
            .unwrap())
        }
    }

    fn schema_ref() -> SchemaRef {
        SchemaRef {
            module: ModuleName::new("drift-test").unwrap(),
            entity: EntityName::new("item").unwrap(),
            version: SchemaVersion::INITIAL,
        }
    }

    fn registry() -> Arc<SchemaRegistry> {
        let mut registry = SchemaRegistry::new();
        registry
            .register(IndexSchema {
                reference: schema_ref(),
                locale_mode: LocaleMode::None,
                fields: vec![IndexField {
                    name: FieldName::new("name").unwrap(),
                    value_type: IndexValueType::String,
                    cardinality: FieldCardinality::One,
                    nullable: false,
                    selectable: true,
                    filterable: true,
                    sortable: true,
                }],
                links: Vec::new(),
            })
            .unwrap();
        Arc::new(registry)
    }

    fn key() -> EntityKey {
        EntityKey {
            tenant_id: Uuid::from_u128(1),
            schema: schema_ref(),
            entity_id: Uuid::from_u128(2),
            locale: None,
        }
    }

    fn record(name: &str, source_version: u64) -> IndexRecord {
        IndexRecord {
            key: key(),
            source_version,
            fields: BTreeMap::from([(
                FieldName::new("name").unwrap(),
                IndexValue::String(name.to_owned()),
            )]),
            links: Vec::new(),
        }
    }

    fn view(state: IndexDriftEntityState) -> IndexDriftSnapshotView {
        IndexDriftSnapshotView::new(
            IndexDriftSnapshotBoundary::new("snapshot:42/0").unwrap(),
            state,
        )
    }

    #[tokio::test]
    async fn equal_snapshot_states_do_not_call_the_recorder() {
        let pair = IndexDriftSnapshotPair::new(
            view(IndexDriftEntityState::upsert(record("same", 7))),
            view(IndexDriftEntityState::upsert(record("same", 7))),
        )
        .unwrap();
        let recorder = RecorderFixture::default();
        let producer =
            IndexDriftDigestProducer::new(registry(), SnapshotFixture { pair }, recorder.clone());

        let outcome = producer
            .produce(IndexDriftDigestRequest::new(key()).unwrap())
            .await
            .unwrap();
        assert!(matches!(
            outcome,
            IndexDriftDigestOutcome::Consistent { .. }
        ));
        assert!(recorder.mismatches.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn unequal_snapshot_states_record_only_bounded_digests() {
        let pair = IndexDriftSnapshotPair::new(
            view(IndexDriftEntityState::upsert(record("source", 8))),
            view(IndexDriftEntityState::upsert(record("index", 7))),
        )
        .unwrap();
        let recorder = RecorderFixture::default();
        let producer =
            IndexDriftDigestProducer::new(registry(), SnapshotFixture { pair }, recorder.clone());

        let outcome = producer
            .produce(IndexDriftDigestRequest::new(key()).unwrap())
            .await
            .unwrap();
        let IndexDriftDigestOutcome::MismatchRecorded {
            source_digest,
            materialized_digest,
            receipt,
        } = outcome
        else {
            panic!("mismatch must be recorded");
        };
        assert_ne!(source_digest, materialized_digest);
        assert!(valid_digest(&source_digest));
        assert!(valid_digest(&materialized_digest));
        assert_eq!(receipt.status(), IndexDriftMismatchRecordStatus::Created);

        let retained = recorder.mismatches.lock().unwrap();
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].key(), &key());
        assert_eq!(retained[0].boundary().as_str(), "snapshot:42/0");
        assert_eq!(retained[0].source_digest(), source_digest);
        assert_eq!(retained[0].materialized_digest(), materialized_digest);
    }

    #[tokio::test]
    async fn missing_candidate_records_only_source_upsert_materialized_missing() {
        let pair = IndexDriftSnapshotPair::new(
            view(IndexDriftEntityState::upsert(record("source", 8))),
            view(IndexDriftEntityState::missing(key())),
        )
        .unwrap();
        let recorder = RecorderFixture::default();
        let producer = IndexDriftDigestProducer::new(
            registry(),
            SnapshotFixture { pair: pair.clone() },
            recorder.clone(),
        );

        let outcome = producer
            .produce_missing_entity_candidate_from_pair(
                IndexDriftDigestRequest::new(key()).unwrap(),
                pair,
            )
            .await
            .unwrap();
        let IndexDriftMissingEntityCandidateOutcome::MissingRecorded {
            source_digest,
            materialized_digest,
            receipt,
        } = outcome
        else {
            panic!("source Upsert plus materialized Missing must record");
        };
        assert_ne!(source_digest, materialized_digest);
        assert!(valid_digest(&source_digest));
        assert!(valid_digest(&materialized_digest));
        assert_eq!(receipt.status(), IndexDriftMismatchRecordStatus::Created);
        assert_eq!(recorder.mismatches.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn missing_candidate_skips_every_other_typed_state_combination() {
        let pairs = vec![
            (
                IndexDriftEntityState::missing(key()),
                IndexDriftEntityState::missing(key()),
            ),
            (
                IndexDriftEntityState::missing(key()),
                IndexDriftEntityState::upsert(record("index", 7)),
            ),
            (
                IndexDriftEntityState::missing(key()),
                IndexDriftEntityState::delete(key(), 7),
            ),
            (
                IndexDriftEntityState::upsert(record("source", 8)),
                IndexDriftEntityState::upsert(record("index", 7)),
            ),
            (
                IndexDriftEntityState::upsert(record("source", 8)),
                IndexDriftEntityState::delete(key(), 7),
            ),
            (
                IndexDriftEntityState::delete(key(), 8),
                IndexDriftEntityState::missing(key()),
            ),
            (
                IndexDriftEntityState::delete(key(), 8),
                IndexDriftEntityState::upsert(record("index", 7)),
            ),
            (
                IndexDriftEntityState::delete(key(), 8),
                IndexDriftEntityState::delete(key(), 7),
            ),
        ];
        let recorder = RecorderFixture::default();

        for (source, materialized) in pairs {
            let pair = IndexDriftSnapshotPair::new(view(source), view(materialized)).unwrap();
            let producer = IndexDriftDigestProducer::new(
                registry(),
                SnapshotFixture { pair: pair.clone() },
                recorder.clone(),
            );
            let outcome = producer
                .produce_missing_entity_candidate_from_pair(
                    IndexDriftDigestRequest::new(key()).unwrap(),
                    pair,
                )
                .await
                .unwrap();
            assert_eq!(
                outcome,
                IndexDriftMissingEntityCandidateOutcome::NotCandidate
            );
        }
        assert!(recorder.mismatches.lock().unwrap().is_empty());
    }

    #[test]
    fn snapshot_pair_rejects_different_boundaries_and_keys() {
        let source = view(IndexDriftEntityState::missing(key()));
        let materialized = IndexDriftSnapshotView::new(
            IndexDriftSnapshotBoundary::new("snapshot:43/0").unwrap(),
            IndexDriftEntityState::missing(key()),
        );
        assert!(matches!(
            IndexDriftSnapshotPair::new(source, materialized),
            Err(IndexDriftDigestError::SnapshotBoundaryMismatch)
        ));

        let mut other_key = key();
        other_key.entity_id = Uuid::from_u128(3);
        assert!(matches!(
            IndexDriftSnapshotPair::new(
                view(IndexDriftEntityState::missing(key())),
                view(IndexDriftEntityState::missing(other_key)),
            ),
            Err(IndexDriftDigestError::SnapshotKeyMismatch)
        ));
    }

    #[tokio::test]
    async fn invalid_materialized_state_fails_before_recording() {
        let pair = IndexDriftSnapshotPair::new(
            view(IndexDriftEntityState::upsert(record("source", 8))),
            view(IndexDriftEntityState::delete(key(), 0)),
        )
        .unwrap();
        let recorder = RecorderFixture::default();
        let producer =
            IndexDriftDigestProducer::new(registry(), SnapshotFixture { pair }, recorder.clone());

        assert!(matches!(
            producer
                .produce(IndexDriftDigestRequest::new(key()).unwrap())
                .await,
            Err(IndexDriftDigestError::InvalidMaterializedState(
                RecordValidationError::ZeroSourceVersion
            ))
        ));
        assert!(recorder.mismatches.lock().unwrap().is_empty());
    }

    #[test]
    fn missing_delete_and_upsert_states_have_distinct_digest_domains() {
        let missing = digest_state(&IndexDriftEntityState::missing(key())).unwrap();
        let deleted = digest_state(&IndexDriftEntityState::delete(key(), 1)).unwrap();
        let upsert = digest_state(&IndexDriftEntityState::upsert(record("value", 1))).unwrap();
        assert_ne!(missing, deleted);
        assert_ne!(missing, upsert);
        assert_ne!(deleted, upsert);
    }
}
