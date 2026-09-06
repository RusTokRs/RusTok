use thiserror::Error;
use tokio::sync::watch;

use crate::dlq_duplicate_alert_policy::{DlqDuplicateAlertEvaluation, DlqDuplicateAlertPolicy};
use crate::dlq_duplicate_inspection::DlqDuplicateSummary;

/// Identifier-free latest-value snapshot for duplicate alert telemetry and health consumers.
///
/// The snapshot contains no source counts, thresholds, broker coordinates, message identity,
/// payload facts, credentials, timestamps, notification routing, or destructive action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DlqDuplicateAlertRuntimeSnapshot {
    generation: u64,
    available: bool,
    evaluation: Option<DlqDuplicateAlertEvaluation>,
}

impl DlqDuplicateAlertRuntimeSnapshot {
    const fn unavailable(generation: u64) -> Self {
        Self {
            generation,
            available: false,
            evaluation: None,
        }
    }

    const fn available(generation: u64, evaluation: DlqDuplicateAlertEvaluation) -> Self {
        Self {
            generation,
            available: true,
            evaluation: Some(evaluation),
        }
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn is_available(&self) -> bool {
        self.available
    }

    pub const fn evaluation(&self) -> Option<DlqDuplicateAlertEvaluation> {
        self.evaluation
    }
}

/// Single-writer publisher side of the in-memory duplicate alert runtime composition.
///
/// It does not scan Iggy, read poison receipts, persist state, dispatch notifications, choose
/// cooldown or suppression, affect readiness, or mutate broker/receipt/Profile state.
pub struct DlqDuplicateAlertRuntimePublisher {
    policy: DlqDuplicateAlertPolicy,
    generation: u64,
    sender: watch::Sender<DlqDuplicateAlertRuntimeSnapshot>,
}

impl DlqDuplicateAlertRuntimePublisher {
    pub fn new(policy: DlqDuplicateAlertPolicy) -> (Self, DlqDuplicateAlertRuntimeSubscriber) {
        let (sender, receiver) = watch::channel(DlqDuplicateAlertRuntimeSnapshot::unavailable(0));
        (
            Self {
                policy,
                generation: 0,
                sender,
            },
            DlqDuplicateAlertRuntimeSubscriber { receiver },
        )
    }

    pub fn subscribe(&self) -> DlqDuplicateAlertRuntimeSubscriber {
        DlqDuplicateAlertRuntimeSubscriber {
            receiver: self.sender.subscribe(),
        }
    }

    /// Evaluates one already-observed count-only summary and replaces the latest snapshot.
    pub fn publish(
        &mut self,
        summary: &DlqDuplicateSummary,
    ) -> Result<DlqDuplicateAlertRuntimeSnapshot, DlqDuplicateAlertRuntimeError> {
        let generation = self.advance_generation()?;
        let snapshot =
            DlqDuplicateAlertRuntimeSnapshot::available(generation, self.policy.evaluate(summary));
        self.sender.send_replace(snapshot);
        Ok(snapshot)
    }

    /// Marks observation unavailable and clears the prior evaluation so stale severity is not
    /// presented as current state.
    pub fn mark_unavailable(
        &mut self,
    ) -> Result<DlqDuplicateAlertRuntimeSnapshot, DlqDuplicateAlertRuntimeError> {
        let snapshot = DlqDuplicateAlertRuntimeSnapshot::unavailable(self.advance_generation()?);
        self.sender.send_replace(snapshot);
        Ok(snapshot)
    }

    fn advance_generation(&mut self) -> Result<u64, DlqDuplicateAlertRuntimeError> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(DlqDuplicateAlertRuntimeError::GenerationOverflow)?;
        Ok(self.generation)
    }
}

/// Read-only subscriber for telemetry and health composition.
pub struct DlqDuplicateAlertRuntimeSubscriber {
    receiver: watch::Receiver<DlqDuplicateAlertRuntimeSnapshot>,
}

impl DlqDuplicateAlertRuntimeSubscriber {
    pub fn current(&self) -> DlqDuplicateAlertRuntimeSnapshot {
        *self.receiver.borrow()
    }

