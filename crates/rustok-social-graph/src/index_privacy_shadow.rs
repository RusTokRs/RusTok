use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use rustok_index::SharedIndexQueryRuntime;
use rustok_telemetry::social_graph_index_privacy_shadow_metrics::{
    SocialGraphIndexPrivacyShadowOperation as ShadowOperation,
    SocialGraphIndexPrivacyShadowOutcome as ShadowOutcome, record_failure, record_observation,
};

use crate::{
    IndexSocialGraphPrivacyReadPort, SocialGraphFollowBatchRequest, SocialGraphFollowBatchResult,
    SocialGraphPairRequest, SocialGraphPrivacyReadPort,
};

pub const SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_TARGET: &str =
    "rustok_social_graph::index_privacy_shadow";

/// Non-authoritative parity observer for Social Graph privacy reads.
///
/// The owner port always determines the returned result. Index is queried only after the
/// owner succeeds, and projection errors or mismatches are recorded without changing policy.
#[derive(Clone)]
pub struct IndexShadowSocialGraphPrivacyReadPort {
    authoritative: Arc<dyn SocialGraphPrivacyReadPort>,
    projected: Arc<dyn SocialGraphPrivacyReadPort>,
}

impl IndexShadowSocialGraphPrivacyReadPort {
    pub fn new(
        authoritative: Arc<dyn SocialGraphPrivacyReadPort>,
        runtime: SharedIndexQueryRuntime,
    ) -> Self {
        Self {
            authoritative,
            projected: Arc::new(IndexSocialGraphPrivacyReadPort::new(runtime)),
        }
    }

