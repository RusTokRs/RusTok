use lazy_static::lazy_static;
use prometheus::{HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts, Registry};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const PAGE_BUILDER_PROVIDER_SOURCE_COMMIT_ENV: &str = "RUSTOK_SOURCE_COMMIT";
pub const PAGE_BUILDER_PROVIDER_OPERATIONS: [&str; 2] = ["preview", "publish"];
pub const PAGE_BUILDER_PROVIDER_OUTCOMES: [&str; 4] = [
    "succeeded",
    "sanitize_failed",
    "runtime_failed",
    "other_failed",
];

lazy_static! {
    /// Runtime source identity for deployment-health target admission.
    ///
    /// The label is emitted only when `RUSTOK_SOURCE_COMMIT` is a canonical 40-character Git SHA.
    /// Missing or malformed source identity therefore leaves the series absent so deployment-health
    /// capture/evaluation can fail closed instead of treating an unknown image as authoritative.
    pub static ref PAGE_BUILDER_PROVIDER_BUILD_INFO: IntGaugeVec = IntGaugeVec::new(
        Opts::new(
            "rustok_page_builder_provider_build_info",
            "Canonical source commit reported by this Page Builder provider target"
        ),
        &["source_commit"],
    )
    .expect("Failed to create Page Builder provider build info gauge");
    pub static ref PAGE_BUILDER_PROVIDER_OPERATION_DURATION_SECONDS: HistogramVec =
        HistogramVec::new(
            HistogramOpts::new(
                "rustok_page_builder_provider_operation_duration_seconds",
                "Canonical Page Builder provider operation duration in seconds"
            )
            .buckets(vec![
                0.05, 0.1, 0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0, 10.0, 15.0,
            ]),
            &["operation"],
        )
        .expect("Failed to create Page Builder provider duration histogram");
    pub static ref PAGE_BUILDER_PROVIDER_OPERATION_COMPLETED_TOTAL: IntCounterVec =
        IntCounterVec::new(
            Opts::new(
                "rustok_page_builder_provider_operation_completed_total",
                "Completed canonical Page Builder provider operations by terminal outcome"
            ),
            &["operation", "outcome"],
        )
        .expect("Failed to create Page Builder provider completion counter");
    pub static ref PAGE_BUILDER_PROVIDER_LAST_OBSERVATION_UNIX_SECONDS: IntGaugeVec =
        IntGaugeVec::new(
            Opts::new(
                "rustok_page_builder_provider_last_observation_unix_seconds",
                "Unix timestamp of the latest canonical Page Builder provider observation"
            ),
            &["operation"],
        )
        .expect("Failed to create Page Builder provider freshness gauge");
}

fn canonical_source_commit(value: &str) -> Option<String> {
    let value = value.trim();
    (value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| value.to_ascii_lowercase())
}

fn deployed_source_commit() -> Option<String> {
    std::env::var(PAGE_BUILDER_PROVIDER_SOURCE_COMMIT_ENV)
        .ok()
        .and_then(|value| canonical_source_commit(&value))
}

pub fn register(registry: &Registry) -> Result<(), prometheus::Error> {
    registry.register(Box::new(PAGE_BUILDER_PROVIDER_BUILD_INFO.clone()))?;
    registry.register(Box::new(
        PAGE_BUILDER_PROVIDER_OPERATION_DURATION_SECONDS.clone(),
    ))?;
    registry.register(Box::new(
        PAGE_BUILDER_PROVIDER_OPERATION_COMPLETED_TOTAL.clone(),
    ))?;
    registry.register(Box::new(
        PAGE_BUILDER_PROVIDER_LAST_OBSERVATION_UNIX_SECONDS.clone(),
    ))?;

    if let Some(source_commit) = deployed_source_commit() {
        PAGE_BUILDER_PROVIDER_BUILD_INFO
            .with_label_values(&[source_commit.as_str()])
            .set(1);
    }

    Ok(())
}

/// Record one terminal canonical Page Builder provider operation in the platform registry.
///
/// Labels are intentionally bounded to the two provider operations and four terminal outcomes.
/// Tenant, page, revision, correlation, host and deployment identifiers are not metric labels.
/// Scrape infrastructure owns target/deployment labels so aggregation can remain operationally
/// bounded. The process itself emits only the canonical source commit build-info series; immutable
/// deployment image digest and expected-target inventory remain execution inputs admitted by the
/// deployment-identity capture contract.
pub fn record_page_builder_provider_operation(
    operation: &'static str,
    outcome: &'static str,
    elapsed: Duration,
) {
    if !PAGE_BUILDER_PROVIDER_OPERATIONS.contains(&operation)
        || !PAGE_BUILDER_PROVIDER_OUTCOMES.contains(&outcome)
    {
        return;
    }

    PAGE_BUILDER_PROVIDER_OPERATION_DURATION_SECONDS
        .with_label_values(&[operation])
        .observe(elapsed.as_secs_f64());
    PAGE_BUILDER_PROVIDER_OPERATION_COMPLETED_TOTAL
        .with_label_values(&[operation, outcome])
        .inc();

    let observed_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64)
        .unwrap_or(0);
    PAGE_BUILDER_PROVIDER_LAST_OBSERVATION_UNIX_SECONDS
        .with_label_values(&[operation])
        .set(observed_at);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_builder_provider_metric_labels_are_bounded() {
        assert_eq!(PAGE_BUILDER_PROVIDER_OPERATIONS, ["preview", "publish"]);
        assert_eq!(
            PAGE_BUILDER_PROVIDER_OUTCOMES,
            [
                "succeeded",
                "sanitize_failed",
                "runtime_failed",
                "other_failed"
            ]
        );
    }

    #[test]
    fn source_commit_identity_is_canonical_and_bounded() {
        let lower = "0123456789abcdef0123456789abcdef01234567";
        let upper = "0123456789ABCDEF0123456789ABCDEF01234567";
        assert_eq!(canonical_source_commit(lower).as_deref(), Some(lower));
        assert_eq!(canonical_source_commit(upper).as_deref(), Some(lower));
        assert!(canonical_source_commit("unknown").is_none());
        assert!(canonical_source_commit("0123456789abcdef").is_none());
    }
}
