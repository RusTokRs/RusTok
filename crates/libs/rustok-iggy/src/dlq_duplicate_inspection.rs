use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

const DLQ_PAYLOAD_DIGEST_DOMAIN: &[u8] = b"rustok.iggy.dlq.physical_payload.v1";

/// One read-only physical DLQ observation reduced to a deterministic message ID
/// and an in-memory payload digest.
///
/// Exact bytes are accepted only by the constructor and are not retained by the
/// observation or exposed by the summary. Empty payloads remain valid. A nil
/// broker message ID is rejected because it cannot identify an immutable raw
/// poison delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DlqDuplicateObservation {
    broker_message_id: Uuid,
    payload_sha256: [u8; 32],
}

impl DlqDuplicateObservation {
    pub fn from_payload(
        broker_message_id: Uuid,
        payload: &[u8],
    ) -> Result<Self, DlqDuplicateInspectionError> {
        if broker_message_id.is_nil() {
            return Err(DlqDuplicateInspectionError::InvalidBrokerMessageId);
        }
        let mut hasher = Sha256::new();
        hash_part(&mut hasher, DLQ_PAYLOAD_DIGEST_DOMAIN);
        hash_part(&mut hasher, payload);
        let digest = hasher.finalize();
        let mut payload_sha256 = [0_u8; 32];
        payload_sha256.copy_from_slice(&digest);
        Ok(Self {
            broker_message_id,
            payload_sha256,
        })
    }
}

/// Count-only physical duplicate view for a bounded operator scan.
///
/// No broker address, stream/topic/partition/offset, message UUID, payload,
/// payload digest, receipt identity, error code, timestamp, or credential is
/// exposed. The summary cannot acknowledge, delete, replay, repair, or claim a
/// delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DlqDuplicateSummary {
    total_messages: u64,
    unique_message_ids: u64,
    duplicate_messages: u64,
    duplicate_groups: u64,
    conflicting_payload_groups: u64,
    max_copies_per_message_id: u64,
}

impl DlqDuplicateSummary {
    pub const fn total_messages(&self) -> u64 {
        self.total_messages
    }

    pub const fn unique_message_ids(&self) -> u64 {
        self.unique_message_ids
    }

    pub const fn duplicate_messages(&self) -> u64 {
        self.duplicate_messages
    }

    pub const fn duplicate_groups(&self) -> u64 {
        self.duplicate_groups
    }

    pub const fn conflicting_payload_groups(&self) -> u64 {
        self.conflicting_payload_groups
    }

    pub const fn max_copies_per_message_id(&self) -> u64 {
        self.max_copies_per_message_id
    }

    pub const fn has_physical_duplicates(&self) -> bool {
        self.duplicate_messages > 0
    }

    pub const fn has_identity_conflicts(&self) -> bool {
        self.conflicting_payload_groups > 0
    }

    /// Conflicting bytes for one deterministic ID are never treated as an
    /// ordinary duplicate and always require manual investigation.
    pub const fn requires_manual_review(&self) -> bool {
        self.has_identity_conflicts()
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum DlqDuplicateInspectionError {
    #[error("physical DLQ observation has a nil broker message ID")]
    InvalidBrokerMessageId,
    #[error("physical DLQ duplicate count overflow")]
    CountOverflow,
}

impl DlqDuplicateInspectionError {
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::InvalidBrokerMessageId => "iggy.dlq_duplicate.identity_invalid",
            Self::CountOverflow => "iggy.dlq_duplicate.count_overflow",
        }
    }
}

#[derive(Debug, Default)]
struct DuplicateGroup {
    copies: u64,
    payload_sha256: BTreeSet<[u8; 32]>,
}

