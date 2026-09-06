use std::sync::{Mutex, OnceLock};

use prometheus::IntGauge;
use prometheus::core::{Collector, Desc};
use prometheus::proto::MetricFamily;

static TENANT_GENERATION_COLLECTOR: OnceLock<Mutex<Option<TenantGenerationCollector>>> =
    OnceLock::new();

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TenantGenerationListenerMetrics {
    pub status: i64,
    pub local_ready: bool,
    pub subscriber_ready: bool,
    pub reconciliation_healthy: bool,
}

#[derive(Clone)]
struct TenantGenerationCollector {
    status: IntGauge,
    local_ready: IntGauge,
    subscriber_ready: IntGauge,
    reconciliation_healthy: IntGauge,
}

impl TenantGenerationCollector {
    fn new() -> Result<Self, prometheus::Error> {
        Ok(Self {
            status: IntGauge::new(
                "rustok_cache_tenant_generation_listener_status",
                "Tenant generation listener status: 0=disabled, 1=starting, 2=healthy, 3=degraded",
            )?,
            local_ready: IntGauge::new(
                "rustok_cache_tenant_generation_local_ready",
                "Whether the local tenant generation invalidation path is ready",
            )?,
            subscriber_ready: IntGauge::new(
                "rustok_cache_tenant_generation_subscriber_ready",
                "Whether the Redis tenant generation subscriber is ready",
            )?,
            reconciliation_healthy: IntGauge::new(
                "rustok_cache_tenant_generation_reconciliation_healthy",
                "Whether durable tenant generation reconciliation is healthy",
            )?,
        })
    }

    fn update(&self, metrics: TenantGenerationListenerMetrics) {
        self.status.set(metrics.status);
        self.local_ready.set(metrics.local_ready as i64);
        self.subscriber_ready.set(metrics.subscriber_ready as i64);
        self.reconciliation_healthy
            .set(metrics.reconciliation_healthy as i64);
    }
}

impl Collector for TenantGenerationCollector {
    fn desc(&self) -> Vec<&Desc> {
        let mut descriptions = Vec::new();
        descriptions.extend(self.status.desc());
        descriptions.extend(self.local_ready.desc());
        descriptions.extend(self.subscriber_ready.desc());
        descriptions.extend(self.reconciliation_healthy.desc());
        descriptions
    }

    fn collect(&self) -> Vec<MetricFamily> {
        let mut metrics = Vec::new();
        metrics.extend(self.status.collect());
        metrics.extend(self.local_ready.collect());
        metrics.extend(self.subscriber_ready.collect());
        metrics.extend(self.reconciliation_healthy.collect());
        metrics
    }
}

/// Update fixed-cardinality tenant generation listener gauges.
///
/// Registration is lazy because cache services can be constructed before telemetry initialization.
/// Failure to register does not affect cache correctness; a later lifecycle update retries it.
pub fn record_tenant_generation_listener_metrics(metrics: TenantGenerationListenerMetrics) {
    let collector = TENANT_GENERATION_COLLECTOR.get_or_init(|| Mutex::new(None));
    let mut collector = collector
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if collector.is_none() {
        let Ok(candidate) = TenantGenerationCollector::new() else {
            return;
        };
        if rustok_telemetry::register_runtime_collector(Box::new(candidate.clone())).is_ok() {
            *collector = Some(candidate);
        }
    }
    if let Some(collector) = collector.as_ref() {
        collector.update(metrics);
    }
}

pub fn format_tenant_generation_listener_prometheus_metrics(
    metrics: TenantGenerationListenerMetrics,
) -> String {
    format!(
        "rustok_cache_tenant_generation_listener_status {status}\n\
         rustok_cache_tenant_generation_local_ready {local_ready}\n\
         rustok_cache_tenant_generation_subscriber_ready {subscriber_ready}\n\
         rustok_cache_tenant_generation_reconciliation_healthy {reconciliation_healthy}\n",
        status = metrics.status,
        local_ready = metrics.local_ready as u8,
        subscriber_ready = metrics.subscriber_ready as u8,
        reconciliation_healthy = metrics.reconciliation_healthy as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_are_label_free_and_component_specific() {
        let payload =
            format_tenant_generation_listener_prometheus_metrics(TenantGenerationListenerMetrics {
                status: 3,
                local_ready: true,
                subscriber_ready: false,
                reconciliation_healthy: true,
            });

        assert!(payload.contains("rustok_cache_tenant_generation_listener_status 3"));
        assert!(payload.contains("rustok_cache_tenant_generation_local_ready 1"));
        assert!(payload.contains("rustok_cache_tenant_generation_subscriber_ready 0"));
        assert!(payload.contains("rustok_cache_tenant_generation_reconciliation_healthy 1"));
        assert!(!payload.contains('{'));
    }
}
