use std::{fmt, sync::Arc};

use async_trait::async_trait;
use thiserror::Error;

use crate::domain::{EntityKey, IndexMutation, IndexRecord, LinkName, LinkedEntityKey};

use super::{
    IndexDriftCandidate, IndexDriftOrphanLinkCandidate, IndexDriftStaleEntityCandidate,
    IndexSourceAbsenceError, IndexSourceError, IndexSourceFailureKind, IndexSourceLoadRequest,
    SharedIndexSourceAbsenceRegistry, SharedIndexSourceRegistry,
};

const MAX_FAILURE_CODE_BYTES: usize = 128;
const SOURCE_UNAVAILABLE: &str = "index_drift_candidate_confirmation_source_unavailable";
const SOURCE_REJECTED: &str = "index_drift_candidate_confirmation_source_rejected";
const SOURCE_CONTRACT_INVALID: &str = "index_drift_candidate_confirmation_source_contract_invalid";
const ABSENCE_UNAVAILABLE: &str = "index_drift_candidate_confirmation_absence_unavailable";
const SOURCE_BEHIND: &str = "index_drift_candidate_confirmation_source_behind";
const SOURCE_CHANGED: &str = "index_drift_candidate_confirmation_source_changed";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftCandidateMaterializedObservation {
    Unchanged,
    Changed,
}

