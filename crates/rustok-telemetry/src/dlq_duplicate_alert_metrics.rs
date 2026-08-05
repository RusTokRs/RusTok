//! Identifier-free Prometheus projection for the physical DLQ duplicate observer.
//!
//! Every label is selected from a closed enum in this module. The API cannot accept broker
//! coordinates, identifiers, payload facts, credentials, raw errors, threshold values, source
//! counts, timestamps, or arbitrary label text.

use lazy_static::lazy_static;
use prometheus::{IntCounterVec, IntGaugeVec, Opts, Registry};

const HEALTH_STATE_LABELS: [&str; 6] = [
    "disabled",
    "not_applicable",
    "starting",
    "available",
    "unavailable",
    "stopped",
];
const EVALUATION_FLAGS: [&str; 5] = [
    "physical_duplicates",
    "identity_conflict",
    "duplicate_messages_threshold",
    "duplicate_groups_threshold",
    "max_copies_threshold",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlqDuplicateAlertDeployment {
    Disabled,
    Unavailable,
    OutboxLocal,
    IggyBundled,
    IggyExternal,
}

impl DlqDuplicateAlertDeployment {
    const fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Unavailable => "unavailable",
            Self::OutboxLocal => "outbox_local",
            Self::IggyBundled => "iggy_bundled",
            Self::IggyExternal => "iggy_external",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlqDuplicateAlertScanMode {
    None,
    GlobalBudget,
    FairWindow,
    MovingWindow,
}

impl DlqDuplicateAlertScanMode {
    const fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::GlobalBudget => "global_budget",
            Self::FairWindow => "fair_window",
            Self::MovingWindow => "moving_window",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlqDuplicateAlertHealthState {
    Disabled,
    NotApplicable,
    Starting,
    Available,
    Unavailable,
    Stopped,
}

impl DlqDuplicateAlertHealthState {
    const fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::NotApplicable => "not_applicable",
            Self::Starting => "starting",
            Self::Available => "available",
            Self::Unavailable => "unavailable",
            Self::Stopped => "stopped",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlqDuplicateAlertMetricLevel {
    None,
    Clear,
    Notice,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DlqDuplicateAlertSnapshotMetrics {
    pub available: bool,
    pub level: DlqDuplicateAlertMetricLevel,
    pub physical_duplicates: bool,
    pub identity_conflict: bool,
    pub duplicate_messages_threshold: bool,
    pub duplicate_groups_threshold: bool,
    pub max_copies_threshold: bool,
}

impl DlqDuplicateAlertMetricLevel {
    const fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Clear => "clear",
            Self::Notice => "notice",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}

lazy_static! {
    /// Current observer health state as a bounded one-hot series.
    pub static ref DLQ_DUPLICATE_ALERT_OBSERVER_STATE: IntGaugeVec = IntGaugeVec::new(
        Opts::new(
            "rustok_dlq_duplicate_alert_observer_state",
            "Current physical DLQ duplicate observer state as a bounded one-hot series"
        ),
        &["deployment", "scan_mode", "state"]
    )
    .expect("Failed to create dlq_duplicate_alert_observer_state");

    /// Total identifier-free runtime snapshots by availability and alert level.
    pub static ref DLQ_DUPLICATE_ALERT_SNAPSHOTS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_dlq_duplicate_alert_snapshots_total",
            "Total physical DLQ duplicate runtime snapshots by bounded availability and level"
        ),
        &["deployment", "scan_mode", "availability", "level"]
    )
    .expect("Failed to create dlq_duplicate_alert_snapshots_total");

    /// Current identifier-free alert evaluation flags as bounded boolean gauges.
    pub static ref DLQ_DUPLICATE_ALERT_EVALUATION_FLAGS: IntGaugeVec = IntGaugeVec::new(
        Opts::new(
            "rustok_dlq_duplicate_alert_evaluation_flags",
            "Current physical DLQ duplicate alert evaluation flags"
        ),
        &["deployment", "scan_mode", "flag"]
    )
    .expect("Failed to create dlq_duplicate_alert_evaluation_flags");
}

pub fn register(registry: &Registry) -> Result<(), prometheus::Error> {
    registry.register(Box::new(DLQ_DUPLICATE_ALERT_OBSERVER_STATE.clone()))?;
    registry.register(Box::new(DLQ_DUPLICATE_ALERT_SNAPSHOTS_TOTAL.clone()))?;
    registry.register(Box::new(DLQ_DUPLICATE_ALERT_EVALUATION_FLAGS.clone()))?;
    Ok(())
}

/// Replace the bounded one-hot observer state for one deployment/scan pair.
pub fn record_state(
    deployment: DlqDuplicateAlertDeployment,
    scan_mode: DlqDuplicateAlertScanMode,
    state: DlqDuplicateAlertHealthState,
) {
    for candidate in HEALTH_STATE_LABELS {
        DLQ_DUPLICATE_ALERT_OBSERVER_STATE
            .with_label_values(&[deployment.label(), scan_mode.label(), candidate])
            .set(if candidate == state.label() { 1 } else { 0 });
    }
}

/// Record one identifier-free runtime snapshot and replace its bounded flag gauges.
pub fn record_snapshot(
    deployment: DlqDuplicateAlertDeployment,
    scan_mode: DlqDuplicateAlertScanMode,
    snapshot: DlqDuplicateAlertSnapshotMetrics,
) {
    DLQ_DUPLICATE_ALERT_SNAPSHOTS_TOTAL
        .with_label_values(&[
            deployment.label(),
            scan_mode.label(),
            if snapshot.available {
                "available"
            } else {
                "unavailable"
            },
            snapshot.level.label(),
        ])
        .inc();

    let values = [
        snapshot.physical_duplicates,
        snapshot.identity_conflict,
        snapshot.duplicate_messages_threshold,
        snapshot.duplicate_groups_threshold,
        snapshot.max_copies_threshold,
    ];
    for (flag, value) in EVALUATION_FLAGS.into_iter().zip(values) {
        DLQ_DUPLICATE_ALERT_EVALUATION_FLAGS
            .with_label_values(&[deployment.label(), scan_mode.label(), flag])
            .set(if value { 1 } else { 0 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_labels_are_closed_and_identifier_free() {
        assert_eq!(HEALTH_STATE_LABELS.len(), 6);
        assert_eq!(EVALUATION_FLAGS.len(), 5);
        assert!(!HEALTH_STATE_LABELS.contains(&"partition"));
        assert!(!EVALUATION_FLAGS.contains(&"offset"));
        assert_eq!(
            DlqDuplicateAlertScanMode::MovingWindow.label(),
            "moving_window"
        );
        assert_eq!(DlqDuplicateAlertMetricLevel::Critical.label(), "critical");
    }
}
