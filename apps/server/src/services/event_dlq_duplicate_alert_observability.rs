use std::env;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use rustok_iggy::{DlqDuplicateAlertLevel, DlqDuplicateAlertRuntimeSnapshot};
use rustok_telemetry::dlq_duplicate_alert_metrics::{
    self, DlqDuplicateAlertDeployment as MetricDeployment,
    DlqDuplicateAlertHealthState as MetricHealthState, DlqDuplicateAlertMetricLevel as MetricLevel,
    DlqDuplicateAlertScanMode as MetricScanMode, DlqDuplicateAlertSnapshotMetrics,
};
use tokio::task::JoinHandle;

use crate::services::app_lifecycle::StopHandle;
use crate::services::event_dlq_duplicate_alert_observer::{
    EventDlqDuplicateAlertObserverHandle, EventDlqDuplicateAlertObserverMode,
};
use crate::services::server_runtime_context::ServerRuntimeContext;

const SCAN_MODE_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE";
const PROJECTION_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventDlqDuplicateAlertObservabilityScanMode {
    GlobalBudget,
    FairWindow,
    MovingWindow,
}

impl EventDlqDuplicateAlertObservabilityScanMode {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::GlobalBudget => "global_budget",
            Self::FairWindow => "fair_window",
            Self::MovingWindow => "moving_window",
        }
    }

    const fn metric(self) -> MetricScanMode {
        match self {
            Self::GlobalBudget => MetricScanMode::GlobalBudget,
            Self::FairWindow => MetricScanMode::FairWindow,
            Self::MovingWindow => MetricScanMode::MovingWindow,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventDlqDuplicateAlertHealthState {
    Disabled,
    NotApplicable,
    Starting,
    Available,
    Unavailable,
    Stopped,
}

impl EventDlqDuplicateAlertHealthState {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::NotApplicable => "not_applicable",
            Self::Starting => "starting",
            Self::Available => "available",
            Self::Unavailable => "unavailable",
            Self::Stopped => "stopped",
        }
    }

    const fn metric(self) -> MetricHealthState {
        match self {
            Self::Disabled => MetricHealthState::Disabled,
            Self::NotApplicable => MetricHealthState::NotApplicable,
            Self::Starting => MetricHealthState::Starting,
            Self::Available => MetricHealthState::Available,
            Self::Unavailable => MetricHealthState::Unavailable,
            Self::Stopped => MetricHealthState::Stopped,
        }
    }
}

/// Identifier-free operational projection for the physical DLQ duplicate observer.
///
/// The projection excludes broker addresses, stream/topic/partition/offset coordinates,
/// identifiers, payloads or digests, credentials, thresholds, source counts, timestamps,
/// raw errors, notification routing, and destructive actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventDlqDuplicateAlertHealthSnapshot {
    mode: EventDlqDuplicateAlertObserverMode,
    scan_mode: Option<EventDlqDuplicateAlertObservabilityScanMode>,
    state: EventDlqDuplicateAlertHealthState,
    generation: Option<u64>,
    level: Option<DlqDuplicateAlertLevel>,
    physical_duplicates: bool,
    identity_conflict: bool,
    task_finished: bool,
}

impl EventDlqDuplicateAlertHealthSnapshot {
    pub const fn mode(self) -> EventDlqDuplicateAlertObserverMode {
        self.mode
    }

    pub const fn scan_mode(self) -> Option<EventDlqDuplicateAlertObservabilityScanMode> {
        self.scan_mode
    }

    pub const fn state(self) -> EventDlqDuplicateAlertHealthState {
        self.state
    }

    pub const fn generation(self) -> Option<u64> {
        self.generation
    }

    pub const fn level(self) -> Option<DlqDuplicateAlertLevel> {
        self.level
    }

    pub const fn has_physical_duplicates(self) -> bool {
        self.physical_duplicates
    }

    pub const fn has_identity_conflict(self) -> bool {
        self.identity_conflict
    }

    pub const fn task_finished(self) -> bool {
        self.task_finished
    }

