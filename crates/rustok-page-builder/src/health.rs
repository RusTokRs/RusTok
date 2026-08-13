use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

pub const PROVIDER_HEALTH_WINDOW_CAPACITY: usize = 256;
pub const PROVIDER_HEALTH_MINIMUM_SAMPLES_PER_OPERATION: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderHealthState {
    Ready,
    Degraded,
    Unavailable,
}

impl ProviderHealthState {
    pub const ALL: [Self; 3] = [Self::Ready, Self::Degraded, Self::Unavailable];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderDegradationReason {
    CapabilityDisabled,
    ProviderUnhealthy,
    SanitizeBackpressure,
    PublishBacklog,
}

impl ProviderDegradationReason {
    pub const ALL: [Self; 4] = [
        Self::CapabilityDisabled,
        Self::ProviderUnhealthy,
        Self::SanitizeBackpressure,
        Self::PublishBacklog,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CapabilityDisabled => "capability_disabled",
            Self::ProviderUnhealthy => "provider_unhealthy",
            Self::SanitizeBackpressure => "sanitize_backpressure",
            Self::PublishBacklog => "publish_backlog",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ProviderSloThresholds {
    pub preview_p95_ms: u64,
    pub publish_p95_ms: u64,
    pub sanitize_failure_rate_max: f64,
    pub runtime_error_rate_max: f64,
}

impl ProviderSloThresholds {
    pub const PILOT: Self = Self {
        preview_p95_ms: 1500,
        publish_p95_ms: 3000,
        sanitize_failure_rate_max: 0.01,
        runtime_error_rate_max: 0.01,
    };
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderHealthSnapshot {
    pub state: ProviderHealthState,
    #[serde(default)]
    pub degradation_reasons: Vec<ProviderDegradationReason>,
    pub thresholds: ProviderSloThresholds,
    pub observed: ProviderSloObservations,
}

impl ProviderHealthSnapshot {
    pub fn evaluate(observed: ProviderSloObservations) -> Self {
        let thresholds = ProviderSloThresholds::PILOT;
        let mut degradation_reasons = Vec::new();

        if observed.preview_p95_ms > thresholds.preview_p95_ms
            || observed.runtime_error_rate > thresholds.runtime_error_rate_max
        {
            degradation_reasons.push(ProviderDegradationReason::ProviderUnhealthy);
        }

        if observed.sanitize_failure_rate > thresholds.sanitize_failure_rate_max {
            degradation_reasons.push(ProviderDegradationReason::SanitizeBackpressure);
        }

        if observed.publish_p95_ms > thresholds.publish_p95_ms {
            degradation_reasons.push(ProviderDegradationReason::PublishBacklog);
        }

        let state = if degradation_reasons.is_empty() {
            ProviderHealthState::Ready
        } else if observed.runtime_error_rate > thresholds.runtime_error_rate_max * 2.0 {
            ProviderHealthState::Unavailable
        } else {
            ProviderHealthState::Degraded
        };

        Self {
            state,
            degradation_reasons,
            thresholds,
            observed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ProviderSloObservations {
    pub preview_p95_ms: u64,
    pub publish_p95_ms: u64,
    pub sanitize_failure_rate: f64,
    pub runtime_error_rate: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProviderHealthEvidence {
    pub module_slug: &'static str,
    pub contract: &'static str,
    pub builder_contract_version: &'static str,
    pub snapshot: ProviderHealthSnapshot,
    pub slo_evaluation: ProviderSloEvaluation,
}

impl ProviderHealthEvidence {
    pub fn from_observations(observed: ProviderSloObservations) -> Self {
        let snapshot = ProviderHealthSnapshot::evaluate(observed);
        let thresholds = snapshot.thresholds;
        let observed = snapshot.observed;

        Self {
            module_slug: "page_builder",
            contract: "grapesjs",
            builder_contract_version: "1.1",
            slo_evaluation: ProviderSloEvaluation::evaluate(observed, thresholds),
            snapshot,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSloStatus {
    Pass,
    Fail,
}

impl ProviderSloStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSloEvaluation {
    pub preview_p95_ms: ProviderSloStatus,
    pub publish_p95_ms: ProviderSloStatus,
    pub sanitize_failure_rate: ProviderSloStatus,
    pub runtime_error_rate: ProviderSloStatus,
    pub overall: ProviderSloStatus,
}

impl ProviderSloEvaluation {
    pub fn evaluate(observed: ProviderSloObservations, thresholds: ProviderSloThresholds) -> Self {
        let preview_p95_ms = status(observed.preview_p95_ms <= thresholds.preview_p95_ms);
        let publish_p95_ms = status(observed.publish_p95_ms <= thresholds.publish_p95_ms);
        let sanitize_failure_rate =
            status(observed.sanitize_failure_rate <= thresholds.sanitize_failure_rate_max);
        let runtime_error_rate =
            status(observed.runtime_error_rate <= thresholds.runtime_error_rate_max);
        let overall = status(
            preview_p95_ms == ProviderSloStatus::Pass
                && publish_p95_ms == ProviderSloStatus::Pass
                && sanitize_failure_rate == ProviderSloStatus::Pass
                && runtime_error_rate == ProviderSloStatus::Pass,
        );

        Self {
            preview_p95_ms,
            publish_p95_ms,
            sanitize_failure_rate,
            runtime_error_rate,
            overall,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderHealthOperation {
    Preview,
    Publish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderHealthOutcome {
    Succeeded,
    SanitizeFailed,
    RuntimeFailed,
    OtherFailed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProviderHealthRuntimeSampleCounts {
    pub preview: usize,
    pub publish: usize,
}

#[derive(Debug, Clone, Copy)]
struct ProviderHealthRuntimeSample {
    elapsed_ms: u64,
    outcome: ProviderHealthOutcome,
}

#[derive(Debug, Default)]
struct ProviderHealthRuntimeWindow {
    preview: VecDeque<ProviderHealthRuntimeSample>,
    publish: VecDeque<ProviderHealthRuntimeSample>,
}

static PROVIDER_HEALTH_RUNTIME_WINDOW: OnceLock<Mutex<ProviderHealthRuntimeWindow>> =
    OnceLock::new();

/// Record one completed provider operation in the bounded process-local SLO window.
///
/// This is deliberately not deployment-wide evidence. The runtime window is reset on process
/// restart and remains unobserved until both preview and publish have the minimum sample count.
/// Callers must not promote rollout/Wave gates from this snapshot without separately retained
/// exact-source deployment evidence.
pub fn record_provider_health_observation(
    operation: ProviderHealthOperation,
    elapsed: Duration,
    outcome: ProviderHealthOutcome,
) {
    let sample = ProviderHealthRuntimeSample {
        elapsed_ms: elapsed.as_millis().min(u64::MAX as u128) as u64,
        outcome,
    };
    let window = PROVIDER_HEALTH_RUNTIME_WINDOW
        .get_or_init(|| Mutex::new(ProviderHealthRuntimeWindow::default()));
    let mut window = window
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let samples = match operation {
        ProviderHealthOperation::Preview => &mut window.preview,
        ProviderHealthOperation::Publish => &mut window.publish,
    };
    if samples.len() >= PROVIDER_HEALTH_WINDOW_CAPACITY {
        samples.pop_front();
    }
    samples.push_back(sample);
}

pub fn provider_health_runtime_sample_counts() -> ProviderHealthRuntimeSampleCounts {
    let Some(window) = PROVIDER_HEALTH_RUNTIME_WINDOW.get() else {
        return ProviderHealthRuntimeSampleCounts::default();
    };
    let window = window
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    ProviderHealthRuntimeSampleCounts {
        preview: window.preview.len(),
        publish: window.publish.len(),
    }
}

/// Return a process-local provider-health snapshot only after a bounded minimum sample exists for
/// both preview and publish. `None` means unobserved and must never be interpreted as healthy.
pub fn provider_health_runtime_snapshot() -> Option<ProviderHealthSnapshot> {
    let window = PROVIDER_HEALTH_RUNTIME_WINDOW.get()?;
    let window = window
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    snapshot_from_runtime_window(&window)
}

fn snapshot_from_runtime_window(
    window: &ProviderHealthRuntimeWindow,
) -> Option<ProviderHealthSnapshot> {
    if window.preview.len() < PROVIDER_HEALTH_MINIMUM_SAMPLES_PER_OPERATION
        || window.publish.len() < PROVIDER_HEALTH_MINIMUM_SAMPLES_PER_OPERATION
    {
        return None;
    }

    let preview_p95_ms = p95_ms(&window.preview);
    let publish_p95_ms = p95_ms(&window.publish);
    let sanitize_failures = window
        .publish
        .iter()
        .filter(|sample| sample.outcome == ProviderHealthOutcome::SanitizeFailed)
        .count();
    let runtime_failures = window
        .preview
        .iter()
        .chain(window.publish.iter())
        .filter(|sample| sample.outcome == ProviderHealthOutcome::RuntimeFailed)
        .count();
    let total_runtime_samples = window.preview.len() + window.publish.len();

    Some(ProviderHealthSnapshot::evaluate(ProviderSloObservations {
        preview_p95_ms,
        publish_p95_ms,
        sanitize_failure_rate: sanitize_failures as f64 / window.publish.len() as f64,
        runtime_error_rate: runtime_failures as f64 / total_runtime_samples as f64,
    }))
}

fn p95_ms(samples: &VecDeque<ProviderHealthRuntimeSample>) -> u64 {
    let mut values: Vec<_> = samples.iter().map(|sample| sample.elapsed_ms).collect();
    values.sort_unstable();
    let rank = (values.len() * 95).div_ceil(100).saturating_sub(1);
    values[rank]
}

const fn status(value: bool) -> ProviderSloStatus {
    if value {
        ProviderSloStatus::Pass
    } else {
        ProviderSloStatus::Fail
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_window_requires_both_operation_sample_floors() {
        let mut window = ProviderHealthRuntimeWindow::default();
        for _ in 0..PROVIDER_HEALTH_MINIMUM_SAMPLES_PER_OPERATION {
            window.preview.push_back(ProviderHealthRuntimeSample {
                elapsed_ms: 100,
                outcome: ProviderHealthOutcome::Succeeded,
            });
        }
        assert!(snapshot_from_runtime_window(&window).is_none());
        for _ in 0..PROVIDER_HEALTH_MINIMUM_SAMPLES_PER_OPERATION {
            window.publish.push_back(ProviderHealthRuntimeSample {
                elapsed_ms: 200,
                outcome: ProviderHealthOutcome::Succeeded,
            });
        }
        let snapshot = snapshot_from_runtime_window(&window).expect("observed snapshot");
        assert_eq!(snapshot.state, ProviderHealthState::Ready);
        assert_eq!(snapshot.observed.preview_p95_ms, 100);
        assert_eq!(snapshot.observed.publish_p95_ms, 200);
    }

    #[test]
    fn runtime_window_evaluates_terminal_failure_rates() {
        let mut window = ProviderHealthRuntimeWindow::default();
        for index in 0..PROVIDER_HEALTH_MINIMUM_SAMPLES_PER_OPERATION {
            window.preview.push_back(ProviderHealthRuntimeSample {
                elapsed_ms: 100 + index as u64,
                outcome: if index == 0 {
                    ProviderHealthOutcome::RuntimeFailed
                } else {
                    ProviderHealthOutcome::Succeeded
                },
            });
            window.publish.push_back(ProviderHealthRuntimeSample {
                elapsed_ms: 200 + index as u64,
                outcome: if index == 0 {
                    ProviderHealthOutcome::SanitizeFailed
                } else {
                    ProviderHealthOutcome::Succeeded
                },
            });
        }
        let snapshot = snapshot_from_runtime_window(&window).expect("observed snapshot");
        assert_eq!(snapshot.observed.sanitize_failure_rate, 0.05);
        assert_eq!(snapshot.observed.runtime_error_rate, 0.025);
        assert_eq!(snapshot.state, ProviderHealthState::Unavailable);
    }
}