/// Reduces physical DLQ observations to a privacy-safe duplicate summary.
///
/// Repeated observations with the same deterministic broker message ID and the
/// same exact bytes are ordinary physical copies. Reuse of one ID with distinct
/// bytes increments `conflicting_payload_groups` and must not be auto-reconciled.
pub fn summarize_dlq_duplicates(
    observations: impl IntoIterator<Item = DlqDuplicateObservation>,
) -> Result<DlqDuplicateSummary, DlqDuplicateInspectionError> {
    let mut groups = BTreeMap::<Uuid, DuplicateGroup>::new();
    let mut total_messages = 0_u64;

    for observation in observations {
        total_messages = total_messages
            .checked_add(1)
            .ok_or(DlqDuplicateInspectionError::CountOverflow)?;
        let group = groups.entry(observation.broker_message_id).or_default();
        group.copies = group
            .copies
            .checked_add(1)
            .ok_or(DlqDuplicateInspectionError::CountOverflow)?;
        group.payload_sha256.insert(observation.payload_sha256);
    }

    let unique_message_ids =
        u64::try_from(groups.len()).map_err(|_| DlqDuplicateInspectionError::CountOverflow)?;
    let duplicate_messages = total_messages
        .checked_sub(unique_message_ids)
        .ok_or(DlqDuplicateInspectionError::CountOverflow)?;
    let mut duplicate_groups = 0_u64;
    let mut conflicting_payload_groups = 0_u64;
    let mut max_copies_per_message_id = 0_u64;

    for group in groups.values() {
        max_copies_per_message_id = max_copies_per_message_id.max(group.copies);
        if group.copies > 1 {
            duplicate_groups = duplicate_groups
                .checked_add(1)
                .ok_or(DlqDuplicateInspectionError::CountOverflow)?;
        }
        if group.payload_sha256.len() > 1 {
            conflicting_payload_groups = conflicting_payload_groups
                .checked_add(1)
                .ok_or(DlqDuplicateInspectionError::CountOverflow)?;
        }
    }

    Ok(DlqDuplicateSummary {
        total_messages,
        unique_message_ids,
        duplicate_messages,
        duplicate_groups,
        conflicting_payload_groups,
        max_copies_per_message_id,
    })
}

fn hash_part(hasher: &mut Sha256, bytes: &[u8]) {
    let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    hasher.update(length.to_be_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(id: u128, payload: &[u8]) -> DlqDuplicateObservation {
        DlqDuplicateObservation::from_payload(Uuid::from_u128(id), payload).unwrap()
    }

    #[test]
    fn repeated_id_and_exact_bytes_are_counted_as_physical_duplicates() {
        let summary = summarize_dlq_duplicates([
            observation(1, &[1, 2, 3]),
            observation(1, &[1, 2, 3]),
            observation(2, &[4]),
        ])
        .unwrap();

        assert_eq!(summary.total_messages(), 3);
        assert_eq!(summary.unique_message_ids(), 2);
        assert_eq!(summary.duplicate_messages(), 1);
        assert_eq!(summary.duplicate_groups(), 1);
        assert_eq!(summary.conflicting_payload_groups(), 0);
        assert_eq!(summary.max_copies_per_message_id(), 2);
        assert!(summary.has_physical_duplicates());
        assert!(!summary.has_identity_conflicts());
        assert!(!summary.requires_manual_review());
    }

    #[test]
    fn one_id_with_distinct_exact_bytes_requires_manual_review() {
        let summary =
            summarize_dlq_duplicates([observation(7, &[1, 2, 3]), observation(7, &[1, 2, 4])])
                .unwrap();

        assert_eq!(summary.total_messages(), 2);
        assert_eq!(summary.unique_message_ids(), 1);
        assert_eq!(summary.duplicate_messages(), 1);
        assert_eq!(summary.duplicate_groups(), 1);
        assert_eq!(summary.conflicting_payload_groups(), 1);
        assert!(summary.has_identity_conflicts());
        assert!(summary.requires_manual_review());
    }

    #[test]
    fn empty_scan_and_empty_payload_are_valid() {
        let empty = summarize_dlq_duplicates([]).unwrap();
        assert_eq!(empty, DlqDuplicateSummary::default());

        let one = summarize_dlq_duplicates([observation(9, &[])]).unwrap();
        assert_eq!(one.total_messages(), 1);
        assert_eq!(one.unique_message_ids(), 1);
        assert_eq!(one.duplicate_messages(), 0);
        assert_eq!(one.max_copies_per_message_id(), 1);
    }

    #[test]
    fn nil_message_id_is_rejected_with_stable_code() {
        let error = DlqDuplicateObservation::from_payload(Uuid::nil(), &[1]).unwrap_err();
        assert_eq!(error, DlqDuplicateInspectionError::InvalidBrokerMessageId);
        assert_eq!(error.stable_code(), "iggy.dlq_duplicate.identity_invalid");
    }
}
