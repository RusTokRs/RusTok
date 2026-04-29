use axum::{
    extract::State,
    http::{header::CONTENT_TYPE, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use loco_rs::{app::AppContext, controller::Routes};

use crate::error::Result;
use rustok_outbox::entity::{Column as SysEventsColumn, Entity as SysEventsEntity, SysEventStatus};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, PaginatorTrait, QueryFilter, Statement,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::middleware::rate_limit::{
    SharedApiRateLimiter, SharedAuthRateLimiter, SharedOAuthRateLimiter,
};
use crate::middleware::tenant::tenant_cache_stats;
use crate::services::auth_lifecycle::AuthLifecycleService;
use crate::services::rbac_consistency::{load_rbac_consistency_stats, RbacConsistencyStats};
use crate::services::rbac_service::{RbacResolverMetricsSnapshot, RbacService};
use crate::services::runtime_guardrails::{
    collect_runtime_guardrail_snapshot, RuntimeGuardrailSnapshot,
};
use rustok_telemetry::metrics::update_queue_depth;
use tracing::warn;

static RBAC_CONSISTENCY_QUERY_FAILURES_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_CONSISTENCY_QUERY_LATENCY_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_CONSISTENCY_QUERY_LATENCY_SAMPLES: AtomicU64 = AtomicU64::new(0);

/// GET /metrics - Prometheus metrics endpoint
#[utoipa::path(
    get,
    path = "/metrics",
    tag = "observability",
    responses(
        (status = 200, description = "Prometheus metrics in text format", content_type = "text/plain"),
        (status = 503, description = "Metrics collection disabled")
    )
)]
pub async fn metrics(State(ctx): State<AppContext>) -> Result<Response> {
    match rustok_telemetry::metrics_handle() {
        Some(handle) => {
            sync_rate_limit_metrics(&ctx).await;
            let mut payload = handle.render();
            payload.push('\n');
            payload.push_str(&render_tenant_cache_metrics(&ctx).await);
            payload.push_str(&render_outbox_metrics(&ctx).await);
            payload.push_str(&render_auth_lifecycle_metrics());
            payload.push_str(&render_rbac_metrics(&ctx).await);
            payload.push_str(&render_search_metrics(&ctx).await);
            payload.push_str(&render_runtime_guardrail_metrics(&ctx).await);

            Ok((
                StatusCode::OK,
                [(CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
                payload,
            )
                .into_response())
        }
        None => Ok((StatusCode::SERVICE_UNAVAILABLE, "metrics disabled").into_response()),
    }
}

pub fn routes() -> Routes {
    Routes::new().prefix("metrics").add("/", get(metrics))
}

async fn sync_rate_limit_metrics(ctx: &AppContext) {
    if let Some(shared) = ctx.shared_store.get::<SharedApiRateLimiter>() {
        if let Err(error) = shared.0.sync_runtime_metrics().await {
            warn!(error = %error, "failed to sync API rate-limit metrics");
        }
    }

    if let Some(shared) = ctx.shared_store.get::<SharedAuthRateLimiter>() {
        if let Err(error) = shared.0.sync_runtime_metrics().await {
            warn!(error = %error, "failed to sync auth rate-limit metrics");
        }
    }

    if let Some(shared) = ctx.shared_store.get::<SharedOAuthRateLimiter>() {
        if let Err(error) = shared.0.sync_runtime_metrics().await {
            warn!(error = %error, "failed to sync oauth rate-limit metrics");
        }
    }
}

async fn render_tenant_cache_metrics(ctx: &AppContext) -> String {
    let stats = tenant_cache_stats(ctx).await;
    format!(
        "rustok_tenant_cache_hits {hits}\n\
rustok_tenant_cache_misses {misses}\n\
rustok_tenant_cache_evictions {evictions}\n\
rustok_tenant_cache_entries {entries}\n\
rustok_tenant_cache_negative_hits {negative_hits}\n\
rustok_tenant_cache_negative_misses {negative_misses}\n\
rustok_tenant_cache_negative_evictions {negative_evictions}\n\
rustok_tenant_cache_negative_entries {negative_entries}\n\
rustok_tenant_cache_negative_inserts {negative_inserts}\n\
rustok_tenant_invalidation_listener_status {invalidation_listener_status}\n",
        hits = stats.hits,
        misses = stats.misses,
        evictions = stats.evictions,
        entries = stats.entries,
        negative_hits = stats.negative_hits,
        negative_misses = stats.negative_misses,
        negative_evictions = stats.negative_evictions,
        negative_entries = stats.negative_entries,
        negative_inserts = stats.negative_inserts,
        invalidation_listener_status = stats.invalidation_listener_status,
    )
}

async fn render_runtime_guardrail_metrics(ctx: &AppContext) -> String {
    let snapshot = collect_runtime_guardrail_snapshot(ctx).await;
    format_runtime_guardrail_metrics(&snapshot)
}

fn format_runtime_guardrail_metrics(snapshot: &RuntimeGuardrailSnapshot) -> String {
    let queue_depth = snapshot.event_bus.current_depth as i64;
    update_queue_depth("server_event_bus", queue_depth);
    let mut rate_limit_lines = String::new();
    for limiter in &snapshot.rate_limits {
        rate_limit_lines.push_str(&format!(
            "rustok_runtime_guardrail_rate_limit_backend_healthy{{namespace=\"{namespace}\",backend=\"{backend}\"}} {healthy}\n\
rustok_runtime_guardrail_rate_limit_state{{namespace=\"{namespace}\"}} {state}\n\
rustok_runtime_guardrail_rate_limit_total_entries{{namespace=\"{namespace}\"}} {total_entries}\n\
rustok_runtime_guardrail_rate_limit_active_clients{{namespace=\"{namespace}\"}} {active_clients}\n\
rustok_runtime_guardrail_rate_limit_config{{namespace=\"{namespace}\",setting=\"enabled\"}} {enabled}\n\
rustok_runtime_guardrail_rate_limit_config{{namespace=\"{namespace}\",setting=\"max_requests\"}} {max_requests}\n\
rustok_runtime_guardrail_rate_limit_config{{namespace=\"{namespace}\",setting=\"window_seconds\"}} {window_seconds}\n\
rustok_runtime_guardrail_rate_limit_config{{namespace=\"{namespace}\",setting=\"trusted_auth_dimensions\"}} {trusted_auth_dimensions}\n\
rustok_runtime_guardrail_rate_limit_config{{namespace=\"{namespace}\",setting=\"memory_warning_entries\"}} {memory_warning_entries}\n\
rustok_runtime_guardrail_rate_limit_config{{namespace=\"{namespace}\",setting=\"memory_critical_entries\"}} {memory_critical_entries}\n",
            namespace = limiter.namespace,
            backend = limiter.backend,
            healthy = if limiter.healthy { 1 } else { 0 },
            state = limiter.state.metric_value(),
            total_entries = limiter.total_entries,
            active_clients = limiter.active_clients,
            enabled = if limiter.policy.enabled { 1 } else { 0 },
            max_requests = limiter.policy.max_requests,
            window_seconds = limiter.policy.window_seconds,
            trusted_auth_dimensions = if limiter.policy.trusted_auth_dimensions {
                1
            } else {
                0
            },
            memory_warning_entries = limiter.policy.memory_warning_entries,
            memory_critical_entries = limiter.policy.memory_critical_entries,
        ));
    }

    let mut payload = format!(
        "rustok_runtime_guardrail_rollout_mode {rollout_mode}\n\
rustok_runtime_guardrail_observed_status {observed_status}\n\
rustok_runtime_guardrail_status {overall_status}\n\
rustok_runtime_guardrail_runtime_dependencies_enabled {runtime_dependencies_enabled}\n\
rustok_runtime_guardrail_host_mode{{mode=\"{host_mode}\"}} 1\n\
rustok_runtime_guardrail_event_transport_fallback_active {relay_fallback_active}\n\
rustok_runtime_guardrail_remote_executor_enabled {remote_executor_enabled}\n\
rustok_runtime_guardrail_remote_executor_token_configured {remote_executor_token_configured}\n\
rustok_runtime_guardrail_remote_executor_state {remote_executor_state}\n\
rustok_runtime_guardrail_remote_executor_active_claims {remote_executor_active_claims}\n\
rustok_runtime_guardrail_remote_executor_expired_claims {remote_executor_expired_claims}\n\
rustok_runtime_guardrail_remote_executor_config{{setting=\"lease_ttl_ms\"}} {remote_executor_lease_ttl_ms}\n\
rustok_runtime_guardrail_remote_executor_config{{setting=\"requeue_scan_interval_ms\"}} {remote_executor_requeue_scan_interval_ms}\n\
rustok_runtime_guardrail_event_backpressure_enabled {backpressure_enabled}\n\
rustok_runtime_guardrail_event_backpressure_state {backpressure_state}\n\
rustok_runtime_guardrail_event_backpressure_current_depth {current_depth}\n\
rustok_runtime_guardrail_event_backpressure_max_depth {max_depth}\n\
rustok_runtime_guardrail_event_backpressure_rejected_total {events_rejected}\n\
rustok_runtime_guardrail_event_backpressure_warning_total {warning_count}\n\
rustok_runtime_guardrail_event_backpressure_critical_total {critical_count}\n",
        rollout_mode = snapshot.rollout.metric_value(),
        observed_status = snapshot.observed_status.metric_value(),
        overall_status = snapshot.status.metric_value(),
        runtime_dependencies_enabled = if snapshot.runtime_dependencies_enabled {
            1
        } else {
            0
        },
        host_mode = snapshot.host_mode,
        relay_fallback_active = if snapshot.event_transport.relay_fallback_active {
            1
        } else {
            0
        },
        remote_executor_enabled = if snapshot.remote_executor.enabled { 1 } else { 0 },
        remote_executor_token_configured = if snapshot.remote_executor.token_configured {
            1
        } else {
            0
        },
        remote_executor_state = snapshot.remote_executor.state.metric_value(),
        remote_executor_active_claims = snapshot.remote_executor.active_claims,
        remote_executor_expired_claims = snapshot.remote_executor.expired_claims,
        remote_executor_lease_ttl_ms = snapshot.remote_executor.lease_ttl_ms,
        remote_executor_requeue_scan_interval_ms = snapshot
            .remote_executor
            .requeue_scan_interval_ms,
        backpressure_enabled = if snapshot.event_bus.backpressure_enabled {
            1
        } else {
            0
        },
        backpressure_state = snapshot.event_bus.state.metric_value(),
        current_depth = snapshot.event_bus.current_depth,
        max_depth = snapshot.event_bus.max_depth,
        events_rejected = snapshot.event_bus.events_rejected,
        warning_count = snapshot.event_bus.warning_count,
        critical_count = snapshot.event_bus.critical_count,
    );
    payload.push_str(&rate_limit_lines);
    payload
}

async fn render_outbox_metrics(ctx: &AppContext) -> String {
    let backlog_size = SysEventsEntity::find()
        .filter(SysEventsColumn::Status.eq(SysEventStatus::Pending))
        .count(&ctx.db)
        .await
        .unwrap_or(0);

    let dlq_total = SysEventsEntity::find()
        .filter(SysEventsColumn::Status.eq(SysEventStatus::Failed))
        .count(&ctx.db)
        .await
        .unwrap_or(0);

    let retries_total = ctx
        .db
        .query_one(Statement::from_string(
            ctx.db.get_database_backend(),
            "SELECT COALESCE(SUM(retry_count), 0) AS total FROM sys_events".to_string(),
        ))
        .await
        .ok()
        .flatten()
        .and_then(|row| row.try_get::<i64>("", "total").ok())
        .unwrap_or(0);

    format_outbox_metrics(backlog_size, dlq_total, retries_total)
}

fn format_outbox_metrics(backlog_size: u64, dlq_total: u64, retries_total: i64) -> String {
    format!(
        "rustok_outbox_backlog_size {backlog_size}\n\
rustok_outbox_dlq_total {dlq_total}\n\
rustok_outbox_retries_total {retries_total}\n\
outbox_backlog_size {backlog_size}\n\
outbox_dlq_total {dlq_total}\n\
outbox_retries_total {retries_total}\n",
    )
}

async fn render_rbac_metrics(ctx: &AppContext) -> String {
    let stats = RbacService::metrics_snapshot();
    let started_at = Instant::now();
    let consistency = match load_rbac_consistency_stats(ctx).await {
        Ok(stats) => stats,
        Err(error) => {
            RBAC_CONSISTENCY_QUERY_FAILURES_TOTAL.fetch_add(1, Ordering::Relaxed);
            warn!(error = %error, "failed to load RBAC consistency stats");
            RbacConsistencyStats::default()
        }
    };
    let latency_ms = started_at.elapsed().as_millis() as u64;
    RBAC_CONSISTENCY_QUERY_LATENCY_MS_TOTAL.fetch_add(latency_ms, Ordering::Relaxed);
    RBAC_CONSISTENCY_QUERY_LATENCY_SAMPLES.fetch_add(1, Ordering::Relaxed);

    format_rbac_metrics(
        stats,
        consistency.users_without_roles_total,
        consistency.orphan_user_roles_total,
        consistency.orphan_role_permissions_total,
    )
}

async fn render_search_metrics(ctx: &AppContext) -> String {
    let backend = ctx.db.get_database_backend();
    let stmt = Statement::from_string(backend, search_metrics_snapshot_query(backend).to_string());

    match ctx.db.query_one(stmt).await {
        Ok(Some(row)) => {
            let read_metric =
                |column: &str| -> i64 { row.try_get::<i64>("", column).unwrap_or(0).max(0) };

            format!(
                "rustok_search_metrics_collection_status 1\n\
rustok_search_documents_total {total_documents}\n\
rustok_search_public_documents_total {public_documents}\n\
rustok_search_stale_documents_total {stale_documents}\n\
rustok_search_tenants_with_documents_total {tenants_with_documents}\n\
rustok_search_lagging_tenants_total {lagging_tenants}\n\
rustok_search_bootstrap_pending_tenants_total {bootstrap_pending_tenants}\n\
rustok_search_max_lag_seconds {max_lag_seconds}\n",
                total_documents = read_metric("total_documents"),
                public_documents = read_metric("public_documents"),
                stale_documents = read_metric("stale_documents"),
                tenants_with_documents = read_metric("tenants_with_documents"),
                lagging_tenants = read_metric("lagging_tenants"),
                bootstrap_pending_tenants = read_metric("bootstrap_pending_tenants"),
                max_lag_seconds = read_metric("max_lag_seconds"),
            )
        }
        Ok(None) => "rustok_search_metrics_collection_status 0\n\
rustok_search_documents_total 0\n\
rustok_search_public_documents_total 0\n\
rustok_search_stale_documents_total 0\n\
rustok_search_tenants_with_documents_total 0\n\
rustok_search_lagging_tenants_total 0\n\
rustok_search_bootstrap_pending_tenants_total 0\n\
rustok_search_max_lag_seconds 0\n"
            .to_string(),
        Err(error) => {
            if !is_missing_relation_error(&error) {
                warn!(error = %error, "failed to load search metrics snapshot");
            }
            "rustok_search_metrics_collection_status 0\n\
rustok_search_documents_total 0\n\
rustok_search_public_documents_total 0\n\
rustok_search_stale_documents_total 0\n\
rustok_search_tenants_with_documents_total 0\n\
rustok_search_lagging_tenants_total 0\n\
rustok_search_bootstrap_pending_tenants_total 0\n\
rustok_search_max_lag_seconds 0\n"
                .to_string()
        }
    }
}

fn is_missing_relation_error(error: &sea_orm::DbErr) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("no such table")
        || message.contains("undefinedtable")
        || message.contains("relation") && message.contains("does not exist")
}

fn search_metrics_snapshot_query(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Sqlite => {
            r#"
            WITH source_tenants AS (
                SELECT tenant_id FROM nodes WHERE deleted_at IS NULL
                UNION
                SELECT tenant_id FROM products
            )
            SELECT
                CAST(COUNT(*) AS INTEGER) AS total_documents,
                CAST(SUM(CASE WHEN is_public THEN 1 ELSE 0 END) AS INTEGER) AS public_documents,
                CAST(SUM(CASE WHEN indexed_at < updated_at THEN 1 ELSE 0 END) AS INTEGER) AS stale_documents,
                CAST(COUNT(DISTINCT tenant_id) AS INTEGER) AS tenants_with_documents,
                CAST(COUNT(DISTINCT CASE WHEN indexed_at < updated_at THEN tenant_id END) AS INTEGER) AS lagging_tenants,
                CAST(
                    COALESCE(
                        MAX(
                            CASE
                                WHEN updated_at > indexed_at THEN CAST((julianday(updated_at) - julianday(indexed_at)) * 86400 AS INTEGER)
                                ELSE 0
                            END
                        ),
                        0
                    ) AS INTEGER
                ) AS max_lag_seconds,
                (
                    SELECT CAST(COUNT(*) AS INTEGER)
                    FROM source_tenants st
                    WHERE NOT EXISTS (
                        SELECT 1
                        FROM search_documents sd
                        WHERE sd.tenant_id = st.tenant_id
                    )
                ) AS bootstrap_pending_tenants
            FROM search_documents
            "#
        }
        _ => {
            r#"
            WITH source_tenants AS (
                SELECT tenant_id FROM nodes WHERE deleted_at IS NULL
                UNION
                SELECT tenant_id FROM products
            )
            SELECT
                COUNT(*)::bigint AS total_documents,
                COUNT(*) FILTER (WHERE is_public)::bigint AS public_documents,
                COUNT(*) FILTER (WHERE indexed_at < updated_at)::bigint AS stale_documents,
                COUNT(DISTINCT tenant_id)::bigint AS tenants_with_documents,
                COUNT(DISTINCT tenant_id) FILTER (WHERE indexed_at < updated_at)::bigint AS lagging_tenants,
                COALESCE(MAX(GREATEST(EXTRACT(EPOCH FROM (updated_at - indexed_at)), 0)), 0)::bigint AS max_lag_seconds,
                (
                    SELECT COUNT(*)::bigint
                    FROM source_tenants st
                    WHERE NOT EXISTS (
                        SELECT 1
                        FROM search_documents sd
                        WHERE sd.tenant_id = st.tenant_id
                    )
                ) AS bootstrap_pending_tenants
            FROM search_documents
            "#
        }
    }
}

