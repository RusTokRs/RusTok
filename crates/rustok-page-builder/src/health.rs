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
