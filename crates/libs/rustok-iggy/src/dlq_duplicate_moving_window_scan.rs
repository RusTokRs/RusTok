use std::collections::{BTreeMap, BTreeSet};

use iggy::prelude::{
    Consumer, ConsumerKind, Identifier, IggyClient, MessageClient, PollingStrategy,
};
use thiserror::Error;
use uuid::Uuid;

use crate::dlq_duplicate_inspection::{DlqDuplicateInspectionError, DlqDuplicateObservation};
use crate::dlq_duplicate_rolling_window::{
    DlqDuplicateRollingWindow, DlqDuplicateRollingWindowError, DlqDuplicateRollingWindowPolicy,
    DlqDuplicateRollingWindowSnapshot,
};

const DLQ_TOPIC: &str = "dlq";
const READ_ONLY_CONSUMER: &str = "rustok-dlq-duplicate-moving-readonly-v1";
const MAX_SCAN_MESSAGES: u32 = 10_000;
const MAX_BATCH_MESSAGES: u32 = 1_000;
const MAX_SCAN_PARTITIONS: usize = 128;
const MAX_STREAM_NAME_BYTES: usize = 255;

/// Explicit bounded policy for one moving physical-DLQ duplicate window.
///
/// Every selected partition owns an independent in-memory next offset. The
/// process starts and resets every cursor to `initial_offset`. No offset or
/// rolling observation is serialized or persisted by this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IggyDlqDuplicateMovingWindowPolicy {
    partitions: Vec<u32>,
    initial_offset: u64,
    per_partition_messages: u32,
    batch_size: u32,
    total_message_budget: u32,
    rolling_policy: DlqDuplicateRollingWindowPolicy,
}

impl IggyDlqDuplicateMovingWindowPolicy {
    pub fn new(
        partitions: Vec<u32>,
        initial_offset: u64,
        per_partition_messages: u32,
        batch_size: u32,
        rolling_policy: DlqDuplicateRollingWindowPolicy,
    ) -> Result<Self, IggyDlqDuplicateMovingWindowError> {
        validate_partitions(&partitions)?;
        validate_message_bounds(per_partition_messages, batch_size)?;

        let partition_count = u32::try_from(partitions.len())
            .map_err(|_| IggyDlqDuplicateMovingWindowError::InvalidPolicy)?;
        let total_message_budget = per_partition_messages
            .checked_mul(partition_count)
            .filter(|total| *total <= MAX_SCAN_MESSAGES)
            .ok_or(IggyDlqDuplicateMovingWindowError::InvalidPolicy)?;

        if rolling_policy.max_observations_per_cycle() < total_message_budget {
            return Err(IggyDlqDuplicateMovingWindowError::InvalidPolicy);
        }

        Ok(Self {
            partitions,
            initial_offset,
            per_partition_messages,
            batch_size,
            total_message_budget,
            rolling_policy,
        })
    }

    pub fn partitions(&self) -> &[u32] {
        &self.partitions
    }

    pub const fn initial_offset(&self) -> u64 {
        self.initial_offset
    }

    pub const fn per_partition_messages(&self) -> u32 {
        self.per_partition_messages
    }

    pub const fn batch_size(&self) -> u32 {
        self.batch_size
    }

    pub const fn total_message_budget(&self) -> u32 {
        self.total_message_budget
    }

    pub const fn rolling_policy(&self) -> DlqDuplicateRollingWindowPolicy {
        self.rolling_policy
    }

    pub const fn progress_persisted(&self) -> bool {
        false
    }

    pub const fn restart_resets_to_initial_offset(&self) -> bool {
        true
    }
}

/// Identifier-free result for one successful complete moving scan cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IggyDlqDuplicateMovingWindowSnapshot {
    rolling: DlqDuplicateRollingWindowSnapshot,
    partition_count: u32,
    advanced_partitions: u32,
    reset_generation: u64,
}

impl IggyDlqDuplicateMovingWindowSnapshot {
    pub const fn rolling(&self) -> &DlqDuplicateRollingWindowSnapshot {
        &self.rolling
    }

    pub const fn partition_count(&self) -> u32 {
        self.partition_count
    }

    pub const fn advanced_partitions(&self) -> u32 {
        self.advanced_partitions
    }

    pub const fn reset_generation(&self) -> u64 {
        self.reset_generation
    }

    pub const fn progress_persisted(&self) -> bool {
        false
    }

    pub const fn restart_resets_to_initial_offset(&self) -> bool {
        true
    }
}

