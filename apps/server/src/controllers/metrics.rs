use axum::{
    extract::State,
    http::{header::CONTENT_TYPE, StatusCode},
    response::{IntoResponse, Response},
};
use loco_rs::{app::AppContext, controller::Routes, prelude::*, Result};
use rustok_outbox::entity::{Column as SysEventsColumn, Entity as SysEventsEntity, SysEventStatus};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, PaginatorTrait, QueryFilter, Statement,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::middleware::tenant::tenant_cache_stats;
use crate::services::auth::AuthService;
use crate::services::auth_lifecycle::AuthLifecycleService;
use crate::services::rbac_consistency::{load_rbac_consistency_stats, RbacConsistencyStats};
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
            let mut payload = handle.render();
            payload.push('\n');
            payload.push_str(&render_tenant_cache_metrics(&ctx).await);
            payload.push_str(&render_outbox_metrics(&ctx).await);
            payload.push_str(&render_auth_lifecycle_metrics());
            payload.push_str(&render_rbac_metrics(&ctx).await);

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
rustok_tenant_cache_negative_inserts {negative_inserts}\n",
        hits = stats.hits,
        misses = stats.misses,
        evictions = stats.evictions,
        entries = stats.entries,
        negative_hits = stats.negative_hits,
        negative_misses = stats.negative_misses,
        negative_evictions = stats.negative_evictions,
        negative_entries = stats.negative_entries,
        negative_inserts = stats.negative_inserts,
    )
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
            DbBackend::Postgres,
            "SELECT COALESCE(SUM(retry_count), 0) AS total FROM sys_events".to_string(),
        ))
        .await
        .ok()
        .flatten()
        .and_then(|row| row.try_get::<i64>("", "total").ok())
        .unwrap_or(0);

    format!(
        "outbox_backlog_size {backlog_size}\n\
outbox_dlq_total {dlq_total}\n\
outbox_retries_total {retries_total}\n",
    )
}

