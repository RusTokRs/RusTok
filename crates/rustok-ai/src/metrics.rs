#![cfg(feature = "server")]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use once_cell::sync::Lazy;
use rustok_telemetry::metrics as telemetry_metrics;
use serde::{Deserialize, Serialize};

use crate::model::{AiRunDecisionTrace, ExecutionMode, ProviderKind};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AiMetricBucket {
    pub label: String,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AiRuntimeMetricsSnapshot {
    pub router_resolutions_total: u64,
    pub router_overrides_total: u64,
    pub selected_auto_total: u64,
    pub selected_direct_total: u64,
    pub selected_mcp_total: u64,
    pub completed_runs_total: u64,
    pub failed_runs_total: u64,
    pub waiting_approval_runs_total: u64,
    pub locale_fallback_total: u64,
    pub run_latency_ms_total: u64,
    pub run_latency_samples: u64,
    pub provider_kind_totals: Vec<AiMetricBucket>,
    pub execution_target_totals: Vec<AiMetricBucket>,
    pub task_profile_totals: Vec<AiMetricBucket>,
    pub resolved_locale_totals: Vec<AiMetricBucket>,
}

static AI_ROUTER_RESOLUTIONS_TOTAL: AtomicU64 = AtomicU64::new(0);
static AI_ROUTER_OVERRIDES_TOTAL: AtomicU64 = AtomicU64::new(0);
static AI_SELECTED_AUTO_TOTAL: AtomicU64 = AtomicU64::new(0);
static AI_SELECTED_DIRECT_TOTAL: AtomicU64 = AtomicU64::new(0);
static AI_SELECTED_MCP_TOTAL: AtomicU64 = AtomicU64::new(0);
static AI_COMPLETED_RUNS_TOTAL: AtomicU64 = AtomicU64::new(0);
static AI_FAILED_RUNS_TOTAL: AtomicU64 = AtomicU64::new(0);
static AI_WAITING_APPROVAL_RUNS_TOTAL: AtomicU64 = AtomicU64::new(0);
static AI_LOCALE_FALLBACK_TOTAL: AtomicU64 = AtomicU64::new(0);
static AI_RUN_LATENCY_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static AI_RUN_LATENCY_SAMPLES: AtomicU64 = AtomicU64::new(0);

static AI_PROVIDER_KIND_TOTALS: Lazy<Mutex<BTreeMap<String, u64>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));
static AI_EXECUTION_TARGET_TOTALS: Lazy<Mutex<BTreeMap<String, u64>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));
static AI_TASK_PROFILE_TOTALS: Lazy<Mutex<BTreeMap<String, u64>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));
static AI_RESOLVED_LOCALE_TOTALS: Lazy<Mutex<BTreeMap<String, u64>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

pub fn observe_router_resolution(entry_point: &str, trace: &AiRunDecisionTrace) {
    AI_ROUTER_RESOLUTIONS_TOTAL.fetch_add(1, Ordering::Relaxed);
    if trace.used_override {
        AI_ROUTER_OVERRIDES_TOTAL.fetch_add(1, Ordering::Relaxed);
    }

    match trace.execution_mode {
        Some(ExecutionMode::Auto) => AI_SELECTED_AUTO_TOTAL.fetch_add(1, Ordering::Relaxed),
        Some(ExecutionMode::Direct) => AI_SELECTED_DIRECT_TOTAL.fetch_add(1, Ordering::Relaxed),
        Some(ExecutionMode::McpTooling) => AI_SELECTED_MCP_TOTAL.fetch_add(1, Ordering::Relaxed),
        None => 0,
    };

    if let Some(kind) = trace.provider_kind {
        increment_bucket(&AI_PROVIDER_KIND_TOTALS, kind.slug());
    }

    telemetry_metrics::record_module_entrypoint_call(
        "ai",
        entry_point,
        trace
            .execution_mode
            .map(ExecutionMode::slug)
            .unwrap_or("unknown"),
    );
}

