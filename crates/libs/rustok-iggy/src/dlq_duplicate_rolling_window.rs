use std::collections::VecDeque;

use thiserror::Error;

use crate::dlq_duplicate_inspection::{
    DlqDuplicateInspectionError, DlqDuplicateObservation, DlqDuplicateSummary,
    summarize_dlq_duplicates,
};

const MAX_ROLLING_WINDOW_CYCLES: u32 = 128;
const MAX_ROLLING_WINDOW_OBSERVATIONS: u32 = 10_000;

/// Explicit memory bounds for retaining complete physical-DLQ scan cycles.
///
/// The checked product of `max_cycles` and `max_observations_per_cycle` must
/// not exceed 10,000 observations. The policy defines no scan cadence,
/// partition cursor, persistence format, or production default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DlqDuplicateRollingWindowPolicy {
    max_cycles: u32,
    max_observations_per_cycle: u32,
    total_observation_capacity: u32,
}

impl DlqDuplicateRollingWindowPolicy {
    pub fn new(
        max_cycles: u32,
        max_observations_per_cycle: u32,
    ) -> Result<Self, DlqDuplicateRollingWindowError> {
        if max_cycles == 0
            || max_cycles > MAX_ROLLING_WINDOW_CYCLES
            || max_observations_per_cycle == 0
        {
            return Err(DlqDuplicateRollingWindowError::InvalidPolicy);
        }

        let total_observation_capacity = max_cycles
            .checked_mul(max_observations_per_cycle)
            .filter(|total| *total <= MAX_ROLLING_WINDOW_OBSERVATIONS)
            .ok_or(DlqDuplicateRollingWindowError::InvalidPolicy)?;

        Ok(Self {
            max_cycles,
            max_observations_per_cycle,
            total_observation_capacity,
        })
    }

    pub const fn max_cycles(&self) -> u32 {
        self.max_cycles
    }

    pub const fn max_observations_per_cycle(&self) -> u32 {
        self.max_observations_per_cycle
    }

    pub const fn total_observation_capacity(&self) -> u32 {
        self.total_observation_capacity
    }
}

/// Identifier-free view of one bounded rolling duplicate window.
///
/// `history_truncated` becomes true after any complete retained cycle is
/// evicted. A truncated snapshot remains useful for the currently retained
/// window but must not be presented as complete history or current-tail proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DlqDuplicateRollingWindowSnapshot {
    summary: DlqDuplicateSummary,
    retained_cycles: u32,
    retained_observations: u32,
    evicted_cycles: u64,
    history_truncated: bool,
}

impl DlqDuplicateRollingWindowSnapshot {
    pub const fn summary(&self) -> &DlqDuplicateSummary {
        &self.summary
    }

    pub const fn retained_cycles(&self) -> u32 {
        self.retained_cycles
    }

    pub const fn retained_observations(&self) -> u32 {
        self.retained_observations
    }

    pub const fn evicted_cycles(&self) -> u64 {
        self.evicted_cycles
    }

    pub const fn history_truncated(&self) -> bool {
        self.history_truncated
    }
}

/// Bounded in-memory state that preserves duplicate identity relationships
/// across complete scan cycles while they remain inside the configured window.
///
/// The state exposes no observation, UUID, payload digest, partition, offset,
/// endpoint, credential, or receipt fact. It does not move or persist broker
/// cursors. When capacity is reached, the oldest complete cycle is evicted and
/// every later snapshot reports `history_truncated = true`.
pub struct DlqDuplicateRollingWindow {
    policy: DlqDuplicateRollingWindowPolicy,
    cycles: VecDeque<Vec<DlqDuplicateObservation>>,
    retained_observations: u32,
    evicted_cycles: u64,
}

impl DlqDuplicateRollingWindow {
    pub fn new(policy: DlqDuplicateRollingWindowPolicy) -> Self {
        Self {
            policy,
            cycles: VecDeque::new(),
            retained_observations: 0,
            evicted_cycles: 0,
        }
    }