    pub async fn changed(
        &mut self,
    ) -> Result<DlqDuplicateAlertRuntimeSnapshot, DlqDuplicateAlertRuntimeError> {
        self.receiver
            .changed()
            .await
            .map_err(|_| DlqDuplicateAlertRuntimeError::PublisherClosed)?;
        Ok(self.current())
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum DlqDuplicateAlertRuntimeError {
    #[error("physical DLQ duplicate alert runtime generation overflow")]
    GenerationOverflow,
    #[error("physical DLQ duplicate alert runtime publisher closed")]
    PublisherClosed,
}

impl DlqDuplicateAlertRuntimeError {
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::GenerationOverflow => "iggy.dlq_duplicate.alert_runtime_generation_overflow",
            Self::PublisherClosed => "iggy.dlq_duplicate.alert_runtime_publisher_closed",
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::{DlqDuplicateObservation, summarize_dlq_duplicates};

    use super::*;

    fn policy() -> DlqDuplicateAlertPolicy {
        DlqDuplicateAlertPolicy::new(2, 4, 2, 3, 3, 5).unwrap()
    }

    fn summary(entries: &[(u128, &[u8])]) -> DlqDuplicateSummary {
        summarize_dlq_duplicates(entries.iter().map(|(id, payload)| {
            DlqDuplicateObservation::from_payload(Uuid::from_u128(*id), payload).unwrap()
        }))
        .unwrap()
    }

    #[test]
    fn initial_snapshot_is_unavailable_without_evaluation() {
        let (_publisher, subscriber) = DlqDuplicateAlertRuntimePublisher::new(policy());
        let snapshot = subscriber.current();
        assert_eq!(snapshot.generation(), 0);
        assert!(!snapshot.is_available());
        assert_eq!(snapshot.evaluation(), None);
    }

    #[tokio::test]
    async fn publish_replaces_latest_identifier_free_evaluation() {
        let (mut publisher, mut subscriber) = DlqDuplicateAlertRuntimePublisher::new(policy());
        let published = publisher
            .publish(&summary(&[(1, &[1]), (1, &[1]), (2, &[2]), (2, &[2])]))
            .unwrap();
        assert_eq!(published.generation(), 1);
        assert!(published.is_available());
        assert_eq!(
            published.evaluation().unwrap().level(),
            crate::DlqDuplicateAlertLevel::Warning
        );

        let observed = subscriber.changed().await.unwrap();
        assert_eq!(observed, published);
    }

    #[test]
    fn unavailable_transition_clears_stale_evaluation() {
        let (mut publisher, subscriber) = DlqDuplicateAlertRuntimePublisher::new(policy());
        publisher
            .publish(&summary(&[(1, &[1]), (1, &[1])]))
            .unwrap();
        let unavailable = publisher.mark_unavailable().unwrap();
        assert_eq!(unavailable.generation(), 2);
        assert!(!unavailable.is_available());
        assert_eq!(unavailable.evaluation(), None);
        assert_eq!(subscriber.current(), unavailable);
    }

    #[test]
    fn independent_subscribers_receive_the_same_latest_snapshot() {
        let (mut publisher, first) = DlqDuplicateAlertRuntimePublisher::new(policy());
        let second = publisher.subscribe();
        let published = publisher
            .publish(&summary(&[(7, &[1]), (7, &[2])]))
            .unwrap();
        assert_eq!(first.current(), published);
        assert_eq!(second.current(), published);
        assert!(published.evaluation().unwrap().requires_manual_review());
    }

    #[tokio::test]
    async fn closed_publisher_has_a_stable_identifier_free_error() {
        let (publisher, mut subscriber) = DlqDuplicateAlertRuntimePublisher::new(policy());
        drop(publisher);
        let error = subscriber.changed().await.unwrap_err();
        assert_eq!(error, DlqDuplicateAlertRuntimeError::PublisherClosed);
        assert_eq!(
            error.stable_code(),
            "iggy.dlq_duplicate.alert_runtime_publisher_closed"
        );
    }
}
