use std::env;
use std::time::Duration;

use rustok_iggy_connector::migrations::{
    ConsumerPoisonReceiptInspector, ConsumerPoisonReceiptSummary,
};
use rustok_social_graph::index_consumer::SOCIAL_GRAPH_INDEX_CONSUMER_GROUP;
use rustok_telemetry::{consumer_poison_metrics, runtime_consumer_metrics};
use tokio::task::JoinHandle;

use crate::error::{Error, Result};
use crate::services::app_lifecycle::StopHandle;
use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::social_graph_index_worker::social_graph_index_consumer_enabled;

const POLL_ENV: &str = "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_POLL_MS";
const DEFAULT_POLL_MS: u64 = 5_000;
const MAX_POLL_MS: u64 = 300_000;
const METRICS_CONSUMER: &str = "social_graph_index";
const STAGE_POISON_SNAPSHOT: &str = "poison_receipt_snapshot";
const CONFIGURATION_ERROR: &str = "iggy.connector.poison_inspection_configuration_invalid";

pub struct SocialGraphIndexPoisonObserverHandle {
    handle: JoinHandle<()>,
}

impl SocialGraphIndexPoisonObserverHandle {
    pub fn is_ready(&self) -> bool {
        !self.handle.is_finished()
    }
}

/// Starts a count-only observer for connector-owned neutral poison receipts.
///
/// The observer reads one aggregate row for the fixed Social Graph Index consumer group. It never
/// claims, releases, publishes, acknowledges, repairs, retains, or deletes a receipt and never
/// exposes delivery identifiers, source coordinates, payloads, error classifications, publisher
/// identities, or timestamps as metric labels.
pub async fn start_social_graph_index_poison_observer_if_enabled(
    ctx: &ServerRuntimeContext,
) -> Result<()> {
    if !ctx.settings().runtime.runs_background_workers()
        || ctx.shared_contains::<SocialGraphIndexPoisonObserverHandle>()
        || !social_graph_index_consumer_enabled()?
    {
        return Ok(());
    }

    if let Err(error) = consumer_poison_metrics::ensure_registered() {
        tracing::debug!(
            worker = METRICS_CONSUMER,
            error = %error,
            "Consumer poison metrics are unavailable"
        );
    }

    let poll = match poison_poll_interval() {
        Ok(poll) => poll,
        Err(_) => {
            consumer_poison_metrics::record_unavailable(METRICS_CONSUMER);
            runtime_consumer_metrics::record_failure(
                METRICS_CONSUMER,
                STAGE_POISON_SNAPSHOT,
                CONFIGURATION_ERROR,
            );
            tracing::warn!(
                worker = METRICS_CONSUMER,
                error_code = CONFIGURATION_ERROR,
                "Consumer poison observer configuration is invalid; projection remains active"
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
        .expect("StopHandle must exist before poison observer startup")
        .subscribe();
    let inspector = ConsumerPoisonReceiptInspector::new(ctx.db_clone());

    tracing::info!(
        worker = METRICS_CONSUMER,
        poll_ms = duration_millis(poll),
        consumer_group = SOCIAL_GRAPH_INDEX_CONSUMER_GROUP,
        "Starting count-only Social Graph Index poison receipt observer"
    );
    ctx.shared_insert(SocialGraphIndexPoisonObserverHandle {
        handle: tokio::spawn(poison_observer_loop(inspector, poll, stop_rx)),
    });
    Ok(())
}

async fn poison_observer_loop(
    inspector: ConsumerPoisonReceiptInspector,
    poll: Duration,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *stop_rx.borrow() {
            consumer_poison_metrics::record_unavailable(METRICS_CONSUMER);
            return;
        }

        match inspector.summarize(SOCIAL_GRAPH_INDEX_CONSUMER_GROUP).await {
            Ok(summary) => record_summary(&summary),
            Err(error) => {
                consumer_poison_metrics::record_unavailable(METRICS_CONSUMER);
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_POISON_SNAPSHOT,
                    error.stable_code(),
                );
                tracing::warn!(
                    worker = METRICS_CONSUMER,
                    error_code = error.stable_code(),
                    "Count-only poison receipt snapshot failed; projection remains active"
                );
            }
        }

        if wait_or_stop(poll, &mut stop_rx).await {
            consumer_poison_metrics::record_unavailable(METRICS_CONSUMER);
            return;
        }
    }
}

fn record_summary(summary: &ConsumerPoisonReceiptSummary) {
    consumer_poison_metrics::record_snapshot(
        METRICS_CONSUMER,
        summary.total(),
        summary.reserved(),
        summary.publishing(),
        summary.expired_publishing(),
        summary.published(),
        summary.acknowledged(),
    );
    tracing::debug!(
        worker = METRICS_CONSUMER,
        total = summary.total(),
        reserved = summary.reserved(),
        publishing = summary.publishing(),
        expired_publishing = summary.expired_publishing(),
        published = summary.published(),
        acknowledged = summary.acknowledged(),
        "Recorded count-only neutral poison receipt snapshot"
    );
}

fn poison_poll_interval() -> Result<Duration> {
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

async fn wait_or_stop(delay: Duration, stop_rx: &mut tokio::sync::watch::Receiver<bool>) -> bool {
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
