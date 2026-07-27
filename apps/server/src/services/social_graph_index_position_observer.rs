use std::env;
use std::sync::Arc;
use std::time::Duration;

use rustok_iggy::{IggyConfig, IggyConsumerPositionObserver, IggyTransport};
use rustok_social_graph::index_consumer::{
    SOCIAL_GRAPH_INDEX_CONSUMER_GROUP, SOCIAL_GRAPH_INDEX_TOPIC,
};
use rustok_telemetry::runtime_consumer_metrics;
use tokio::task::JoinHandle;

use crate::error::{Error, Result};
use crate::services::app_lifecycle::StopHandle;
use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::social_graph_index_worker::social_graph_index_consumer_enabled;

const POLL_ENV: &str = "RUSTOK_SOCIAL_GRAPH_INDEX_POSITION_POLL_MS";
const DEFAULT_POLL_MS: u64 = 5_000;
const MAX_POLL_MS: u64 = 300_000;
const METRICS_CONSUMER: &str = "social_graph_index";
const STAGE_POSITION_SNAPSHOT: &str = "position_snapshot";
const POSITION_CONFIG_ERROR: &str = "iggy.consumer_position.configuration_invalid";

pub struct SocialGraphIndexPositionObserverHandle {
    _handle: JoinHandle<()>,
}

impl SocialGraphIndexPositionObserverHandle {
    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }
}

/// Starts a read-only position observer for the explicitly enabled durable consumer.
///
/// The task creates a second SDK client connection to the already-running Iggy endpoint. It never
/// constructs or shuts down an `IggyTransport`, never starts a bundled broker process, and never
/// mutates consumer offsets. Observation failures affect metrics only and do not stop projection.
pub async fn start_social_graph_index_position_observer_if_enabled(
    ctx: &ServerRuntimeContext,
) -> Result<()> {
    if !ctx.settings().runtime.runs_background_workers()
        || ctx.shared_contains::<SocialGraphIndexPositionObserverHandle>()
        || !social_graph_index_consumer_enabled()?
    {
        return Ok(());
    }

    if let Err(error) = runtime_consumer_metrics::ensure_registered() {
        tracing::debug!(
            error = %error,
            worker = METRICS_CONSUMER,
            "Runtime consumer metrics are unavailable for position observation"
        );
    }

    let transport = ctx.shared_get::<Arc<IggyTransport>>().ok_or_else(|| {
        Error::Message(
            "Social Graph Index position observer requires the shared EventRuntime Iggy transport"
                .to_string(),
        )
    })?;
    let config = transport.config().clone();
    let poll = match position_poll_interval() {
        Ok(poll) => poll,
        Err(error) => {
            record_position_unavailable();
            runtime_consumer_metrics::record_failure(
                METRICS_CONSUMER,
                STAGE_POSITION_SNAPSHOT,
                POSITION_CONFIG_ERROR,
            );
            tracing::warn!(
                worker = METRICS_CONSUMER,
                error = %error,
                "Consumer-position observer configuration is invalid; projection remains active"
            );
            return Ok(());
        }
    };

    if !ctx.shared_contains::<StopHandle>() {
        let (stop_handle, _stop_rx) = StopHandle::new();
        ctx.shared_insert(stop_handle);
    }
    let stop_rx = ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must exist before position observer startup")
        .subscribe();

    tracing::info!(
        worker = METRICS_CONSUMER,
        poll_ms = duration_millis(poll),
        consumer_group = SOCIAL_GRAPH_INDEX_CONSUMER_GROUP,
        topic = SOCIAL_GRAPH_INDEX_TOPIC,
        "Starting read-only Social Graph Index consumer-position observer"
    );
    ctx.shared_insert(SocialGraphIndexPositionObserverHandle {
        _handle: tokio::spawn(position_observer_loop(config, poll, stop_rx)),
    });
    Ok(())
}

async fn position_observer_loop(
    config: IggyConfig,
    poll: Duration,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    let mut observer = None;
    loop {
        if *stop_rx.borrow() {
            record_position_unavailable();
            return;
        }

        if observer.is_none() {
            match IggyConsumerPositionObserver::connect(
                &config,
                SOCIAL_GRAPH_INDEX_CONSUMER_GROUP,
                SOCIAL_GRAPH_INDEX_TOPIC,
            )
            .await
            {
                Ok(connected) => observer = Some(connected),
                Err(error) => {
                    record_position_unavailable();
                    runtime_consumer_metrics::record_failure(
                        METRICS_CONSUMER,
                        STAGE_POSITION_SNAPSHOT,
                        error.stable_code(),
                    );
                    tracing::warn!(
                        worker = METRICS_CONSUMER,
                        error_code = error.stable_code(),
                        error = %error,
                        "Consumer-position observer could not connect; projection remains active"
                    );
                }
            }
        }

        let snapshot_result = match observer.as_ref() {
            Some(connected) => Some(connected.snapshot().await),
            None => None,
        };
        if let Some(snapshot_result) = snapshot_result {
            match snapshot_result {
                Ok(snapshot) => {
                    let total_lag = snapshot.total_lag();
                    let max_lag = snapshot.max_lag();
                    runtime_consumer_metrics::record_position_snapshot(
                        METRICS_CONSUMER,
                        snapshot.captured_at_unix_seconds,
                        snapshot.partition_count(),
                        total_lag,
                        max_lag,
                    );
                    tracing::debug!(
                        worker = METRICS_CONSUMER,
                        partitions = snapshot.partition_count(),
                        complete = snapshot.is_complete(),
                        total_lag = ?total_lag,
                        max_lag = ?max_lag,
                        "Recorded partition-qualified Social Graph Index consumer position"
                    );
                }
                Err(error) => {
                    record_position_unavailable();
                    runtime_consumer_metrics::record_failure(
                        METRICS_CONSUMER,
                        STAGE_POSITION_SNAPSHOT,
                        error.stable_code(),
                    );
                    tracing::warn!(
                        worker = METRICS_CONSUMER,
                        error_code = error.stable_code(),
                        error = %error,
                        "Consumer-position snapshot failed; reconnecting observer without stopping projection"
                    );
                    observer = None;
                }
            }
        }

        if wait_or_stop(poll, &mut stop_rx).await {
            record_position_unavailable();
            return;
        }
    }
}

fn record_position_unavailable() {
    runtime_consumer_metrics::record_position_snapshot(METRICS_CONSUMER, 0, 0, None, None);
}

fn position_poll_interval() -> Result<Duration> {
    let value = match env::var(POLL_ENV) {
        Ok(value) => value
            .parse::<u64>()
            .map_err(|error| Error::Message(format!("{POLL_ENV} is invalid: {error}")))?,
        Err(env::VarError::NotPresent) => DEFAULT_POLL_MS,
        Err(error) => {
            return Err(Error::Message(format!(
                "failed to read {POLL_ENV}: {error}"
            )));
        }
    };
    if value == 0 || value > MAX_POLL_MS {
        return Err(Error::Message(format!(
            "{POLL_ENV} must be between 1 and {MAX_POLL_MS}"
        )));
    }
    Ok(Duration::from_millis(value))
}

async fn wait_or_stop(
    delay: Duration,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(delay) => false,
        changed = stop_rx.changed() => changed.is_err() || *stop_rx.borrow(),
    }
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_interval_is_bounded() {
        assert_eq!(duration_millis(Duration::from_secs(5)), 5_000);
        assert_eq!(MAX_POLL_MS, 300_000);
    }
}
