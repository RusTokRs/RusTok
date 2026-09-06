use crate::{CacheGenerationStats, CacheRefreshStats};

/// Render bounded stale-refresh metrics without labels or user-controlled cardinality.
pub fn format_cache_refresh_prometheus_metrics(stats: &CacheRefreshStats) -> String {
    format!(
        "rustok_cache_refresh_started_total {started}\n\
         rustok_cache_refresh_completed_total {completed}\n\
         rustok_cache_refresh_failed_total {failed}\n\
         rustok_cache_refresh_deduplicated_total {deduplicated}\n\
         rustok_cache_refresh_saturated_total {saturated}\n\
         rustok_cache_refresh_rejected_total {rejected}\n\
         rustok_cache_refresh_runtime_unavailable_total {runtime_unavailable}\n\
         rustok_cache_refresh_in_flight {in_flight}\n",
        started = stats.started,
        completed = stats.completed,
        failed = stats.failed,
        deduplicated = stats.deduplicated,
        saturated = stats.saturated,
        rejected = stats.rejected,
        runtime_unavailable = stats.runtime_unavailable,
        in_flight = stats.in_flight,
    )
}

/// Render shared namespace-generation metrics without namespace labels.
///
/// Per-namespace labels are deliberately excluded because namespaces can be module-defined and
/// would otherwise create unbounded Prometheus cardinality.
pub fn format_cache_generation_prometheus_metrics(stats: &CacheGenerationStats) -> String {
    format!(
        "rustok_cache_generation_shared_reads_total {shared_reads}\n\
         rustok_cache_generation_shared_bumps_total {shared_bumps}\n\
         rustok_cache_generation_read_failures_total {read_failures}\n\
         rustok_cache_generation_bump_failures_total {bump_failures}\n\
         rustok_cache_generation_local_fallback_reads_total {local_fallback_reads}\n",
        shared_reads = stats.shared_reads,
        shared_bumps = stats.shared_bumps,
        read_failures = stats.read_failures,
        bump_failures = stats.bump_failures,
        local_fallback_reads = stats.local_fallback_reads,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_metrics_include_capacity_and_failure_signals() {
        let metrics = format_cache_refresh_prometheus_metrics(&CacheRefreshStats {
            started: 5,
            completed: 3,
            failed: 1,
            deduplicated: 4,
            saturated: 2,
            rejected: 2,
            runtime_unavailable: 1,
            in_flight: 1,
        });

        assert!(metrics.contains("rustok_cache_refresh_started_total 5"));
        assert!(metrics.contains("rustok_cache_refresh_failed_total 1"));
        assert!(metrics.contains("rustok_cache_refresh_saturated_total 2"));
        assert!(metrics.contains("rustok_cache_refresh_rejected_total 2"));
        assert!(metrics.contains("rustok_cache_refresh_in_flight 1"));
    }

    #[test]
    fn generation_metrics_include_shared_and_degraded_signals() {
        let metrics = format_cache_generation_prometheus_metrics(&CacheGenerationStats {
            shared_reads: 8,
            shared_bumps: 2,
            read_failures: 3,
            bump_failures: 1,
            local_fallback_reads: 3,
        });

        assert!(metrics.contains("rustok_cache_generation_shared_reads_total 8"));
        assert!(metrics.contains("rustok_cache_generation_bump_failures_total 1"));
        assert!(metrics.contains("rustok_cache_generation_local_fallback_reads_total 3"));
    }
}