/// Process-local moving scan state.
///
/// Cursors and opaque rolling observations are intentionally private. A state
/// instance advances only after every selected partition has completed one
/// bounded explicit-offset poll and the combined rolling candidate has been
/// accepted. Any scan, validation, or rolling-window failure leaves both cursor
/// and rolling state unchanged.
pub struct IggyDlqDuplicateMovingWindowState {
    policy: IggyDlqDuplicateMovingWindowPolicy,
    cursors: BTreeMap<u32, u64>,
    rolling: DlqDuplicateRollingWindow,
    reset_generation: u64,
}

impl IggyDlqDuplicateMovingWindowState {
    pub fn new(policy: IggyDlqDuplicateMovingWindowPolicy) -> Self {
        let cursors = initial_cursors(&policy);
        let rolling = DlqDuplicateRollingWindow::new(policy.rolling_policy());
        Self {
            policy,
            cursors,
            rolling,
            reset_generation: 0,
        }
    }

    pub fn policy(&self) -> &IggyDlqDuplicateMovingWindowPolicy {
        &self.policy
    }

    pub fn snapshot(
        &self,
    ) -> Result<IggyDlqDuplicateMovingWindowSnapshot, IggyDlqDuplicateMovingWindowError> {
        let partition_count = u32::try_from(self.policy.partitions.len())
            .map_err(|_| IggyDlqDuplicateMovingWindowError::CountOverflow)?;
        Ok(IggyDlqDuplicateMovingWindowSnapshot {
            rolling: self.rolling.snapshot()?,
            partition_count,
            advanced_partitions: 0,
            reset_generation: self.reset_generation,
        })
    }

    /// Explicitly discards process-local cursor and rolling state.
    ///
    /// This is also the documented process-restart behavior: a reconstructed
    /// state starts again from the reviewed `initial_offset`. The reset is
    /// transactional if the reset-generation counter is exhausted.
    pub fn reset_to_initial_offset(
        &mut self,
    ) -> Result<IggyDlqDuplicateMovingWindowSnapshot, IggyDlqDuplicateMovingWindowError> {
        let reset_generation = self
            .reset_generation
            .checked_add(1)
            .ok_or(IggyDlqDuplicateMovingWindowError::ResetGenerationOverflow)?;
        let cursors = initial_cursors(&self.policy);
        let rolling = DlqDuplicateRollingWindow::new(self.policy.rolling_policy());
        let partition_count = u32::try_from(self.policy.partitions.len())
            .map_err(|_| IggyDlqDuplicateMovingWindowError::CountOverflow)?;
        let snapshot = IggyDlqDuplicateMovingWindowSnapshot {
            rolling: rolling.snapshot()?,
            partition_count,
            advanced_partitions: 0,
            reset_generation,
        };

        self.cursors = cursors;
        self.rolling = rolling;
        self.reset_generation = reset_generation;
        Ok(snapshot)
    }

    fn next_offset(&self, partition_id: u32) -> Result<u64, IggyDlqDuplicateMovingWindowError> {
        self.cursors
            .get(&partition_id)
            .copied()
            .ok_or(IggyDlqDuplicateMovingWindowError::InvalidCycle)
    }

    fn apply_complete_cycle(
        &mut self,
        partitions: Vec<CollectedPartitionCycle>,
    ) -> Result<IggyDlqDuplicateMovingWindowSnapshot, IggyDlqDuplicateMovingWindowError> {
        if partitions.len() != self.policy.partitions.len() {
            return Err(IggyDlqDuplicateMovingWindowError::InvalidCycle);
        }

        let mut candidate_cursors = self.cursors.clone();
        let mut observations = Vec::with_capacity(self.policy.total_message_budget as usize);
        let mut advanced_partitions = 0_u32;

        for (&expected_partition, partition) in self.policy.partitions.iter().zip(partitions) {
            let expected_offset = self.next_offset(expected_partition)?;
            if partition.partition_id != expected_partition
                || partition.start_offset != expected_offset
                || partition.next_offset < partition.start_offset
                || partition.observations.len() > self.policy.per_partition_messages as usize
            {
                return Err(IggyDlqDuplicateMovingWindowError::InvalidCycle);
            }

            if partition.next_offset > partition.start_offset {
                advanced_partitions = advanced_partitions
                    .checked_add(1)
                    .ok_or(IggyDlqDuplicateMovingWindowError::CountOverflow)?;
            }
            candidate_cursors.insert(expected_partition, partition.next_offset);
            observations.extend(partition.observations);
        }

        if observations.len() > self.policy.total_message_budget as usize {
            return Err(IggyDlqDuplicateMovingWindowError::InvalidCycle);
        }

        let rolling = self.rolling.push_cycle(observations)?;
        self.cursors = candidate_cursors;

        let partition_count = u32::try_from(self.policy.partitions.len())
            .map_err(|_| IggyDlqDuplicateMovingWindowError::CountOverflow)?;
        Ok(IggyDlqDuplicateMovingWindowSnapshot {
            rolling,
            partition_count,
            advanced_partitions,
            reset_generation: self.reset_generation,
        })
    }
}

