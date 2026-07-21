use serde::{Deserialize, Serialize};

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

const fn status(value: bool) -> ProviderSloStatus {
    if value {
        ProviderSloStatus::Pass
    } else {
        ProviderSloStatus::Fail
    }
}
