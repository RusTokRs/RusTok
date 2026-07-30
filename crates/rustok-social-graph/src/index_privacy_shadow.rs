use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use rustok_index::SharedIndexQueryRuntime;

use crate::{
    IndexSocialGraphPrivacyReadPort, SocialGraphFollowBatchRequest, SocialGraphFollowBatchResult,
    SocialGraphPairRequest, SocialGraphPrivacyReadPort,
};

pub const SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_TARGET: &str =
    "rustok_social_graph::index_privacy_shadow";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexPrivacyShadowOperation {
    BlocksBetween,
    SourceMutesTarget,
    SourceFollowsTarget,
    SourceFollowsTargets,
}

impl IndexPrivacyShadowOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BlocksBetween => "blocks_between",
            Self::SourceMutesTarget => "source_mutes_target",
            Self::SourceFollowsTarget => "source_follows_target",
            Self::SourceFollowsTargets => "source_follows_targets",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexPrivacyShadowOutcome {
    MatchPositive,
    MatchNegative,
    FalseNegative,
    FalsePositive,
    MatchBatchEmpty,
    MatchBatchNonempty,
    BatchMissing,
    BatchExtra,
    BatchMixed,
    Error,
}

impl IndexPrivacyShadowOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MatchPositive => "match_positive",
            Self::MatchNegative => "match_negative",
            Self::FalseNegative => "false_negative",
            Self::FalsePositive => "false_positive",
            Self::MatchBatchEmpty => "match_batch_empty",
            Self::MatchBatchNonempty => "match_batch_nonempty",
            Self::BatchMissing => "batch_missing",
            Self::BatchExtra => "batch_extra",
            Self::BatchMixed => "batch_mixed",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexPrivacyShadowFailureCode {
    Unavailable,
    ContractInvalid,
    Other,
}

impl IndexPrivacyShadowFailureCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "social_graph.index_privacy_unavailable",
            Self::ContractInvalid => "social_graph.index_privacy_contract_invalid",
            Self::Other => "other",
        }
    }

    fn from_port_error(error: &PortError) -> Self {
        match error.code.as_str() {
            "social_graph.index_privacy_unavailable" => Self::Unavailable,
            "social_graph.index_privacy_contract_invalid" => Self::ContractInvalid,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexPrivacyShadowObservation {
    pub operation: IndexPrivacyShadowOperation,
    pub outcome: IndexPrivacyShadowOutcome,
    pub comparison_duration: Duration,
    pub failure_code: Option<IndexPrivacyShadowFailureCode>,
    pub retryable: Option<bool>,
}

pub trait IndexPrivacyShadowObserver: Send + Sync {
    fn observe(&self, observation: IndexPrivacyShadowObservation);
}

#[derive(Default)]
struct NoopIndexPrivacyShadowObserver;

impl IndexPrivacyShadowObserver for NoopIndexPrivacyShadowObserver {
    fn observe(&self, _observation: IndexPrivacyShadowObservation) {}
}

/// Non-authoritative parity observer for Social Graph privacy reads.
///
/// The owner port always determines the returned result. Index is queried only after the
/// owner succeeds, and projection errors, timeouts, or mismatches are recorded without
/// changing policy or extending the caller's deadline budget.
#[derive(Clone)]
pub struct IndexShadowSocialGraphPrivacyReadPort {
    authoritative: Arc<dyn SocialGraphPrivacyReadPort>,
    projected: Arc<dyn SocialGraphPrivacyReadPort>,
    observer: Arc<dyn IndexPrivacyShadowObserver>,
}

impl IndexShadowSocialGraphPrivacyReadPort {
    pub fn new(
        authoritative: Arc<dyn SocialGraphPrivacyReadPort>,
        runtime: SharedIndexQueryRuntime,
    ) -> Self {
        Self::with_observer(
            authoritative,
            runtime,
            Arc::new(NoopIndexPrivacyShadowObserver),
        )
    }

    pub fn with_observer(
        authoritative: Arc<dyn SocialGraphPrivacyReadPort>,
        runtime: SharedIndexQueryRuntime,
        observer: Arc<dyn IndexPrivacyShadowObserver>,
    ) -> Self {
        Self {
            authoritative,
            projected: Arc::new(IndexSocialGraphPrivacyReadPort::new(runtime)),
            observer,
        }
    }

    #[cfg(test)]
    fn from_ports(
        authoritative: Arc<dyn SocialGraphPrivacyReadPort>,
        projected: Arc<dyn SocialGraphPrivacyReadPort>,
        observer: Arc<dyn IndexPrivacyShadowObserver>,
    ) -> Self {
        Self {
            authoritative,
            projected,
            observer,
        }
    }
}

#[async_trait]
impl SocialGraphPrivacyReadPort for IndexShadowSocialGraphPrivacyReadPort {
    async fn blocks_between(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<bool, PortError> {
        let operation_started_at = Instant::now();
        let budget = shadow_budget(&context);
        let authoritative = self
            .authoritative
            .blocks_between(context.clone(), request)
            .await?;
        let comparison_started_at = Instant::now();
        let projected = projected_within_remaining_budget(
            operation_started_at,
            budget,
            self.projected.blocks_between(context, request),
        )
        .await;
        observe_bool(
            self.observer.as_ref(),
            IndexPrivacyShadowOperation::BlocksBetween,
            authoritative,
            projected,
            comparison_started_at.elapsed(),
        );
        Ok(authoritative)
    }

    async fn source_mutes_target(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<bool, PortError> {
        let operation_started_at = Instant::now();
        let budget = shadow_budget(&context);
        let authoritative = self
            .authoritative
            .source_mutes_target(context.clone(), request)
            .await?;
        let comparison_started_at = Instant::now();
        let projected = projected_within_remaining_budget(
            operation_started_at,
            budget,
            self.projected.source_mutes_target(context, request),
        )
        .await;
        observe_bool(
            self.observer.as_ref(),
            IndexPrivacyShadowOperation::SourceMutesTarget,
            authoritative,
            projected,
            comparison_started_at.elapsed(),
        );
        Ok(authoritative)
    }

    async fn source_follows_target(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<bool, PortError> {
        let operation_started_at = Instant::now();
        let budget = shadow_budget(&context);
        let authoritative = self
            .authoritative
            .source_follows_target(context.clone(), request)
            .await?;
        let comparison_started_at = Instant::now();
        let projected = projected_within_remaining_budget(
            operation_started_at,
            budget,
            self.projected.source_follows_target(context, request),
        )
        .await;
        observe_bool(
            self.observer.as_ref(),
            IndexPrivacyShadowOperation::SourceFollowsTarget,
            authoritative,
            projected,
            comparison_started_at.elapsed(),
        );
        Ok(authoritative)
    }

    async fn source_follows_targets(
        &self,
        context: PortContext,
        request: SocialGraphFollowBatchRequest,
    ) -> Result<SocialGraphFollowBatchResult, PortError> {
        let operation_started_at = Instant::now();
        let budget = shadow_budget(&context);
        let authoritative = self
            .authoritative
            .source_follows_targets(context.clone(), request.clone())
            .await?;
        let comparison_started_at = Instant::now();
        let projected = projected_within_remaining_budget(
            operation_started_at,
            budget,
            self.projected.source_follows_targets(context, request),
        )
        .await;
        observe_batch(
            self.observer.as_ref(),
            IndexPrivacyShadowOperation::SourceFollowsTargets,
            &authoritative,
            projected,
            comparison_started_at.elapsed(),
        );
        Ok(authoritative)
    }
}

fn shadow_budget(context: &PortContext) -> Duration {
    Duration::from_millis(context.deadline_ms.unwrap_or_default())
}

async fn projected_within_remaining_budget<T>(
    operation_started_at: Instant,
    budget: Duration,
    future: impl std::future::Future<Output = Result<T, PortError>>,
) -> Result<T, PortError> {
    let remaining = budget
        .checked_sub(operation_started_at.elapsed())
        .unwrap_or_default();
    if remaining.is_zero() {
        return Err(shadow_timeout());
    }
    tokio::time::timeout(remaining, future)
        .await
        .map_err(|_| shadow_timeout())?
}

fn shadow_timeout() -> PortError {
    PortError::timeout(
        "social_graph.index_privacy_unavailable",
        "social graph Index privacy shadow exceeded the caller deadline budget",
    )
}

fn observe_bool(
    observer: &dyn IndexPrivacyShadowObserver,
    operation: IndexPrivacyShadowOperation,
    authoritative: bool,
    projected: Result<bool, PortError>,
    comparison_duration: Duration,
) {
    match projected {
        Ok(projected) => {
            let outcome = classify_bool(authoritative, projected);
            observer.observe(IndexPrivacyShadowObservation {
                operation,
                outcome,
                comparison_duration,
                failure_code: None,
                retryable: None,
            });
            match outcome {
                IndexPrivacyShadowOutcome::MatchPositive
                | IndexPrivacyShadowOutcome::MatchNegative => {
                    tracing::debug!(
                        target: SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_TARGET,
                        operation = operation.as_str(),
                        outcome = outcome.as_str(),
                        authoritative,
                        projected,
                        comparison_duration_ms = comparison_duration.as_millis() as u64,
                        "Social Graph Index privacy shadow matched authoritative owner result"
                    );
                }
                _ => {
                    tracing::warn!(
                        target: SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_TARGET,
                        operation = operation.as_str(),
                        outcome = outcome.as_str(),
                        authoritative,
                        projected,
                        comparison_duration_ms = comparison_duration.as_millis() as u64,
                        "Social Graph Index privacy shadow mismatch"
                    );
                }
            }
        }
        Err(error) => {
            let failure_code = IndexPrivacyShadowFailureCode::from_port_error(&error);
            observer.observe(IndexPrivacyShadowObservation {
                operation,
                outcome: IndexPrivacyShadowOutcome::Error,
                comparison_duration,
                failure_code: Some(failure_code),
                retryable: Some(error.retryable),
            });
            tracing::warn!(
                target: SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_TARGET,
                operation = operation.as_str(),
                outcome = IndexPrivacyShadowOutcome::Error.as_str(),
                error_code = failure_code.as_str(),
                retryable = error.retryable,
                comparison_duration_ms = comparison_duration.as_millis() as u64,
                "Social Graph Index privacy shadow read failed"
            );
        }
    }
}

fn classify_bool(authoritative: bool, projected: bool) -> IndexPrivacyShadowOutcome {
    match (authoritative, projected) {
        (true, true) => IndexPrivacyShadowOutcome::MatchPositive,
        (false, false) => IndexPrivacyShadowOutcome::MatchNegative,
        (true, false) => IndexPrivacyShadowOutcome::FalseNegative,
        (false, true) => IndexPrivacyShadowOutcome::FalsePositive,
    }
}

fn observe_batch(
    observer: &dyn IndexPrivacyShadowObserver,
    operation: IndexPrivacyShadowOperation,
    authoritative: &SocialGraphFollowBatchResult,
    projected: Result<SocialGraphFollowBatchResult, PortError>,
    comparison_duration: Duration,
) {
    match projected {
        Ok(projected) => {
            let outcome = classify_batch(authoritative, &projected);
            observer.observe(IndexPrivacyShadowObservation {
                operation,
                outcome,
                comparison_duration,
                failure_code: None,
                retryable: None,
            });
            match outcome {
                IndexPrivacyShadowOutcome::MatchBatchEmpty
                | IndexPrivacyShadowOutcome::MatchBatchNonempty => {
                    tracing::debug!(
                        target: SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_TARGET,
                        operation = operation.as_str(),
                        outcome = outcome.as_str(),
                        authoritative_count = authoritative.followed_target_user_ids.len(),
                        projected_count = projected.followed_target_user_ids.len(),
                        comparison_duration_ms = comparison_duration.as_millis() as u64,
                        "Social Graph Index privacy shadow matched authoritative owner result"
                    );
                }
                _ => {
                    tracing::warn!(
                        target: SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_TARGET,
                        operation = operation.as_str(),
                        outcome = outcome.as_str(),
                        authoritative_count = authoritative.followed_target_user_ids.len(),
                        projected_count = projected.followed_target_user_ids.len(),
                        comparison_duration_ms = comparison_duration.as_millis() as u64,
                        "Social Graph Index privacy shadow mismatch"
                    );
                }
            }
        }
        Err(error) => {
            let failure_code = IndexPrivacyShadowFailureCode::from_port_error(&error);
            observer.observe(IndexPrivacyShadowObservation {
                operation,
                outcome: IndexPrivacyShadowOutcome::Error,
                comparison_duration,
                failure_code: Some(failure_code),
                retryable: Some(error.retryable),
            });
            tracing::warn!(
                target: SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_TARGET,
                operation = operation.as_str(),
                outcome = IndexPrivacyShadowOutcome::Error.as_str(),
                error_code = failure_code.as_str(),
                retryable = error.retryable,
                comparison_duration_ms = comparison_duration.as_millis() as u64,
                "Social Graph Index privacy shadow read failed"
            );
        }
    }
}

fn classify_batch(
    authoritative: &SocialGraphFollowBatchResult,
    projected: &SocialGraphFollowBatchResult,
) -> IndexPrivacyShadowOutcome {
    let authoritative = authoritative
        .followed_target_user_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let projected = projected
        .followed_target_user_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    if authoritative == projected {
        return if authoritative.is_empty() {
            IndexPrivacyShadowOutcome::MatchBatchEmpty
        } else {
            IndexPrivacyShadowOutcome::MatchBatchNonempty
        };
    }

    let has_missing = authoritative.difference(&projected).next().is_some();
    let has_extra = projected.difference(&authoritative).next().is_some();
    match (has_missing, has_extra) {
        (true, false) => IndexPrivacyShadowOutcome::BatchMissing,
        (false, true) => IndexPrivacyShadowOutcome::BatchExtra,
        (true, true) => IndexPrivacyShadowOutcome::BatchMixed,
        (false, false) => unreachable!("unequal sets must have a missing or extra value"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::Duration;

    use async_trait::async_trait;
    use rustok_api::PortActor;
    use uuid::Uuid;

    use super::*;

    #[derive(Clone)]
    struct FixedPrivacyPort {
        boolean: bool,
        batch: Vec<Uuid>,
        fail: bool,
    }

    #[async_trait]
    impl SocialGraphPrivacyReadPort for FixedPrivacyPort {
        async fn blocks_between(
            &self,
            _context: PortContext,
            _request: SocialGraphPairRequest,
        ) -> Result<bool, PortError> {
            self.boolean_result()
        }

        async fn source_mutes_target(
            &self,
            _context: PortContext,
            _request: SocialGraphPairRequest,
        ) -> Result<bool, PortError> {
            self.boolean_result()
        }

        async fn source_follows_target(
            &self,
            _context: PortContext,
            _request: SocialGraphPairRequest,
        ) -> Result<bool, PortError> {
            self.boolean_result()
        }

        async fn source_follows_targets(
            &self,
            _context: PortContext,
            _request: SocialGraphFollowBatchRequest,
        ) -> Result<SocialGraphFollowBatchResult, PortError> {
            if self.fail {
                Err(projected_failure())
            } else {
                Ok(SocialGraphFollowBatchResult {
                    followed_target_user_ids: self.batch.clone(),
                })
            }
        }
    }

    impl FixedPrivacyPort {
        fn boolean_result(&self) -> Result<bool, PortError> {
            if self.fail {
                Err(projected_failure())
            } else {
                Ok(self.boolean)
            }
        }
    }

    #[derive(Clone)]
    struct SlowPrivacyPort;

    #[async_trait]
    impl SocialGraphPrivacyReadPort for SlowPrivacyPort {
        async fn blocks_between(
            &self,
            _context: PortContext,
            _request: SocialGraphPairRequest,
        ) -> Result<bool, PortError> {
            slow_boolean().await
        }

        async fn source_mutes_target(
            &self,
            _context: PortContext,
            _request: SocialGraphPairRequest,
        ) -> Result<bool, PortError> {
            slow_boolean().await
        }

        async fn source_follows_target(
            &self,
            _context: PortContext,
            _request: SocialGraphPairRequest,
        ) -> Result<bool, PortError> {
            slow_boolean().await
        }

        async fn source_follows_targets(
            &self,
            _context: PortContext,
            _request: SocialGraphFollowBatchRequest,
        ) -> Result<SocialGraphFollowBatchResult, PortError> {
            tokio::time::sleep(Duration::from_millis(250)).await;
            Ok(SocialGraphFollowBatchResult {
                followed_target_user_ids: Vec::new(),
            })
        }
    }

    async fn slow_boolean() -> Result<bool, PortError> {
        tokio::time::sleep(Duration::from_millis(250)).await;
        Ok(false)
    }

    #[derive(Default)]
    struct RecordingObserver {
        observations: Mutex<Vec<IndexPrivacyShadowObservation>>,
    }

    impl IndexPrivacyShadowObserver for RecordingObserver {
        fn observe(&self, observation: IndexPrivacyShadowObservation) {
            self.observations
                .lock()
                .expect("observation lock")
                .push(observation);
        }
    }

    fn context() -> PortContext {
        context_with_deadline(Duration::from_secs(1))
    }

    fn context_with_deadline(deadline: Duration) -> PortContext {
        PortContext::new(
            Uuid::from_u128(1).to_string(),
            PortActor::service("privacy-shadow-test"),
            "und",
            "privacy-shadow-test-correlation",
        )
        .with_deadline(deadline)
    }

    fn pair() -> SocialGraphPairRequest {
        SocialGraphPairRequest {
            source_user_id: Uuid::from_u128(2),
            target_user_id: Uuid::from_u128(3),
        }
    }

    fn batch(values: &[u128]) -> SocialGraphFollowBatchResult {
        SocialGraphFollowBatchResult {
            followed_target_user_ids: values.iter().copied().map(Uuid::from_u128).collect(),
        }
    }

    fn projected_failure() -> PortError {
        PortError::unavailable(
            "social_graph.index_privacy_unavailable",
            "projected privacy state is unavailable",
        )
    }

    #[test]
    fn boolean_outcomes_distinguish_negative_safety() {
        assert_eq!(
            classify_bool(true, true),
            IndexPrivacyShadowOutcome::MatchPositive
        );
        assert_eq!(
            classify_bool(false, false),
            IndexPrivacyShadowOutcome::MatchNegative
        );
        assert_eq!(
            classify_bool(true, false),
            IndexPrivacyShadowOutcome::FalseNegative
        );
        assert_eq!(
            classify_bool(false, true),
            IndexPrivacyShadowOutcome::FalsePositive
        );
    }

    #[test]
    fn batch_outcomes_distinguish_missing_extra_and_mixed() {
        assert_eq!(
            classify_batch(&batch(&[]), &batch(&[])),
            IndexPrivacyShadowOutcome::MatchBatchEmpty
        );
        assert_eq!(
            classify_batch(&batch(&[1]), &batch(&[1])),
            IndexPrivacyShadowOutcome::MatchBatchNonempty
        );
        assert_eq!(
            classify_batch(&batch(&[1, 2]), &batch(&[1])),
            IndexPrivacyShadowOutcome::BatchMissing
        );
        assert_eq!(
            classify_batch(&batch(&[1]), &batch(&[1, 2])),
            IndexPrivacyShadowOutcome::BatchExtra
        );
        assert_eq!(
            classify_batch(&batch(&[1, 2]), &batch(&[2, 3])),
            IndexPrivacyShadowOutcome::BatchMixed
        );
    }

    #[tokio::test]
    async fn mismatch_returns_authoritative_boolean_and_observes_false_negative() {
        let observer = Arc::new(RecordingObserver::default());
        let shadow = IndexShadowSocialGraphPrivacyReadPort::from_ports(
            Arc::new(FixedPrivacyPort {
                boolean: true,
                batch: Vec::new(),
                fail: false,
            }),
            Arc::new(FixedPrivacyPort {
                boolean: false,
                batch: Vec::new(),
                fail: false,
            }),
            observer.clone(),
        );

        assert!(shadow.blocks_between(context(), pair()).await.unwrap());
        let observations = observer.observations.lock().expect("observation lock");
        assert_eq!(observations.len(), 1);
        assert_eq!(
            observations[0].outcome,
            IndexPrivacyShadowOutcome::FalseNegative
        );
        assert_eq!(observations[0].failure_code, None);
    }

    #[tokio::test]
    async fn projected_error_returns_authoritative_batch_and_bounded_failure() {
        let expected = vec![Uuid::from_u128(4), Uuid::from_u128(5)];
        let observer = Arc::new(RecordingObserver::default());
        let shadow = IndexShadowSocialGraphPrivacyReadPort::from_ports(
            Arc::new(FixedPrivacyPort {
                boolean: true,
                batch: expected.clone(),
                fail: false,
            }),
            Arc::new(FixedPrivacyPort {
                boolean: false,
                batch: Vec::new(),
                fail: true,
            }),
            observer.clone(),
        );

        let result = shadow
            .source_follows_targets(
                context(),
                SocialGraphFollowBatchRequest {
                    source_user_id: Uuid::from_u128(2),
                    target_user_ids: vec![Uuid::from_u128(4), Uuid::from_u128(5)],
                },
            )
            .await
            .unwrap();

        assert_eq!(result.followed_target_user_ids, expected);
        let observations = observer.observations.lock().expect("observation lock");
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].outcome, IndexPrivacyShadowOutcome::Error);
        assert_eq!(
            observations[0].failure_code,
            Some(IndexPrivacyShadowFailureCode::Unavailable)
        );
        assert_eq!(observations[0].retryable, Some(true));
    }

    #[tokio::test]
    async fn projected_timeout_returns_authoritative_result_within_caller_budget() {
        let observer = Arc::new(RecordingObserver::default());
        let shadow = IndexShadowSocialGraphPrivacyReadPort::from_ports(
            Arc::new(FixedPrivacyPort {
                boolean: true,
                batch: Vec::new(),
                fail: false,
            }),
            Arc::new(SlowPrivacyPort),
            observer.clone(),
        );

        let result = tokio::time::timeout(
            Duration::from_millis(100),
            shadow.blocks_between(
                context_with_deadline(Duration::from_millis(10)),
                pair(),
            ),
        )
        .await
        .expect("shadow observation must not outlive the caller budget")
        .unwrap();

        assert!(result);
        let observations = observer.observations.lock().expect("observation lock");
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].outcome, IndexPrivacyShadowOutcome::Error);
        assert_eq!(
            observations[0].failure_code,
            Some(IndexPrivacyShadowFailureCode::Unavailable)
        );
        assert_eq!(observations[0].retryable, Some(true));
    }
}