fn render_auth_lifecycle_metrics() -> String {
    let stats = AuthLifecycleService::metrics_snapshot();
    format!(
        "rustok_auth_password_reset_sessions_revoked_total {password_reset_sessions_revoked_total}\n\
rustok_auth_change_password_sessions_revoked_total {change_password_sessions_revoked_total}\n\
rustok_auth_flow_inconsistency_total {flow_inconsistency_total}\n\
rustok_auth_login_inactive_user_attempt_total {login_inactive_user_attempt_total}\n\
auth_password_reset_sessions_revoked_total {password_reset_sessions_revoked_total}\n\
auth_change_password_sessions_revoked_total {change_password_sessions_revoked_total}\n\
auth_flow_inconsistency_total {flow_inconsistency_total}\n\
auth_login_inactive_user_attempt_total {login_inactive_user_attempt_total}\n",
        password_reset_sessions_revoked_total = stats.password_reset_sessions_revoked_total,
        change_password_sessions_revoked_total = stats.change_password_sessions_revoked_total,
        flow_inconsistency_total = stats.flow_inconsistency_total,
        login_inactive_user_attempt_total = stats.login_inactive_user_attempt_total,
    )
}

fn format_rbac_metrics(
    stats: RbacResolverMetricsSnapshot,
    users_without_roles_total: i64,
    orphan_user_roles_total: i64,
    orphan_role_permissions_total: i64,
) -> String {
    let consistency_query_failures_total =
        RBAC_CONSISTENCY_QUERY_FAILURES_TOTAL.load(Ordering::Relaxed);
    let consistency_query_latency_ms_total =
        RBAC_CONSISTENCY_QUERY_LATENCY_MS_TOTAL.load(Ordering::Relaxed);
    let consistency_query_latency_samples =
        RBAC_CONSISTENCY_QUERY_LATENCY_SAMPLES.load(Ordering::Relaxed);
    format!(
        concat!(
            "rustok_rbac_permission_cache_hits {cache_hits}\n",
            "rustok_rbac_permission_cache_misses {cache_misses}\n",
            "rustok_rbac_permission_checks_allowed {checks_allowed}\n",
            "rustok_rbac_permission_checks_denied {checks_denied}\n",
            "rustok_rbac_permission_check_latency_ms_total {check_latency_ms_total}\n",
            "rustok_rbac_permission_check_latency_samples {check_latency_samples}\n",
            "rustok_rbac_permission_lookup_latency_ms_total {lookup_latency_ms_total}\n",
            "rustok_rbac_permission_lookup_latency_samples {lookup_latency_samples}\n",
            "rustok_rbac_permission_denied_reason_no_permissions_resolved {denied_no_permissions_resolved}\n",
            "rustok_rbac_permission_denied_reason_missing_permissions {denied_missing_permissions}\n",
            "rustok_rbac_permission_denied_reason_unknown {denied_unknown}\n",
            "rustok_rbac_claim_role_mismatch_total {claim_role_mismatch_total}\n",
            "rustok_rbac_engine_decisions_casbin_total {engine_decisions_casbin_total}\n",
            "rustok_rbac_engine_eval_duration_ms_total {engine_eval_duration_ms_total}\n",
            "rustok_rbac_engine_eval_duration_samples {engine_eval_duration_samples}\n",
            "rustok_rbac_users_without_roles_total {users_without_roles_total}\n",
            "rustok_rbac_orphan_user_roles_total {orphan_user_roles_total}\n",
            "rustok_rbac_orphan_role_permissions_total {orphan_role_permissions_total}\n",
            "rustok_rbac_consistency_query_failures_total {consistency_query_failures_total}\n",
            "rustok_rbac_consistency_query_latency_ms_total {consistency_query_latency_ms_total}\n",
            "rustok_rbac_consistency_query_latency_samples {consistency_query_latency_samples}\n"
        ),
        cache_hits = stats.permission_cache_hits,
        cache_misses = stats.permission_cache_misses,
        checks_allowed = stats.permission_checks_allowed,
        checks_denied = stats.permission_checks_denied,
        check_latency_ms_total = stats.permission_check_latency_ms_total,
        check_latency_samples = stats.permission_check_latency_samples,
        lookup_latency_ms_total = stats.permission_lookup_latency_ms_total,
        lookup_latency_samples = stats.permission_lookup_latency_samples,
        denied_no_permissions_resolved = stats.denied_no_permissions_resolved,
        denied_missing_permissions = stats.denied_missing_permissions,
        denied_unknown = stats.denied_unknown,
        claim_role_mismatch_total = stats.claim_role_mismatch_total,
        engine_decisions_casbin_total = stats.engine_decisions_casbin_total,
        engine_eval_duration_ms_total = stats.engine_eval_duration_ms_total,
        engine_eval_duration_samples = stats.engine_eval_duration_samples,
        users_without_roles_total = users_without_roles_total,
        orphan_user_roles_total = orphan_user_roles_total,
        orphan_role_permissions_total = orphan_role_permissions_total,
        consistency_query_failures_total = consistency_query_failures_total,
        consistency_query_latency_ms_total = consistency_query_latency_ms_total,
        consistency_query_latency_samples = consistency_query_latency_samples,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        format_outbox_metrics, format_rbac_metrics, format_runtime_guardrail_metrics,
        render_auth_lifecycle_metrics,
    };
    use crate::services::auth_lifecycle::AuthLifecycleService;
    use crate::services::rbac_service::RbacService;
    use crate::services::runtime_guardrails::{
        EventBusGuardrailSnapshot, EventTransportGuardrailSnapshot, RateLimitGuardrailSnapshot,
        RateLimitPolicySnapshot, RemoteExecutorGuardrailSnapshot, RuntimeGuardrailRollout,
        RuntimeGuardrailSnapshot, RuntimeGuardrailStatus,
    };

    fn assert_metric_line(payload: &str, metric_name: &str) {
        let has_exact_line = payload.lines().any(|line| {
            line.starts_with(metric_name) && line.as_bytes().get(metric_name.len()) == Some(&b' ')
        });
        assert!(
            has_exact_line,
            "metric line `{metric_name}` not found in payload: {payload}"
        );
    }

    fn assert_metric_labeled_line(payload: &str, metric_name: &str, labels: &str) {
        let prefix = format!("{metric_name}{labels}");
        let has_exact_line = payload.lines().any(|line| {
            line.starts_with(&prefix) && line.as_bytes().get(prefix.len()) == Some(&b' ')
        });
        assert!(
            has_exact_line,
            "labeled metric line `{prefix}` not found in payload: {payload}"
        );
    }

    #[test]
    fn rbac_metrics_include_claim_role_mismatch_counter() {
        let payload = format_rbac_metrics(RbacService::metrics_snapshot(), 0, 0, 0);
        assert!(payload.contains("rustok_rbac_claim_role_mismatch_total"));
    }

    #[test]
    fn rbac_metrics_include_engine_decision_and_latency_counters() {
        let payload = format_rbac_metrics(RbacService::metrics_snapshot(), 0, 0, 0);
        assert_metric_line(&payload, "rustok_rbac_engine_decisions_casbin_total");
        assert_metric_line(&payload, "rustok_rbac_engine_eval_duration_ms_total");
        assert_metric_line(&payload, "rustok_rbac_engine_eval_duration_samples");
    }

    #[test]
    fn rbac_metrics_include_users_without_roles_counter() {
        let payload = format_rbac_metrics(RbacService::metrics_snapshot(), 0, 0, 0);
        assert!(payload.contains("rustok_rbac_users_without_roles_total"));
    }

    #[test]
    fn rbac_metrics_include_orphan_user_roles_counter() {
        let payload = format_rbac_metrics(RbacService::metrics_snapshot(), 0, 0, 0);
        assert!(payload.contains("rustok_rbac_orphan_user_roles_total"));
    }

    #[test]
    fn rbac_metrics_include_orphan_role_permissions_counter() {
        let payload = format_rbac_metrics(RbacService::metrics_snapshot(), 0, 0, 0);
        assert!(payload.contains("rustok_rbac_orphan_role_permissions_total"));
    }

    #[test]
    fn rbac_metrics_include_consistency_query_failures_counter() {
        let payload = format_rbac_metrics(RbacService::metrics_snapshot(), 0, 0, 0);
        assert!(payload.contains("rustok_rbac_consistency_query_failures_total"));
    }

    #[test]
    fn rbac_metrics_include_consistency_query_latency_counters() {
        let payload = format_rbac_metrics(RbacService::metrics_snapshot(), 0, 0, 0);
        assert!(payload.contains("rustok_rbac_consistency_query_latency_ms_total"));
        assert!(payload.contains("rustok_rbac_consistency_query_latency_samples"));
    }

    #[test]
    fn rbac_metrics_render_consistency_values() {
        let payload = format_rbac_metrics(RbacService::metrics_snapshot(), 7, 3, 1);
        assert!(payload.contains("rustok_rbac_users_without_roles_total 7"));
        assert!(payload.contains("rustok_rbac_orphan_user_roles_total 3"));
        assert!(payload.contains("rustok_rbac_orphan_role_permissions_total 1"));
    }

    #[test]
    fn auth_lifecycle_metrics_include_required_counters() {
        AuthLifecycleService::record_flow_inconsistency();

        let payload = render_auth_lifecycle_metrics();

        assert!(payload.contains("rustok_auth_password_reset_sessions_revoked_total"));
        assert!(payload.contains("rustok_auth_change_password_sessions_revoked_total"));
        assert!(payload.contains("rustok_auth_flow_inconsistency_total"));
        assert!(payload.contains("rustok_auth_login_inactive_user_attempt_total"));
        assert!(payload.contains("auth_password_reset_sessions_revoked_total"));
        assert!(payload.contains("auth_change_password_sessions_revoked_total"));
        assert!(payload.contains("auth_flow_inconsistency_total"));
        assert!(payload.contains("auth_login_inactive_user_attempt_total"));
    }

    #[test]
    fn outbox_metrics_include_canonical_names_and_compatibility_aliases() {
        let payload = format_outbox_metrics(11, 2, 7);

        assert_metric_line(&payload, "rustok_outbox_backlog_size");
        assert_metric_line(&payload, "rustok_outbox_dlq_total");
        assert_metric_line(&payload, "rustok_outbox_retries_total");
        assert_metric_line(&payload, "outbox_backlog_size");
        assert_metric_line(&payload, "outbox_dlq_total");
        assert_metric_line(&payload, "outbox_retries_total");
        assert!(payload.contains("rustok_outbox_backlog_size 11"));
        assert!(payload.contains("rustok_outbox_dlq_total 2"));
        assert!(payload.contains("rustok_outbox_retries_total 7"));
    }

    #[test]
    fn runtime_guardrail_metrics_include_rate_limit_policy_config() {
        let payload = format_runtime_guardrail_metrics(&RuntimeGuardrailSnapshot {
            status: RuntimeGuardrailStatus::Ok,
            observed_status: RuntimeGuardrailStatus::Ok,
            rollout: RuntimeGuardrailRollout::Observe,
            host_mode: "full".to_string(),
            runtime_dependencies_enabled: true,
            reasons: Vec::new(),
            rate_limits: vec![RateLimitGuardrailSnapshot {
                namespace: "oauth",
                backend: "redis",
                distributed: true,
                policy: RateLimitPolicySnapshot {
                    enabled: true,
                    max_requests: 35,
                    window_seconds: 60,
                    trusted_auth_dimensions: true,
                    memory_warning_entries: 1_000,
                    memory_critical_entries: 5_000,
                },
                active_clients: 7,
                total_entries: 11,
                healthy: true,
                state: RuntimeGuardrailStatus::Ok,
            }],
            event_bus: EventBusGuardrailSnapshot {
                backpressure_enabled: false,
                current_depth: 0,
                max_depth: 0,
                state: RuntimeGuardrailStatus::Ok,
                events_rejected: 0,
                warning_count: 0,
                critical_count: 0,
            },
            event_transport: EventTransportGuardrailSnapshot {
                relay_fallback_active: false,
                channel_capacity: 128,
            },
            remote_executor: RemoteExecutorGuardrailSnapshot {
                enabled: true,
                token_configured: true,
                lease_ttl_ms: 120_000,
                requeue_scan_interval_ms: 15_000,
                active_claims: 2,
                expired_claims: 1,
                state: RuntimeGuardrailStatus::Degraded,
            },
        });

        assert_metric_labeled_line(
            &payload,
            "rustok_runtime_guardrail_rate_limit_config",
            "{namespace=\"oauth\",setting=\"enabled\"}",
        );
        assert_metric_labeled_line(
            &payload,
            "rustok_runtime_guardrail_rate_limit_config",
            "{namespace=\"oauth\",setting=\"max_requests\"}",
        );
        assert_metric_labeled_line(
            &payload,
            "rustok_runtime_guardrail_rate_limit_config",
            "{namespace=\"oauth\",setting=\"trusted_auth_dimensions\"}",
        );
        assert_metric_labeled_line(
            &payload,
            "rustok_runtime_guardrail_rate_limit_config",
            "{namespace=\"oauth\",setting=\"memory_critical_entries\"}",
        );
        assert!(payload.contains("rustok_runtime_guardrail_runtime_dependencies_enabled 1"));
        assert!(payload.contains("rustok_runtime_guardrail_host_mode{mode=\"full\"} 1"));
        assert!(payload.contains("rustok_runtime_guardrail_remote_executor_enabled 1"));
        assert!(payload.contains("rustok_runtime_guardrail_remote_executor_expired_claims 1"));
    }
}
