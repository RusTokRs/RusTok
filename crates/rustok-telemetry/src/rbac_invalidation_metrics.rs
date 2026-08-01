use lazy_static::lazy_static;
use prometheus::{IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry};

lazy_static! {
    /// Durable generation currently stored in PostgreSQL.
    pub static ref RBAC_INVALIDATION_DATABASE_GENERATION: IntGauge = IntGauge::new(
        "rustok_rbac_invalidation_database_generation",
        "Durable RBAC permission invalidation generation stored in PostgreSQL"
    )
    .expect("Failed to create rbac_invalidation_database_generation");

    /// Highest durable generation applied to this process.
    pub static ref RBAC_INVALIDATION_APPLIED_GENERATION: IntGauge = IntGauge::new(
        "rustok_rbac_invalidation_applied_generation",
        "Highest durable RBAC permission invalidation generation applied to this process"
    )
    .expect("Failed to create rbac_invalidation_applied_generation");

    /// Difference between the PostgreSQL and process-applied generations.
    pub static ref RBAC_INVALIDATION_GENERATION_LAG: IntGauge = IntGauge::new(
        "rustok_rbac_invalidation_generation_lag",
        "RBAC permission invalidation generation lag for this process"
    )
    .expect("Failed to create rbac_invalidation_generation_lag");

    /// Whether a supervised invalidation worker is currently running.
    pub static ref RBAC_INVALIDATION_WORKER_RUNNING: IntGaugeVec = IntGaugeVec::new(
        Opts::new(
            "rustok_rbac_invalidation_worker_running",
            "Whether a supervised RBAC invalidation worker is currently running (1=yes, 0=no)"
        ),
        &["worker"]
    )
    .expect("Failed to create rbac_invalidation_worker_running");

    /// Supervised invalidation worker restarts.
    pub static ref RBAC_INVALIDATION_WORKER_RESTARTS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_rbac_invalidation_worker_restarts_total",
            "Total supervised RBAC invalidation worker restarts"
        ),
        &["worker", "reason"]
    )
    .expect("Failed to create rbac_invalidation_worker_restarts_total");

    /// Durable-generation recovery actions.
    pub static ref RBAC_INVALIDATION_RECOVERIES_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_rbac_invalidation_recoveries_total",
            "Total RBAC durable-generation recovery actions"
        ),
        &["reason"]
    )
    .expect("Failed to create rbac_invalidation_recoveries_total");

    /// Fail-safe full permission-snapshot clears.
    pub static ref RBAC_INVALIDATION_FULL_CLEARS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_rbac_invalidation_full_clears_total",
            "Total fail-safe full RBAC permission-snapshot clears"
        ),
        &["reason"]
    )
    .expect("Failed to create rbac_invalidation_full_clears_total");
}

pub fn register(registry: &Registry) -> Result<(), prometheus::Error> {
    registry.register(Box::new(RBAC_INVALIDATION_DATABASE_GENERATION.clone()))?;
    registry.register(Box::new(RBAC_INVALIDATION_APPLIED_GENERATION.clone()))?;
    registry.register(Box::new(RBAC_INVALIDATION_GENERATION_LAG.clone()))?;
    registry.register(Box::new(RBAC_INVALIDATION_WORKER_RUNNING.clone()))?;
    registry.register(Box::new(RBAC_INVALIDATION_WORKER_RESTARTS_TOTAL.clone()))?;
    registry.register(Box::new(RBAC_INVALIDATION_RECOVERIES_TOTAL.clone()))?;
    registry.register(Box::new(RBAC_INVALIDATION_FULL_CLEARS_TOTAL.clone()))?;
    Ok(())
}

pub fn observe_generations(database_generation: u64, applied_generation: Option<u64>) {
    let applied_generation = applied_generation.unwrap_or_default();
    RBAC_INVALIDATION_DATABASE_GENERATION.set(generation_to_i64(database_generation));
    RBAC_INVALIDATION_APPLIED_GENERATION.set(generation_to_i64(applied_generation));
    RBAC_INVALIDATION_GENERATION_LAG.set(generation_to_i64(
        database_generation.saturating_sub(applied_generation),
    ));
}

pub fn observe_applied_generation(generation: u64) {
    RBAC_INVALIDATION_APPLIED_GENERATION.set(generation_to_i64(generation));
}

pub fn set_worker_running(worker: &str, running: bool) {
    RBAC_INVALIDATION_WORKER_RUNNING
        .with_label_values(&[worker])
        .set(if running { 1 } else { 0 });
}

pub fn record_worker_restart(worker: &str, reason: &str) {
    RBAC_INVALIDATION_WORKER_RESTARTS_TOTAL
        .with_label_values(&[worker, reason])
        .inc();
}

pub fn record_recovery(reason: &str) {
    RBAC_INVALIDATION_RECOVERIES_TOTAL
        .with_label_values(&[reason])
        .inc();
}

pub fn record_full_clear(reason: &str) {
    RBAC_INVALIDATION_FULL_CLEARS_TOTAL
        .with_label_values(&[reason])
        .inc();
}

fn generation_to_i64(generation: u64) -> i64 {
    i64::try_from(generation).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_conversion_saturates_at_prometheus_integer_limit() {
        assert_eq!(generation_to_i64(7), 7);
        assert_eq!(generation_to_i64(u64::MAX), i64::MAX);
    }
}