    pub const fn policy(&self) -> DlqDuplicateRollingWindowPolicy {
        self.policy
    }

    /// Adds one complete scan cycle transactionally.
    ///
    /// Empty successful cycles are retained and may eventually evict older
    /// cycles. An oversized cycle or arithmetic/classification error leaves the
    /// existing state unchanged.
    pub fn push_cycle(
        &mut self,
        observations: impl IntoIterator<Item = DlqDuplicateObservation>,
    ) -> Result<DlqDuplicateRollingWindowSnapshot, DlqDuplicateRollingWindowError> {
        let maximum = self.policy.max_observations_per_cycle as usize;
        let mut incoming = Vec::new();
        for observation in observations {
            if incoming.len() >= maximum {
                return Err(DlqDuplicateRollingWindowError::CycleTooLarge);
            }
            incoming.push(observation);
        }

        let incoming_count = u32::try_from(incoming.len())
            .map_err(|_| DlqDuplicateRollingWindowError::CountOverflow)?;
        let mut candidate_cycles = self.cycles.clone();
        let mut candidate_retained = self.retained_observations;
        let mut candidate_evicted = self.evicted_cycles;

        if candidate_cycles.len() == self.policy.max_cycles as usize {
            let evicted = candidate_cycles
                .pop_front()
                .ok_or(DlqDuplicateRollingWindowError::CountOverflow)?;
            let evicted_count = u32::try_from(evicted.len())
                .map_err(|_| DlqDuplicateRollingWindowError::CountOverflow)?;
            candidate_retained = candidate_retained
                .checked_sub(evicted_count)
                .ok_or(DlqDuplicateRollingWindowError::CountOverflow)?;
            candidate_evicted = candidate_evicted
                .checked_add(1)
                .ok_or(DlqDuplicateRollingWindowError::CountOverflow)?;
        }

        candidate_retained = candidate_retained
            .checked_add(incoming_count)
            .filter(|count| *count <= self.policy.total_observation_capacity)
            .ok_or(DlqDuplicateRollingWindowError::CountOverflow)?;
        candidate_cycles.push_back(incoming);

        let snapshot = summarize_window(&candidate_cycles, candidate_retained, candidate_evicted)?;

        self.cycles = candidate_cycles;
        self.retained_observations = candidate_retained;
        self.evicted_cycles = candidate_evicted;
        Ok(snapshot)
    }

    pub fn snapshot(
        &self,
    ) -> Result<DlqDuplicateRollingWindowSnapshot, DlqDuplicateRollingWindowError> {
        summarize_window(
            &self.cycles,
            self.retained_observations,
            self.evicted_cycles,
        )
    }
}

