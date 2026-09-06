use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lazy_static::lazy_static;
use prometheus::core::{Collector, Desc};
use prometheus::proto::MetricFamily;
use prometheus::{HistogramOpts, HistogramVec, IntCounterVec, IntGauge, IntGaugeVec, Opts};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SocialGraphIndexPrivacyShadowOperation {
    BlocksBetween,
    SourceMutesTarget,
    SourceFollowsTarget,
    SourceFollowsTargets,
}

impl SocialGraphIndexPrivacyShadowOperation {
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
pub enum SocialGraphIndexPrivacyShadowOutcome {
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

impl SocialGraphIndexPrivacyShadowOutcome {
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

#[derive(Clone)]
struct SocialGraphIndexPrivacyShadowMetrics {
    collector_started_timestamp_seconds: IntGauge,
    observations_total: IntCounterVec,
    failures_total: IntCounterVec,
    comparison_duration_seconds: HistogramVec,
    last_observation_timestamp_seconds: IntGaugeVec,
}

impl SocialGraphIndexPrivacyShadowMetrics {
    fn new() -> Self {
        let collector_started_timestamp_seconds = IntGauge::new(
            "rustok_social_graph_index_privacy_shadow_collector_started_timestamp_seconds",
            "Unix timestamp when the Social Graph Index privacy shadow collector was initialized",
        )
        .expect("Failed to create Social Graph Index privacy shadow collector epoch gauge");
        collector_started_timestamp_seconds.set(unix_timestamp_seconds());

        Self {
            collector_started_timestamp_seconds,
            observations_total: IntCounterVec::new(
                Opts::new(
                    "rustok_social_graph_index_privacy_shadow_observations_total",
                    "Total non-authoritative Social Graph Index privacy shadow observations",
                ),
                &["operation", "outcome"],
            )
            .expect("Failed to create Social Graph Index privacy shadow observation counter"),
            failures_total: IntCounterVec::new(
                Opts::new(
                    "rustok_social_graph_index_privacy_shadow_failures_total",
                    "Total Social Graph Index privacy shadow projection failures by bounded code",
                ),
                &["operation", "error_code", "retryable"],
            )
            .expect("Failed to create Social Graph Index privacy shadow failure counter"),
            comparison_duration_seconds: HistogramVec::new(
                HistogramOpts::new(
                    "rustok_social_graph_index_privacy_shadow_comparison_duration_seconds",
                    "Duration of the non-authoritative Index comparison after the owner privacy read",
                )
                .buckets(vec![
                    0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5,
                    1.0, 2.5, 5.0,
                ]),
                &["operation", "outcome"],
            )
            .expect("Failed to create Social Graph Index privacy shadow duration histogram"),
            last_observation_timestamp_seconds: IntGaugeVec::new(
                Opts::new(
                    "rustok_social_graph_index_privacy_shadow_last_observation_timestamp_seconds",
                    "Unix timestamp of the last Social Graph Index privacy shadow observation",
                ),
                &["operation", "outcome"],
            )
            .expect("Failed to create Social Graph Index privacy shadow timestamp gauge"),
        }
    }
}

impl Collector for SocialGraphIndexPrivacyShadowMetrics {
    fn desc(&self) -> Vec<&Desc> {
        let mut descriptions = Vec::new();
        descriptions.extend(self.collector_started_timestamp_seconds.desc());
        descriptions.extend(self.observations_total.desc());
        descriptions.extend(self.failures_total.desc());
        descriptions.extend(self.comparison_duration_seconds.desc());
        descriptions.extend(self.last_observation_timestamp_seconds.desc());
        descriptions
    }

    fn collect(&self) -> Vec<MetricFamily> {
        let mut families = Vec::new();
        families.extend(self.collector_started_timestamp_seconds.collect());
        families.extend(self.observations_total.collect());
        families.extend(self.failures_total.collect());
        families.extend(self.comparison_duration_seconds.collect());
        families.extend(self.last_observation_timestamp_seconds.collect());
        families
    }
}

lazy_static! {
    static ref SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_METRICS: SocialGraphIndexPrivacyShadowMetrics =
        SocialGraphIndexPrivacyShadowMetrics::new();
}

static SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_METRICS_REGISTERING: AtomicBool = AtomicBool::new(false);

/// Registers the bounded privacy-shadow collector in the process Prometheus registry.
///
/// Registration is idempotent for one process. An enabled evidence shadow should treat a
/// registration error as an activation failure instead of silently running without metrics.
pub fn ensure_registered() -> Result<(), prometheus::Error> {
    if SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_METRICS_REGISTERING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(());
    }

    match crate::register_runtime_collector(Box::new(
        SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_METRICS.clone(),
    )) {
        Ok(()) => Ok(()),
        Err(error) => {
            SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_METRICS_REGISTERING.store(false, Ordering::Release);
            Err(error)
        }
    }
}

pub fn record_observation(
    operation: SocialGraphIndexPrivacyShadowOperation,
    outcome: SocialGraphIndexPrivacyShadowOutcome,
    comparison_duration: Duration,
) {
    let operation = operation.as_str();
    let outcome = outcome.as_str();
    SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_METRICS
        .observations_total
        .with_label_values(&[operation, outcome])
        .inc();
    SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_METRICS
        .comparison_duration_seconds
        .with_label_values(&[operation, outcome])
        .observe(comparison_duration.as_secs_f64());
    SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_METRICS
        .last_observation_timestamp_seconds
        .with_label_values(&[operation, outcome])
        .set(unix_timestamp_seconds());
}

pub fn record_failure(
    operation: SocialGraphIndexPrivacyShadowOperation,
    error_code: &str,
    retryable: bool,
    comparison_duration: Duration,
) {
    record_observation(
        operation,
        SocialGraphIndexPrivacyShadowOutcome::Error,
        comparison_duration,
    );
    SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_METRICS
        .failures_total
        .with_label_values(&[
            operation.as_str(),
            bounded_error_code(error_code),
            if retryable { "true" } else { "false" },
        ])
        .inc();
}

fn bounded_error_code(error_code: &str) -> &'static str {
    match error_code {
        "social_graph.index_privacy_unavailable" => "social_graph.index_privacy_unavailable",
        "social_graph.index_privacy_contract_invalid" => {
            "social_graph.index_privacy_contract_invalid"
        }
        _ => "other",
    }
}

fn unix_timestamp_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{
        SocialGraphIndexPrivacyShadowOperation as Operation,
        SocialGraphIndexPrivacyShadowOutcome as Outcome, bounded_error_code,
    };

    #[test]
    fn operation_and_outcome_labels_are_fixed() {
        assert_eq!(Operation::BlocksBetween.as_str(), "blocks_between");
        assert_eq!(Operation::SourceMutesTarget.as_str(), "source_mutes_target");
        assert_eq!(Outcome::FalseNegative.as_str(), "false_negative");
        assert_eq!(Outcome::BatchMixed.as_str(), "batch_mixed");
        assert_eq!(Outcome::Error.as_str(), "error");
    }

    #[test]
    fn error_code_label_is_bounded() {
        assert_eq!(
            bounded_error_code("social_graph.index_privacy_unavailable"),
            "social_graph.index_privacy_unavailable"
        );
        assert_eq!(
            bounded_error_code("social_graph.index_privacy_contract_invalid"),
            "social_graph.index_privacy_contract_invalid"
        );
        assert_eq!(bounded_error_code("tenant-specific-secret"), "other");
    }
}
