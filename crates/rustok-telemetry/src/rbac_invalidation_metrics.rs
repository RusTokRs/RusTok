use lazy_static::lazy_static;
use prometheus::{IntCounter, IntCounterVec, IntGauge, Opts, Registry};

lazy_static! {
    /// Durable RBAC invalidation generation read from the database source of truth.
    pub static ref RBAC_INVALIDATION_DURABLE_GENERATION: IntGauge = IntGauge::new(
        "rustok_rbac_invalidation_durable_generation",
        "Durable RBAC invalidation generation read from the database source of truth"
    )
    .expect("Failed to create rbac_invalidation_durable_generation");

    /// RBAC invalidation generation already applied to this process.
    pub static ref RBAC_INVALIDATION_APPLIED_GENERATION: IntGauge = IntGauge::new(
        "rustok_rbac_invalidation_applied_generation",
        "RBAC invalidation generation already applied to this process"
    )
    .expect("Failed to create rbac_invalidation_applied_generation");

    /// Signed durable-minus-applied generation lag. Negative values indicate regression.
    pub static ref RBAC_INVALIDATION_GENERATION_LAG: IntGauge = IntGauge::new(
        "rustok_rbac_invalidation_generation_lag",
        "Signed durable minus applied RBAC invalidation generation lag; negative means database regression"
    )
    .expect("Failed to create rbac_invalidation_generation_lag");

    /// Whether the durable RBAC invalidation watchdog worker is currently running.
    pub static ref RBAC_INVALIDATION_WATCHDOG_RUNNING: IntGauge = IntGauge::new(
        "rustok_rbac_invalidation_watchdog_running",
        "Whether the durable RBAC invalidation watchdog worker is running (1=yes, 0=no)"
    )
    .expect("Failed to create rbac_invalidation_watchdog_running");

    /// Total durable generation database read failures.
    pub static ref RBAC_INVALIDATION_DATABASE_READ_ERRORS_TOTAL: IntCounter = IntCounter::new(
        "rustok_rbac_invalidation_database_read_errors_total",
        "Total failures reading the durable RBAC invalidation generation"
    )
    .expect("Failed to create rbac_invalidation_database_read_errors_total");

    /// Total watchdog restarts by bounded reason.
    pub static ref RBAC_INVALIDATION_WATCHDOG_RESTARTS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_rbac_invalidation_watchdog_restarts_total",
            "Total durable RBAC invalidation watchdog restarts by reason"
        ),
        &["reason"]
    )
    .expect("Failed to create rbac_invalidation_watchdog_restarts_total");

    /// Total durable-generation recovery actions by bounded reason.
    pub static ref RBAC_INVALIDATION_RECOVERIES_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_rbac_invalidation_recoveries_total",
            "Total RBAC durable-generation recovery actions by reason"
        ),
        &["reason"]
    )
    .expect("Failed to create rbac_invalidation_recoveries_total");

    /// Total process-wide permission snapshot clears by bounded recovery reason.
    pub static ref RBAC_INVALIDATION_FULL_CLEARS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_rbac_invalidation_full_clears_total",
            "Total process-wide RBAC permission snapshot clears by recovery reason"
        ),
        &["reason"]
    )
    .expect("Failed to create rbac_invalidation_full_clears_total");
}

pub fn register(registry: &Registry) -> Result<(), prometheus::Error> {
    registry.register(Box::new(RBAC_INVALIDATION_DURABLE_GENERATION.clone()))?;
    registry.register(Box::new(RBAC_INVALIDATION_APPLIED_GENERATION.clone()))?;
    registry.register(Box::new(RBAC_INVALIDATION_GENERATION_LAG.clone()))?;
    registry.register(Box::new(RBAC_INVALIDATION_WATCHDOG_RUNNING.clone()))?;
    registry.register(Box::new(
        RBAC_INVALIDATION_DATABASE_READ_ERRORS_TOTAL.clone(),
    ))?;
    registry.register(Box::new(RBAC_INVALIDATION_WATCHDOG_RESTARTS_TOTAL.clone()))?;
    registry.register(Box::new(RBAC_INVALIDATION_RECOVERIES_TOTAL.clone()))?;
    registry.register(Box::new(RBAC_INVALIDATION_FULL_CLEARS_TOTAL.clone()))?;
    Ok(())
}

fn generation_as_i128(generation: u64) -> i128 {
    i128::from(generation)
}

pub fn signed_generation_lag(durable: u64, applied: Option<u64>) -> i64 {
    let lag = generation_as_i128(durable) - generation_as_i128(applied.unwrap_or(0));
    lag.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn generation_as_i64(generation: u64) -> i64 {
    generation.min(i64::MAX as u64) as i64
}

pub fn update_generations(durable: u64, applied: Option<u64>) {
    RBAC_INVALIDATION_DURABLE_GENERATION.set(generation_as_i64(durable));
    RBAC_INVALIDATION_APPLIED_GENERATION.set(generation_as_i64(applied.unwrap_or(0)));
    RBAC_INVALIDATION_GENERATION_LAG.set(signed_generation_lag(durable, applied));
}

pub fn set_watchdog_running(running: bool) {
    RBAC_INVALIDATION_WATCHDOG_RUNNING.set(if running { 1 } else { 0 });
}

pub fn record_database_read_error() {
    RBAC_INVALIDATION_DATABASE_READ_ERRORS_TOTAL.inc();
}

pub fn record_watchdog_restart(reason: &'static str) {
    RBAC_INVALIDATION_WATCHDOG_RESTARTS_TOTAL
        .with_label_values(&[reason])
        .inc();
}

pub fn record_recovery(reason: &'static str) {
    RBAC_INVALIDATION_RECOVERIES_TOTAL
        .with_label_values(&[reason])
        .inc();
}

pub fn record_full_clear(reason: &'static str) {
    RBAC_INVALIDATION_FULL_CLEARS_TOTAL
        .with_label_values(&[reason])
        .inc();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_lag_distinguishes_catch_up_and_regression() {
        assert_eq!(signed_generation_lag(7, None), 7);
        assert_eq!(signed_generation_lag(7, Some(5)), 2);
        assert_eq!(signed_generation_lag(7, Some(7)), 0);
        assert_eq!(signed_generation_lag(5, Some(7)), -2);
    }

    #[test]
    fn registration_exposes_the_bounded_metric_families() {
        let registry = Registry::new();
        register(&registry).expect("RBAC invalidation metrics must register");
        update_generations(9, Some(7));
        set_watchdog_running(true);
        record_database_read_error();
        record_watchdog_restart("panic");
        record_recovery("generation_advanced");
        record_full_clear("generation_advanced");

        let names = registry
            .gather()
            .into_iter()
            .map(|family| family.name().to_string())
            .collect::<Vec<_>>();

        for expected in [
            "rustok_rbac_invalidation_durable_generation",
            "rustok_rbac_invalidation_applied_generation",
            "rustok_rbac_invalidation_generation_lag",
            "rustok_rbac_invalidation_watchdog_running",
            "rustok_rbac_invalidation_database_read_errors_total",
            "rustok_rbac_invalidation_watchdog_restarts_total",
            "rustok_rbac_invalidation_recoveries_total",
            "rustok_rbac_invalidation_full_clears_total",
        ] {
            assert!(
                names.iter().any(|name| name == expected),
                "missing {expected}"
            );
        }
    }
}