fn summarize_window(
    cycles: &VecDeque<Vec<DlqDuplicateObservation>>,
    retained_observations: u32,
    evicted_cycles: u64,
) -> Result<DlqDuplicateRollingWindowSnapshot, DlqDuplicateRollingWindowError> {
    let retained_cycles =
        u32::try_from(cycles.len()).map_err(|_| DlqDuplicateRollingWindowError::CountOverflow)?;
    let summary = summarize_dlq_duplicates(cycles.iter().flat_map(|cycle| cycle.iter().cloned()))?;

    Ok(DlqDuplicateRollingWindowSnapshot {
        summary,
        retained_cycles,
        retained_observations,
        evicted_cycles,
        history_truncated: evicted_cycles > 0,
    })
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum DlqDuplicateRollingWindowError {
    #[error("physical DLQ duplicate rolling-window policy is invalid")]
    InvalidPolicy,
    #[error("physical DLQ duplicate scan cycle exceeds its configured bound")]
    CycleTooLarge,
    #[error("physical DLQ duplicate rolling-window count overflow")]
    CountOverflow,
    #[error(transparent)]
    Inspection(#[from] DlqDuplicateInspectionError),
}

impl DlqDuplicateRollingWindowError {
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::InvalidPolicy => "iggy.dlq_duplicate.rolling_window_policy_invalid",
            Self::CycleTooLarge => "iggy.dlq_duplicate.rolling_window_cycle_too_large",
            Self::CountOverflow => "iggy.dlq_duplicate.rolling_window_count_overflow",
            Self::Inspection(error) => error.stable_code(),
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn observation(id: u128, payload: &[u8]) -> DlqDuplicateObservation {
        DlqDuplicateObservation::from_payload(Uuid::from_u128(id), payload).unwrap()
    }

    #[test]
    fn invalid_policy_and_capacity_overflow_fail_closed() {
        for candidate in [
            DlqDuplicateRollingWindowPolicy::new(0, 1),
            DlqDuplicateRollingWindowPolicy::new(1, 0),
            DlqDuplicateRollingWindowPolicy::new(MAX_ROLLING_WINDOW_CYCLES + 1, 1),
            DlqDuplicateRollingWindowPolicy::new(128, 79),
        ] {
            let error = candidate.unwrap_err();
            assert_eq!(error, DlqDuplicateRollingWindowError::InvalidPolicy);
            assert_eq!(
                error.stable_code(),
                "iggy.dlq_duplicate.rolling_window_policy_invalid"
            );
        }
    }

    #[test]
    fn ordinary_duplicate_split_across_cycles_is_detected() {
        let policy = DlqDuplicateRollingWindowPolicy::new(2, 2).unwrap();
        let mut window = DlqDuplicateRollingWindow::new(policy);

        let first = window.push_cycle([observation(1, &[1])]).unwrap();
        assert!(!first.summary().has_physical_duplicates());

        let second = window.push_cycle([observation(1, &[1])]).unwrap();
        assert_eq!(second.retained_cycles(), 2);
        assert_eq!(second.retained_observations(), 2);
        assert_eq!(second.summary().duplicate_messages(), 1);
        assert_eq!(second.summary().duplicate_groups(), 1);
        assert!(!second.history_truncated());
    }

    #[test]
    fn identity_conflict_split_across_cycles_requires_manual_review() {
        let policy = DlqDuplicateRollingWindowPolicy::new(2, 1).unwrap();
        let mut window = DlqDuplicateRollingWindow::new(policy);

        window.push_cycle([observation(7, &[1])]).unwrap();
        let snapshot = window.push_cycle([observation(7, &[2])]).unwrap();

        assert_eq!(snapshot.summary().conflicting_payload_groups(), 1);
        assert!(snapshot.summary().has_identity_conflicts());
        assert!(snapshot.summary().requires_manual_review());
    }

    #[test]
    fn oldest_complete_cycle_eviction_marks_history_truncated() {
        let policy = DlqDuplicateRollingWindowPolicy::new(2, 1).unwrap();
        let mut window = DlqDuplicateRollingWindow::new(policy);

        window.push_cycle([observation(1, &[1])]).unwrap();
        let duplicate = window.push_cycle([observation(1, &[1])]).unwrap();
        assert_eq!(duplicate.summary().duplicate_messages(), 1);

        let truncated = window.push_cycle([observation(2, &[2])]).unwrap();
        assert_eq!(truncated.evicted_cycles(), 1);
        assert!(truncated.history_truncated());
        assert_eq!(truncated.retained_cycles(), 2);
        assert_eq!(truncated.summary().duplicate_messages(), 0);
    }

    #[test]
    fn oversized_cycle_rejection_preserves_existing_state() {
        let policy = DlqDuplicateRollingWindowPolicy::new(2, 1).unwrap();
        let mut window = DlqDuplicateRollingWindow::new(policy);
        window.push_cycle([observation(1, &[1])]).unwrap();
        let before = window.snapshot().unwrap();

        let error = window
            .push_cycle([observation(2, &[2]), observation(3, &[3])])
            .unwrap_err();
        assert_eq!(error, DlqDuplicateRollingWindowError::CycleTooLarge);
        assert_eq!(
            error.stable_code(),
            "iggy.dlq_duplicate.rolling_window_cycle_too_large"
        );
        assert_eq!(window.snapshot().unwrap(), before);
    }
}