    pub const fn affects_readiness(self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Projection {
    health: EventDlqDuplicateAlertHealthSnapshot,
    runtime: Option<DlqDuplicateAlertRuntimeSnapshot>,
}

pub struct EventDlqDuplicateAlertObservabilityHandle {
    snapshot: Arc<RwLock<EventDlqDuplicateAlertHealthSnapshot>>,
    task: Option<JoinHandle<()>>,
}

impl EventDlqDuplicateAlertObservabilityHandle {
    pub fn current(&self) -> EventDlqDuplicateAlertHealthSnapshot {
        *self
            .snapshot
            .read()
            .expect("DLQ duplicate observability snapshot lock poisoned")
    }

    pub fn is_finished(&self) -> bool {
        self.task.as_ref().is_some_and(JoinHandle::is_finished)
    }
}

/// Starts a read-only companion that projects the existing observer handle.
///
/// This does not alter observer configuration, scanning, retry, cursor state, event delivery,
/// readiness, liveness, or Profiles authorization.
pub fn start_event_dlq_duplicate_alert_observability(ctx: &ServerRuntimeContext) {
    if ctx.shared_contains::<EventDlqDuplicateAlertObservabilityHandle>() {
        return;
    }

    let configured_scan = configured_scan_mode();
    let initial = collect_projection(ctx, configured_scan);
    record_projection(initial, None);
    let shared = Arc::new(RwLock::new(initial.health));

    let active = matches!(
        initial.health.mode,
        EventDlqDuplicateAlertObserverMode::IggyBundled
            | EventDlqDuplicateAlertObserverMode::IggyExternal
    );
    if !active || !ctx.settings().runtime.runs_background_workers() {
        ctx.shared_insert(EventDlqDuplicateAlertObservabilityHandle {
            snapshot: shared,
            task: None,
        });
        return;
    }

    if !ctx.shared_contains::<StopHandle>() {
        let (stop_handle, _stop_rx) = StopHandle::new();
        ctx.shared_insert(stop_handle);
    }
    let mut stop_rx = ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must exist before DLQ duplicate observability startup")
        .subscribe();
    let runtime_ctx = ctx.clone();
    let task_snapshot = shared.clone();
    let task = tokio::spawn(async move {
        let mut previous = initial;
        loop {
            if wait_or_stop(PROJECTION_INTERVAL, &mut stop_rx).await {
                let stopped = Projection {
                    health: EventDlqDuplicateAlertHealthSnapshot {
                        state: EventDlqDuplicateAlertHealthState::Stopped,
                        level: None,
                        physical_duplicates: false,
                        identity_conflict: false,
                        task_finished: true,
                        ..previous.health
                    },
                    runtime: previous.runtime,
                };
                record_projection(stopped, Some(previous));
                *task_snapshot
                    .write()
                    .expect("DLQ duplicate observability snapshot lock poisoned") = stopped.health;
                return;
            }

            let current = collect_projection(&runtime_ctx, configured_scan);
            if current != previous {
                record_projection(current, Some(previous));
                *task_snapshot
                    .write()
                    .expect("DLQ duplicate observability snapshot lock poisoned") = current.health;
                previous = current;
            }
        }
    });

    ctx.shared_insert(EventDlqDuplicateAlertObservabilityHandle {
        snapshot: shared,
        task: Some(task),
    });
}

fn collect_projection(
    ctx: &ServerRuntimeContext,
    configured_scan: Option<EventDlqDuplicateAlertObservabilityScanMode>,
) -> Projection {
    ctx.shared_map::<EventDlqDuplicateAlertObserverHandle, _>(|handle| {
        project_runtime(
            handle.mode(),
            configured_scan,
            handle.current_snapshot(),
            handle.is_finished(),
        )
    })
    .unwrap_or_else(|| {
        project_runtime(
            EventDlqDuplicateAlertObserverMode::Unavailable,
            None,
            None,
            false,
        )
    })
}

fn project_runtime(
    mode: EventDlqDuplicateAlertObserverMode,
    configured_scan: Option<EventDlqDuplicateAlertObservabilityScanMode>,
    runtime: Option<DlqDuplicateAlertRuntimeSnapshot>,
    task_finished: bool,
) -> Projection {
    let scan_mode = matches!(
        mode,
        EventDlqDuplicateAlertObserverMode::IggyBundled
            | EventDlqDuplicateAlertObserverMode::IggyExternal
    )
    .then_some(configured_scan)
    .flatten();

    let (state, generation, level, physical_duplicates, identity_conflict) = match mode {
        EventDlqDuplicateAlertObserverMode::Disabled => (
            EventDlqDuplicateAlertHealthState::Disabled,
            None,
            None,
            false,
            false,
        ),
        EventDlqDuplicateAlertObserverMode::NotApplicableOutbox => (
            EventDlqDuplicateAlertHealthState::NotApplicable,
            None,
            None,
            false,
            false,
        ),
        EventDlqDuplicateAlertObserverMode::Unavailable => (
            EventDlqDuplicateAlertHealthState::Unavailable,
            None,
            None,
            false,
            false,
        ),
        EventDlqDuplicateAlertObserverMode::IggyBundled
        | EventDlqDuplicateAlertObserverMode::IggyExternal
            if task_finished =>
        {
            (
                EventDlqDuplicateAlertHealthState::Stopped,
                runtime.map(|snapshot| snapshot.generation()),
                None,
                false,
                false,
            )
        }
        EventDlqDuplicateAlertObserverMode::IggyBundled
        | EventDlqDuplicateAlertObserverMode::IggyExternal => match runtime {
            Some(snapshot) if snapshot.is_available() => {
                let evaluation = snapshot
                    .evaluation()
                    .expect("available DLQ duplicate runtime snapshot must have evaluation");
                (
                    EventDlqDuplicateAlertHealthState::Available,
                    Some(snapshot.generation()),
                    Some(evaluation.level()),
                    evaluation.has_physical_duplicates(),
                    evaluation.has_identity_conflict(),
                )
            }
            Some(snapshot) if snapshot.generation() == 0 => (
                EventDlqDuplicateAlertHealthState::Starting,
                Some(0),
                None,
                false,
                false,
            ),
            Some(snapshot) => (
                EventDlqDuplicateAlertHealthState::Unavailable,
                Some(snapshot.generation()),
                None,
                false,
                false,
            ),
            None => (
                EventDlqDuplicateAlertHealthState::Starting,
                None,
                None,
                false,
                false,
            ),
        },
    };

    Projection {
        health: EventDlqDuplicateAlertHealthSnapshot {
            mode,
            scan_mode,
            state,
            generation,
            level,
            physical_duplicates,
            identity_conflict,
            task_finished,
        },
        runtime,
    }
}

fn record_projection(current: Projection, previous: Option<Projection>) {
    if previous.map(|value| value.health.state) != Some(current.health.state) {
        dlq_duplicate_alert_metrics::record_state(
            metric_deployment(current.health.mode),
            current.health.scan_mode.map_or(
                MetricScanMode::None,
                EventDlqDuplicateAlertObservabilityScanMode::metric,
            ),
            current.health.state.metric(),
        );
    }

    let generation_changed = current.health.generation.is_some()
        && previous.and_then(|value| value.health.generation) != current.health.generation;
    if !generation_changed {
        return;
    }

    let evaluation = current.runtime.and_then(|snapshot| snapshot.evaluation());
    dlq_duplicate_alert_metrics::record_snapshot(
        metric_deployment(current.health.mode),
        current.health.scan_mode.map_or(
            MetricScanMode::None,
            EventDlqDuplicateAlertObservabilityScanMode::metric,
        ),
        DlqDuplicateAlertSnapshotMetrics {
            available: current.health.state == EventDlqDuplicateAlertHealthState::Available,
            level: current.health.level.map_or(MetricLevel::None, metric_level),
            physical_duplicates: evaluation.is_some_and(|value| value.has_physical_duplicates()),
            identity_conflict: evaluation.is_some_and(|value| value.has_identity_conflict()),
            duplicate_messages_threshold: evaluation
                .is_some_and(|value| value.duplicate_messages_threshold_reached()),
            duplicate_groups_threshold: evaluation
                .is_some_and(|value| value.duplicate_groups_threshold_reached()),
            max_copies_threshold: evaluation
                .is_some_and(|value| value.max_copies_threshold_reached()),
        },
    );
}

const fn metric_deployment(mode: EventDlqDuplicateAlertObserverMode) -> MetricDeployment {
    match mode {
        EventDlqDuplicateAlertObserverMode::Disabled => MetricDeployment::Disabled,
        EventDlqDuplicateAlertObserverMode::Unavailable => MetricDeployment::Unavailable,
        EventDlqDuplicateAlertObserverMode::NotApplicableOutbox => MetricDeployment::Outbox,
        EventDlqDuplicateAlertObserverMode::IggyBundled => MetricDeployment::IggyBundled,
        EventDlqDuplicateAlertObserverMode::IggyExternal => MetricDeployment::IggyExternal,
    }
}

const fn metric_level(level: DlqDuplicateAlertLevel) -> MetricLevel {
    match level {
        DlqDuplicateAlertLevel::Clear => MetricLevel::Clear,
        DlqDuplicateAlertLevel::Notice => MetricLevel::Notice,
        DlqDuplicateAlertLevel::Warning => MetricLevel::Warning,
        DlqDuplicateAlertLevel::Critical => MetricLevel::Critical,
    }
}

fn configured_scan_mode() -> Option<EventDlqDuplicateAlertObservabilityScanMode> {
    match env::var(SCAN_MODE_ENV) {
        Ok(value) => parse_scan_mode(&value),
        Err(env::VarError::NotPresent) => {
            Some(EventDlqDuplicateAlertObservabilityScanMode::GlobalBudget)
        }
        Err(_) => None,
    }
}

fn parse_scan_mode(value: &str) -> Option<EventDlqDuplicateAlertObservabilityScanMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "global" | "global_budget" => {
            Some(EventDlqDuplicateAlertObservabilityScanMode::GlobalBudget)
        }
        "fair" | "fair_window" => Some(EventDlqDuplicateAlertObservabilityScanMode::FairWindow),
        "moving" | "moving_window" => {
            Some(EventDlqDuplicateAlertObservabilityScanMode::MovingWindow)
        }
        _ => None,
    }
}