/// Read-only external-Iggy collector for process-local moving windows.
///
/// The scanner uses a standalone consumer, explicit partition offsets, and
/// `auto_commit = false`. It never stores consumer progress. A complete cycle is
/// handed to [`IggyDlqDuplicateMovingWindowState`] only after every configured
/// partition has been polled successfully.
pub struct IggyDlqDuplicateMovingWindowScanner<'a> {
    client: &'a IggyClient,
    stream_id: Identifier,
    topic_id: Identifier,
    consumer: Consumer,
}

impl<'a> IggyDlqDuplicateMovingWindowScanner<'a> {
    pub fn new(
        client: &'a IggyClient,
        stream_name: &str,
    ) -> Result<Self, IggyDlqDuplicateMovingWindowError> {
        validate_stream_name(stream_name)?;
        let stream_id: Identifier = stream_name
            .to_owned()
            .try_into()
            .map_err(|_| IggyDlqDuplicateMovingWindowError::InvalidPolicy)?;
        let topic_id: Identifier = DLQ_TOPIC
            .to_owned()
            .try_into()
            .map_err(|_| IggyDlqDuplicateMovingWindowError::InvalidPolicy)?;
        let consumer_id: Identifier = READ_ONLY_CONSUMER
            .to_owned()
            .try_into()
            .map_err(|_| IggyDlqDuplicateMovingWindowError::InvalidPolicy)?;

        Ok(Self {
            client,
            stream_id,
            topic_id,
            consumer: Consumer {
                kind: ConsumerKind::Consumer,
                id: consumer_id,
            },
        })
    }

    pub async fn scan_cycle(
        &self,
        state: &mut IggyDlqDuplicateMovingWindowState,
    ) -> Result<IggyDlqDuplicateMovingWindowSnapshot, IggyDlqDuplicateMovingWindowError> {
        let policy = state.policy().clone();
        let mut partitions = Vec::with_capacity(policy.partitions.len());

        for &partition_id in &policy.partitions {
            let start_offset = state.next_offset(partition_id)?;
            partitions.push(
                self.collect_partition(
                    partition_id,
                    start_offset,
                    policy.per_partition_messages,
                    policy.batch_size,
                )
                .await?,
            );
        }

        state.apply_complete_cycle(partitions)
    }

    async fn collect_partition(
        &self,
        partition_id: u32,
        start_offset: u64,
        max_messages: u32,
        batch_size: u32,
    ) -> Result<CollectedPartitionCycle, IggyDlqDuplicateMovingWindowError> {
        let mut remaining = max_messages;
        let mut next_offset = start_offset;
        let mut observations = Vec::with_capacity(max_messages as usize);

        loop {
            let requested_count = batch_size.min(remaining);
            let polled = self
                .client
                .poll_messages(
                    &self.stream_id,
                    &self.topic_id,
                    Some(partition_id),
                    &self.consumer,
                    &PollingStrategy::offset(next_offset),
                    requested_count,
                    false,
                )
                .await
                .map_err(|_| IggyDlqDuplicateMovingWindowError::PollFailed)?;

            if polled.partition_id != partition_id
                || polled.count as usize != polled.messages.len()
                || polled.count > requested_count
            {
                return Err(IggyDlqDuplicateMovingWindowError::InvalidBrokerResponse);
            }
            if polled.messages.is_empty() {
                break;
            }

            let received_count = polled.count;
            let mut previous_offset = None;
            for message in polled.messages {
                let offset = message.header.offset;
                if offset < next_offset
                    || previous_offset.is_some_and(|previous| offset <= previous)
                {
                    return Err(IggyDlqDuplicateMovingWindowError::InvalidBrokerResponse);
                }
                observations.push(DlqDuplicateObservation::from_payload(
                    Uuid::from_u128(message.header.id),
                    message.payload.as_ref(),
                )?);
                remaining = remaining
                    .checked_sub(1)
                    .ok_or(IggyDlqDuplicateMovingWindowError::InvalidBrokerResponse)?;
                previous_offset = Some(offset);
            }

            let last_offset =
                previous_offset.ok_or(IggyDlqDuplicateMovingWindowError::InvalidBrokerResponse)?;
            next_offset = last_offset
                .checked_add(1)
                .ok_or(IggyDlqDuplicateMovingWindowError::OffsetOverflow)?;

            if remaining == 0 || received_count < requested_count {
                break;
            }
        }

        Ok(CollectedPartitionCycle {
            partition_id,
            start_offset,
            next_offset,
            observations,
        })
    }
}