#[async_trait]
pub trait IndexDriftCandidateMaterializedObserver: Send + Sync {
    async fn observe_candidate(
        &self,
        candidate: &IndexDriftCandidate,
    ) -> Result<IndexDriftCandidateMaterializedObservation, IndexDriftCandidateConfirmationFailure>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftCandidateNotCandidateReason {
    MaterializedChanged,
    SourcePresent,
    SourceAbsent,
    SourceVersionChanged,
    SourceLinkChanged,
    TargetPresent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftConfirmedMissingEntity {
    key: EntityKey,
    indexed_source_version: u64,
    absence_source_version: u64,
}

impl IndexDriftConfirmedMissingEntity {
    pub fn key(&self) -> &EntityKey {
        &self.key
    }

    pub fn indexed_source_version(&self) -> u64 {
        self.indexed_source_version
    }

    pub fn absence_source_version(&self) -> u64 {
        self.absence_source_version
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftConfirmedOrphanLink {
    source_key: EntityKey,
    indexed_source_version: u64,
    link_name: LinkName,
    ordinal: u32,
    target: LinkedEntityKey,
    target_absence_source_version: u64,
}

impl IndexDriftConfirmedOrphanLink {
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

    pub fn target_absence_source_version(&self) -> u64 {
        self.target_absence_source_version
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDriftConfirmedCandidate {
    MissingEntity(IndexDriftConfirmedMissingEntity),
    OrphanLink(IndexDriftConfirmedOrphanLink),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDriftCandidateConfirmationOutcome {
    Confirmed(IndexDriftConfirmedCandidate),
    NotCandidate(IndexDriftCandidateNotCandidateReason),
}

impl IndexDriftCandidateConfirmationOutcome {
    pub fn is_confirmed(&self) -> bool {
        matches!(self, Self::Confirmed(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftCandidateConfirmationFailureKind {
    Retryable,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("Index drift candidate confirmation reported a {kind:?} failure ({code})")]
pub struct IndexDriftCandidateConfirmationFailure {
    kind: IndexDriftCandidateConfirmationFailureKind,
    code: String,
}

impl IndexDriftCandidateConfirmationFailure {
    pub fn retryable(
        code: impl Into<String>,
    ) -> Result<Self, IndexDriftCandidateConfirmationError> {
        Self::new(IndexDriftCandidateConfirmationFailureKind::Retryable, code)
    }

    pub fn permanent(
        code: impl Into<String>,
    ) -> Result<Self, IndexDriftCandidateConfirmationError> {
        Self::new(IndexDriftCandidateConfirmationFailureKind::Permanent, code)
    }

    fn new(
        kind: IndexDriftCandidateConfirmationFailureKind,
        code: impl Into<String>,
    ) -> Result<Self, IndexDriftCandidateConfirmationError> {
        let code = code.into();
        if !valid_machine_name(&code) {
            return Err(IndexDriftCandidateConfirmationError::InvalidFailureCode(
                code,
            ));
        }
        Ok(Self { kind, code })
    }

    pub fn kind(&self) -> IndexDriftCandidateConfirmationFailureKind {
        self.kind
    }

    pub fn code(&self) -> &str {
        &self.code
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexDriftCandidateConfirmationError {
    #[error("Index drift candidate confirmation failure code is invalid: {0}")]
    InvalidFailureCode(String),
}

#[derive(Clone)]
pub struct IndexDriftCandidateConfirmer {
    sources: SharedIndexSourceRegistry,
    absence: Option<SharedIndexSourceAbsenceRegistry>,
    materialized: Arc<dyn IndexDriftCandidateMaterializedObserver>,
}

impl IndexDriftCandidateConfirmer {
    pub fn new<O>(sources: SharedIndexSourceRegistry, materialized: O) -> Self
    where
        O: IndexDriftCandidateMaterializedObserver + 'static,
    {
        Self {
            sources,
            absence: None,
            materialized: Arc::new(materialized),
        }
    }

    pub fn new_boxed(
        sources: SharedIndexSourceRegistry,
        materialized: Arc<dyn IndexDriftCandidateMaterializedObserver>,
    ) -> Self {
        Self {
            sources,
            absence: None,
            materialized,
        }
    }

    pub fn with_absence_registry(mut self, absence: SharedIndexSourceAbsenceRegistry) -> Self {
        self.absence = Some(absence);
        self
    }

    pub async fn confirm_candidate(
        &self,
        candidate: &IndexDriftCandidate,
    ) -> Result<IndexDriftCandidateConfirmationOutcome, IndexDriftCandidateConfirmationFailure>
    {
        if self.materialized.observe_candidate(candidate).await?
            != IndexDriftCandidateMaterializedObservation::Unchanged
        {
            return Ok(not_candidate(
                IndexDriftCandidateNotCandidateReason::MaterializedChanged,
            ));
        }

        let outcome = match candidate {
            IndexDriftCandidate::StaleEntity(candidate) => {
                self.confirm_stale_entity(candidate).await?
            }
            IndexDriftCandidate::OrphanLink(candidate) => {
                self.confirm_orphan_link(candidate).await?
            }
        };
        if !outcome.is_confirmed() {
            return Ok(outcome);
        }

        if self.materialized.observe_candidate(candidate).await?
            != IndexDriftCandidateMaterializedObservation::Unchanged
        {
            return Ok(not_candidate(
                IndexDriftCandidateNotCandidateReason::MaterializedChanged,
            ));
        }
        Ok(outcome)
    }

    async fn confirm_stale_entity(
        &self,
        candidate: &IndexDriftStaleEntityCandidate,
    ) -> Result<IndexDriftCandidateConfirmationOutcome, IndexDriftCandidateConfirmationFailure>
    {
        let first = self.load_entity_authority(candidate.key()).await?;
        let first_absence = match first {
            EntityAuthoritySummary::Present => {
                return Ok(not_candidate(
                    IndexDriftCandidateNotCandidateReason::SourcePresent,
                ));
            }
            EntityAuthoritySummary::Absent { source_version } => source_version,
        };
        if first_absence < candidate.indexed_source_version() {
            return Err(retryable_failure(SOURCE_BEHIND));
        }

        let second = self.load_entity_authority(candidate.key()).await?;
        match second {
            EntityAuthoritySummary::Present => {
                return Ok(not_candidate(
                    IndexDriftCandidateNotCandidateReason::SourcePresent,
                ));
            }
            EntityAuthoritySummary::Absent { source_version }
                if source_version == first_absence => {}
            EntityAuthoritySummary::Absent { .. } => {
                return Err(retryable_failure(SOURCE_CHANGED));
            }
        }

        Ok(IndexDriftCandidateConfirmationOutcome::Confirmed(
            IndexDriftConfirmedCandidate::MissingEntity(IndexDriftConfirmedMissingEntity {
                key: candidate.key().clone(),
                indexed_source_version: candidate.indexed_source_version(),
                absence_source_version: first_absence,
            }),
        ))
    }

    async fn confirm_orphan_link(
        &self,
        candidate: &IndexDriftOrphanLinkCandidate,
    ) -> Result<IndexDriftCandidateConfirmationOutcome, IndexDriftCandidateConfirmationFailure>
    {
        let first_source = self.load_source_link_authority(candidate).await?;
        match &first_source {
            SourceLinkAuthoritySummary::Absent => {
                return Ok(not_candidate(
                    IndexDriftCandidateNotCandidateReason::SourceAbsent,
                ));
            }
            SourceLinkAuthoritySummary::Present { source_version, .. }
                if *source_version != candidate.indexed_source_version() =>
            {
                return Ok(not_candidate(
                    IndexDriftCandidateNotCandidateReason::SourceVersionChanged,
                ));
            }
            SourceLinkAuthoritySummary::Present {
                exact_link_present: false,
                ..
            } => {
                return Ok(not_candidate(
                    IndexDriftCandidateNotCandidateReason::SourceLinkChanged,
                ));
            }
            SourceLinkAuthoritySummary::Present { .. } => {}
        }

        let target_key = target_entity_key(candidate);
        let first_target = self.load_entity_authority(&target_key).await?;
        let first_target_absence = match first_target {
            EntityAuthoritySummary::Present => {
                return Ok(not_candidate(
                    IndexDriftCandidateNotCandidateReason::TargetPresent,
                ));
            }
            EntityAuthoritySummary::Absent { source_version } => source_version,
        };

        let second_source = self.load_source_link_authority(candidate).await?;
        if second_source != first_source {
            return Ok(not_candidate(
                IndexDriftCandidateNotCandidateReason::SourceLinkChanged,
            ));
        }

        let second_target = self.load_entity_authority(&target_key).await?;
        match second_target {
            EntityAuthoritySummary::Present => {
                return Ok(not_candidate(
                    IndexDriftCandidateNotCandidateReason::TargetPresent,
                ));
            }
            EntityAuthoritySummary::Absent { source_version }
                if source_version == first_target_absence => {}
            EntityAuthoritySummary::Absent { .. } => {
                return Err(retryable_failure(SOURCE_CHANGED));
            }
        }

        Ok(IndexDriftCandidateConfirmationOutcome::Confirmed(
            IndexDriftConfirmedCandidate::OrphanLink(IndexDriftConfirmedOrphanLink {
                source_key: candidate.source_key().clone(),
                indexed_source_version: candidate.indexed_source_version(),
                link_name: candidate.link_name().clone(),
                ordinal: candidate.ordinal(),
                target: candidate.target().clone(),
                target_absence_source_version: first_target_absence,
            }),
        ))
    }

    async fn load_entity_authority(
        &self,
        key: &EntityKey,
    ) -> Result<EntityAuthoritySummary, IndexDriftCandidateConfirmationFailure> {
        match self.load_single_mutation(key).await? {
            Some(IndexMutation::Upsert { record, .. }) => {
                validate_record_identity(&record, key)?;
                Ok(EntityAuthoritySummary::Present)
            }
            Some(IndexMutation::Delete {
                key: returned_key,
                source_version,
                ..
            }) => {
                if &returned_key != key || source_version == 0 {
                    return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
                }
                Ok(EntityAuthoritySummary::Absent { source_version })
            }
            None => self.load_retained_absence(key).await,
        }
    }

    async fn load_source_link_authority(
        &self,
        candidate: &IndexDriftOrphanLinkCandidate,
    ) -> Result<SourceLinkAuthoritySummary, IndexDriftCandidateConfirmationFailure> {
        match self.load_single_mutation(candidate.source_key()).await? {
            Some(IndexMutation::Upsert { record, .. }) => {
                validate_record_identity(&record, candidate.source_key())?;
                let exact_link_present = record_has_exact_link(&record, candidate)?;
                Ok(SourceLinkAuthoritySummary::Present {
                    source_version: record.source_version,
                    exact_link_present,
                })
            }
            Some(IndexMutation::Delete {
                key,
                source_version,
                ..
            }) => {
                if &key != candidate.source_key() || source_version == 0 {
                    return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
                }
                Ok(SourceLinkAuthoritySummary::Absent)
            }
            None => match self.load_retained_absence(candidate.source_key()).await? {
                EntityAuthoritySummary::Absent { .. } => Ok(SourceLinkAuthoritySummary::Absent),
                EntityAuthoritySummary::Present => Err(permanent_failure(SOURCE_CONTRACT_INVALID)),
            },
        }
    }

    async fn load_single_mutation(
        &self,
        key: &EntityKey,
    ) -> Result<Option<IndexMutation>, IndexDriftCandidateConfirmationFailure> {
        let request = IndexSourceLoadRequest::new(vec![key.clone()])
            .map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))?;
        let batch = self.sources.load(request).await.map_err(map_source_error)?;
        let mut mutations = batch.into_mutations();
        if mutations.len() > 1 {
            return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
        }
        Ok(mutations.pop())
    }

    async fn load_retained_absence(
        &self,
        key: &EntityKey,
    ) -> Result<EntityAuthoritySummary, IndexDriftCandidateConfirmationFailure> {
        let Some(absence) = &self.absence else {
            return Err(permanent_failure(ABSENCE_UNAVAILABLE));
        };
        if absence.provider_for_schema(&key.schema).is_none() {
            return Err(permanent_failure(ABSENCE_UNAVAILABLE));
        }
        let watermark = absence
            .load(key.clone())
            .await
            .map_err(map_absence_error)?
            .ok_or_else(|| permanent_failure(ABSENCE_UNAVAILABLE))?;
        if watermark.key() != key || watermark.source_version() == 0 {
            return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
        }
        Ok(EntityAuthoritySummary::Absent {
            source_version: watermark.source_version(),
        })
    }
}

impl fmt::Debug for IndexDriftCandidateConfirmer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftCandidateConfirmer")
            .field("source_count", &self.sources.len())
            .field(
                "absence_provider_count",
                &self.absence.as_ref().map(|value| value.len()),
            )
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EntityAuthoritySummary {
    Present,
    Absent { source_version: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SourceLinkAuthoritySummary {
    Present {
        source_version: u64,
        exact_link_present: bool,
    },
    Absent,
}

fn target_entity_key(candidate: &IndexDriftOrphanLinkCandidate) -> EntityKey {
    EntityKey {
        tenant_id: candidate.source_key().tenant_id,
        schema: candidate.target().schema.clone(),
        entity_id: candidate.target().entity_id,
        locale: candidate.target().locale.clone(),
    }
}

fn validate_record_identity(
    record: &IndexRecord,
    expected: &EntityKey,
) -> Result<(), IndexDriftCandidateConfirmationFailure> {
    if &record.key != expected || record.source_version == 0 {
        return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
    }
    Ok(())
}

fn record_has_exact_link(
    record: &IndexRecord,
    candidate: &IndexDriftOrphanLinkCandidate,
) -> Result<bool, IndexDriftCandidateConfirmationFailure> {
    let mut values = record
        .links
        .iter()
        .filter(|value| &value.name == candidate.link_name());
    let Some(value) = values.next() else {
        return Ok(false);
    };
    if values.next().is_some() {
        return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
    }
    let ordinal = usize::try_from(candidate.ordinal())
        .map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))?;
    Ok(value.targets.get(ordinal) == Some(candidate.target()))
}

fn map_source_error(error: IndexSourceError) -> IndexDriftCandidateConfirmationFailure {
    match error {
        IndexSourceError::SourceFailure { failure, .. } => match failure.kind() {
            IndexSourceFailureKind::Retryable => retryable_failure(SOURCE_UNAVAILABLE),
            IndexSourceFailureKind::Permanent => permanent_failure(SOURCE_REJECTED),
        },
        _ => permanent_failure(SOURCE_CONTRACT_INVALID),
    }
}

fn map_absence_error(error: IndexSourceAbsenceError) -> IndexDriftCandidateConfirmationFailure {
    match error {
        IndexSourceAbsenceError::ProviderFailure { failure, .. } => match failure.kind() {
            IndexSourceFailureKind::Retryable => retryable_failure(SOURCE_UNAVAILABLE),
            IndexSourceFailureKind::Permanent => permanent_failure(SOURCE_REJECTED),
        },
        IndexSourceAbsenceError::UnknownSchemaProvider(_) => permanent_failure(ABSENCE_UNAVAILABLE),
        _ => permanent_failure(SOURCE_CONTRACT_INVALID),
    }
}

fn not_candidate(
    reason: IndexDriftCandidateNotCandidateReason,
) -> IndexDriftCandidateConfirmationOutcome {
    IndexDriftCandidateConfirmationOutcome::NotCandidate(reason)
}

fn retryable_failure(code: &str) -> IndexDriftCandidateConfirmationFailure {
    IndexDriftCandidateConfirmationFailure::retryable(code)
        .expect("static candidate confirmation failure code is valid")
}

fn permanent_failure(code: &str) -> IndexDriftCandidateConfirmationFailure {
    IndexDriftCandidateConfirmationFailure::permanent(code)
        .expect("static candidate confirmation failure code is valid")
}

fn valid_machine_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_FAILURE_CODE_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
}