async fn wait_or_stop(delay: Duration, stop_rx: &mut tokio::sync::watch::Receiver<bool>) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(delay) => false,
        changed = stop_rx.changed() => changed.is_err() || *stop_rx.borrow(),
    }
}

#[cfg(test)]
mod tests {
    use rustok_iggy::{
        DlqDuplicateAlertPolicy, DlqDuplicateAlertRuntimePublisher, DlqDuplicateSummary,
    };

    use super::*;

    fn policy() -> DlqDuplicateAlertPolicy {
        DlqDuplicateAlertPolicy::new(2, 4, 2, 3, 3, 5).unwrap()
    }

    #[test]
    fn health_projection_tracks_identifier_free_runtime_transitions() {
        let (mut publisher, subscriber) = DlqDuplicateAlertRuntimePublisher::new(policy());
        let starting = project_runtime(
            EventDlqDuplicateAlertObserverMode::IggyExternal,
            Some(EventDlqDuplicateAlertObservabilityScanMode::MovingWindow),
            Some(subscriber.current()),
            false,
        );
        assert_eq!(
            starting.health.state(),
            EventDlqDuplicateAlertHealthState::Starting
        );
        assert_eq!(starting.health.generation(), Some(0));
        assert!(!starting.health.affects_readiness());

        let available_runtime = publisher.publish(&DlqDuplicateSummary::default()).unwrap();
        let available = project_runtime(
            EventDlqDuplicateAlertObserverMode::IggyExternal,
            Some(EventDlqDuplicateAlertObservabilityScanMode::MovingWindow),
            Some(available_runtime),
            false,
        );
        assert_eq!(
            available.health.state(),
            EventDlqDuplicateAlertHealthState::Available
        );
        assert_eq!(
            available.health.level(),
            Some(DlqDuplicateAlertLevel::Clear)
        );

        let unavailable_runtime = publisher.mark_unavailable().unwrap();
        let unavailable = project_runtime(
            EventDlqDuplicateAlertObserverMode::IggyExternal,
            Some(EventDlqDuplicateAlertObservabilityScanMode::MovingWindow),
            Some(unavailable_runtime),
            false,
        );
        assert_eq!(
            unavailable.health.state(),
            EventDlqDuplicateAlertHealthState::Unavailable
        );
        assert_eq!(unavailable.health.generation(), Some(2));
        assert_eq!(unavailable.health.level(), None);
    }

    #[test]
    fn static_modes_have_no_scan_or_readiness_effect() {
        for (mode, state) in [
            (
                EventDlqDuplicateAlertObserverMode::Disabled,
                EventDlqDuplicateAlertHealthState::Disabled,
            ),
            (
                EventDlqDuplicateAlertObserverMode::Unavailable,
                EventDlqDuplicateAlertHealthState::Unavailable,
            ),
        ] {
            let projection = project_runtime(mode, None, None, false);
            assert_eq!(projection.health.state(), state);
            assert_eq!(projection.health.scan_mode(), None);
            assert!(!projection.health.affects_readiness());
        }
    }

    #[test]
    fn scan_mode_labels_are_bounded() {
        assert_eq!(
            parse_scan_mode("global_budget"),
            Some(EventDlqDuplicateAlertObservabilityScanMode::GlobalBudget)
        );
        assert_eq!(
            parse_scan_mode("fair_window"),
            Some(EventDlqDuplicateAlertObservabilityScanMode::FairWindow)
        );
        assert_eq!(
            parse_scan_mode("moving_window"),
            Some(EventDlqDuplicateAlertObservabilityScanMode::MovingWindow)
        );
        assert_eq!(parse_scan_mode("partition-17"), None);
    }
}