struct CollectedPartitionCycle {
    partition_id: u32,
    start_offset: u64,
    next_offset: u64,
    observations: Vec<DlqDuplicateObservation>,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum IggyDlqDuplicateMovingWindowError {
    #[error("external Iggy DLQ moving-window policy is invalid")]
    InvalidPolicy,
    #[error("external Iggy DLQ moving-window polling failed")]
    PollFailed,
    #[error("external Iggy DLQ moving-window poll response is invalid")]
    InvalidBrokerResponse,
    #[error("external Iggy DLQ moving-window offset overflow")]
    OffsetOverflow,
    #[error("external Iggy DLQ moving-window complete cycle is invalid")]
    InvalidCycle,
    #[error("external Iggy DLQ moving-window count overflow")]
    CountOverflow,
    #[error("external Iggy DLQ moving-window reset generation overflow")]
    ResetGenerationOverflow,
    #[error(transparent)]
    Inspection(#[from] DlqDuplicateInspectionError),
    #[error(transparent)]
    Rolling(#[from] DlqDuplicateRollingWindowError),
}

impl IggyDlqDuplicateMovingWindowError {
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::InvalidPolicy => "iggy.dlq_duplicate.moving_window_policy_invalid",
            Self::PollFailed => "iggy.dlq_duplicate.moving_window_poll_failed",
            Self::InvalidBrokerResponse => "iggy.dlq_duplicate.moving_window_response_invalid",
            Self::OffsetOverflow => "iggy.dlq_duplicate.moving_window_offset_overflow",
            Self::InvalidCycle => "iggy.dlq_duplicate.moving_window_cycle_invalid",
            Self::CountOverflow => "iggy.dlq_duplicate.moving_window_count_overflow",
            Self::ResetGenerationOverflow => "iggy.dlq_duplicate.moving_window_reset_overflow",
            Self::Inspection(error) => error.stable_code(),
            Self::Rolling(error) => error.stable_code(),
        }
    }
}

fn initial_cursors(policy: &IggyDlqDuplicateMovingWindowPolicy) -> BTreeMap<u32, u64> {
    policy
        .partitions
        .iter()
        .map(|partition| (*partition, policy.initial_offset))
        .collect()
}

fn validate_partitions(partitions: &[u32]) -> Result<(), IggyDlqDuplicateMovingWindowError> {
    if partitions.is_empty() || partitions.len() > MAX_SCAN_PARTITIONS {
        return Err(IggyDlqDuplicateMovingWindowError::InvalidPolicy);
    }
    let mut unique = BTreeSet::new();
    for &partition in partitions {
        if partition == 0 || !unique.insert(partition) {
            return Err(IggyDlqDuplicateMovingWindowError::InvalidPolicy);
        }
    }
    Ok(())
}

fn validate_message_bounds(
    max_messages: u32,
    batch_size: u32,
) -> Result<(), IggyDlqDuplicateMovingWindowError> {
    if max_messages == 0
        || max_messages > MAX_SCAN_MESSAGES
        || batch_size == 0
        || batch_size > MAX_BATCH_MESSAGES
        || batch_size > max_messages
    {
        return Err(IggyDlqDuplicateMovingWindowError::InvalidPolicy);
    }
    Ok(())
}

fn validate_stream_name(stream_name: &str) -> Result<(), IggyDlqDuplicateMovingWindowError> {
    if stream_name.is_empty()
        || stream_name.trim() != stream_name
        || stream_name.len() > MAX_STREAM_NAME_BYTES
        || stream_name.chars().any(char::is_control)
    {
        return Err(IggyDlqDuplicateMovingWindowError::InvalidPolicy);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn observation(id: u128, payload: &[u8]) -> DlqDuplicateObservation {
        DlqDuplicateObservation::from_payload(Uuid::from_u128(id), payload).unwrap()
    }

    fn policy() -> IggyDlqDuplicateMovingWindowPolicy {
        let rolling = DlqDuplicateRollingWindowPolicy::new(3, 4).unwrap();
        IggyDlqDuplicateMovingWindowPolicy::new(vec![1, 2], 10, 2, 1, rolling).unwrap()
    }

    fn partition(
        partition_id: u32,
        start_offset: u64,
        next_offset: u64,
        observations: Vec<DlqDuplicateObservation>,
    ) -> CollectedPartitionCycle {
        CollectedPartitionCycle {
            partition_id,
            start_offset,
            next_offset,
            observations,
        }
    }

    #[test]
    fn policy_requires_complete_fair_cycle_to_fit_rolling_capacity() {
        let rolling = DlqDuplicateRollingWindowPolicy::new(2, 3).unwrap();
        for result in [
            IggyDlqDuplicateMovingWindowPolicy::new(vec![], 0, 1, 1, rolling),
            IggyDlqDuplicateMovingWindowPolicy::new(vec![0], 0, 1, 1, rolling),
            IggyDlqDuplicateMovingWindowPolicy::new(vec![1, 1], 0, 1, 1, rolling),
            IggyDlqDuplicateMovingWindowPolicy::new(vec![1, 2], 0, 2, 1, rolling),
        ] {
            assert_eq!(
                result.unwrap_err(),
                IggyDlqDuplicateMovingWindowError::InvalidPolicy
            );
        }
    }

    #[test]
    fn complete_cycle_advances_partition_cursors_independently() {
        let mut state = IggyDlqDuplicateMovingWindowState::new(policy());
        let snapshot = state
            .apply_complete_cycle(vec![
                partition(1, 10, 12, vec![observation(1, &[1])]),
                partition(2, 10, 10, vec![]),
            ])
            .unwrap();

        assert_eq!(snapshot.partition_count(), 2);
        assert_eq!(snapshot.advanced_partitions(), 1);
        assert_eq!(state.cursors.get(&1), Some(&12));
        assert_eq!(state.cursors.get(&2), Some(&10));
        assert!(!snapshot.progress_persisted());
        assert!(snapshot.restart_resets_to_initial_offset());
    }

    #[test]
    fn duplicate_split_across_advancing_cycles_remains_count_only() {
        let mut state = IggyDlqDuplicateMovingWindowState::new(policy());
        state
            .apply_complete_cycle(vec![
                partition(1, 10, 11, vec![observation(7, &[1])]),
                partition(2, 10, 10, vec![]),
            ])
            .unwrap();

        let snapshot = state
            .apply_complete_cycle(vec![
                partition(1, 11, 12, vec![observation(7, &[1])]),
                partition(2, 10, 10, vec![]),
            ])
            .unwrap();

        assert_eq!(snapshot.rolling().summary().duplicate_messages(), 1);
        assert_eq!(snapshot.rolling().summary().duplicate_groups(), 1);
        assert!(!snapshot.rolling().summary().requires_manual_review());
    }

    #[test]
    fn incomplete_cycle_preserves_cursors_and_rolling_state() {
        let mut state = IggyDlqDuplicateMovingWindowState::new(policy());
        state
            .apply_complete_cycle(vec![
                partition(1, 10, 11, vec![observation(1, &[1])]),
                partition(2, 10, 10, vec![]),
            ])
            .unwrap();
        let before = state.snapshot().unwrap();
        let cursors = state.cursors.clone();

        let error = state
            .apply_complete_cycle(vec![partition(1, 11, 12, vec![])])
            .unwrap_err();

        assert_eq!(error, IggyDlqDuplicateMovingWindowError::InvalidCycle);
        assert_eq!(state.snapshot().unwrap(), before);
        assert_eq!(state.cursors, cursors);
    }

    #[test]
    fn explicit_reset_rewinds_cursors_and_clears_rolling_history() {
        let mut state = IggyDlqDuplicateMovingWindowState::new(policy());
        state
            .apply_complete_cycle(vec![
                partition(1, 10, 11, vec![observation(1, &[1])]),
                partition(2, 10, 11, vec![observation(2, &[2])]),
            ])
            .unwrap();

        let reset = state.reset_to_initial_offset().unwrap();

        assert_eq!(reset.reset_generation(), 1);
        assert_eq!(reset.rolling().retained_cycles(), 0);
        assert_eq!(reset.rolling().retained_observations(), 0);
        assert_eq!(state.cursors.get(&1), Some(&10));
        assert_eq!(state.cursors.get(&2), Some(&10));
        assert!(!reset.progress_persisted());
        assert!(reset.restart_resets_to_initial_offset());
    }
}