    #[cfg(test)]
    fn from_ports(
        authoritative: Arc<dyn SocialGraphPrivacyReadPort>,
        projected: Arc<dyn SocialGraphPrivacyReadPort>,
    ) -> Self {
        Self {
            authoritative,
            projected,
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
        let authoritative = self
            .authoritative
            .blocks_between(context.clone(), request)
            .await?;
        let started_at = Instant::now();
        let projected = self.projected.blocks_between(context, request).await;
        observe_bool(
            ShadowOperation::BlocksBetween,
            authoritative,
            projected,
            started_at.elapsed(),
        );
        Ok(authoritative)
    }

    async fn source_mutes_target(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<bool, PortError> {
        let authoritative = self
            .authoritative
            .source_mutes_target(context.clone(), request)
            .await?;
        let started_at = Instant::now();
        let projected = self.projected.source_mutes_target(context, request).await;
        observe_bool(
            ShadowOperation::SourceMutesTarget,
            authoritative,
            projected,
            started_at.elapsed(),
        );
        Ok(authoritative)
    }

    async fn source_follows_target(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<bool, PortError> {
        let authoritative = self
            .authoritative
            .source_follows_target(context.clone(), request)
            .await?;
        let started_at = Instant::now();
        let projected = self.projected.source_follows_target(context, request).await;
        observe_bool(
            ShadowOperation::SourceFollowsTarget,
            authoritative,
            projected,
            started_at.elapsed(),
        );
        Ok(authoritative)
    }

    async fn source_follows_targets(
        &self,
        context: PortContext,
        request: SocialGraphFollowBatchRequest,
    ) -> Result<SocialGraphFollowBatchResult, PortError> {
        let authoritative = self
            .authoritative
            .source_follows_targets(context.clone(), request.clone())
            .await?;
        let started_at = Instant::now();
        let projected = self.projected.source_follows_targets(context, request).await;
        observe_batch(
            ShadowOperation::SourceFollowsTargets,
            &authoritative,
            projected,
            started_at.elapsed(),
        );
        Ok(authoritative)
    }
}

fn observe_bool(
    operation: ShadowOperation,
    authoritative: bool,
    projected: Result<bool, PortError>,
    comparison_duration: Duration,
) {
    match projected {
        Ok(projected) => {
            let outcome = classify_bool(authoritative, projected);
            record_observation(operation, outcome, comparison_duration);
            match outcome {
                ShadowOutcome::MatchPositive | ShadowOutcome::MatchNegative => {
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
            record_failure(
                operation,
                &error.code,
                error.retryable,
                comparison_duration,
            );
            tracing::warn!(
                target: SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_TARGET,
                operation = operation.as_str(),
                outcome = ShadowOutcome::Error.as_str(),
                error_code = %error.code,
                retryable = error.retryable,
                comparison_duration_ms = comparison_duration.as_millis() as u64,
                "Social Graph Index privacy shadow read failed"
            );
        }
    }
}

fn classify_bool(authoritative: bool, projected: bool) -> ShadowOutcome {
    match (authoritative, projected) {
        (true, true) => ShadowOutcome::MatchPositive,
        (false, false) => ShadowOutcome::MatchNegative,
        (true, false) => ShadowOutcome::FalseNegative,
        (false, true) => ShadowOutcome::FalsePositive,
    }
}

fn observe_batch(
    operation: ShadowOperation,
    authoritative: &SocialGraphFollowBatchResult,
    projected: Result<SocialGraphFollowBatchResult, PortError>,
    comparison_duration: Duration,
) {
    match projected {
        Ok(projected) => {
            let outcome = classify_batch(authoritative, &projected);
            record_observation(operation, outcome, comparison_duration);
            match outcome {
                ShadowOutcome::MatchBatchEmpty | ShadowOutcome::MatchBatchNonempty => {
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
            record_failure(
                operation,
                &error.code,
                error.retryable,
                comparison_duration,
            );
            tracing::warn!(
                target: SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_TARGET,
                operation = operation.as_str(),
                outcome = ShadowOutcome::Error.as_str(),
                error_code = %error.code,
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
) -> ShadowOutcome {
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
            ShadowOutcome::MatchBatchEmpty
        } else {
            ShadowOutcome::MatchBatchNonempty
        };
    }

    let has_missing = authoritative.difference(&projected).next().is_some();
    let has_extra = projected.difference(&authoritative).next().is_some();
    match (has_missing, has_extra) {
        (true, false) => ShadowOutcome::BatchMissing,
        (false, true) => ShadowOutcome::BatchExtra,
        (true, true) => ShadowOutcome::BatchMixed,
        (false, false) => unreachable!("unequal sets must have a missing or extra value"),
    }
}

#[cfg(test)]
mod tests {
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

    fn context() -> PortContext {
        PortContext::new(
            Uuid::from_u128(1).to_string(),
            PortActor::service("privacy-shadow-test"),
            "und",
            "privacy-shadow-test-correlation",
        )
        .with_deadline(Duration::from_secs(1))
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
        assert_eq!(classify_bool(true, true), ShadowOutcome::MatchPositive);
        assert_eq!(classify_bool(false, false), ShadowOutcome::MatchNegative);
        assert_eq!(classify_bool(true, false), ShadowOutcome::FalseNegative);
        assert_eq!(classify_bool(false, true), ShadowOutcome::FalsePositive);
    }

    #[test]
    fn batch_outcomes_distinguish_missing_extra_and_mixed() {
        assert_eq!(
            classify_batch(&batch(&[]), &batch(&[])),
            ShadowOutcome::MatchBatchEmpty
        );
        assert_eq!(
            classify_batch(&batch(&[1]), &batch(&[1])),
            ShadowOutcome::MatchBatchNonempty
        );
        assert_eq!(
            classify_batch(&batch(&[1, 2]), &batch(&[1])),
            ShadowOutcome::BatchMissing
        );
        assert_eq!(
            classify_batch(&batch(&[1]), &batch(&[1, 2])),
            ShadowOutcome::BatchExtra
        );
        assert_eq!(
            classify_batch(&batch(&[1, 2]), &batch(&[2, 3])),
            ShadowOutcome::BatchMixed
        );
    }

    #[tokio::test]
    async fn mismatch_returns_authoritative_boolean() {
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
        );

        assert!(shadow.blocks_between(context(), pair()).await.unwrap());
    }

    #[tokio::test]
    async fn projected_error_returns_authoritative_batch() {
        let expected = vec![Uuid::from_u128(4), Uuid::from_u128(5)];
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
    }
}