async fn render_rbac_metrics(ctx: &AppContext) -> String {
    let stats = AuthService::metrics_snapshot();
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

fn render_auth_lifecycle_metrics() -> String {
    let stats = AuthLifecycleService::metrics_snapshot();
    format!(
        "auth_password_reset_sessions_revoked_total {password_reset_sessions_revoked_total}\n\
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
    stats: crate::services::auth::RbacResolverMetricsSnapshot,
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
    let engine_decisions_total =
        stats.engine_decisions_relation_total + stats.engine_decisions_casbin_total;

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
            "rustok_rbac_decision_mismatch_total {decision_mismatch_total}\n",
            "rustok_rbac_shadow_compare_failures_total {shadow_compare_failures_total}\n",
            "rbac_engine_decisions_total {engine_decisions_total}\n",
            "rustok_rbac_engine_decisions_relation_total {engine_decisions_relation_total}\n",
            "rustok_rbac_engine_decisions_casbin_total {engine_decisions_casbin_total}\n",
            "rbac_engine_mismatch_total {engine_mismatch_total}\n",
            "rbac_engine_mismatch_total{{source=\"relation\",target=\"casbin\"}} {engine_mismatch_total}\n",
            "rustok_rbac_engine_mismatch_total {engine_mismatch_total}\n",
            "rbac_engine_eval_duration_ms {engine_eval_duration_ms_total}\n",
            "rbac_engine_eval_latency_ms{{engine=\"casbin\"}} {engine_eval_duration_ms_total}\n",
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
        decision_mismatch_total = stats.decision_mismatch_total,
        shadow_compare_failures_total = stats.shadow_compare_failures_total,
        engine_decisions_total = engine_decisions_total,
        engine_decisions_relation_total = stats.engine_decisions_relation_total,
        engine_decisions_casbin_total = stats.engine_decisions_casbin_total,
        engine_mismatch_total = stats.engine_mismatch_total,
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
    use super::{format_rbac_metrics, render_auth_lifecycle_metrics};
    use crate::services::auth::AuthService;
    use crate::services::auth_lifecycle::AuthLifecycleService;

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
        let payload = format_rbac_metrics(AuthService::metrics_snapshot(), 0, 0, 0);
        assert!(payload.contains("rustok_rbac_claim_role_mismatch_total"));
    }

    #[test]
    fn rbac_metrics_include_decision_mismatch_counter() {
        let payload = format_rbac_metrics(AuthService::metrics_snapshot(), 0, 0, 0);
        assert!(payload.contains("rustok_rbac_decision_mismatch_total"));
    }

    #[test]
    fn rbac_metrics_include_shadow_compare_failures_counter() {
        let payload = format_rbac_metrics(AuthService::metrics_snapshot(), 0, 0, 0);
        assert!(payload.contains("rustok_rbac_shadow_compare_failures_total"));
    }

    #[test]
    fn rbac_metrics_include_engine_mismatch_counter() {
        let payload = format_rbac_metrics(AuthService::metrics_snapshot(), 0, 0, 0);
        assert!(payload.contains("rustok_rbac_engine_mismatch_total"));
    }

    #[test]
    fn rbac_metrics_include_engine_decision_and_latency_counters() {
        let payload = format_rbac_metrics(AuthService::metrics_snapshot(), 0, 0, 0);
        assert_metric_line(&payload, "rbac_engine_decisions_total");
        assert_metric_line(&payload, "rustok_rbac_engine_decisions_relation_total");
        assert_metric_line(&payload, "rustok_rbac_engine_decisions_casbin_total");
        assert_metric_line(&payload, "rbac_engine_mismatch_total");
        assert_metric_labeled_line(
            &payload,
            "rbac_engine_mismatch_total",
            "{source=\"relation\",target=\"casbin\"}",
        );
        assert_metric_line(&payload, "rustok_rbac_engine_mismatch_total");
        assert_metric_line(&payload, "rbac_engine_eval_duration_ms");
        assert_metric_labeled_line(
            &payload,
            "rbac_engine_eval_latency_ms",
            "{engine=\"casbin\"}",
        );
        assert_metric_line(&payload, "rustok_rbac_engine_eval_duration_ms_total");
        assert_metric_line(&payload, "rustok_rbac_engine_eval_duration_samples");
    }

    #[test]
    fn rbac_metrics_include_users_without_roles_counter() {
        let payload = format_rbac_metrics(AuthService::metrics_snapshot(), 0, 0, 0);
        assert!(payload.contains("rustok_rbac_users_without_roles_total"));
    }

    #[test]
    fn rbac_metrics_include_orphan_user_roles_counter() {
        let payload = format_rbac_metrics(AuthService::metrics_snapshot(), 0, 0, 0);
        assert!(payload.contains("rustok_rbac_orphan_user_roles_total"));
    }

    #[test]
    fn rbac_metrics_include_orphan_role_permissions_counter() {
        let payload = format_rbac_metrics(AuthService::metrics_snapshot(), 0, 0, 0);
        assert!(payload.contains("rustok_rbac_orphan_role_permissions_total"));
    }

    #[test]
    fn rbac_metrics_include_consistency_query_failures_counter() {
        let payload = format_rbac_metrics(AuthService::metrics_snapshot(), 0, 0, 0);
        assert!(payload.contains("rustok_rbac_consistency_query_failures_total"));
    }

    #[test]
    fn rbac_metrics_include_consistency_query_latency_counters() {
        let payload = format_rbac_metrics(AuthService::metrics_snapshot(), 0, 0, 0);
        assert!(payload.contains("rustok_rbac_consistency_query_latency_ms_total"));
        assert!(payload.contains("rustok_rbac_consistency_query_latency_samples"));
    }

    #[test]
    fn rbac_metrics_render_consistency_values() {
        let payload = format_rbac_metrics(AuthService::metrics_snapshot(), 7, 3, 1);
        assert!(payload.contains("rustok_rbac_users_without_roles_total 7"));
        assert!(payload.contains("rustok_rbac_orphan_user_roles_total 3"));
        assert!(payload.contains("rustok_rbac_orphan_role_permissions_total 1"));
    }

    #[test]
    fn auth_lifecycle_metrics_include_required_counters() {
        AuthLifecycleService::record_flow_inconsistency();

        let payload = render_auth_lifecycle_metrics();

        assert!(payload.contains("auth_password_reset_sessions_revoked_total"));
        assert!(payload.contains("auth_change_password_sessions_revoked_total"));
        assert!(payload.contains("auth_flow_inconsistency_total"));
        assert!(payload.contains("auth_login_inactive_user_attempt_total"));
    }
}