pub fn observe_locale_resolution(requested_locale: Option<&str>, resolved_locale: &str) {
    if requested_locale.is_some_and(|value| !locale_tags_match(value, resolved_locale)) {
        AI_LOCALE_FALLBACK_TOTAL.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn observe_run_outcome(
    execution_mode: ExecutionMode,
    execution_target: Option<&str>,
    provider_kind: ProviderKind,
    task_profile_slug: Option<&str>,
    resolved_locale: Option<&str>,
    status: &str,
    latency_ms: u64,
) {
    AI_RUN_LATENCY_MS_TOTAL.fetch_add(latency_ms, Ordering::Relaxed);
    AI_RUN_LATENCY_SAMPLES.fetch_add(1, Ordering::Relaxed);
    increment_bucket(&AI_PROVIDER_KIND_TOTALS, provider_kind.slug());
    increment_bucket(
        &AI_EXECUTION_TARGET_TOTALS,
        execution_target.unwrap_or_else(|| execution_mode.slug()),
    );
    if let Some(task_profile_slug) = task_profile_slug.filter(|value| !value.trim().is_empty()) {
        increment_bucket(&AI_TASK_PROFILE_TOTALS, task_profile_slug);
    }
    if let Some(resolved_locale) = resolved_locale.filter(|value| !value.trim().is_empty()) {
        increment_bucket(&AI_RESOLVED_LOCALE_TOTALS, resolved_locale);
    }

    let operation = match execution_mode {
        ExecutionMode::Auto => "ai.run.auto",
        ExecutionMode::Direct => "ai.run.direct",
        ExecutionMode::McpTooling => "ai.run.mcp_tooling",
    };
    telemetry_metrics::record_span_duration(operation, latency_ms as f64 / 1000.0);

    match status {
        "completed" => {
            AI_COMPLETED_RUNS_TOTAL.fetch_add(1, Ordering::Relaxed);
            telemetry_metrics::record_module_entrypoint_call(
                "ai",
                "run_completed",
                execution_mode.slug(),
            );
        }
        "failed" => {
            AI_FAILED_RUNS_TOTAL.fetch_add(1, Ordering::Relaxed);
            telemetry_metrics::record_module_error("ai", "run_failed", "error");
            telemetry_metrics::record_span_error(operation, "run_failed");
        }
        "waiting_approval" => {
            AI_WAITING_APPROVAL_RUNS_TOTAL.fetch_add(1, Ordering::Relaxed);
            telemetry_metrics::record_module_entrypoint_call(
                "ai",
                "run_waiting_approval",
                execution_mode.slug(),
            );
        }
        _ => {}
    }
}

pub fn metrics_snapshot() -> AiRuntimeMetricsSnapshot {
    AiRuntimeMetricsSnapshot {
        router_resolutions_total: AI_ROUTER_RESOLUTIONS_TOTAL.load(Ordering::Relaxed),
        router_overrides_total: AI_ROUTER_OVERRIDES_TOTAL.load(Ordering::Relaxed),
        selected_auto_total: AI_SELECTED_AUTO_TOTAL.load(Ordering::Relaxed),
        selected_direct_total: AI_SELECTED_DIRECT_TOTAL.load(Ordering::Relaxed),
        selected_mcp_total: AI_SELECTED_MCP_TOTAL.load(Ordering::Relaxed),
        completed_runs_total: AI_COMPLETED_RUNS_TOTAL.load(Ordering::Relaxed),
        failed_runs_total: AI_FAILED_RUNS_TOTAL.load(Ordering::Relaxed),
        waiting_approval_runs_total: AI_WAITING_APPROVAL_RUNS_TOTAL.load(Ordering::Relaxed),
        locale_fallback_total: AI_LOCALE_FALLBACK_TOTAL.load(Ordering::Relaxed),
        run_latency_ms_total: AI_RUN_LATENCY_MS_TOTAL.load(Ordering::Relaxed),
        run_latency_samples: AI_RUN_LATENCY_SAMPLES.load(Ordering::Relaxed),
        provider_kind_totals: snapshot_buckets(&AI_PROVIDER_KIND_TOTALS),
        execution_target_totals: snapshot_buckets(&AI_EXECUTION_TARGET_TOTALS),
        task_profile_totals: snapshot_buckets(&AI_TASK_PROFILE_TOTALS),
        resolved_locale_totals: snapshot_buckets(&AI_RESOLVED_LOCALE_TOTALS),
    }
}

#[cfg(test)]
pub fn reset_metrics_for_tests() {
    AI_ROUTER_RESOLUTIONS_TOTAL.store(0, Ordering::Relaxed);
    AI_ROUTER_OVERRIDES_TOTAL.store(0, Ordering::Relaxed);
    AI_SELECTED_AUTO_TOTAL.store(0, Ordering::Relaxed);
    AI_SELECTED_DIRECT_TOTAL.store(0, Ordering::Relaxed);
    AI_SELECTED_MCP_TOTAL.store(0, Ordering::Relaxed);
    AI_COMPLETED_RUNS_TOTAL.store(0, Ordering::Relaxed);
    AI_FAILED_RUNS_TOTAL.store(0, Ordering::Relaxed);
    AI_WAITING_APPROVAL_RUNS_TOTAL.store(0, Ordering::Relaxed);
    AI_LOCALE_FALLBACK_TOTAL.store(0, Ordering::Relaxed);
    AI_RUN_LATENCY_MS_TOTAL.store(0, Ordering::Relaxed);
    AI_RUN_LATENCY_SAMPLES.store(0, Ordering::Relaxed);
    AI_PROVIDER_KIND_TOTALS
        .lock()
        .expect("provider kind metrics")
        .clear();
    AI_EXECUTION_TARGET_TOTALS
        .lock()
        .expect("execution target metrics")
        .clear();
    AI_TASK_PROFILE_TOTALS
        .lock()
        .expect("task profile metrics")
        .clear();
    AI_RESOLVED_LOCALE_TOTALS
        .lock()
        .expect("resolved locale metrics")
        .clear();
}

fn increment_bucket(store: &Lazy<Mutex<BTreeMap<String, u64>>>, label: &str) {
    let mut guard = store.lock().expect("AI metrics bucket store");
    *guard.entry(label.to_string()).or_insert(0) += 1;
}

fn snapshot_buckets(store: &Lazy<Mutex<BTreeMap<String, u64>>>) -> Vec<AiMetricBucket> {
    store
        .lock()
        .expect("AI metrics bucket snapshot")
        .iter()
        .map(|(label, total)| AiMetricBucket {
            label: label.clone(),
            total: *total,
        })
        .collect()
}

fn locale_tags_match(left: &str, right: &str) -> bool {
    normalize_locale_tag(left).eq_ignore_ascii_case(&normalize_locale_tag(right))
}

fn normalize_locale_tag(value: &str) -> String {
    value.trim().replace('_', "-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AiRunDecisionTrace, ExecutionMode, ProviderKind};

    #[test]
    fn metrics_snapshot_tracks_router_and_outcomes() {
        reset_metrics_for_tests();
        observe_router_resolution(
            "run_task_job",
            &AiRunDecisionTrace {
                provider_kind: Some(ProviderKind::Gemini),
                execution_mode: Some(ExecutionMode::Direct),
                used_override: true,
                ..AiRunDecisionTrace::default()
            },
        );
        observe_locale_resolution(Some("pt_BR"), "pt-BR");
        observe_locale_resolution(Some("fr"), "en");
        observe_run_outcome(
            ExecutionMode::Direct,
            Some("direct:media"),
            ProviderKind::Gemini,
            Some("image_asset"),
            Some("fr"),
            "completed",
            125,
        );
        observe_run_outcome(
            ExecutionMode::McpTooling,
            Some("mcp:rustok-mcp"),
            ProviderKind::Anthropic,
            Some("operator_chat"),
            Some("en"),
            "waiting_approval",
            40,
        );

        let snapshot = metrics_snapshot();
        assert_eq!(snapshot.router_resolutions_total, 1);
        assert_eq!(snapshot.router_overrides_total, 1);
        assert_eq!(snapshot.selected_direct_total, 1);
        assert_eq!(snapshot.completed_runs_total, 1);
        assert_eq!(snapshot.waiting_approval_runs_total, 1);
        assert_eq!(snapshot.locale_fallback_total, 1);
        assert_eq!(snapshot.run_latency_ms_total, 165);
        assert_eq!(snapshot.run_latency_samples, 2);
        assert!(snapshot
            .provider_kind_totals
            .iter()
            .any(|bucket| bucket.label == "gemini" && bucket.total == 2));
        assert!(snapshot
            .execution_target_totals
            .iter()
            .any(|bucket| bucket.label == "direct:media" && bucket.total == 1));
        assert!(snapshot
            .task_profile_totals
            .iter()
            .any(|bucket| bucket.label == "image_asset" && bucket.total == 1));
        assert!(snapshot
            .resolved_locale_totals
            .iter()
            .any(|bucket| bucket.label == "fr" && bucket.total == 1));
    }
}
